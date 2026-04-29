use crate::app::session::{SessionState, SshSession, SshStage};
use crate::app::App;
use crate::app_event::SshEvent;
use crate::constants::{
    SSH_CONNECT_TIMEOUT_S, SSH_EVENT_CHANNEL_BUFFER, SSH_KEEPALIVE_COUNT_MAX,
    SSH_KEEPALIVE_INTERVAL_S, SSH_PRE_LAUNCH_DELAY, SSH_TEST_TIMEOUT_S,
};
use crate::models::SshHost;
use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc::{self, Sender};

impl App {
    /// Suspend the TUI for the duration of a foreground SSH child process.
    pub fn transition_to_ssh_mode<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        disable_raw_mode().context("Failed to disable raw mode for SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, LeaveAlternateScreen, DisableMouseCapture)
            .context("Failed to leave alternate screen for SSH")?;
        terminal
            .show_cursor()
            .context("Failed to show cursor for SSH")?;

        tracing::info!("TUI disabled for SSH mode - main thread will suspend polling");
        Ok(())
    }

    /// Re-enable the TUI after the SSH child process exits.
    pub fn restore_tui_mode<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        enable_raw_mode().context("Failed to re-enable raw mode post-SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to re-enter alternate screen post-SSH")?;

        terminal
            .clear()
            .context("Failed to clear terminal post-SSH")?;
        tracing::info!("TUI restored after SSH session - resuming main thread polling");
        Ok(())
    }

    /// Begin connecting to `host` over SSH. Sets `SessionState::Ssh(Connecting)`,
    /// emits a status banner, and spawns the worker that drives the ssh child.
    pub fn connect_ssh(&mut self, host: SshHost) {
        tracing::info!("Connecting to host: {:?}", host.alias);
        self.ui.status_message =
            Some((format!("Connecting to {}...", host.alias), Instant::now()));

        let (sender, receiver) = mpsc::channel::<SshEvent>(SSH_EVENT_CHANNEL_BUFFER);
        self.session = SessionState::Ssh(SshSession {
            host: host.clone(),
            stage: SshStage::Connecting,
            event_rx: receiver,
        });

        let strict = self.ctx.strict_host_key_checking.clone();
        tokio::task::spawn_blocking(move || {
            Self::ssh_thread_worker(sender, host, strict);
        });
    }

    fn ssh_thread_worker(
        sender: Sender<SshEvent>,
        host: SshHost,
        strict_host_key_checking: String,
    ) {
        tracing::info!("SSH thread started for host: {}", host.alias);

        if sender.blocking_send(SshEvent::Connecting).is_err() {
            tracing::error!("Failed to send Connecting event");
            return;
        }

        match Self::test_ssh_connection(&host, &strict_host_key_checking) {
            Ok(_) => {
                tracing::info!("SSH connection test successful for {}", host.alias);

                if sender.blocking_send(SshEvent::Connected).is_ok() {
                    thread::sleep(SSH_PRE_LAUNCH_DELAY);

                    tracing::info!("Starting SSH session for {}", host.alias);
                    match Self::execute_ssh_blocking(&host) {
                        Ok(_) => {
                            tracing::info!("SSH session ended normally for {}", host.alias);
                            let _ = sender.blocking_send(SshEvent::Disconnected);
                        }
                        Err(e) => {
                            tracing::error!("SSH session error for {}: {}", host.alias, e);
                            let _ = sender.blocking_send(SshEvent::Error(e.to_string()));
                        }
                    }
                } else {
                    tracing::error!("Failed to send Connected event");
                }
            }
            Err(e) => {
                tracing::error!("SSH connection test failed for {}: {}", host.alias, e);
                let _ = sender
                    .blocking_send(SshEvent::Error(format!("Connection test failed: {}", e)));
            }
        }

        tracing::info!("SSH thread ending for host: {}", host.alias);
    }

    fn test_ssh_connection(host: &SshHost, strict_host_key_checking: &str) -> Result<()> {
        use std::process::Command;

        let port_str = host.port.unwrap_or(22).to_string();

        tracing::info!(
            "Testing SSH connection to {}@{}:{}",
            host.user,
            host.host,
            port_str
        );

        let output = Command::new("ssh")
            .arg(format!("{}@{}", host.user, host.host))
            .arg("-p")
            .arg(&port_str)
            .arg("-o")
            .arg(format!("ConnectTimeout={}", SSH_TEST_TIMEOUT_S))
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg(format!("StrictHostKeyChecking={}", strict_host_key_checking))
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg("exit")
            .output()
            .context("Failed to test SSH connection")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!(
                "SSH connection test failed: {}",
                stderr.trim()
            ))
        }
    }

    fn execute_ssh_blocking(host: &SshHost) -> Result<()> {
        use std::process::Command;

        let port_str = host.port.unwrap_or(22).to_string();
        let connection_str = format!("{}@{}", host.user, host.host);

        tracing::info!("Executing SSH: ssh {} -p {}", connection_str, port_str);

        let status = Command::new("ssh")
            .arg(&connection_str)
            .arg("-p")
            .arg(&port_str)
            .arg("-o")
            .arg(format!("ConnectTimeout={}", SSH_CONNECT_TIMEOUT_S))
            .arg("-o")
            .arg(format!("ServerAliveInterval={}", SSH_KEEPALIVE_INTERVAL_S))
            .arg("-o")
            .arg(format!("ServerAliveCountMax={}", SSH_KEEPALIVE_COUNT_MAX))
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to execute SSH command")?;

        if status.success() {
            tracing::info!("SSH command completed successfully");
            Ok(())
        } else {
            let error_msg = format!("SSH command failed with status: {}", status);
            tracing::error!("{}", error_msg);
            Err(anyhow::anyhow!(error_msg))
        }
    }
}
