use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected: usize,
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
        Self {
            should_quit: false,
            hosts: Vec::new(),
            selected: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        // TODO: Load hosts from config file
        Self::default()
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
            _ => {}
        }
    }
}