use crate::app::App;
use crate::models::SshHost;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::net::ToSocketAddrs;

impl App {
    /// Open the hosts.toml file in the user's preferred editor and reload
    /// hosts after it closes.
    pub fn open_hosts_editor(&mut self) -> Result<()> {
        let hosts_path = self.ctx.config_manager.get_hosts_path();

        if !hosts_path.exists() {
            if let Some(parent) = hosts_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(hosts_path, "")?;
        }

        if let Err(e) = open::that(hosts_path) {
            tracing::error!("Failed to open editor: {}", e);
            return Err(anyhow::anyhow!("Failed to open editor: {}", e));
        }

        self.load_all_hosts()?;
        Ok(())
    }
}

impl App {
    pub fn load_all_hosts(&mut self) -> Result<()> {
        // Custom hosts (hosts.toml) are loaded first so they take precedence
        // over system hosts (~/.ssh/config) when aliases collide. A user who
        // groups a host in hosts.toml has expressed explicit intent, whereas
        // ~/.ssh/config is loaded passively.
        self.load_custom_hosts()
            .context("Failed to load custom hosts")?;
        self.load_ssh_config()
            .context("Failed to load SSH config")?;
        self.handle_duplicate_hosts();

        // Update groups after loading all hosts
        self.hosts.rebuild_groups();

        if self.hosts.hosts.is_empty() {
            self.hosts.selected_host = 0;
        } else if self.hosts.selected_host >= self.hosts.hosts.len() {
            self.hosts.selected_host = self.hosts.hosts.len().saturating_sub(1);
        }
        self.filter_hosts();
        Ok(())
    }

    pub fn load_ssh_config(&mut self) -> Result<()> {
        // Clear only system-loaded hosts to allow custom hosts to persist across reloads
        self.hosts.hosts.retain(|h| h.group.is_some());

        if !self.ctx.ssh_config_path.exists() {
            tracing::warn!(
                "System SSH config file not found at {:?}",
                self.ctx.ssh_config_path
            );
            return Ok(());
        }

        let config_content = fs::read_to_string(&self.ctx.ssh_config_path)
            .context("Failed to read SSH config file")?;

        let mut current_host: Option<SshHost> = None;

        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.to_lowercase().starts_with("host ") {
                if let Some(host) = current_host.take() {
                    if !self.hosts.hosts.iter().any(|h| h.alias == host.alias) {
                        self.hosts.hosts.push(host);
                    } else {
                        tracing::warn!(
                            "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                            host.alias
                        );
                    }
                }

                let alias = line[5..].trim().to_string();
                // Skip wildcard/pattern entries (e.g. "Host *")
                if alias.contains('*') || alias.contains('?') || alias.contains('!') {
                    current_host = None;
                    continue;
                }
                current_host = Some(SshHost::new(alias, String::new(), "root".to_string()));
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

        tracing::info!("Loaded {} hosts from SSH config", self.hosts.hosts.len());

        if let Some(host) = current_host {
            if !self.hosts.hosts.iter().any(|h| h.alias == host.alias) {
                self.hosts.hosts.push(host);
            } else {
                tracing::warn!(
                    "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                    host.alias
                );
            }
        }

        tracing::info!(
            "Loaded {} hosts from SSH config (after merging with custom hosts)",
            self.hosts.hosts.len()
        );

        // Check reachability for each host
        for host in &mut self.hosts.hosts {
            if host.group.is_none() {
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
        }

        Ok(())
    }

    /// Load custom hosts from hosts.toml.
    pub fn load_custom_hosts(&mut self) -> Result<()> {
        // Drop previously-loaded custom hosts so reloads don't accumulate.
        self.hosts.hosts.retain(|h| h.group.is_none());

        match self.ctx.config_manager.load_hosts() {
            Ok(mut custom_hosts) => {
                let mut seen_aliases: HashSet<String> = HashSet::new();
                custom_hosts.retain(|host| {
                    if seen_aliases.contains(&host.alias) {
                        tracing::warn!(
                            "Skipping duplicate custom host '{}' (defined more than once in hosts.toml).",
                            host.alias
                        );
                        false
                    } else {
                        seen_aliases.insert(host.alias.clone());
                        true
                    }
                });

                self.hosts.hosts.splice(0..0, custom_hosts);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load custom hosts: {}", e);
                Ok(())
            }
        }
    }

    /// Remove duplicate hosts.
    pub fn handle_duplicate_hosts(&mut self) {
        let mut seen_aliases = HashSet::new();
        let mut unique_hosts = Vec::new();
        for host in self.hosts.hosts.drain(..) {
            if seen_aliases.contains(&host.alias) {
                tracing::warn!("Duplicate alias found: {}", host.alias);
            } else {
                seen_aliases.insert(host.alias.clone());
                unique_hosts.push(host);
            }
        }
        self.hosts.hosts = unique_hosts;
    }
}
