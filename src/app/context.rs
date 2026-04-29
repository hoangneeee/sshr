use crate::config::ConfigManager;
use crate::theme::ResolvedTheme;
use std::path::PathBuf;
use std::sync::Arc;

/// Long-lived application context: config, paths, theme.
///
/// Loaded once at startup, immutable thereafter (except `theme` which is
/// behind an Arc so cloning across frames is cheap).
#[derive(Debug)]
pub struct AppContext {
    pub config_manager: ConfigManager,
    pub ssh_config_path: PathBuf,
    pub strict_host_key_checking: String,
    pub theme: Arc<ResolvedTheme>,
}
