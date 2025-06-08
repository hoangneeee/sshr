use anyhow::{Context, Result};
use std::process::Command;
use super::types::{FileItem, AppSftpState};

impl AppSftpState {
    /// Refresh the remote file list
    pub fn refresh_remote(&mut self) -> Result<()> {
        self.set_status_message("Loading remote directory...");
        self.remote_files = Self::read_remote_directory(
            &self.ssh_user, 
            &self.ssh_host, 
            self.ssh_port, 
            &self.remote_current_path
        )?;
        if self.remote_selected >= self.remote_files.len() {
            self.remote_selected = self.remote_files.len().saturating_sub(1);
        }
        self.clear_status_message();
        Ok(())
    }
    
    /// Navigate up in the remote file list
    pub fn navigate_remote_up(&mut self) {
        if self.remote_selected > 0 {
            self.remote_selected -= 1;
        } else if !self.remote_files.is_empty() {
            self.remote_selected = self.remote_files.len() - 1;
        }
        self.remote_list_state.select(Some(self.remote_selected));
    }
    
    /// Navigate down in the remote file list
    pub fn navigate_remote_down(&mut self) {
        if self.remote_selected < self.remote_files.len().saturating_sub(1) {
            self.remote_selected += 1;
        } else {
            self.remote_selected = 0;
        }
        self.remote_list_state.select(Some(self.remote_selected));
    }
    
    /// Open the selected item in the remote file list
    pub fn open_remote_selected(&mut self) -> Result<()> {
        if let Some(item) = self.remote_files.get(self.remote_selected) {
            match item {
                FileItem::Directory { name } => {
                    if name == ".." {
                        // Go to parent directory
                        if self.remote_current_path != "/" {
                            let mut path_parts: Vec<&str> = self.remote_current_path
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
                        // Enter directory
                        self.remote_current_path = if self.remote_current_path.ends_with('/') {
                            format!("{}{}", self.remote_current_path, name)
                        } else {
                            format!("{}/{}", self.remote_current_path, name)
                        };
                    }
                    self.remote_selected = 0;
                    self.remote_list_state.select(Some(self.remote_selected));
                    self.refresh_remote()?;
                }
                FileItem::File { .. } => {
                    // Files can't be opened in file browser context
                }
            }
        }
        Ok(())
    }
    
    /// Go up one directory in the remote file system
    pub fn go_remote_back(&mut self) -> Result<()> {
        if self.remote_current_path != "/" {
            let mut path_parts: Vec<&str> = self.remote_current_path
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
                self.remote_selected = 0;
                self.refresh_remote()?;
            }
        }
        Ok(())
    }
    
    /// Read the contents of a remote directory
    fn read_remote_directory(
        user: &str,
        host: &str,
        port: u16,
        remote_path: &str,
    ) -> Result<Vec<FileItem>> {
        let output = Command::new("ssh")
            .arg(format!("{}@{}", user, host))
            .arg("-p")
            .arg(port.to_string())
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("ls -la '{}'", remote_path))
            .output()
            .context("Failed to execute remote ls command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Remote ls failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut items = Vec::new();
        
        // Add parent directory entry if not at root
        if remote_path != "/" {
            items.push(FileItem::Directory {
                name: "..".to_string(),
            });
        }
        
        for line in stdout.lines().skip(1) { // Skip total line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }
            
            let permissions = parts[0];
            let file_name = parts[8..].join(" ");
            
            // Skip . and .. entries (we handle .. manually)
            if file_name == "." || file_name == ".." {
                continue;
            }
            
            if permissions.starts_with('d') {
                items.push(FileItem::Directory { name: file_name });
            } else {
                let size = parts[4].parse::<u64>().unwrap_or(0);
                items.push(FileItem::File {
                    name: file_name,
                    size,
                });
            }
        }
        
        // Sort: directories first, then files, both alphabetically
        items.sort_by(|a, b| {
            use std::cmp::Ordering;
            match (a, b) {
                (FileItem::Directory { name: name_a }, FileItem::Directory { name: name_b }) => {
                    if name_a == ".." {
                        Ordering::Less
                    } else if name_b == ".." {
                        Ordering::Greater
                    } else {
                        name_a.cmp(name_b)
                    }
                }
                (FileItem::Directory { .. }, FileItem::File { .. }) => Ordering::Less,
                (FileItem::File { .. }, FileItem::Directory { .. }) => Ordering::Greater,
                (FileItem::File { name: name_a, .. }, FileItem::File { name: name_b, .. }) => {
                    name_a.cmp(name_b)
                }
            }
        });
        
        Ok(items)
    }
}