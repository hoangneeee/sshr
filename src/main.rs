use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs::File;
use std::io;
use std::path::Path;
use tracing_subscriber::{fmt, EnvFilter};

mod app;
mod app_event;
mod config;
mod constants;
mod models;
mod sftp_logic;
mod theme;
mod ui;

use crate::app::{App, ScreenAction};
use crate::constants::{POLL_CONNECTING, POLL_NORMAL};

/// A TUI for managing and connecting to SSH hosts
/// Git: https://github.com/hoangneeee/sshr
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

fn setup_logging() -> Result<()> {
    let log_dir = if cfg!(debug_assertions) {
        let dir = "logs";
        if !Path::new(dir).exists() {
            std::fs::create_dir_all(dir).context("Failed to create log directory")?;
        }
        dir.to_string()
    } else {
        let dir = "/tmp/sshr_logs";
        if !Path::new(dir).exists() {
            std::fs::create_dir_all(dir).context("Failed to create /tmp/sshr_logs directory")?;
        }
        dir.to_string()
    };

    let log_file_name = format!("{}/sshr_debug.log", log_dir);
    let log_file = File::create(&log_file_name).context("Failed to create log file")?;

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sshr=debug")),
        )
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    tracing::info!("SSHr started (log file: {})", log_file_name);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _args = Args::parse();

    if let Err(e) = setup_logging() {
        eprintln!("Failed to setup logging: {}", e);
    }

    let app = App::new().context("Failed to initialize application")?;

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen or enable mouse capture")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    tracing::info!("Running application");
    let res = run_app(&mut terminal, app).await;

    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen or disable mouse capture")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    if let Err(err) = res {
        eprintln!("\nApplication error: {:?}", err);
        tracing::error!("Application exited with error: {:?}", err);
    } else {
        tracing::info!("sshr exited successfully");
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<()> {
    loop {
        if app.screens.is_empty() {
            tracing::error!("Screen stack is empty; quitting");
            return Ok(());
        }

        // Take the top screen out of the stack so we can pass `&mut App`
        // to its methods without aliasing the stack itself.
        let mut current = app.screens.pop().expect("non-empty by check above");

        let action = current.poll(&mut app, terminal)?;
        let action = match action {
            ScreenAction::None => {
                if !current.wants_input() {
                    // Foreground process owns the terminal; block on the next
                    // event from this screen instead of polling/redrawing.
                    current.await_blocking(&mut app, terminal).await?
                } else {
                    current.draw_and_handle_input(&mut app, terminal).await?
                }
            }
            other => other,
        };

        // Dispatch the action.
        let next = match action {
            ScreenAction::None => {
                app.screens.push(current);
                None
            }
            ScreenAction::Quit => {
                app.should_quit = true;
                app.screens.push(current);
                None
            }
            ScreenAction::Push(new) => {
                app.screens.push(current);
                Some(new)
            }
            ScreenAction::Pop => None,
        };
        if let Some(next) = next {
            app.screens.push(next);
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

// Helper extension on AppScreen to keep run_app slim.
impl crate::app::AppScreen {
    async fn draw_and_handle_input<B: ratatui::backend::Backend>(
        &mut self,
        app: &mut App,
        terminal: &mut Terminal<B>,
    ) -> Result<ScreenAction> {
        terminal.draw(|f| self.draw(f, app))?;

        let poll_timeout = if app.session.is_ssh_connecting() {
            POLL_CONNECTING
        } else {
            POLL_NORMAL
        };

        if event::poll(poll_timeout).context("Event poll failed")? {
            if let CrosstermEvent::Key(key_event) = event::read().context("Event read failed")? {
                if key_event.kind == event::KeyEventKind::Press {
                    return self.handle_key(key_event, app, terminal);
                }
            }
        }
        Ok(ScreenAction::None)
    }
}
