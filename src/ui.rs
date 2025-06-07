use std::time::SystemTime;

use crate::app::{App, InputMode};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub fn draw<B: Backend>(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(3),    // Main content
                Constraint::Length(1), // Status bar
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_hosts_list::<B>(f, app, chunks[0]);
    draw_status_bar::<B>(f, app, chunks[1]);
    draw_footer::<B>(f, app, chunks[2]);

    // Draw loading overlay if connecting
    if app.is_connecting {
        draw_enhanced_loading_overlay::<B>(f, app);
    }
}

/// Helper function to center a rectangle with given width and height
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length((r.height.saturating_sub(height)) / 2),
                Constraint::Length(height),
                Constraint::Length((r.height.saturating_sub(height)) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn draw_hosts_list<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    let title = match app.input_mode {
        InputMode::Normal => "SSHr - SSH Manager: Easy control your SSH hosts".to_string(),
        InputMode::Search => format!("Search: {}_", app.search_query),
        InputMode::Sftp => "SFTP MODE".to_string(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .title(title);

    let list_items: Vec<ListItem> = match app.input_mode {
        InputMode::Normal => {
            app.hosts
                .iter()
                .enumerate()
                .map(|(i, host)| {
                    let is_selected = i == app.selected;

                    // Build the host display text
                    let mut text = vec![];

                    // Add selection indicator
                    text.push(Span::styled(
                        if is_selected { "> " } else { "  " },
                        Style::default().fg(Color::Green),
                    ));

                    // Add host number
                    text.push(Span::styled(
                        format!("[{}] ", i + 1),
                        Style::default().fg(Color::Yellow),
                    ));

                    // Add alias and connection info
                    text.push(Span::styled(
                        format!(
                            "{} ({}@{}:{})",
                            host.alias,
                            host.user,
                            host.host,
                            host.port.unwrap_or(22)
                        ),
                        Style::default().fg(if is_selected {
                            Color::Black
                        } else {
                            Color::White
                        }),
                    ));

                    // Add group if exists
                    if let Some(group) = &host.group {
                        text.push(Span::raw(" "));
                        text.push(Span::styled(
                            format!("[Group: {}]", group),
                            Style::default()
                                .fg(if is_selected {
                                    Color::Black
                                } else {
                                    Color::Gray
                                })
                                .add_modifier(Modifier::DIM),
                        ));
                    }

                    let style = if is_selected {
                        Style::default()
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(Line::from(text)).style(style)
                })
                .collect()
        }
        InputMode::Search => {
            app.filtered_hosts
                .iter()
                .enumerate()
                .map(|(i, &host_index)| {
                    let host = &app.hosts[host_index];
                    let is_selected = i == app.search_selected;
                    let mut text = vec![];

                    // Add selection indicator
                    text.push(Span::styled(
                        if is_selected { "> " } else { "  " },
                        Style::default().fg(Color::Green),
                    ));
                    // Add host number (show original index)
                    text.push(Span::styled(
                        format!("[{}] ", host_index + 1),
                        Style::default().fg(Color::Yellow),
                    ));

                    // Highlight matching text
                    let query_lower = app.search_query.to_lowercase();
                    let alias_highlighted = highlight_text(&host.alias, &query_lower);
                    let host_highlighted = highlight_text(&host.host, &query_lower);

                    text.extend(alias_highlighted);
                    text.push(Span::raw(" ("));
                    text.push(Span::styled(
                        format!("{}@", host.user),
                        Style::default().fg(if is_selected {
                            Color::Black
                        } else {
                            Color::White
                        }),
                    ));
                    text.extend(host_highlighted);
                    text.push(Span::styled(
                        format!(":{})", host.port.unwrap_or(22)),
                        Style::default().fg(if is_selected {
                            Color::Black
                        } else {
                            Color::White
                        }),
                    ));

                    let style = if is_selected {
                        Style::default()
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(Line::from(text)).style(style)
                })
                .collect()
        }
        InputMode::Sftp => {
            vec![]
        }
    };

    let list = List::new(list_items)
        .block(block)
        .highlight_symbol(">")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

fn draw_status_bar<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some((message, timestamp)) = &app.status_message {
        // Clear messages older than 5 seconds (except when connecting)
        let should_show = if app.is_connecting {
            true // Always show status during connection
        } else {
            timestamp.elapsed().as_secs() < 5
        };

        if should_show {
            let style = if message.to_lowercase().contains("error")
                || message.to_lowercase().contains("failed")
            {
                Style::default().fg(Color::Red)
            } else if message.to_lowercase().contains("success")
                || message.to_lowercase().contains("successful")
                || message.to_lowercase().contains("ended")
            {
                Style::default().fg(Color::Green)
            } else if message.to_lowercase().contains("connecting")
                || message.to_lowercase().contains("testing")
            {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let paragraph = Paragraph::new(message.as_str())
                .style(style)
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(paragraph, area);
        } else {
            // Clear the status message if it's expired
            app.clear_status_message();
        }
    }
}

fn draw_footer<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (nav_text, action_text) = match app.input_mode {
        InputMode::Normal if app.is_connecting => {
            ("Connecting to SSH host...", "[Ctrl+C] Cancel")
        }
        InputMode::Normal => {
            ("↑/k: Up  ↓/j: Down  [Enter] Connect  [s] Search", "[e] Edit [r] Reload [q] Quit")
        }
        InputMode::Search => {
            ("↑: Up  ↓: Down  [Enter] Connect", "[Esc] Exit Search  Type to filter")
        }
        InputMode::Sftp => {
            ("↑: Up  ↓: Down  [Enter] Connect", "[Esc] Exit Search  Type to filter")
        }
    };
    

    let nav_help = Paragraph::new(nav_text).style(Style::default().fg(if app.is_connecting {
        Color::Yellow
    } else {
        Color::Gray
    }));

    let action_help = Paragraph::new(action_text)
        .style(Style::default().fg(if app.is_connecting {
            Color::Red
        } else {
            Color::Gray
        }))
        .alignment(ratatui::layout::Alignment::Right);

    f.render_widget(nav_help, footer[0]);
    f.render_widget(action_help, footer[1]);
}

fn draw_enhanced_loading_overlay<B: Backend>(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 10, f.size());

    // Get current time for animation
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    // Create animated dots
    let dots_count = (now / 500) % 4;
    let dots = ".".repeat(dots_count as usize);
    let padding = " ".repeat(3 - dots_count as usize);

    // Get status message or default
    let status_text = if let Some((msg, _)) = &app.status_message {
        msg.clone()
    } else {
        "Connecting".to_string()
    };

    // Create loading content with animation
    let loading_content = if let Some(host) = app.get_selected_host() {
        vec![
            Line::from(vec![
                Span::styled("🔗 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("SSH Connection to {}", host.alias),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📡 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    format!("{}{}", status_text, dots),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(padding),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Host: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}@{}:{}", host.user, host.host, host.port.unwrap_or(22)),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Press Ctrl+C to cancel",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("🔗 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "SSH Connection",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("{}{}", status_text, dots),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(padding),
            ]),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" SSH Manager ")
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(loading_content)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

    // Clear the area và render loading overlay
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn highlight_text<'a>(text: &'a str, query: &str) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::raw(text)];
    }

    let mut spans = Vec::new();
    let text_lower = text.to_lowercase();
    
    if let Some(pos) = text_lower.find(query) {
        // Before match
        if pos > 0 {
            spans.push(Span::raw(&text[..pos]));
        }
        
        // Matched part (highlighted)
        spans.push(Span::styled(
            &text[pos..pos + query.len()],
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        ));
        
        // After match
        if pos + query.len() < text.len() {
            spans.push(Span::raw(&text[pos + query.len()..]));
        }
    } else {
        spans.push(Span::raw(text));
    }
    
    spans
}