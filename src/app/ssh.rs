use crate::app::App;
use crate::app_event::SshEvent;
use crate::constants::{
    SSH_CONNECT_TIMEOUT_S, SSH_KEEPALIVE_COUNT_MAX, SSH_KEEPALIVE_INTERVAL_S,
    SSH_PRE_LAUNCH_DELAY, SSH_TEST_TIMEOUT_S,
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
use tokio::sync::mpsc::Sender;

impl App {
    pub(crate) fn transition_to_ssh_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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

    pub(crate) fn restore_tui_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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

    pub fn ssh_thread_worker(sender: Sender<SshEvent>, host: SshHost, strict_host_key_checking: String) {
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
                let _ = sender.blocking_send(SshEvent::Error(format!("Connection test failed: {}", e)));
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

    /// Apply a single SshEvent. Returns `true` if the SSH session ended
    /// (caller should redraw the host list).
    fn apply_ssh_event<B: Backend>(
        &mut self,
        event: SshEvent,
        terminal: &mut Terminal<B>,
    ) -> Result<bool> {
        match event {
            SshEvent::Connecting => {
                self.status_message =
                    Some(("Testing connection...".to_string(), Instant::now()));
                Ok(false)
            }
            SshEvent::Connected => {
                self.status_message = Some((
                    "Connection successful! Launching SSH...".to_string(),
                    Instant::now(),
                ));
                self.transition_to_ssh_mode(terminal)?;
                self.ssh_ready_for_terminal = true;
                Ok(false)
            }
            SshEvent::Error(err) => {
                tracing::error!("SSH error: {}", err);
                self.is_connecting = false;
                self.connecting_host = None;
                self.ssh_ready_for_terminal = false;
                self.ssh_receiver = None;
                self.status_message = Some((format!("SSH Error: {}", err), Instant::now()));

                if let Err(e) = self.restore_tui_mode(terminal) {
                    tracing::error!("Failed to restore TUI mode after SSH error: {}", e);
                }
                Ok(false)
            }
            SshEvent::Disconnected => {
                tracing::info!("SSH session disconnected, restoring TUI");
                self.restore_tui_mode(terminal)?;
                self.is_connecting = false;
                self.connecting_host = None;
                self.ssh_ready_for_terminal = false;
                self.ssh_receiver = None;
                self.status_message =
                    Some(("SSH session ended".to_string(), Instant::now()));
                Ok(true)
            }
        }
    }

    /// Non-blocking poll: drain at most one pending SSH event.
    pub fn process_ssh_events<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<bool> {
        let event = self
            .ssh_receiver
            .as_mut()
            .and_then(|rx| rx.try_recv().ok());
        match event {
            Some(ev) => self.apply_ssh_event(ev, terminal),
            None => Ok(false),
        }
    }

    /// Blocking await: wait for the next SSH event, then process it.
    /// Returns `Ok(true)` when the SSH session has ended (caller should
    /// resume the main loop).
    pub async fn await_next_ssh_event<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<bool> {
        let event = match self.ssh_receiver.as_mut() {
            Some(rx) => rx.recv().await,
            None => return Ok(true),
        };
        match event {
            Some(ev) => self.apply_ssh_event(ev, terminal),
            // Sender dropped without a Disconnected event — treat as end-of-session.
            None => Ok(true),
        }
    }
}
