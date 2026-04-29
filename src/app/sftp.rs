use crate::app::session::{SessionState, SftpSession, SftpStage};
use crate::app::App;
use crate::app_event::{SftpEvent, TransferEvent};
use crate::constants::{
    SFTP_EVENT_CHANNEL_BUFFER, SSH_PRE_LAUNCH_DELAY, TRANSFER_CHANNEL_BUFFER,
};
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc::{self, Sender};

impl App {
    /// Begin connecting to `host` over SFTP. Sets `SessionState::Sftp(Loading)`,
    /// emits a status banner, and spawns the worker that establishes the
    /// SFTP session.
    pub fn connect_sftp(&mut self, host: SshHost) {
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
        tokio::task::spawn_blocking(move || {
            Self::sftp_thread_worker(sftp_sender, host, transfer_sender, strict);
        });
    }

    fn sftp_thread_worker(
        sender: Sender<SftpEvent>,
        host: SshHost,
        transfer_tx: Sender<TransferEvent>,
        strict_host_key_checking: String,
    ) {
        tracing::info!("SFTP thread started for host: {}", host.alias);

        if sender.blocking_send(SftpEvent::Connecting).is_err() {
            tracing::error!("Failed to send Connecting event");
            return;
        }

        match AppSftpState::new(
            &host.user,
            &host.host,
            host.port.unwrap_or(22),
            transfer_tx,
            &strict_host_key_checking,
        ) {
            Ok(sftp_state) => {
                tracing::info!("SFTP connection test successful for {}", host.alias);

                if sender
                    .blocking_send(SftpEvent::PreConnected(Box::new(sftp_state)))
                    .is_err()
                {
                    tracing::error!("Failed to send PreConnected event");
                    return;
                }
                if sender.blocking_send(SftpEvent::Connected).is_ok() {
                    thread::sleep(SSH_PRE_LAUNCH_DELAY);
                    tracing::info!("Starting SFTP session for {}", host.alias);
                } else {
                    tracing::error!("Failed to send Connected event");
                }
            }
            Err(e) => {
                tracing::error!("SFTP connection test failed for {}: {}", host.alias, e);
                let _ = sender
                    .blocking_send(SftpEvent::Error(format!("Connection test failed: {}", e)));
            }
        }
    }
}
