use super::types::{AppSftpState, FileItem};
use anyhow::{Context, Result};

impl AppSftpState {
    /// Refresh the remote file list using the persistent SFTP session.
    pub async fn refresh_remote(&mut self) -> Result<()> {
        self.set_status_message("Loading remote directory...");
        self.remote_files =
            list_remote_dir(&self.client.sftp, &self.remote_current_path).await?;
        if self.remote_selected >= self.remote_files.len() {
            self.remote_selected = self.remote_files.len().saturating_sub(1);
        }
        self.clear_status_message();
        Ok(())
    }

    pub fn navigate_remote_up(&mut self) {
        if self.remote_selected > 0 {
            self.remote_selected -= 1;
        } else if !self.remote_files.is_empty() {
            self.remote_selected = self.remote_files.len() - 1;
        }
        self.remote_list_state.select(Some(self.remote_selected));
    }

    pub fn navigate_remote_down(&mut self) {
        if self.remote_selected < self.remote_files.len().saturating_sub(1) {
            self.remote_selected += 1;
        } else {
            self.remote_selected = 0;
        }
        self.remote_list_state.select(Some(self.remote_selected));
    }

    pub async fn open_remote_selected(&mut self) -> Result<()> {
        let Some(item) = self.remote_files.get(self.remote_selected).cloned() else {
            return Ok(());
        };
        match item {
            FileItem::Directory { name } => {
                if name == ".." {
                    if self.remote_current_path != "/" {
                        let mut path_parts: Vec<&str> = self
                            .remote_current_path
                            .trim_end_matches('/')
                            .split('/')
                            .collect();
                        if path_parts.len() > 1 {
                            path_parts.pop();
                            self.remote_current_path = if path_parts.len() == 1 {
                                "/".to_string()
                            } else {
                                path_parts.join("/")
                            };
                        }
                    }
                } else {
                    self.remote_current_path = if self.remote_current_path.ends_with('/') {
                        format!("{}{}", self.remote_current_path, name)
                    } else {
                        format!("{}/{}", self.remote_current_path, name)
                    };
                }
                self.remote_selected = 0;
                self.remote_list_state.select(Some(self.remote_selected));
                self.refresh_remote().await?;
            }
            FileItem::File { .. } => {
                // Files can't be opened in file browser context
            }
        }
        Ok(())
    }
}

async fn list_remote_dir(
    sftp: &russh_sftp::client::SftpSession,
    remote_path: &str,
) -> Result<Vec<FileItem>> {
    let mut items = Vec::new();
    if remote_path != "/" {
        items.push(FileItem::Directory {
            name: "..".to_string(),
        });
    }

    let entries = sftp
        .read_dir(remote_path)
        .await
        .with_context(|| format!("read_dir({}) failed", remote_path))?;

    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let metadata = entry.metadata();
        if metadata.is_dir() {
            items.push(FileItem::Directory { name });
        } else {
            items.push(FileItem::File {
                name,
                size: metadata.size.unwrap_or(0),
            });
        }
    }

    items.sort_by(|a, b| {
        use std::cmp::Ordering;
        match (a, b) {
            (FileItem::Directory { name: na }, FileItem::Directory { name: nb }) => {
                if na == ".." {
                    Ordering::Less
                } else if nb == ".." {
                    Ordering::Greater
                } else {
                    na.cmp(nb)
                }
            }
            (FileItem::Directory { .. }, FileItem::File { .. }) => Ordering::Less,
            (FileItem::File { .. }, FileItem::Directory { .. }) => Ordering::Greater,
            (FileItem::File { name: na, .. }, FileItem::File { name: nb, .. }) => na.cmp(nb),
        }
    });

    Ok(items)
}
