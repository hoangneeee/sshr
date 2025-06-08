use anyhow::{Context, Result};
use std::fs;
use std::path::{Path};
use super::types::{FileItem, AppSftpState};

impl AppSftpState {
    /// Refresh the local file list
    pub fn refresh_local(&mut self) -> Result<()> {
        self.local_files = Self::read_local_directory(&self.local_current_path)?;
        if self.local_selected >= self.local_files.len() {
            self.local_selected = self.local_files.len().saturating_sub(1);
        }
        Ok(())
    }
    
    /// Navigate up in the local file list
    pub fn navigate_local_up(&mut self) {
        if self.local_selected > 0 {
            self.local_selected -= 1;
        } else if !self.local_files.is_empty() {
            self.local_selected = self.local_files.len() - 1;
        }
        self.local_list_state.select(Some(self.local_selected));
    }
    
    /// Navigate down in the local file list
    pub fn navigate_local_down(&mut self) {
        if self.local_selected < self.local_files.len().saturating_sub(1) {
            self.local_selected += 1;
        } else {
            self.local_selected = 0;
        }
        self.local_list_state.select(Some(self.local_selected));
    }
    
    /// Open the selected item in the local file list
    pub fn open_local_selected(&mut self) -> Result<()> {
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
                    self.local_list_state.select(Some(self.local_selected));
                    self.refresh_local()?;
                }
                FileItem::File { .. } => {
                    // Files can't be opened in file browser context
                }
            }
        }
        Ok(())
    }
    
    /// Go up one directory in the local file system
    pub fn go_local_back(&mut self) -> Result<()> {
        if let Some(parent) = self.local_current_path.parent() {
            self.local_current_path = parent.to_path_buf();
            self.local_selected = 0;
            self.refresh_local()?;
        }
        Ok(())
    }
    
    /// Read the contents of a local directory
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
}