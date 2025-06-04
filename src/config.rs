use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::SshHost;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub text: String,
    pub highlight: String,
    pub error: String,
    pub warning: String,
    pub success: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Theme {
    pub name: String,
    pub colors: ThemeColors,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_theme: String,
    pub themes: Vec<Theme>,
    pub ssh_file_config: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostGroup {
    pub name: String,
    pub description: Option<String>,
    pub hosts: Vec<SshHost>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostsConfig {
    pub groups: Vec<HostGroup>,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            primary: "#454545".to_string(),
            secondary: "#454545".to_string(),
            background: "#1a202c".to_string(),
            text: "#ffffff".to_string(),
            highlight: "#454545".to_string(),
            error: "#ff005f".to_string(),
            warning: "#ffb86c".to_string(),
            success: "#50fa7b".to_string(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            colors: ThemeColors::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {

        // Set default ssh config path
        let ssh_config_path = dirs::home_dir().unwrap().join(".ssh").join("config");
        Self {
            default_theme: "default".to_string(),
            themes: vec![Theme::default()],
            ssh_file_config: ssh_config_path.to_str().unwrap().to_string(),
        }
    }
}

impl Default for HostsConfig {
    fn default() -> Self {
        Self { groups: Vec::new() }
    }
}

#[derive(Debug)]
pub struct ConfigManager {
    #[allow(dead_code)]
    config_dir: PathBuf,
    config_file: PathBuf,
    hosts_file: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("sshr");

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        }

        let config_file = config_dir.join("sshr.toml");
        let hosts_file = config_dir.join("hosts.toml");

        Ok(Self {
            config_dir,
            config_file,
            hosts_file,
        })
    }

    // pub fn get_config_dir(&self) -> &Path {
    //     &self.config_dir
    // }

    pub fn load_config(&self) -> Result<AppConfig> {
        // If config file doesn't exist, create it with default values
        if !self.config_file.exists() {
            let default_config = AppConfig::default();
            self.save_config(&default_config)?;
        }

        let content: String =
            fs::read_to_string(&self.config_file).context("Failed to read config file")?;

        let mut config: AppConfig =
            toml::from_str(&content).context("Failed to parse config file")?;

        // Ensure there's always at least the default theme
        if config.themes.is_empty() {
            config.themes.push(Theme::default());
        }

        // Ensure the default theme exists
        if !config.themes.iter().any(|t| t.name == config.default_theme) {
            config.default_theme = config.themes[0].name.clone();
        }

        Ok(config)
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let toml = toml::to_string_pretty(config).context("Failed to serialize config")?;
        fs::write(&self.config_file, toml).context("Failed to write config file")?;
        Ok(())
    }

    // TODO: Add theme support
    // pub fn get_theme(&self, theme_name: Option<&str>) -> Result<Theme> {
    //     let config = self.load_config()?;
    //     let theme_name = theme_name.unwrap_or(&config.default_theme);

    //     config
    //         .themes
    //         .iter()
    //         .find(|t| t.name == *theme_name)
    //         .or_else(|| config.themes.first())
    //         .cloned()
    //         .ok_or_else(|| anyhow::anyhow!("No themes available"))
    // }

    // pub fn get_config_path(&self) -> &Path {
    //     &self.config_file
    // }

    pub fn load_hosts(&self) -> Result<Vec<SshHost>> {
        // If hosts file doesn't exist, return empty vector
        if !self.hosts_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.hosts_file)
            .context("Failed to read hosts file")?;

        let config: HostsConfig = toml::from_str(&content)
            .context("Failed to parse hosts file")?;

        // Flatten groups into a single vector of hosts
        let mut hosts = Vec::new();
        for group in config.groups {
            for mut host in group.hosts {
                // Set group name for each host
                host.group = Some(group.name.clone());
                hosts.push(host);
            }
        }

        Ok(hosts)
    }

    // pub fn save_hosts(&self, groups: &[HostGroup]) -> Result<()> {
    //     // Create hosts file if it doesn't exist
    //     if !self.hosts_file.exists() {
    //         fs::write(&self.hosts_file, "").context("Failed to create hosts file")?;
    //     }

    //     let config = HostsConfig {
    //         groups: groups.to_vec(),
    //     };
        
    //     let toml = toml::to_string_pretty(&config)
    //         .context("Failed to serialize hosts")?;
            
    //     fs::write(&self.hosts_file, toml)
    //         .context("Failed to write hosts file")?;
            
    //     Ok(())
    // }

    pub fn get_hosts_path(&self) -> &Path {
        &self.hosts_file
    }
}
