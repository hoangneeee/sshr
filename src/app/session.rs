use crate::app_event::{SftpEvent, SshEvent, TransferEvent};
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use tokio::sync::mpsc::Receiver;

/// Lifecycle of an in-flight SSH session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshStage {
    /// Worker is testing the connection or waiting for the test to complete.
    Connecting,
    /// SSH is the foreground process; main loop is suspended.
    Active,
}

/// Lifecycle of an in-flight SFTP session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpStage {
    /// Worker is establishing the SFTP connection.
    Loading,
    /// SFTP UI is the foreground; user can browse/transfer.
    Active,
}

#[derive(Debug)]
pub struct SshSession {
    pub host: SshHost,
    pub stage: SshStage,
    pub event_rx: Receiver<SshEvent>,
}

#[derive(Debug)]
pub struct SftpSession {
    pub stage: SftpStage,
    pub event_rx: Receiver<SftpEvent>,
    pub transfer_rx: Receiver<TransferEvent>,
    /// Populated by the worker once the SFTP connection is established.
    pub data: Option<AppSftpState>,
}

/// What the user is currently doing. SSH and SFTP are mutually exclusive.
#[derive(Debug, Default)]
pub enum SessionState {
    #[default]
    Idle,
    Ssh(SshSession),
    Sftp(SftpSession),
}

impl SessionState {
    /// True while an SSH session is being negotiated (pre-launch).
    pub fn is_ssh_connecting(&self) -> bool {
        matches!(self, Self::Ssh(s) if s.stage == SshStage::Connecting)
    }

    /// True while the SSH child process owns the terminal.
    pub fn is_ssh_active(&self) -> bool {
        matches!(self, Self::Ssh(s) if s.stage == SshStage::Active)
    }

    pub fn is_sftp_loading(&self) -> bool {
        matches!(self, Self::Sftp(s) if s.stage == SftpStage::Loading)
    }

    /// The host being connected to, if an SSH session is in flight.
    pub fn connecting_host(&self) -> Option<&SshHost> {
        match self {
            Self::Ssh(s) => Some(&s.host),
            _ => None,
        }
    }

    pub fn ssh_session_mut(&mut self) -> Option<&mut SshSession> {
        match self {
            Self::Ssh(s) => Some(s),
            _ => None,
        }
    }

    pub fn sftp_session(&self) -> Option<&SftpSession> {
        match self {
            Self::Sftp(s) => Some(s),
            _ => None,
        }
    }

    pub fn sftp_session_mut(&mut self) -> Option<&mut SftpSession> {
        match self {
            Self::Sftp(s) => Some(s),
            _ => None,
        }
    }

    pub fn sftp_data(&self) -> Option<&AppSftpState> {
        self.sftp_session().and_then(|s| s.data.as_ref())
    }

    pub fn sftp_data_mut(&mut self) -> Option<&mut AppSftpState> {
        self.sftp_session_mut().and_then(|s| s.data.as_mut())
    }

    /// Reset to Idle, dropping any active session (closes channels).
    pub fn reset(&mut self) {
        *self = Self::Idle;
    }
}
