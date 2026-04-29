use super::types::{AppSftpState, DownloadProgress, UploadProgress};
use crate::app_event::TransferEvent;

impl AppSftpState {
    /// Upload the currently selected local file via the persistent SFTP
    /// session. Spawns a tokio task that streams the file and posts
    /// progress back through the transfer channel.
    pub fn upload_file(&mut self) {
        let Some(super::FileItem::File { name, .. }) =
            self.local_files.get(self.local_selected).cloned()
        else {
            self.set_status_message("Please select a file to upload");
            return;
        };

        let local_path = self.local_current_path.join(&name);
        let remote_path = if self.remote_current_path.ends_with('/') {
            format!("{}{}", self.remote_current_path, name)
        } else {
            format!("{}/{}", self.remote_current_path, name)
        };

        // Mark progress eagerly so the UI blocks repeat 'u' presses before
        // the worker emits its first UploadProgress event.
        self.upload_progress = Some(UploadProgress {
            file_name: name.clone(),
            uploaded_size: 0,
            total_size: 0,
        });

        let client = self.client.clone();
        let tx = self.transfer_tx.clone();

        tokio::spawn(async move {
            let name_clone = name.clone();
            let progress_tx = tx.clone();
            let result = client
                .upload(&local_path, &remote_path, move |uploaded, total| {
                    let _ = progress_tx.try_send(TransferEvent::UploadProgress(
                        name_clone.clone(),
                        uploaded,
                        total,
                    ));
                })
                .await;

            match result {
                Ok(_) => {
                    tracing::info!("Successfully uploaded {}", name);
                    let _ = tx.send(TransferEvent::UploadComplete(name)).await;
                }
                Err(e) => {
                    tracing::error!("Failed to upload {}: {}", name, e);
                    let _ = tx
                        .send(TransferEvent::UploadError(name, e.to_string()))
                        .await;
                }
            }
        });
    }

    /// Download the currently selected remote file via the persistent SFTP
    /// session.
    pub fn download_file(&mut self) {
        let Some(super::FileItem::File { name, .. }) =
            self.remote_files.get(self.remote_selected).cloned()
        else {
            self.set_status_message("Please select a file to download");
            return;
        };

        let remote_path = if self.remote_current_path.ends_with('/') {
            format!("{}{}", self.remote_current_path, name)
        } else {
            format!("{}/{}", self.remote_current_path, name)
        };
        let local_path = self.local_current_path.join(&name);

        // Mark progress eagerly so the UI blocks repeat 'd' presses before
        // the worker emits its first DownloadProgress event.
        self.download_progress = Some(DownloadProgress {
            file_name: name.clone(),
            downloaded_size: 0,
            total_size: 0,
        });

        let client = self.client.clone();
        let tx = self.transfer_tx.clone();

        tokio::spawn(async move {
            let name_clone = name.clone();
            let progress_tx = tx.clone();
            let result = client
                .download(&remote_path, &local_path, move |downloaded, total| {
                    let _ = progress_tx.try_send(TransferEvent::DownloadProgress(
                        name_clone.clone(),
                        downloaded,
                        total,
                    ));
                })
                .await;

            match result {
                Ok(_) => {
                    tracing::info!("Successfully downloaded {}", name);
                    let _ = tx.send(TransferEvent::DownloadComplete(name)).await;
                }
                Err(e) => {
                    tracing::error!("Failed to download {}: {}", name, e);
                    let _ = tx
                        .send(TransferEvent::DownloadError(name, e.to_string()))
                        .await;
                }
            }
        });
    }
}
