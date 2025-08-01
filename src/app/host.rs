use crate::app::App;
use crate::models::SshHost;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::net::ToSocketAddrs;

impl App {
    /// Update the list of groups and the hosts in the current group
    pub fn update_groups(&mut self) {
        // Extract unique group names from hosts, treating None as "Unknown"
        let mut groups: Vec<String> = self
            .hosts
            .iter()
            .map(|host| host.group.clone().unwrap_or_else(|| "Unknown".to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Sort groups alphabetically
        groups.sort();

        // Update groups list
        self.groups = groups;

        // If no groups, clear the current group selection
        if self.groups.is_empty() {
            self.hosts_in_current_group.clear();
            return;
        }

        // Ensure selected_group is within bounds
        if self.selected_group >= self.groups.len() {
            self.selected_group = self.groups.len().saturating_sub(1);
        }

        // Update hosts for the current group
        let selected_group_name = self.groups[self.selected_group].clone();
        if selected_group_name == "Unknown" {
            self.hosts_in_current_group = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, host)| host.group.is_none())
                .map(|(i, _)| i)
                .collect();
        } else {
            self.hosts_in_current_group = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, host)| host.group.as_deref() == Some(selected_group_name.as_str()))
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub fn load_all_hosts(&mut self) -> Result<()> {
        // The order of loading is important here.
        // 1. Load from SSH config first.
        self.load_ssh_config()
            .context("Failed to load SSH config")?;

        // 2. Load custom hosts, which will override any existing hosts with the same alias.
        self.load_custom_hosts()
            .context("Failed to load custom hosts")?;

        // 3. Handle any duplicates that might still exist if the logic above changes.
        self.handle_duplicate_hosts();

        // Update groups after loading all hosts
        self.update_groups();

        if self.hosts.is_empty() {
            self.selected_host = 0;
        } else if self.selected_host >= self.hosts.len() {
            self.selected_host = self.hosts.len().saturating_sub(1);
        }
        self.filter_hosts();
        Ok(())
    }

    pub fn load_ssh_config(&mut self) -> Result<()> {
        // Clear all hosts before loading from system config.
        self.hosts.clear();

        if !self.ssh_config_path.exists() {
            tracing::warn!(
                "System SSH config file not found at {:?}",
                self.ssh_config_path
            );
            return Ok(());
        }

        let config_content =
            fs::read_to_string(&self.ssh_config_path).context("Failed to read SSH config file")?;

        let mut current_host: Option<SshHost> = None;

        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.to_lowercase().starts_with("host ") {
                if let Some(mut host) = current_host.take() {
                    if host.group.is_none() {
                        host.group = Some("Unknown".to_string());
                    }
                    self.hosts.push(host);
                }

                let alias = line[5..].trim().to_string();
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

        if let Some(mut host) = current_host.take() {
            if host.group.is_none() {
                host.group = Some("Unknown".to_string());
            }
            self.hosts.push(host);
        }

        tracing::info!("Loaded {} hosts from SSH config", self.hosts.len());

        for host in &mut self.hosts {
            if host.group.as_deref() == Some("Unknown") {
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

    // Load custom hosts from hosts.toml
    pub fn load_custom_hosts(&mut self) -> Result<()> {
        match self.config_manager.load_hosts() {
            Ok(custom_hosts) => {
                for custom_host in custom_hosts {
                    // Check if a host with the same alias already exists.
                    if let Some(existing_host) = self.hosts.iter_mut().find(|h| h.alias == custom_host.alias) {
                        // If it exists, update it with the custom configuration.
                        *existing_host = custom_host;
                        tracing::info!("Overwriting host '{}' with custom configuration.", existing_host.alias);
                    } else {
                        // If it doesn't exist, add it as a new host.
                        self.hosts.push(custom_host);
                    }
                }
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load custom hosts: {}", e);
                // Don't propagate error, just log it, so app can still run.
                Ok(())
            }
        }
    }

    // Remove duplicate hosts
    pub fn handle_duplicate_hosts(&mut self) {
        let mut seen_aliases = HashSet::new();
        let mut unique_hosts = Vec::new();
        for host in self.hosts.drain(..) {
            // Use drain to consume self.hosts
            if seen_aliases.contains(&host.alias) {
                tracing::warn!("Duplicate alias found: {}", host.alias);
            } else {
                seen_aliases.insert(host.alias.clone());
                unique_hosts.push(host);
            }
        }
        self.hosts = unique_hosts;
    }
}
