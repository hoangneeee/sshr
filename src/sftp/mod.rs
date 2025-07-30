//! SFTP module for handling local and remote file operations

mod local;
mod remote;
pub mod state;
mod transfer;
pub mod ui;

pub use state::AppSftpState;
