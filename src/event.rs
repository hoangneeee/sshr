

#[derive(Debug, Clone)]
pub enum SshEvent {
    Connecting,
    Connected,
    Error(String),
    Disconnected,
}