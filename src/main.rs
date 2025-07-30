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
use std::path::Path;
use std::{fs::File, time::Instant};
use std::{io, time::Duration};
use tracing_subscriber::{fmt, EnvFilter};

mod events;
mod config;
mod models;
mod sftp;
mod app;
mod ui;

use ui::{
    hosts_list::{draw},
};

use crate::app::{App, InputMode};

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
        // Process events
        let needs_redraw = app.process_ssh_events::<B>(terminal)?;

        // Process SFTP events
        let _ = app.process_sftp_events::<B>(terminal)?;
        
        app.process_transfer_events()?;

        // If we're in SSH mode, suspend the main loop until SSH ends
        if app.ssh_ready_for_terminal {
            tracing::info!("SSH mode active - suspending main loop");

            // Wait for SSH session to end with longer intervals
            loop {
                // Check for SSH events with longer timeout
                let ssh_ended = app.process_ssh_events::<B>(terminal)?;
                if ssh_ended || !app.ssh_ready_for_terminal {
                    tracing::info!("SSH session ended or interrupted - resuming main loop");
                    break;
                }

                // Sleep longer to avoid interfering with SSH session
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // Force redraw when returning from SSH
            terminal.draw(|f| draw::<B>(f, &mut app))?;
            continue;
        }

        // Draw UI (only when not in SSH mode)
        terminal.draw(|f: &mut ratatui::Frame<'_>| match app.input_mode {
            InputMode::Sftp => {
                if let Some(sftp_state) = &mut app.sftp_state {
                    sftp::ui::draw_sftp::<B>(f, sftp_state);
                } else {
                    draw::<B>(f, &mut app);
                }
            }
            _ => draw::<B>(f, &mut app),
        })?;

        // Handle terminal events with appropriate timeout
        let poll_timeout = if app.is_connecting {
            Duration::from_millis(50) // Faster polling when connecting
        } else {
            Duration::from_millis(100) // Normal polling
        };

        if event::poll(poll_timeout).context("Event poll failed")? {
            if let CrosstermEvent::Key(key_event) = event::read().context("Event read failed")? {
                // Only handle keys if not connecting and not in SSH mode
                if key_event.kind == event::KeyEventKind::Press
                    && !app.is_connecting
                    && !app.ssh_ready_for_terminal
                {
                    handle_key_events(&mut app, key_event, terminal).await?;
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }

        // Force redraw if needed
        if needs_redraw {
            terminal.draw(|f| match app.input_mode {
                InputMode::Sftp => {
                    if let Some(sftp_state) = &mut app.sftp_state {
                        sftp::ui::draw_sftp::<B>(f, sftp_state);
                    } else {
                        draw::<B>(f, &mut app);
                    }
                }
                _ => draw::<B>(f, &mut app),
            })?;
        }
    }
}

async fn handle_key_events<B: ratatui::backend::Backend>(
    app: &mut App,
    key_event: crossterm::event::KeyEvent,
    terminal: &mut Terminal<B>,
) -> Result<()> {
    match app.input_mode {
        InputMode::Normal => match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.handle_key_q()?;
            }
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                app.should_quit = true;
            }
            KeyCode::Char('s') => {
                // Enter search mode
                app.enter_search_mode();
            }
            KeyCode::Tab => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    app.handle_shift_tab()?;
                } else {
                    app.handle_key_tab()?;
                }
            }
            KeyCode::Right => {
                app.handle_key_right()?;
            }
            KeyCode::Left => {
                app.handle_key_left()?;
            }
            KeyCode::Char('f') => {
                // Enter SFTP mode
                app.enter_sftp_mode(terminal)?;
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
                    app.status_message =
                        Some((format!("Failed to open editor: {}", e), Instant::now()));
                }
            }
            KeyCode::Esc => {
                app.handle_key_esc()?;
            }
            KeyCode::Enter => {
                app.handle_key_enter(terminal)?;
            }
            KeyCode::Char('r') => {
                tracing::info!("Reloading SSH config...");
                if let Err(e) = app.load_all_hosts() {
                    tracing::error!("Failed to reload SSH config: {}", e);
                    app.status_message = Some((format!("Reload failed: {}", e), Instant::now()));
                } else {
                    app.status_message =
                        Some(("Config reloaded successfully".to_string(), Instant::now()));
                }
            }
            _ => {}
        },
        InputMode::Search => {
            match key_event.code {
                KeyCode::Char(c) => {
                    app.search_query.push(c);
                    app.filter_hosts();
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    app.search_query.pop();
                    app.filter_hosts();
                }
                KeyCode::Enter => {
                    // Connect to selected filtered host
                    app.handle_key_enter(terminal)?;
                    app.clear_search();
                }
                KeyCode::Esc => {
                    app.clear_search();
                }
                KeyCode::Up => {
                    app.search_select_previous();
                }
                KeyCode::Down => {
                    app.search_select_next();
                }
                _ => {}
            }
        }
        
        // SFTP INPUT MODE
        InputMode::Sftp => app.handle_sftp_key(key_event).await?,
    }
    Ok(())
}
