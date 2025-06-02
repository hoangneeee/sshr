use std::path::PathBuf;
use std::fs;
use std::net::ToSocketAddrs;
use anyhow::{Context, Result};
use log::debug;
use serde::{Deserialize, Serialize};
use dirs;

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected: usize,
    pub ssh_config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHost {
    pub alias: String,
    pub host: String,
    pub user: String,
    pub port: Option<u16>,
    pub description: Option<String>,
    pub group: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        let home_dir = dirs::home_dir().expect("Could not find home directory");
        let ssh_config_path = home_dir.join(".ssh").join("config");
        
        Self {
            should_quit: false,
            hosts: Vec::new(),
            selected: 0,
            ssh_config_path,
        }
    }
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self::default();
        app.load_ssh_config()?;
        Ok(app)
    }

    pub fn load_ssh_config(&mut self) -> Result<()> {
        self.hosts.clear();

        if !self.ssh_config_path.exists() {
            return Ok(());
        }

        let config_content = fs::read_to_string(&self.ssh_config_path)
            .context("Failed to read SSH config file")?;

        let mut current_host: Option<SshHost> = None;

        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!("Line: {}", line);
            if line.to_lowercase().starts_with("host ") {
                // Save previous host if exists
                if let Some(host) = current_host.take() {
                    self.hosts.push(host);
                }
                
                // Start new host
                let alias = line[5..].trim().to_string();
                current_host = Some(SshHost {
                    alias,
                    host: String::new(),
                    user: "root".to_string(),
                    port: None,
                    description: None,
                    group: None,
                });
            } else if let Some(host) = &mut current_host {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                
                match parts[0].to_lowercase().as_str() {
                    "hostname" => host.host = parts[1].to_string(),
                    "user" => host.user = parts[1].to_string(),
                    "port" => {
                        if let Ok(port) = parts[1].parse::<u16>() {
                            host.port = Some(port);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Don't forget to add the last host
        if let Some(host) = current_host {
            self.hosts.push(host);
        }

        // Check reachability for each host
        for host in &mut self.hosts {
            let socket_addr = format!("{}:{}", host.host, host.port.unwrap_or(22))
                .to_socket_addrs()
                .ok()
                .and_then(|mut addrs| addrs.next());

            host.description = if socket_addr.is_some() {
                Some("Reachable".to_string())
            } else {
                Some("Unreachable".to_string())
            };
        }

        Ok(())
    }

    pub fn on_key(&mut self, key: char) {
        match key {
            'q' => self.should_quit = true,
            'j' => {
                if self.selected < self.hosts.len().saturating_sub(1) {
                    self.selected += 1;
                }
            }
            'k' => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            'r' => {
                if let Err(e) = self.load_ssh_config() {
                    eprintln!("Failed to reload SSH config: {}", e);
                }
            }
            _ => {}
        }
    }
}