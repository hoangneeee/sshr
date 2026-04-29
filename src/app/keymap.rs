use crate::app::hosts_state::ActivePanel;
use crate::app::{App, InputMode};
use anyhow::Result;
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::time::Instant;

impl App {
    // Handle key
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

            self.ui.status_message = Some((
                format!("Connecting to {}...", selected_host.alias),
                Instant::now(),
            ));

            // Set up SessionState::Ssh and get the worker's sender end.
            let sender = self.start_ssh_session(selected_host.clone());

            // Worker shells out to ssh (blocking) — run on tokio's blocking pool
            // so we don't tie up an executor thread.
            let strict_host_key_checking = self.ctx.strict_host_key_checking.clone();
            tokio::task::spawn_blocking(move || {
                Self::ssh_thread_worker(sender, selected_host, strict_host_key_checking);
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
        let hosts_path = self.ctx.config_manager.get_hosts_path();

        // Create the file if it doesn't exist
        if !hosts_path.exists() {
            if let Some(parent) = hosts_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&hosts_path, "")?;
        }

        // TODO: Can use nvim, vim, nano if exist instead of default text editor
        if let Err(e) = open::that(&hosts_path) {
            tracing::error!("Failed to open editor: {}", e);
            return Err(anyhow::anyhow!("Failed to open editor: {}", e));
        }

        // Reload the config after the editor is closed
        self.load_all_hosts()?;

        Ok(())
    }

    pub fn handle_key_esc(&mut self) -> Result<()> {
        self.ui.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn handle_key_tab(&mut self) -> Result<()> {
        self.hosts.switch_panel();
        Ok(())
    }

    pub fn handle_key_right(&mut self) -> Result<()> {
        match self.hosts.active_panel {
            ActivePanel::Groups => self.hosts.cycle_group(true),
            ActivePanel::Hosts => self.hosts.select_next(),
        }
        Ok(())
    }

    pub fn handle_key_left(&mut self) -> Result<()> {
        match self.hosts.active_panel {
            ActivePanel::Groups => self.hosts.cycle_group(false),
            ActivePanel::Hosts => self.hosts.select_previous(),
        }
        Ok(())
    }

    pub fn handle_shift_tab(&mut self) -> Result<()> {
        self.hosts.cycle_group(false);
        Ok(())
    }
}
