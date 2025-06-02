use anyhow::Result;
use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::fs::File;
use std::{io, time::Duration};
use tracing::{Level, debug};
use tracing_subscriber::{EnvFilter, fmt};

mod app;
mod error;
mod ui;

use app::App;
use error::Error;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    let log_dir = "logs";
    if !std::path::Path::new(log_dir).exists() {
        std::fs::create_dir_all(log_dir)?;
    }

    // Tạo tên file log với timestamp
    let log_file = format!(
        "{}/sshr_{}.log",
        log_dir,
        Local::now().format("%Y%m%d_%H%M%S")
    );

    // Ghi log ra file
    let file = File::create(&log_file)?;

    fmt()
        .with_max_level(Level::DEBUG)
        .with_env_filter(EnvFilter::from_default_env().add_directive("sshr=debug".parse()?))
        .with_ansi(false)
        .with_writer(file)
        .init();

    debug!("Khởi tạo ứng dụng...");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new().expect("Failed to create app");
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<(), Error> {
    loop {
        terminal.draw(|f| ui::draw::<B>(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char(c) => app.on_key(c),
                    KeyCode::Up => app.on_key('k'),
                    KeyCode::Down => app.on_key('j'),
                    _ => {}
                }

                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}
