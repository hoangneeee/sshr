use crate::app::App;
use crate::models::SshHost;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::net::ToSocketAddrs;

impl App {
    /// Update the list of groups and the hosts in the current group
    pub fn update_groups(&mut self) {
        // Extract unique group names from hosts
        let mut groups: Vec<String> = self.hosts
            .iter()
            .filter_map(|host| host.group.clone())
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
        self.update_hosts_for_selected_group();
    }

    pub fn load_all_hosts(&mut self) -> Result<()> {
        self.load_ssh_config()
            .context("Failed to load SSH config")?;
        self.load_custom_hosts()
            .context("Failed to load custom hosts")?;
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
        // Clear only system-loaded hosts to allow custom hosts to persist across reloads
        self.hosts.retain(|h| h.group.is_some()); // Retain only custom hosts (those with a group)

        if !self.ssh_config_path.exists() {
            tracing::warn!(
                "System SSH config file not found at {:?}",
                self.ssh_config_path
            );
            return Ok(());
        }

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
                // Save previous host if exists
                if let Some(host) = current_host.take() {
                    // Check if a host with this alias already exists from custom config
                    if !self.hosts.iter().any(|h| h.alias == host.alias) {
                        self.hosts.push(host);
                    } else {
                        tracing::warn!(
                            "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                            host.alias
                        );
                    }
                }

                // Start new host
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

        tracing::info!("Loaded {} hosts from SSH config", self.hosts.len());

        // Don't forget to add the last host
        if let Some(host) = current_host {
            if !self.hosts.iter().any(|h| h.alias == host.alias) {
                self.hosts.push(host);
            } else {
                tracing::warn!(
                    "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                    host.alias
                );
            }
        }

        tracing::info!(
            "Loaded {} hosts from SSH config (after merging with custom hosts)",
            self.hosts.len()
        );

        // Check reachability for each host
        for host in &mut self.hosts {
            if host.group.is_none() {
                // Only update description for system hosts if not already set by custom
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

    // Load custome hosts from hosts.toml
    pub fn load_custom_hosts(&mut self) -> Result<()> {
        match self.config_manager.load_hosts() {
            Ok(mut custom_hosts) => {
                // Prepend custom hosts to the list, as they often take precedence or are more frequently used.
                // Filter out any custom hosts that might have duplicate aliases with existing system hosts
                // (though `handle_duplicate_hosts` will catch this later, this is a good defensive step).
                let mut existing_aliases: HashSet<String> =
                    self.hosts.iter().map(|h| h.alias.clone()).collect();
                custom_hosts.retain(|host| {
              if existing_aliases.contains(&host.alias) {
                  tracing::warn!("Skipping custom host '{}' due to duplicate alias. System host will be used.", host.alias);
                  false
              } else {
                  existing_aliases.insert(host.alias.clone());
                  true
              }
          });

                // Insert custom hosts at the beginning
                self.hosts.splice(0..0, custom_hosts);
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
