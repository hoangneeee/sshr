use super::types::AppSftpState;
use crate::app_event::TransferEvent;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

impl AppSftpState {
    /// Upload a file to the remote server
    pub fn upload_file(&mut self) {
        if let Some(super::FileItem::File { name, .. }) =
            self.local_files.get(self.local_selected).cloned()
        {
            let name = name.clone();
            let local_path = self.local_current_path.join(&name);
            let remote_path = if self.remote_current_path.ends_with('/') {
                format!("{}{}", self.remote_current_path, name)
            } else {
                format!("{}/{}", self.remote_current_path, name)
            };

            let ssh_user = self.ssh_user.clone();
            let ssh_host = self.ssh_host.clone();
            let ssh_port = self.ssh_port;
            let tx = self.transfer_tx.clone().unwrap();

            tokio::spawn(async move {
                let name_clone = name.clone();
                let progress_tx = tx.clone();
                let result = Self::sftp_upload(
                    &ssh_user,
                    &ssh_host,
                    ssh_port,
                    &local_path,
                    &remote_path,
                    move |uploaded, total| {
                        let _ = progress_tx.try_send(TransferEvent::UploadProgress(
                            name_clone.clone(),
                            uploaded,
                            total,
                        ));
                    },
                )
                .await;

                match result {
                    Ok(_) => {
                        tracing::info!("Successfully uploaded {}", name);
                        let _ = tx.send(TransferEvent::UploadComplete(name.clone())).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to upload {}", name);
                        tracing::error!("Error: {}", e.to_string());
                        let _ = tx
                            .send(TransferEvent::UploadError(name.clone(), e.to_string()))
                            .await;
                    }
                }
            });
        } else {
            self.set_status_message("Please select a file to upload");
        }
    }

    /// Download a file from the remote server
    pub fn download_file(&mut self) {
        if let Some(super::FileItem::File { name, .. }) =
            self.remote_files.get(self.remote_selected).cloned()
        {
            let name = name.clone();
            let remote_path = if self.remote_current_path.ends_with('/') {
                format!("{}{}", self.remote_current_path, name)
            } else {
                format!("{}/{}", self.remote_current_path, name)
            };
            let local_path = self.local_current_path.join(&name);

            // self.set_status_message(&format!("Downloading {}...", name));

            let ssh_user = self.ssh_user.clone();
            let ssh_host = self.ssh_host.clone();
            let ssh_port = self.ssh_port;
            let tx = self.transfer_tx.clone().unwrap();

            tokio::spawn(async move {
                let name_clone = name.clone();
                let progress_tx = tx.clone();
                let result = Self::sftp_download(
                    &ssh_user,
                    &ssh_host,
                    ssh_port,
                    &remote_path,
                    &local_path,
                    move |downloaded, total| {
                        tracing::info!("Downloading try send {}", name_clone);
                        let _ = progress_tx.try_send(TransferEvent::DownloadProgress(
                            name_clone.clone(),
                            downloaded,
                            total,
                        ));
                    },
                )
                .await;

                match result {
                    Ok(_) => {
                        tracing::info!("Successfully downloaded {}", name);
                        let _ = tx.send(TransferEvent::DownloadComplete(name.clone())).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to download {}", name);
                        tracing::error!("Error: {}", e.to_string());
                        let _ = tx
                            .send(TransferEvent::DownloadError(name.clone(), e.to_string()))
                            .await;
                    }
                }
            });
        } else {
            self.set_status_message("Please select a file to download");
        }
    }

    /// Upload a file using SCP with progress tracking
    async fn sftp_upload<F>(
        user: &str,
        host: &str,
        port: u16,
        local_path: &Path,
        remote_path: &str,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64) + Send + 'static,
    {
        let file = File::open(local_path).context("Failed to open local file")?;
        let metadata = file.metadata().context("Failed to get file metadata")?;
        let total_size = metadata.len();

        let mut command = Command::new("scp")
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
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start scp upload command")?;

        let mut stdin = command
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get scp stdin"))?;

        let mut file = File::open(local_path).context("Failed to reopen local file")?;
        let mut buffer = [0u8; 8192];
        let mut uploaded = 0;

        loop {
            let bytes_read = file.read(&mut buffer).context("Failed to read from file")?;
            if bytes_read == 0 {
                break;
            }

            stdin
                .write_all(&buffer[..bytes_read])
                .await
                .context("Failed to write to scp")?;
            uploaded += bytes_read as u64;

            progress_callback(uploaded, total_size);

            // Allow other tasks to run
            tokio::task::yield_now().await;
        }

        drop(stdin); // Close stdin to signal end of data

        let output = command
            .wait_with_output()
            .await
            .context("Failed to complete scp upload command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("SCP upload failed: {}", stderr));
        }

        Ok(())
    }

    /// Download a file using SCP with progress tracking
    async fn sftp_download<F: Fn(u64, u64) + Send + 'static>(
        user: &str,
        host: &str,
        port: u16,
        remote_path: &str,
        local_path: &Path,
        progress_callback: F,
    ) -> Result<()> {
        // First, get the remote file size
        let size_output = Command::new("ssh")
            .arg("-p")
            .arg(port.to_string())
            .arg("-o")
            .arg("ConnectTimeout=30")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("LogLevel=ERROR")
            .arg(format!("{}@{}", user, host))
            .arg(format!("stat -c%s {}", remote_path))
            .output()
            .await
            .context("Failed to get remote file size")?;

        if !size_output.status.success() {
            let stderr = String::from_utf8_lossy(&size_output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to get remote file size: {}",
                stderr
            ));
        }

        let total_size = String::from_utf8_lossy(&size_output.stdout)
            .trim()
            .parse::<u64>()
            .context("Failed to parse remote file size")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .context("Failed to create local directory")?;
        }

        // Start the download in a separate task
        let download_handle = {
            let local_path = local_path.to_path_buf();
            let user = user.to_string();
            let host = host.to_string();
            let remote_path = remote_path.to_string();
            
            tokio::spawn(async move {
                Command::new("scp")
                    .arg("-P")
                    .arg(port.to_string())
                    .arg("-o")
                    .arg("ConnectTimeout=30")
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg("-o")
                    .arg("LogLevel=ERROR")
                    .arg(format!("{}@{}:{}", user, host, remote_path))
                    .arg(&local_path)
                    .status()
                    .await
            })
        };

        // Monitor the download progress
        let start_time = std::time::Instant::now();
        let mut last_size = 0u64;
        
        // Initial progress update
        progress_callback(0, total_size);

        // Create a channel for the download task to signal completion
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        
        // Spawn a task to wait for the download to complete
        let download_task = tokio::spawn({
            let download_handle = download_handle;
            async move {
                match download_handle.await {
                    Ok(status_result) => {
                        let _ = tx.send(status_result);
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                    }
                }
            }
        });

        // Monitor progress until download completes
        let mut download_complete = false;
        while !download_complete {
            // Check if download is complete
            match rx.try_recv() {
                Ok(Ok(status)) => {
                    if !status.success() {
                        download_task.abort();
                        return Err(anyhow::anyhow!("SCP download failed with status: {}", status));
                    }
                    download_complete = true;
                }
                Ok(Err(e)) => {
                    download_task.abort();
                    return Err(e.into());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Download still in progress
                }
                Err(_) => {
                    download_task.abort();
                    return Err(anyhow::anyhow!("Download channel error"));
                }
            }

            // Get current file size
            if let Ok(metadata) = tokio::fs::metadata(local_path).await {
                let current_size = metadata.len();
                if current_size > last_size {
                    progress_callback(current_size, total_size);
                    last_size = current_size;
                }
            }

            // Check if download is taking too long without progress
            if start_time.elapsed().as_secs() > 300 && last_size == 0 { // 5 minutes without progress
                download_task.abort();
                return Err(anyhow::anyhow!("Download timed out with no progress"));
            }

            // Don't check too frequently
            if !download_complete {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        // Final progress update
        progress_callback(total_size, total_size);

        Ok(())
    }
}
