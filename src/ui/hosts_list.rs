use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::time::SystemTime;

use crate::app::{App, InputMode, ActivePanel};
use super::footer::draw_footer;
use super::status_bar::draw_status_bar;

fn _elapsed() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn draw<B: Backend>(f: &mut Frame, app: &mut App) {
    let size = f.size();

    // Create a layout with three sections: main content, status bar, and footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main content
            Constraint::Length(1), // Status bar
            Constraint::Length(1), // Footer
        ].as_ref())
        .split(size);

    // Draw the main content with two-panel layout
    draw_hosts_list::<B>(f, app, chunks[0]);

    // Draw the status bar
    draw_status_bar::<B>(f, app, chunks[1]);

    // Draw the footer with navigation help
    draw_footer::<B>(f, app, chunks[2]);

    // Draw loading overlay if needed
    if app.is_connecting {
        draw_enhanced_loading_overlay::<B>(f, app);
    }
}

fn draw_hosts_list<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Groups panel
            Constraint::Percentage(70), // Hosts panel
        ].as_ref())
        .split(area);

    // Draw groups panel
    draw_groups_panel::<B>(f, app, chunks[0]);
    
    // Draw hosts panel
    draw_hosts_panel::<B>(f, app, chunks[1]);
}

fn draw_groups_panel<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_panel == ActivePanel::Groups;
    let title = format!(
        " {} 🫂 Groups ",
        if is_active { ">" } else { " " }
    );
    
    let items: Vec<ListItem> = app.groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let is_selected = i == app.selected_group && is_active;
            let prefix = if is_selected { "> " } else { "  " };
            
            let (text_style, bg_style) = if is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    Style::default().bg(Color::Green)
                )
            } else {
                (Style::default().fg(Color::White), Style::default())
            };
            
            let mut spans = vec![
                Span::styled(prefix, text_style),
                Span::styled(
                    format!("[{}] {}", i + 1, group),
                    if is_selected {
                        text_style
                    } else {
                        text_style.fg(Color::LightYellow).add_modifier(Modifier::BOLD)
                    }
                )
            ];
            
            let line = Line::from(spans);
            ListItem::new(line).style(bg_style)
        })
        .collect();
    
    let border_style = if is_active {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
    );
    
    f.render_stateful_widget(list, area, &mut app.group_list_state);
}

fn draw_hosts_panel<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    let is_search_mode = app.input_mode == InputMode::Search;
    let is_active = app.active_panel == ActivePanel::Hosts;
    let title = if is_search_mode {
        format!(
            " {} Search: {} Results ({} matches) ",
            if is_active { ">" } else { " " },
            app.search_query,
            app.filtered_hosts.len()
        )
    } else {
        format!(
            " {} 👤 Hosts ",
            if is_active { ">" } else { " " }
        )
    };
    
    // Get the appropriate list of hosts to display
    let hosts_to_display = if is_search_mode {
        &app.filtered_hosts
    } else {
        &app.hosts_in_current_group
    };
    
    let items: Vec<ListItem> = hosts_to_display
        .iter()
        .enumerate()
        .filter_map(|(i, &host_idx)| {
            app.hosts.get(host_idx).map(|host| {
                let is_selected = if is_search_mode {
                    i == app.search_selected && app.active_panel == ActivePanel::Hosts
                } else {
                    i == app.selected_host && app.active_panel == ActivePanel::Hosts
                };
                
                let prefix = if is_selected { "> " } else { "  " };
                
                let (text_style, bg_style) = if is_selected {
                    (
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(Color::Green)
                    )
                } else {
                    (Style::default().fg(Color::White), Style::default())
                };
                
                let mut spans = vec![Span::styled(prefix, text_style)];
                
                // Add host number
                spans.push(Span::styled(
                    format!("[{}] ", i + 1),
                    text_style.add_modifier(Modifier::BOLD).fg(Color::LightYellow)
                ));
                
                // Add host alias with search highlighting if in search mode
                if is_search_mode && !app.search_query.is_empty() {
                    let alias_lower = host.alias.to_lowercase();
                    let query_lower = app.search_query.to_lowercase();
                    let mut start = 0;
                    
                    if let Some(match_start) = alias_lower.find(&query_lower) {
                        // Add text before match
                        if match_start > 0 {
                            spans.push(Span::styled(
                                host.alias[..match_start].to_string(),
                                text_style
                            ));
                        }
                        
                        // Add matched text with highlight
                        spans.push(Span::styled(
                            host.alias[match_start..match_start + query_lower.len()].to_string(),
                            Style::default()
                                .bg(Color::Yellow)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD)
                        ));
                        
                        // Add remaining text after match
                        let remaining_start = match_start + query_lower.len();
                        if remaining_start < host.alias.len() {
                            spans.push(Span::styled(
                                host.alias[remaining_start..].to_string(),
                                text_style
                            ));
                        }
                    } else {
                        // No match found, just add the alias
                        spans.push(Span::styled(host.alias.clone(), text_style));
                    }
                } else {
                    // Not in search mode, just add the alias
                    spans.push(Span::styled(host.alias.clone(), text_style));
                }

                // Add host details
                let details = format!(" ({}@{}:{})", host.user, host.host, host.port.unwrap_or(22));
                spans.push(Span::styled(details, text_style.fg(Color::Gray)));
                
                let item_text = Line::from(spans);
                ListItem::new(item_text).style(bg_style)
            })
        })
        .collect();
    
    let list = if items.is_empty() {
        let message = if is_search_mode {
            format!("No results for '{}'", app.search_query)
        } else {
            "No hosts in this group".to_string()
        };
        List::new(vec![ListItem::new(Span::styled(
            message,
            Style::default().fg(Color::Gray).not_italic()
        ))])
    } else {
        List::new(items)
    };
    
    let border_style = if is_search_mode {
        Style::default().fg(Color::Yellow)
    } else if is_active {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    let list = list.block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
    );
    
    f.render_stateful_widget(list, area, &mut app.host_list_state);
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
    let loading_content = if app.is_sftp_loading {
        let status_text = if let Some((msg, _)) = &app.status_message {
            msg.clone()
        } else {
            "Initializing SFTP".to_string()
        };
        vec![
            Line::from(vec![
                Span::styled("🔄 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "SFTP Initialization",
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
                Span::styled("💡 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Please wait...",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
            ]),
        ]
    } else if let Some(host) = app.get_selected_host() {
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
    
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}
