//! Native SSH/SFTP client built on `russh` + `russh-sftp`.
//!
//! Replaces the per-operation `ssh`/`scp` shell-outs with a persistent
//! in-process SSH session. One session is opened per SFTP UI session
//! and reused for listing, metadata, and file transfers.

use anyhow::{Context, Result};
use russh::client::{self, Handle};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg, PublicKey};
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constants::{SSH_CONNECT_TIMEOUT_S, TRANSFER_BUFFER_SIZE};

/// Distinguished error type so callers can decide whether to prompt the
/// user for a password vs. report a hard failure.
#[derive(Debug, Error)]
pub enum SshClientError {
    #[error("authentication failed; no key in agent or ~/.ssh worked. Provide a password or fix the SSH config")]
    AuthRequiresPassword,
    #[error("authentication failed: bad password")]
    BadPassword,
    #[error("connection refused or unreachable: {0}")]
    Connect(String),
    #[error("SFTP subsystem error: {0}")]
    Sftp(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct ConnectOpts {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub strict_host_key_checking: String,
    /// If `Some`, try password auth (after key/agent attempts also failed,
    /// or as the only attempt).
    pub password: Option<String>,
}

/// Persistent SFTP client. Holds the SSH session open for the lifetime of
/// the SFTP UI session.
pub struct SftpClient {
    pub sftp: SftpSession,
    /// Keep the underlying SSH session alive — dropping it disconnects.
    _session: Handle<ClientHandler>,
}

impl std::fmt::Debug for SftpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpClient").finish_non_exhaustive()
    }
}

impl SftpClient {
    /// Establish a new SSH session and open the SFTP subsystem.
    /// Tries auth in order: ssh-agent → ~/.ssh keys → password (if provided).
    pub async fn connect(opts: &ConnectOpts) -> Result<Self, SshClientError> {
        let mut session = open_session(opts).await?;

        let auth_ok = try_authenticate(&mut session, opts).await?;
        if !auth_ok {
            return Err(if opts.password.is_some() {
                SshClientError::BadPassword
            } else {
                SshClientError::AuthRequiresPassword
            });
        }

        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| SshClientError::Sftp(format!("channel_open_session: {}", e)))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| SshClientError::Sftp(format!("request_subsystem: {}", e)))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| SshClientError::Sftp(format!("SftpSession::new: {}", e)))?;

        Ok(Self {
            sftp,
            _session: session,
        })
    }

    /// Run a one-shot remote command and capture its output. Reserved for
    /// future use (connect-test / heartbeat paths).
    #[allow(dead_code)]
    pub async fn exec(&self, cmd: &str) -> Result<(u32, Vec<u8>, Vec<u8>)> {
        let session = &self._session;
        let mut channel = session
            .channel_open_session()
            .await
            .context("channel_open_session for exec")?;
        channel
            .exec(true, cmd)
            .await
            .context("channel.exec")?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit = 255u32;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext: 1 } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => exit = exit_status,
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
        Ok((exit, stdout, stderr))
    }

    /// Stream a local file up to the remote, invoking `progress` after each
    /// chunk with (uploaded_bytes, total_bytes).
    pub async fn upload<F>(
        &self,
        local_path: &std::path::Path,
        remote_path: &str,
        mut progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64) + Send,
    {
        use russh_sftp::protocol::OpenFlags;

        let total_size = tokio::fs::metadata(local_path)
            .await
            .context("Failed to read local file metadata")?
            .len();

        let mut local = tokio::fs::File::open(local_path)
            .await
            .context("Failed to open local file")?;
        let mut remote = self
            .sftp
            .open_with_flags(
                remote_path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await
            .map_err(|e| anyhow::anyhow!("open remote file for write: {}", e))?;

        let mut buf = vec![0u8; TRANSFER_BUFFER_SIZE];
        let mut uploaded = 0u64;
        progress(uploaded, total_size);
        loop {
            let n = local.read(&mut buf).await.context("read local")?;
            if n == 0 {
                break;
            }
            remote
                .write_all(&buf[..n])
                .await
                .context("write remote")?;
            uploaded += n as u64;
            progress(uploaded, total_size);
        }
        remote.flush().await.context("flush remote")?;
        remote.shutdown().await.context("close remote")?;
        Ok(())
    }

    /// Stream a remote file down to the local path, invoking `progress`
    /// after each chunk with (downloaded_bytes, total_bytes).
    pub async fn download<F>(
        &self,
        remote_path: &str,
        local_path: &std::path::Path,
        mut progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64) + Send,
    {
        use russh_sftp::protocol::OpenFlags;

        let total_size = self
            .sftp
            .metadata(remote_path)
            .await
            .map_err(|e| anyhow::anyhow!("stat remote: {}", e))?
            .size
            .unwrap_or(0);

        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create local parent dir")?;
        }

        let mut remote = self
            .sftp
            .open_with_flags(remote_path, OpenFlags::READ)
            .await
            .map_err(|e| anyhow::anyhow!("open remote file for read: {}", e))?;
        let mut local = tokio::fs::File::create(local_path)
            .await
            .context("create local file")?;

        let mut buf = vec![0u8; TRANSFER_BUFFER_SIZE];
        let mut downloaded = 0u64;
        progress(downloaded, total_size);
        loop {
            let n = remote.read(&mut buf).await.context("read remote")?;
            if n == 0 {
                break;
            }
            local
                .write_all(&buf[..n])
                .await
                .context("write local")?;
            downloaded += n as u64;
            progress(downloaded, total_size);
        }
        local.flush().await.context("flush local")?;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------------

async fn open_session(opts: &ConnectOpts) -> Result<Handle<ClientHandler>, SshClientError> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(SSH_CONNECT_TIMEOUT_S * 10)),
        ..Default::default()
    });
    let handler = ClientHandler {
        strict: opts.strict_host_key_checking.clone(),
    };
    let session = client::connect(config, (opts.host.as_str(), opts.port), handler)
        .await
        .map_err(|e| SshClientError::Connect(e.to_string()))?;
    Ok(session)
}

/// Try, in order: ssh-agent identities → key files in ~/.ssh → password.
/// Returns Ok(true) on success, Ok(false) if all attempts were rejected,
/// Err only on transport-level failure.
async fn try_authenticate(
    session: &mut Handle<ClientHandler>,
    opts: &ConnectOpts,
) -> Result<bool, SshClientError> {
    if let Some(success) = try_agent_auth(session, &opts.user).await? {
        if success {
            tracing::info!("SSH auth succeeded via ssh-agent");
            return Ok(true);
        }
    }

    if let Some(success) = try_keyfile_auth(session, &opts.user).await? {
        if success {
            tracing::info!("SSH auth succeeded via ~/.ssh key file");
            return Ok(true);
        }
    }

    if let Some(pw) = &opts.password {
        let result = session
            .authenticate_password(&opts.user, pw)
            .await
            .map_err(|e| SshClientError::Other(anyhow::anyhow!("authenticate_password: {}", e)))?;
        if result.success() {
            tracing::info!("SSH auth succeeded via password");
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(unix)]
async fn try_agent_auth(
    session: &mut Handle<ClientHandler>,
    user: &str,
) -> Result<Option<bool>, SshClientError> {
    use russh::keys::agent::{client::AgentClient, AgentIdentity};

    let Ok(sock) = std::env::var("SSH_AUTH_SOCK") else {
        return Ok(None);
    };
    let stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("ssh-agent socket {} unavailable: {}", sock, e);
            return Ok(None);
        }
    };
    let mut agent = AgentClient::connect(stream);
    let identities = match agent.request_identities().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::debug!("ssh-agent request_identities failed: {}", e);
            return Ok(None);
        }
    };
    if identities.is_empty() {
        return Ok(None);
    }

    let hash_alg = session
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();

    for identity in identities {
        // authenticate_publickey_with takes the bare PublicKey; the signer
        // (AgentClient) converts back to AgentIdentity internally.
        let pk = match &identity {
            AgentIdentity::PublicKey { key, .. } => key.clone(),
            AgentIdentity::Certificate { certificate, .. } => {
                certificate.public_key().clone().into()
            }
        };
        let result = session
            .authenticate_publickey_with(user, pk, hash_alg, &mut agent)
            .await;
        match result {
            Ok(r) if r.success() => return Ok(Some(true)),
            Ok(_) => continue,
            Err(e) => {
                tracing::debug!("agent key auth attempt errored: {}", e);
                continue;
            }
        }
    }
    Ok(Some(false))
}

#[cfg(not(unix))]
async fn try_agent_auth(
    _session: &mut Handle<ClientHandler>,
    _user: &str,
) -> Result<Option<bool>, SshClientError> {
    Ok(None)
}

async fn try_keyfile_auth(
    session: &mut Handle<ClientHandler>,
    user: &str,
) -> Result<Option<bool>, SshClientError> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    let candidates = [
        home.join(".ssh/id_ed25519"),
        home.join(".ssh/id_rsa"),
        home.join(".ssh/id_ecdsa"),
    ];

    let mut tried_any = false;
    for path in candidates {
        if !path.exists() {
            continue;
        }
        tried_any = true;
        let key = match load_secret_key(&path, None) {
            Ok(k) => k,
            Err(e) => {
                tracing::debug!("could not load {}: {}", path.display(), e);
                continue;
            }
        };
        let hash_alg = session
            .best_supported_rsa_hash()
            .await
            .ok()
            .flatten()
            .flatten();
        let result = session
            .authenticate_publickey(
                user,
                PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
            )
            .await
            .map_err(|e| {
                SshClientError::Other(anyhow::anyhow!(
                    "authenticate_publickey({}): {}",
                    path.display(),
                    e
                ))
            })?;
        if result.success() {
            return Ok(Some(true));
        }
    }
    if tried_any {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

/// russh client handler. Implements host-key checking based on
/// `strict_host_key_checking`:
/// - "no" / "off" / "accept-new": accept any key
/// - "yes" / "strict": require entry in ~/.ssh/known_hosts (best-effort
///   parse — falls back to accept if the file is missing).
struct ClientHandler {
    strict: String,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        match self.strict.as_str() {
            "no" | "off" | "accept-new" => Ok(true),
            _ => Ok(check_known_hosts(server_public_key)),
        }
    }
}

fn check_known_hosts(key: &PublicKey) -> bool {
    let Some(home) = dirs::home_dir() else {
        return true;
    };
    let known_hosts = home.join(".ssh/known_hosts");
    if !known_hosts.exists() {
        // No known_hosts file — fall back to trust on first use, since the
        // user has explicitly opted into strict checking but has no
        // baseline to compare against.
        return true;
    }
    let key_str = match key.to_openssh() {
        Ok(s) => s,
        Err(_) => return false,
    };
    match std::fs::read_to_string(&known_hosts) {
        Ok(contents) => contents.lines().any(|line| line.contains(&key_str)),
        Err(_) => true,
    }
}

