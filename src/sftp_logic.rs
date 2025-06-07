use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum FileItem {
    Directory { name: String },
    File { name: String, size: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PanelSide {
    Local,
    Remote,
}

#[derive(Debug, Clone)]
pub struct AppSftpState {
    pub active_panel: PanelSide,
    
    // Local panel state
    pub local_current_path: PathBuf,
    pub local_files: Vec<FileItem>,
    pub local_selected: usize,
    
    // Remote panel state
    pub remote_current_path: String,
    pub remote_files: Vec<FileItem>,
    pub remote_selected: usize,
    
    // SFTP connection info
    pub ssh_host: String,
    pub ssh_user: String,
    pub ssh_port: u16,
    
    // UI state
    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,
}

impl AppSftpState {
    pub fn new(ssh_user: &str, ssh_host: &str, ssh_port: u16) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        
        let mut state = Self {
            active_panel: PanelSide::Local,
            local_current_path: current_dir,
            local_files: Vec::new(),
            local_selected: 0,
            remote_current_path: "/".to_string(),
            remote_files: Vec::new(),
            remote_selected: 0,
            ssh_host: ssh_host.to_string(),
            ssh_user: ssh_user.to_string(),
            ssh_port,
            status_message: None,
            status_message_time: None,
        };
        
        // Load initial directory contents
        state.refresh_local()?;
        state.refresh_remote()?;
        
        Ok(state)
    }
    
    pub fn refresh_local(&mut self) -> Result<()> {
        self.local_files = Self::read_local_directory(&self.local_current_path)?;
        if self.local_selected >= self.local_files.len() {
            self.local_selected = self.local_files.len().saturating_sub(1);
        }
        Ok(())
    }
    
    pub fn refresh_remote(&mut self) -> Result<()> {
        self.set_status_message("Loading remote directory...");
        self.remote_files = Self::read_remote_directory(&self.ssh_user, &self.ssh_host, self.ssh_port, &self.remote_current_path)?;
        if self.remote_selected >= self.remote_files.len() {
            self.remote_selected = self.remote_files.len().saturating_sub(1);
        }
        self.clear_status_message();
        Ok(())
    }
    
    pub fn navigate_up(&mut self) {
        match self.active_panel {
            PanelSide::Local => {
                if self.local_selected > 0 {
                    self.local_selected -= 1;
                } else if !self.local_files.is_empty() {
                    self.local_selected = self.local_files.len() - 1;
                }
            }
            PanelSide::Remote => {
                if self.remote_selected > 0 {
                    self.remote_selected -= 1;
                } else if !self.remote_files.is_empty() {
                    self.remote_selected = self.remote_files.len() - 1;
                }
            }
        }
    }
    
    pub fn navigate_down(&mut self) {
        match self.active_panel {
            PanelSide::Local => {
                if self.local_selected < self.local_files.len().saturating_sub(1) {
                    self.local_selected += 1;
                } else {
                    self.local_selected = 0;
                }
            }
            PanelSide::Remote => {
                if self.remote_selected < self.remote_files.len().saturating_sub(1) {
                    self.remote_selected += 1;
                } else {
                    self.remote_selected = 0;
                }
            }
        }
    }
    
    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        };
    }
    
    pub fn open_selected(&mut self) -> Result<()> {
        match self.active_panel {
            PanelSide::Local => {
                if let Some(item) = self.local_files.get(self.local_selected) {
                    match item {
                        FileItem::Directory { name } => {
                            if name == ".." {
                                if let Some(parent) = self.local_current_path.parent() {
                                    self.local_current_path = parent.to_path_buf();
                                }
                            } else {
                                self.local_current_path = self.local_current_path.join(name);
                            }
                            self.local_selected = 0;
                            self.refresh_local()?;
                        }
                        FileItem::File { .. } => {
                            // Files can't be opened in file browser context
                        }
                    }
                }
            }
            PanelSide::Remote => {
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
                            self.refresh_remote()?;
                        }
                        FileItem::File { .. } => {
                            // Files can't be opened in file browser context
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    pub fn go_back(&mut self) -> Result<()> {
        match self.active_panel {
            PanelSide::Local => {
                if let Some(parent) = self.local_current_path.parent() {
                    self.local_current_path = parent.to_path_buf();
                    self.local_selected = 0;
                    self.refresh_local()?;
                }
            }
            PanelSide::Remote => {
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
            }
        }
        Ok(())
    }
    
    pub async fn upload_file(&mut self) -> Result<()> {
        if let Some(FileItem::File { name, .. }) = self.local_files.get(self.local_selected) {
            let name = name.clone();
            let local_path = self.local_current_path.join(&name);
            let remote_path = if self.remote_current_path.ends_with('/') {
                format!("{}{}", self.remote_current_path, name)
            } else {
                format!("{}/{}", self.remote_current_path, name)
            };
            
            self.set_status_message(&format!("Uploading {}...", name));
            
            let result = Self::sftp_upload(
                &self.ssh_user,
                &self.ssh_host,
                self.ssh_port,
                &local_path,
                &remote_path,
            ).await;
            
            match result {
                Ok(_) => {
                    self.set_status_message(&format!("Successfully uploaded {}", name));
                    self.refresh_remote()?;
                }
                Err(e) => {
                    self.set_status_message(&format!("Upload failed: {}", e));
                }
            }
        } else {
            self.set_status_message("Please select a file to upload");
        }
        Ok(())
    }
    
    pub async fn download_file(&mut self) -> Result<()> {
        if let Some(FileItem::File { name, .. }) = self.remote_files.get(self.remote_selected) {
            let name = name.clone();
            let remote_path = if self.remote_current_path.ends_with('/') {
                format!("{}{}", self.remote_current_path, name)
            } else {
                format!("{}/{}", self.remote_current_path, name)
            };
            let local_path = self.local_current_path.join(&name);
            
            self.set_status_message(&format!("Downloading {}...", name));
            
            let result = Self::sftp_download(
                &self.ssh_user,
                &self.ssh_host,
                self.ssh_port,
                &remote_path,
                &local_path,
            ).await;
            
            match result {
                Ok(_) => {
                    self.set_status_message(&format!("Successfully downloaded {}", name));
                    self.refresh_local()?;
                }
                Err(e) => {
                    self.set_status_message(&format!("Download failed: {}", e));
                }
            }
        } else {
            self.set_status_message("Please select a file to download");
        }
        Ok(())
    }
    
    pub fn set_status_message(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
        self.status_message_time = Some(Instant::now());
    }
    
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
        self.status_message_time = None;
    }
    
    pub fn should_clear_status(&self) -> bool {
        if let Some(time) = self.status_message_time {
            time.elapsed() > Duration::from_secs(3)
        } else {
            false
        }
    }
    
    fn read_local_directory(path: &Path) -> Result<Vec<FileItem>> {
        let mut items = Vec::new();
        
        // Add parent directory entry if not at root
        if path.parent().is_some() {
            items.push(FileItem::Directory {
                name: "..".to_string(),
            });
        }
        
        let entries = fs::read_dir(path).context("Failed to read local directory")?;
        
        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().context("Failed to read file metadata")?;
            
            if metadata.is_dir() {
                items.push(FileItem::Directory { name: file_name });
            } else {
                items.push(FileItem::File {
                    name: file_name,
                    size: metadata.len(),
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
    
    async fn sftp_upload(
        user: &str,
        host: &str,
        port: u16,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<()> {
        let output = Command::new("scp")
            .arg("-P")
            .arg(port.to_string())
            .arg("-o")
            .arg("ConnectTimeout=30")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(local_path)
            .arg(format!("{}@{}:{}", user, host, remote_path))
            .output()
            .context("Failed to execute scp upload command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP upload failed: {}", stderr));
        }
        
        Ok(())
    }
    
    async fn sftp_download(
        user: &str,
        host: &str,
        port: u16,
        remote_path: &str,
        local_path: &Path,
    ) -> Result<()> {
        let output = Command::new("scp")
            .arg("-P")
            .arg(port.to_string())
            .arg("-o")
            .arg("ConnectTimeout=30")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("{}@{}:{}", user, host, remote_path))
            .arg(local_path)
            .output()
            .context("Failed to execute scp download command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP download failed: {}", stderr));
        }
        
        Ok(())
    }
}