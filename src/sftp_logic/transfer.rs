use super::types::{AppSftpState, DownloadProgress, UploadProgress};
use crate::app_event::TransferEvent;
use crate::constants::{
    DOWNLOAD_NO_PROGRESS_TIMEOUT_S, SSH_CONNECT_TIMEOUT_S, TRANSFER_BUFFER_SIZE,
    TRANSFER_PROGRESS_POLL,
};
use anyhow::{Context, Result};
use shell_escape::unix::escape;
use std::borrow::Cow;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

impl AppSftpState {
    /// Upload a file to the remote server
    pub fn upload_file(&mut self) {
        let Some(super::FileItem::File { name, .. }) =
            self.local_files.get(self.local_selected).cloned()
        else {
            self.set_status_message("Please select a file to upload");
            return;
        };

        let local_path = self.local_current_path.join(&name);
        let remote_path = if self.remote_current_path.ends_with('/') {
            format!("{}{}", self.remote_current_path, name)
        } else {
            format!("{}/{}", self.remote_current_path, name)
        };

        // Mark progress eagerly so the UI blocks repeat 'u' presses before
        // the worker emits its first UploadProgress event.
        self.upload_progress = Some(UploadProgress {
            file_name: name.clone(),
            uploaded_size: 0,
            total_size: 0,
        });

        let ssh_user = self.ssh_user.clone();
        let ssh_host = self.ssh_host.clone();
        let ssh_port = self.ssh_port;
        let strict_host_key_checking = self.strict_host_key_checking.clone();
        let tx = self.transfer_tx.clone();

        tokio::spawn(async move {
            let name_clone = name.clone();
            let progress_tx = tx.clone();
            let result = Self::sftp_upload(
                &ssh_user,
                &ssh_host,
                ssh_port,
                &local_path,
                &remote_path,
                &strict_host_key_checking,
                move |uploaded, total| {
                    let _ = progress_tx.try_send(TransferEvent::UploadProgress(
                        name_clone.clone(),
                        uploaded,
                        total,
                    ));
                },
            )
            .await;

            match result {
                Ok(_) => {
                    tracing::info!("Successfully uploaded {}", name);
                    let _ = tx.send(TransferEvent::UploadComplete(name.clone())).await;
                }
                Err(e) => {
                    tracing::error!("Failed to upload {}: {}", name, e);
                    let _ = tx
                        .send(TransferEvent::UploadError(name.clone(), e.to_string()))
                        .await;
                }
            }
        });
    }

    /// Download a file from the remote server
    pub fn download_file(&mut self) {
        let Some(super::FileItem::File { name, .. }) =
            self.remote_files.get(self.remote_selected).cloned()
        else {
            self.set_status_message("Please select a file to download");
            return;
        };

        let remote_path = if self.remote_current_path.ends_with('/') {
            format!("{}{}", self.remote_current_path, name)
        } else {
            format!("{}/{}", self.remote_current_path, name)
        };
        let local_path = self.local_current_path.join(&name);

        // Mark progress eagerly so the UI blocks repeat 'd' presses before
        // the worker emits its first DownloadProgress event.
        self.download_progress = Some(DownloadProgress {
            file_name: name.clone(),
            downloaded_size: 0,
            total_size: 0,
        });

        let ssh_user = self.ssh_user.clone();
        let ssh_host = self.ssh_host.clone();
        let ssh_port = self.ssh_port;
        let strict_host_key_checking = self.strict_host_key_checking.clone();
        let tx = self.transfer_tx.clone();

        tokio::spawn(async move {
            let name_clone = name.clone();
            let progress_tx = tx.clone();
            let result = Self::sftp_download(
                &ssh_user,
                &ssh_host,
                ssh_port,
                &remote_path,
                &local_path,
                &strict_host_key_checking,
                move |downloaded, total| {
                    let _ = progress_tx.try_send(TransferEvent::DownloadProgress(
                        name_clone.clone(),
                        downloaded,
                        total,
                    ));
                },
            )
            .await;

            match result {
                Ok(_) => {
                    tracing::info!("Successfully downloaded {}", name);
                    let _ = tx.send(TransferEvent::DownloadComplete(name.clone())).await;
                }
                Err(e) => {
                    tracing::error!("Failed to download {}: {}", name, e);
                    let _ = tx
                        .send(TransferEvent::DownloadError(name.clone(), e.to_string()))
                        .await;
                }
            }
        });
    }

    /// Upload a file using SCP with progress tracking
    async fn sftp_upload<F>(
        user: &str,
        host: &str,
        port: u16,
        local_path: &Path,
        remote_path: &str,
        strict_host_key_checking: &str,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64) + Send + 'static,
    {
        let total_size = tokio::fs::metadata(local_path)
            .await
            .context("Failed to read local file metadata")?
            .len();

        let escaped_remote_path = escape(Cow::Borrowed(remote_path)).into_owned();
        let mut command = Command::new("scp")
            .arg("-P")
            .arg(port.to_string())
            .arg("-o")
            .arg(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT_S))
            .arg("-o")
            .arg(format!("StrictHostKeyChecking={}", strict_host_key_checking))
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(local_path)
            .arg(format!("{}@{}:{}", user, host, escaped_remote_path))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start scp upload command")?;

        let mut stdin = command
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get scp stdin"))?;

        // NOTE: scp(1) reads the source from its positional argument, not stdin.
        // This loop drives the progress callback but doesn't actually feed scp.
        // Phase 4 will replace this with russh-sftp for accurate progress.
        let mut file = File::open(local_path)
            .await
            .context("Failed to open local file")?;
        let mut buffer = vec![0u8; TRANSFER_BUFFER_SIZE];
        let mut uploaded = 0u64;

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .await
                .context("Failed to read from file")?;
            if bytes_read == 0 {
                break;
            }

            stdin
                .write_all(&buffer[..bytes_read])
                .await
                .context("Failed to write to scp")?;
            uploaded += bytes_read as u64;

            progress_callback(uploaded, total_size);
        }

        drop(stdin); // Close stdin to signal end of data

        let output = command
            .wait_with_output()
            .await
            .context("Failed to complete scp upload command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP upload failed: {}", stderr));
        }

        Ok(())
    }

    /// Download a file using SCP with progress tracking
    async fn sftp_download<F: Fn(u64, u64) + Send + 'static>(
        user: &str,
        host: &str,
        port: u16,
        remote_path: &str,
        local_path: &Path,
        strict_host_key_checking: &str,
        progress_callback: F,
    ) -> Result<()> {
        let escaped_remote_path = escape(Cow::Borrowed(remote_path)).into_owned();

        // First, get the remote file size
        let size_output = Command::new("ssh")
            .arg("-p")
            .arg(port.to_string())
            .arg("-o")
            .arg(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT_S))
            .arg("-o")
            .arg(format!("StrictHostKeyChecking={}", strict_host_key_checking))
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("{}@{}", user, host))
            .arg(format!("stat -c%s {}", escaped_remote_path))
            .output()
            .await
            .context("Failed to get remote file size")?;

        if !size_output.status.success() {
            let stderr = String::from_utf8_lossy(&size_output.stderr);
            return Err(anyhow::anyhow!("Failed to get remote file size: {}", stderr));
        }

        let total_size = String::from_utf8_lossy(&size_output.stdout)
            .trim()
            .parse::<u64>()
            .context("Failed to parse remote file size")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create local directory")?;
        }

        // Initial progress update
        progress_callback(0, total_size);

        // Drive the scp child to completion while polling the local file size
        // for progress. tokio::select! lets us wait on the process exit OR a
        // poll tick, no oneshot/abort dance required.
        let scp_fut = Command::new("scp")
            .arg("-P")
            .arg(port.to_string())
            .arg("-o")
            .arg(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT_S))
            .arg("-o")
            .arg(format!("StrictHostKeyChecking={}", strict_host_key_checking))
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("{}@{}:{}", user, host, escaped_remote_path))
            .arg(local_path)
            .status();
        tokio::pin!(scp_fut);

        let start = std::time::Instant::now();
        let mut last_size = 0u64;

        loop {
            tokio::select! {
                status = &mut scp_fut => {
                    let status = status.context("scp download did not run to completion")?;
                    if !status.success() {
                        return Err(anyhow::anyhow!(
                            "SCP download failed with status: {}", status
                        ));
                    }
                    progress_callback(total_size, total_size);
                    return Ok(());
                }
                _ = tokio::time::sleep(TRANSFER_PROGRESS_POLL) => {
                    if let Ok(meta) = tokio::fs::metadata(local_path).await {
                        let current = meta.len();
                        if current > last_size {
                            last_size = current;
                            progress_callback(current, total_size);
                        }
                    }
                    if last_size == 0
                        && start.elapsed().as_secs() > DOWNLOAD_NO_PROGRESS_TIMEOUT_S
                    {
                        return Err(anyhow::anyhow!("Download timed out with no progress"));
                    }
                }
            }
        }
    }
}
