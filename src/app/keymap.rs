use crate::app_event::SshEvent;
use crate::app::{App, InputMode};
use crate::ui::hosts_list::draw;
use anyhow::Result;
use ratatui::{backend::Backend, Terminal};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Instant;


impl App {
    // Handle key
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

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

            // Redraw UI để hiển thị loading
            terminal.draw(|f| draw::<B>(f, self))?;
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
}
