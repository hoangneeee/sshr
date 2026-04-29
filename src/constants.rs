//! Centralized constants for timeouts, polling intervals, and channel sizes.

use std::time::Duration;

// Event loop polling
pub const POLL_NORMAL: Duration = Duration::from_millis(100);
pub const POLL_CONNECTING: Duration = Duration::from_millis(50);

// SSH session
pub const SSH_SUSPEND_POLL: Duration = Duration::from_millis(500);
pub const SSH_PRE_LAUNCH_DELAY: Duration = Duration::from_millis(200);
pub const SSH_TEST_TIMEOUT_S: u64 = 5;
pub const SSH_CONNECT_TIMEOUT_S: u64 = 30;
pub const SSH_REMOTE_LIST_TIMEOUT_S: u64 = 10;
pub const SSH_KEEPALIVE_INTERVAL_S: u64 = 60;
pub const SSH_KEEPALIVE_COUNT_MAX: u64 = 3;

// Transfer (upload/download) progress polling
pub const TRANSFER_PROGRESS_POLL: Duration = Duration::from_millis(100);
pub const DOWNLOAD_NO_PROGRESS_TIMEOUT_S: u64 = 300;
pub const TRANSFER_CHANNEL_BUFFER: usize = 100;
pub const TRANSFER_BUFFER_SIZE: usize = 8192;
