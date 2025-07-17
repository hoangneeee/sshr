//! SFTP module for handling local and remote file operations

mod local;
mod remote;
mod state;
mod transfer;
pub mod types;

pub use types::AppSftpState;
pub use types::{FileItem, PanelSide};