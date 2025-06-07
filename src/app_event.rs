use crate::sftp_logic::AppSftpState;

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
    Disconnected,
    Error(String),
    // ListLocalDone(Vec<crate::models::DirEntry>),
    // ListRemoteDone(Vec<crate::models::DirEntry>),
    // ChangeDirDone(String), // New path
    // UploadStarted(String),
    // UploadProgress(String, u64, u64),
    // UploadCompleted(String),
    // UploadFailed(String, String),
    // DownloadStarted(String),
    // DownloadProgress(String, u64, u64),
    // DownloadCompleted(String),
    // DownloadFailed(String, String),
    // StatusUpdate(String),
}