use crate::sftp::AppSftpState;

#[derive(Debug, Clone)]
pub enum SshEvent {
    Connecting,
    Connected,
    Error(String),
    Disconnected,
}

#[derive(Debug, Clone)]
pub enum SftpEvent {
    Connecting,
    PreConnected(AppSftpState),
    Connected,
    #[allow(dead_code)]
    Disconnected,
    Error(String),
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
