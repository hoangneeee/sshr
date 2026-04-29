use crate::app::session::{SessionState, SftpSession, SftpStage};
use crate::app::App;
use crate::app_event::{SftpEvent, TransferEvent};
use crate::constants::{SFTP_EVENT_CHANNEL_BUFFER, TRANSFER_CHANNEL_BUFFER};
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use crate::ssh_client::SshClientError;
use std::time::Instant;
use tokio::sync::mpsc::{self, Sender};

impl App {
    /// Begin connecting to `host` over SFTP. Sets `SessionState::Sftp(Loading)`,
    /// emits a status banner, and spawns the worker that establishes the
    /// SFTP session. If `password` is `Some`, it is used as a fallback
    /// after key/agent auth fails.
    pub fn connect_sftp(&mut self, host: SshHost, password: Option<String>) {
        let (sftp_sender, sftp_rx) = mpsc::channel::<SftpEvent>(SFTP_EVENT_CHANNEL_BUFFER);
        let (transfer_sender, transfer_rx) =
            mpsc::channel::<TransferEvent>(TRANSFER_CHANNEL_BUFFER);

        self.session = SessionState::Sftp(SftpSession {
            stage: SftpStage::Loading,
            event_rx: sftp_rx,
            transfer_rx,
            data: None,
        });
        self.ui.status_message = Some((
            format!("Initializing SFTP for {}...", host.alias),
            Instant::now(),
        ));

        let strict = self.ctx.strict_host_key_checking.clone();
        tokio::spawn(sftp_async_worker(
            sftp_sender,
            host,
            transfer_sender,
            strict,
            password,
        ));
    }
}

async fn sftp_async_worker(
    sender: Sender<SftpEvent>,
    host: SshHost,
    transfer_tx: Sender<TransferEvent>,
    strict_host_key_checking: String,
    password: Option<String>,
) {
    tracing::info!("SFTP async worker started for host: {}", host.alias);

    if sender.send(SftpEvent::Connecting).await.is_err() {
        tracing::error!("Failed to send Connecting event");
        return;
    }

    let attempted_with_password = password.is_some();
    let result = AppSftpState::new(
        &host.user,
        &host.host,
        host.port.unwrap_or(22),
        transfer_tx,
        &strict_host_key_checking,
        password,
    )
    .await;

    match result {
        Ok(sftp_state) => {
            tracing::info!("SFTP connection established for {}", host.alias);
            if sender
                .send(SftpEvent::PreConnected(Box::new(sftp_state)))
                .await
                .is_err()
            {
                tracing::error!("Failed to send PreConnected event");
                return;
            }
            let _ = sender.send(SftpEvent::Connected).await;
        }
        Err(SshClientError::AuthRequiresPassword) | Err(SshClientError::BadPassword) => {
            tracing::warn!(
                "SFTP auth needs password for {} (retry={})",
                host.alias,
                attempted_with_password
            );
            let _ = sender
                .send(SftpEvent::AuthRequired {
                    host,
                    retry: attempted_with_password,
                })
                .await;
        }
        Err(e) => {
            tracing::error!("SFTP connection failed for {}: {}", host.alias, e);
            let _ = sender.send(SftpEvent::Error(e.to_string())).await;
        }
    }
}
