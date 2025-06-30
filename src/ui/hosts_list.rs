use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::time::SystemTime;

use crate::app::{App, InputMode, AppKeymapExt};
use crate::models::SshHost;
use crate::ui::{footer::draw_footer, status_bar::draw_status_bar};

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

    // Draw loading overlay if connecting or initializing SFTP
    if app.is_connecting || app.is_sftp_loading {
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

fn draw_hosts_list<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
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
            // Group hosts by their group name (or "Ungrouped" if none)
            use std::collections::BTreeMap;
            let mut groups: BTreeMap<String, Vec<&SshHost>> = BTreeMap::new();
            
            // Group hosts by their group
            for host in &app.hosts {
                let group_name = host.group.as_deref().unwrap_or("Ungrouped").to_string();
                groups.entry(group_name).or_default().push(host);
            }
            
            // Update app groups
            app.groups = groups.keys().cloned().collect();
            
            let mut items = Vec::new();
            let mut global_index = 0;
            
            // Add each group with its hosts
            for (group_name, hosts) in groups {
                let is_collapsed = app.collapsed_groups.contains(&group_name);
                let is_current_group = app.get_current_group() == Some(&group_name[..]);
                
                // Add group header with collapse indicator
                let header = if is_collapsed {
                    format!("▶ [{}] (collapsed)", group_name)
                } else {
                    format!("▼ [{}]", group_name)
                };
                
                let header_style = if is_current_group {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                
                items.push(ListItem::new(Line::from(Span::styled(header, header_style))));
                
                // Skip hosts if group is collapsed
                if is_collapsed {
                    continue;
                }
                
                // Add hosts in this group
                for (local_index, host) in hosts.iter().enumerate() {
                    let is_selected = global_index == app.selected;
                    let mut text = vec![];

                    // Add selection indicator with indentation
                    text.push(Span::styled(
                        if is_selected { "  > " } else { "    " },
                        Style::default().fg(Color::Green),
                    ));

                    // Add host number
                    text.push(Span::styled(
                        format!("[{}] ", global_index + 1),
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

                    let style = if is_selected {
                        Style::default()
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    items.push(ListItem::new(Line::from(text)).style(style));
                    global_index += 1;
                }
                
                // Add empty line between groups
                // items.push(ListItem::new(""));
            }
            
            items
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
        .block(block);
        // .highlight_symbol(">")
        // .highlight_style(Style::default().add_modifier(Modifier::BOLD));

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
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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
