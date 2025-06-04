use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs::File;
use std::path::Path;
use std::{io, time::Duration};
use tracing_subscriber::{fmt, EnvFilter};

mod app;
mod config;
mod models;
mod ssh_service;
mod ui;
mod cmd;

use app::{App};

/// A TUI for managing and connecting to SSH hosts
/// Git: https://github.com/hoangneeee/sshr
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // No need for a custom version flag as clap provides it by default
}

fn setup_logging() -> Result<()> {
    let log_dir = if cfg!(debug_assertions) {
        // In debug mode, log to ./logs
        let dir = "logs";
        if !Path::new(dir).exists() {
            std::fs::create_dir_all(dir).context("Failed to create log directory")?;
        }
        dir.to_string()
    } else {
        // In release mode, log to /tmp/sshr_logs
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

    // Setup logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to setup logging: {}", e);
        // Continue running even if logging setup fails
    }

    // Initialize the app with configuration
    let app = App::new().context("Failed to initialize application")?;

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen or enable mouse capture")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Run the application
    tracing::info!("Running application");
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
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
        terminal.draw(|f| ui::draw::<B>(f, &mut app))?;

        if event::poll(Duration::from_millis(100)).context("Event poll failed")? {
            if let CrosstermEvent::Key(key_event) = event::read().context("Event read failed")? {
                // Only handle when key is pressed (not repeated when holding the key)
                if key_event.kind == event::KeyEventKind::Press {
                    match key_event.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app.handle_key_q()?;
                        }
                        KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                            app.should_quit = true; // Ctrl+C to quit
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.select_previous();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.select_next();
                        }
                        KeyCode::Char('e') => {
                            if let Err(e) = app.handle_key_e() {
                                tracing::error!("Failed to open editor: {}", e);
                            }
                        }
                        KeyCode::Esc => {
                            app.handle_key_esc()?;
                        }
                        KeyCode::Enter => {
                            app.handle_key_enter(terminal)?;
                        }
                        KeyCode::Char('r') => {
                            // Reload config
                            tracing::info!("Reloading SSH config...");
                            if let Err(e) = app.load_all_hosts() {
                                tracing::error!("Failed to reload SSH config: {}", e);
                                // app.status_message = Some(format!("Reload failed: {}", e));
                            } else {
                                // app.status_message = Some("Config reloaded.".to_string());
                            }
                        }
                        _ => {}
                    }
                }

                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}