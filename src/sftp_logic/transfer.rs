use anyhow::{Context, Result};
use std::path::Path;
use super::types::AppSftpState;

impl AppSftpState {
    /// Upload a file to the remote server
    pub async fn upload_file(&mut self) -> Result<()> {
        if let Some(super::FileItem::File { name, .. }) = self.local_files.get(self.local_selected) {
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
    
    /// Download a file from the remote server
    pub async fn download_file(&mut self) -> Result<()> {
        if let Some(super::FileItem::File { name, .. }) = self.remote_files.get(self.remote_selected) {
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
    
    /// Upload a file using SCP
    async fn sftp_upload(
        user: &str,
        host: &str,
        port: u16,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<()> {
        let output = tokio::process::Command::new("scp")
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
            .await
            .context("Failed to execute scp upload command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP upload failed: {}", stderr));
        }
        
        Ok(())
    }
    
    /// Download a file using SCP
    async fn sftp_download(
        user: &str,
        host: &str,
        port: u16,
        remote_path: &str,
        local_path: &Path,
    ) -> Result<()> {
        let output = tokio::process::Command::new("scp")
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
            .await
            .context("Failed to execute scp download command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP download failed: {}", stderr));
        }
        
        Ok(())
    }
}