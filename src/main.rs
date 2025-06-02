use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::fs::File;
use std::{io, time::Duration};
use tracing_subscriber::{EnvFilter, fmt};
use std::path::Path;

mod app;
mod ui;
mod models;
mod ssh_service;

use app::App;

/// A TUI for managing and connecting to SSH hosts
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // No need for a custom version flag as clap provides it by default
}

fn setup_logging() -> Result<()> {
    let log_dir = if cfg!(debug_assertions) {
        // Ở chế độ debug: ghi log vào thư mục ./logs
        let dir = "logs";
        if !Path::new(dir).exists() {
            std::fs::create_dir_all(dir).context("Failed to create log directory")?;
        }
        dir.to_string()
    } else {
        // Ở chế độ release: ghi log vào /tmp/sshr_logs
        let dir = "/tmp/sshr_logs";
        if !Path::new(dir).exists() {
            std::fs::create_dir_all(dir).context("Failed to create /tmp/sshr_logs directory")?;
        }
        dir.to_string()
    };

    let log_file_name = format!(
        "{}/sshr_{}.log",
        log_dir,
        Local::now().format("%Y%m%d_%H%M%S")
    );

    let log_file = File::create(&log_file_name).context("Failed to create log file")?;
    
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sshr=debug")),
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

    // Cài đặt logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to setup logging: {}", e);
        // Vẫn tiếp tục chạy ngay cả khi không setup được logging
    }

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen or enable mouse capture")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new().expect("Failed to create app");
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
) -> Result<()> { // Trả về anyhow::Result<()>
    loop {
        terminal.draw(|f| ui::draw::<B>(f, &mut app))?;

        if event::poll(Duration::from_millis(100)).context("Event poll failed")? {
            if let CrosstermEvent::Key(key_event) = event::read().context("Event read failed")? {
                // Chỉ xử lý khi phím được nhấn (không phải lặp lại khi giữ phím)
                 if key_event.kind == event::KeyEventKind::Press {
                    match key_event.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                            app.should_quit = true; // Ctrl+C để thoát
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.select_previous();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.select_next();
                        }
                        KeyCode::Enter => {
                            if let Some(selected_host) = app.get_selected_host().cloned() { // Clone để tránh borrow issue
                                tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

                                // 1. Tạm dừng TUI, khôi phục terminal về trạng thái bình thường
                                disable_raw_mode().context("Failed to disable raw mode for SSH")?;
                                let mut stdout = io::stdout(); 
                                execute!(
                                    &mut stdout,
                                    LeaveAlternateScreen,
                                    DisableMouseCapture // Quan trọng: Nếu bạn có dùng mouse capture
                                )
                                .context("Failed to leave alternate screen for SSH")?;
                                terminal.show_cursor().context("Failed to show cursor for SSH")?;

                                // Xóa màn hình trước khi chạy ssh để output của ssh được sạch sẽ
                                // (Tùy chọn, ssh thường sẽ tự quản lý màn hình)
                                // print!("\x1B[2J\x1B[1;1H");
                                // io::stdout().flush().unwrap();


                                // 2. Thực thi lệnh SSH
                                match ssh_service::connect_to_host(&selected_host) {
                                    Ok(_) => {
                                        tracing::info!("SSH session for {} ended.", selected_host.alias);
                                    }
                                    Err(e) => {
                                        // Lỗi này sẽ được log, ssh thường tự hiển thị lỗi của nó
                                        tracing::error!("SSH connection to {} failed: {:?}", selected_host.alias, e);
                                        // Có thể hiển thị thông báo lỗi ngắn gọn trên TUI sau khi quay lại
                                        // app.status_message = Some(format!("SSH failed: {}", e));
                                    }
                                }

                                // 3. Khôi phục TUI
                                // Quan trọng: phải clear terminal để vẽ lại TUI sau khi ssh kết thúc
                                terminal.clear().context("Failed to clear terminal post-SSH")?; // Xóa dấu vết của ssh
                                enable_raw_mode().context("Failed to re-enable raw mode post-SSH")?;
                                let mut stdout = io::stdout(); 
                                execute!(
                                    &mut stdout,
                                    EnterAlternateScreen,
                                    EnableMouseCapture // Nếu bạn có dùng mouse capture
                                )
                                .context("Failed to re-enter alternate screen post-SSH")?;
                                // Không cần terminal.show_cursor() ở đây nếu TUI không dùng con trỏ

                                // Yêu cầu vẽ lại toàn bộ UI
                                terminal.draw(|f| ui::draw::<B>(f, &mut app))?;

                            } else {
                                tracing::warn!("Enter pressed but no host selected.");
                            }
                        }
                        KeyCode::Char('r') => { // Tải lại config
                            tracing::info!("Reloading SSH config...");
                            if let Err(e) = app.load_ssh_config() {
                                tracing::error!("Failed to reload SSH config: {}", e);
                                // app.status_message = Some(format!("Reload failed: {}", e));
                            } else {
                                // app.status_message = Some("Config reloaded.".to_string());
                            }
                        }
                        _ => {
                            // Xử lý các phím khác nếu cần
                            // if let KeyCode::Char(c) = key_event.code {
                            //     // app.on_other_key(c);
                            // }
                        }
                    }
                }

                if app.should_quit {
                    return Ok(());
                }
            }
        }
    }
}
