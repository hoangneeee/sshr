use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHost {
    pub alias: String,
    pub host: String,
    pub user: String,
    pub port: Option<u16>,
    pub description: Option<String>,
    pub group: Option<String>,
}

impl SshHost {
    pub fn new(alias: String, host: String, user: String) -> Self {
        Self {
            alias,
            host,
            user,
            port: None,
            description: None,
            group: None,
        }
    }
}