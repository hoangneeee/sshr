use crate::sftp_logic::AppSftpState;

#[derive(Debug, Clone)]
pub enum SshEvent {
    Connecting,
    Connected,
    Error(String),
    Disconnected,
}

#[derive(Debug)]
pub enum SftpEvent {
    Connecting,
    PreConnected(Box<AppSftpState>),
    Connected,
    #[allow(dead_code)]
    Disconnected,
    Error(String),
    /// Auth could not be established with keys/agent and the worker needs
    /// the user to supply a password to retry.
    AuthRequired {
        host: crate::models::SshHost,
        /// `true` if a password was already tried and was rejected.
        retry: bool,
    },
}

#[derive(Debug, Clone)]
pub enum TransferEvent {
    UploadProgress(String, u64, u64),
    UploadComplete(String),
    UploadError(String, String),
    DownloadProgress(String, u64, u64),
    DownloadComplete(String),
    DownloadError(String, String),
}
