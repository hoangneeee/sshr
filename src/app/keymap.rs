use crate::app::ActivePanel;
use crate::app::{App, InputMode};
use crate::events::SshEvent;
use anyhow::Result;
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

impl App {
    // Group navigation and management
    pub fn next_group(&mut self) {
        if !self.groups.is_empty() {
            self.selected_group = (self.selected_group + 1) % self.groups.len();
            self.group_list_state.select(Some(self.selected_group));
            self.update_hosts_for_selected_group();
            self.selected_host = 0;
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    pub fn previous_group(&mut self) {
        if !self.groups.is_empty() {
            self.selected_group = (self.selected_group + self.groups.len() - 1) % self.groups.len();
            self.group_list_state.select(Some(self.selected_group));
            self.update_hosts_for_selected_group();
            self.selected_host = 0;
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    #[allow(dead_code)]
    pub fn get_current_group(&self) -> Option<&str> {
        self.groups.get(self.selected_group).map(|s| s.as_str())
    }

    // update_hosts_for_selected_group is now implemented in state.rs
    // Handle key
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

            // Store the connecting host
            self.connecting_host = Some(selected_host.clone());

            // Tạo channel để communication
            let (sender, receiver) = mpsc::channel::<SshEvent>();
            self.ssh_receiver = Some(receiver);

            // Set connecting state
            self.is_connecting = true;
            self.ssh_ready_for_terminal = false;
            self.status_message = Some((
                format!("Connecting to {}...", selected_host.alias),
                Instant::now(),
            ));

            // Spawn SSH thread
            let host_clone = selected_host.clone();
            thread::spawn(move || {
                Self::ssh_thread_worker(sender, host_clone);
            });

            // Redraw UI to show loading
            terminal.draw(|f| crate::ui::hosts_list::draw::<B>(f, self))?;
        }
        Ok(())
    }

    pub fn handle_key_q(&mut self) -> Result<()> {
        self.should_quit = true;
        Ok(())
    }

    pub fn handle_key_e(&mut self) -> Result<()> {
        // Get the path to the hosts file
        let hosts_path = self.config_manager.get_hosts_path();

        // Create the file if it doesn't exist
        if !hosts_path.exists() {
            if let Some(parent) = hosts_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&hosts_path, "")?;
        }

        // TODO: Can use nvim, vim, nano if exist instead of default text editor
        // Open the file with the default text editor
        if let Err(e) = open::that(&hosts_path) {
            tracing::error!("Failed to open editor: {}", e);
            return Err(anyhow::anyhow!("Failed to open editor: {}", e));
        }

        // Reload the config after the editor is closed
        self.load_all_hosts()?;

        Ok(())
    }

    pub fn handle_key_esc(&mut self) -> Result<()> {
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn handle_key_tab(&mut self) -> Result<()> {
        self.switch_panel();
        Ok(())
    }

    pub fn handle_key_right(&mut self) -> Result<()> {
        match self.active_panel {
            ActivePanel::Groups => self.next_group(),
            ActivePanel::Hosts => self.select_next(),
        }
        Ok(())
    }

    pub fn handle_key_left(&mut self) -> Result<()> {
        match self.active_panel {
            ActivePanel::Groups => self.previous_group(),
            ActivePanel::Hosts => self.select_previous(),
        }
        Ok(())
    }

    pub fn handle_shift_tab(&mut self) -> Result<()> {
        self.previous_group();
        Ok(())
    }
}
