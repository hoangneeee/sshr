use anyhow::{Context, Result};
use shell_escape::unix::escape;
use std::borrow::Cow;
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
            &self.remote_current_path,
            &self.strict_host_key_checking,
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
        strict_host_key_checking: &str,
    ) -> Result<Vec<FileItem>> {
        let escaped_path = escape(Cow::Borrowed(remote_path));
        let ssh_common_args = [
            format!("{}@{}", user, host),
            "-p".to_string(),
            port.to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-o".to_string(),
            format!("StrictHostKeyChecking={}", strict_host_key_checking),
            "-o".to_string(),
            "LogLevel=ERROR".to_string(),
        ];

        // Try GNU find first (handles spaces and is locale-independent)
        let find_cmd = format!(
            "find {} -maxdepth 1 -printf '%y\\t%s\\t%f\\n' 2>/dev/null",
            escaped_path
        );
        let find_output = Command::new("ssh")
            .args(&ssh_common_args)
            .arg(&find_cmd)
            .output()
            .context("Failed to execute remote find command")?;

        if find_output.status.success() {
            let stdout = String::from_utf8_lossy(&find_output.stdout);
            return Ok(Self::parse_find_listing(&stdout, remote_path));
        }

        // Fallback to ls -la for non-GNU systems (e.g. macOS remotes)
        let ls_output = Command::new("ssh")
            .args(&ssh_common_args)
            .arg(format!("ls -la {}", escaped_path))
            .output()
            .context("Failed to execute remote ls command")?;

        if !ls_output.status.success() {
            let stderr = String::from_utf8_lossy(&ls_output.stderr);
            return Err(anyhow::anyhow!("Remote ls failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&ls_output.stdout);
        Ok(Self::parse_file_listing(&stdout, remote_path))
    }

    pub(crate) fn parse_find_listing(output: &str, remote_path: &str) -> Vec<FileItem> {
        let mut items = Vec::new();

        if remote_path != "/" {
            items.push(FileItem::Directory { name: "..".to_string() });
        }

        for line in output.lines() {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() != 3 {
                continue;
            }
            let entry_type = parts[0];
            let size: u64 = parts[1].parse().unwrap_or(0);
            let name = parts[2].to_string();

            // Skip the directory itself (. entry from find)
            if name == "." || name == remote_path.trim_end_matches('/').split('/').last().unwrap_or("") {
                // find's first result is the directory itself; skip it
                if entry_type == "d" && (name == "." || name.is_empty()) {
                    continue;
                }
            }

            // Skip . and ..
            if name == "." || name == ".." {
                continue;
            }

            if entry_type == "d" {
                items.push(FileItem::Directory { name });
            } else {
                items.push(FileItem::File { name, size });
            }
        }

        items.sort_by(|a, b| {
            use std::cmp::Ordering;
            match (a, b) {
                (FileItem::Directory { name: na }, FileItem::Directory { name: nb }) => {
                    if na == ".." { Ordering::Less }
                    else if nb == ".." { Ordering::Greater }
                    else { na.cmp(nb) }
                }
                (FileItem::Directory { .. }, FileItem::File { .. }) => Ordering::Less,
                (FileItem::File { .. }, FileItem::Directory { .. }) => Ordering::Greater,
                (FileItem::File { name: na, .. }, FileItem::File { name: nb, .. }) => na.cmp(nb),
            }
        });

        items
    }

    pub(crate) fn parse_file_listing(output: &str, remote_path: &str) -> Vec<FileItem> {
        let mut items = Vec::new();

        // Add parent directory entry if not at root
        if remote_path != "/" {
            items.push(FileItem::Directory {
                name: "..".to_string(),
            });
        }

        for line in output.lines().skip(1) { // Skip total line
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

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp_logic::types::AppSftpState;

    fn parse(output: &str, path: &str) -> Vec<FileItem> {
        AppSftpState::parse_file_listing(output, path)
    }

    #[test]
    fn test_parse_empty_output() {
        let items = parse("total 0\n", "/home/user");
        // Only ".." entry since not at root
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], FileItem::Directory { name } if name == ".."));
    }

    #[test]
    fn test_parse_at_root_no_dotdot() {
        let items = parse("total 0\n", "/");
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_normal_files_and_dirs() {
        let output = "total 16\n\
            drwxr-xr-x 2 user user 4096 Jan 1 10:00 subdir\n\
            -rw-r--r-- 1 user user 1234 Jan 1 10:00 file.txt\n";
        let items = parse(output, "/home/user");

        // ".." + "subdir" + "file.txt" = 3 items
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], FileItem::Directory { name } if name == ".."));
        assert!(matches!(&items[1], FileItem::Directory { name } if name == "subdir"));
        assert!(matches!(&items[2], FileItem::File { name, size: 1234 } if name == "file.txt"));
    }

    #[test]
    fn test_parse_filename_with_spaces() {
        let output = "total 8\n\
            -rw-r--r-- 1 user user 512 Jan 1 10:00 my file name.txt\n";
        let items = parse(output, "/home/user");

        assert_eq!(items.len(), 2); // ".." + file
        assert!(matches!(&items[1], FileItem::File { name, .. } if name == "my file name.txt"));
    }

    #[test]
    fn test_parse_skips_dot_entries() {
        let output = "total 8\n\
            drwxr-xr-x 2 user user 4096 Jan 1 10:00 .\n\
            drwxr-xr-x 3 root root 4096 Jan 1 10:00 ..\n\
            -rw-r--r-- 1 user user 100 Jan 1 10:00 readme.txt\n";
        let items = parse(output, "/home/user");

        // ".." (added manually) + "readme.txt"
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], FileItem::Directory { name } if name == ".."));
        assert!(matches!(&items[1], FileItem::File { name, .. } if name == "readme.txt"));
    }

    #[test]
    fn test_parse_find_listing_normal() {
        // find output: type\tsize\tname
        let output = "d\t4096\t.\nd\t4096\tsubdir\n-\t1234\tfile.txt\n";
        let items = AppSftpState::parse_find_listing(output, "/home/user");

        // ".." + "subdir" + "file.txt"
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], FileItem::Directory { name } if name == ".."));
        assert!(matches!(&items[1], FileItem::Directory { name } if name == "subdir"));
        assert!(matches!(&items[2], FileItem::File { name, size: 1234 } if name == "file.txt"));
    }

    #[test]
    fn test_parse_find_listing_filename_with_spaces() {
        let output = "d\t4096\t.\n-\t512\tmy file name.txt\n";
        let items = AppSftpState::parse_find_listing(output, "/home/user");

        assert_eq!(items.len(), 2); // ".." + file
        assert!(matches!(&items[1], FileItem::File { name, .. } if name == "my file name.txt"));
    }

    #[test]
    fn test_parse_find_listing_at_root_no_dotdot() {
        let output = "d\t4096\t.\nd\t4096\tetc\n";
        let items = AppSftpState::parse_find_listing(output, "/");
        // No ".." at root, just "etc"
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], FileItem::Directory { name } if name == "etc"));
    }

    #[test]
    fn test_parse_dirs_sort_before_files() {
        let output = "total 16\n\
            -rw-r--r-- 1 user user 100 Jan 1 10:00 aaa.txt\n\
            drwxr-xr-x 2 user user 4096 Jan 1 10:00 zzz_dir\n";
        let items = parse(output, "/home/user");

        assert!(matches!(&items[0], FileItem::Directory { name } if name == ".."));
        assert!(matches!(&items[1], FileItem::Directory { name } if name == "zzz_dir"));
        assert!(matches!(&items[2], FileItem::File { name, .. } if name == "aaa.txt"));
    }
}
