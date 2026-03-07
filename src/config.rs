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
    #[serde(default = "default_strict_host_key_checking")]
    pub strict_host_key_checking: String,
}

fn default_strict_host_key_checking() -> String {
    "accept-new".to_string()
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
            primary: "#50fa7b".to_string(),   // green → active/selected
            secondary: "#8be9fd".to_string(), // cyan → info text
            background: "#1a202c".to_string(),
            text: "#f8f8f2".to_string(),      // off-white → normal text
            highlight: "#f1fa8c".to_string(), // yellow → search/accent
            error: "#ff5555".to_string(),     // red → errors/match highlight
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
            strict_host_key_checking: "accept-new".to_string(),
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
        // If config file doesn't exist or is empty, create it with default values
        let needs_init = !self.config_file.exists()
            || fs::metadata(&self.config_file).map(|m| m.len() == 0).unwrap_or(true);

        if needs_init {
            let default_config = AppConfig::default();
            self.save_config(&default_config)?;
            return Ok(default_config);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_colors_default_values() {
        let colors = ThemeColors::default();
        assert_eq!(colors.primary, "#50fa7b");
        assert_eq!(colors.error, "#ff5555");
        assert_eq!(colors.success, "#50fa7b");
        assert!(!colors.background.is_empty());
    }

    #[test]
    fn test_app_config_default_strict_host_key_checking() {
        let config = AppConfig::default();
        assert_eq!(config.strict_host_key_checking, "accept-new");
    }

    #[test]
    fn test_app_config_default_theme() {
        let config = AppConfig::default();
        assert_eq!(config.default_theme, "default");
        assert!(!config.themes.is_empty());
        assert_eq!(config.themes[0].name, "default");
    }

    #[test]
    fn test_toml_parse_config_with_strict_host_key_checking() {
        let toml_str = r##"
default_theme = "default"
ssh_file_config = "/home/user/.ssh/config"
strict_host_key_checking = "yes"

[[themes]]
name = "default"
[themes.colors]
primary = "#ff0000"
secondary = "#00ff00"
background = "#000000"
text = "#ffffff"
highlight = "#0000ff"
error = "#ff005f"
warning = "#ffb86c"
success = "#50fa7b"
"##;
        let config: AppConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.strict_host_key_checking, "yes");
        assert_eq!(config.default_theme, "default");
    }

    #[test]
    fn test_toml_parse_config_defaults_strict_host_key_checking_when_missing() {
        let toml_str = r##"
default_theme = "default"
ssh_file_config = "/home/user/.ssh/config"

[[themes]]
name = "default"
[themes.colors]
primary = "#454545"
secondary = "#454545"
background = "#1a202c"
text = "#ffffff"
highlight = "#454545"
error = "#ff005f"
warning = "#ffb86c"
success = "#50fa7b"
"##;
        let config: AppConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.strict_host_key_checking, "accept-new");
    }
}
