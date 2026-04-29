//! Centralized constants for timeouts, polling intervals, and channel sizes.

use std::time::Duration;

// Event loop polling
pub const POLL_NORMAL: Duration = Duration::from_millis(100);
pub const POLL_CONNECTING: Duration = Duration::from_millis(50);

// SSH session
pub const SSH_PRE_LAUNCH_DELAY: Duration = Duration::from_millis(200);
pub const SSH_TEST_TIMEOUT_S: u64 = 5;
pub const SSH_CONNECT_TIMEOUT_S: u64 = 30;
pub const SSH_KEEPALIVE_INTERVAL_S: u64 = 60;
pub const SSH_KEEPALIVE_COUNT_MAX: u64 = 3;

// File transfer
pub const TRANSFER_CHANNEL_BUFFER: usize = 100;
pub const TRANSFER_BUFFER_SIZE: usize = 8192;

// Control-plane event channels (SSH connect/SFTP setup)
pub const SSH_EVENT_CHANNEL_BUFFER: usize = 16;
pub const SFTP_EVENT_CHANNEL_BUFFER: usize = 16;
