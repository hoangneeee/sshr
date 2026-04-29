use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::time::SystemTime;

use crate::app::{ActivePanel, App};
use super::footer::{draw_footer, FooterKind};
use super::status_bar::draw_status_bar;

/// Draw the host browser screen.
///
/// `is_search_mode` controls whether the search input + filtered results
/// are shown instead of the group's hosts.
pub fn draw(f: &mut Frame, app: &mut App, is_search_mode: bool) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main content
            Constraint::Length(1), // Status bar
            Constraint::Length(1), // Footer
        ].as_ref())
        .split(size);

    draw_hosts_list(f, app, chunks[0], is_search_mode);
    draw_status_bar(f, app, chunks[1]);

    let footer_kind = if app.session.is_ssh_connecting() {
        FooterKind::Connecting
    } else if is_search_mode {
        FooterKind::Search
    } else {
        FooterKind::Normal
    };
    draw_footer(f, app, chunks[2], footer_kind);

    if app.session.is_ssh_connecting() {
        draw_enhanced_loading_overlay(f, app);
    }
}

fn draw_hosts_list(f: &mut Frame, app: &mut App, area: Rect, is_search_mode: bool) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Groups panel
            Constraint::Percentage(70), // Hosts panel
        ].as_ref())
        .split(area);

    draw_groups_panel(f, app, chunks[0]);
    draw_hosts_panel(f, app, chunks[1], is_search_mode);
}

fn draw_groups_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.hosts.active_panel == ActivePanel::Groups;
    let title = format!(
        " {} 🫂 Groups ",
        if is_active { ">" } else { " " }
    );

    let items: Vec<ListItem> = app.hosts.groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let is_selected = i == app.hosts.selected_group && is_active;
            let prefix = if is_selected { "> " } else { "  " };

            let (text_style, bg_style) = if is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(app.ctx.theme.primary)
                        .add_modifier(Modifier::BOLD),
                    Style::default().bg(app.ctx.theme.primary)
                )
            } else {
                (Style::default().fg(app.ctx.theme.text), Style::default())
            };

            let spans = vec![
                Span::styled(prefix, text_style),
                Span::styled(
                    format!("[{}] {}", i + 1, group),
                    if is_selected {
                        text_style
                    } else {
                        text_style.fg(app.ctx.theme.highlight).add_modifier(Modifier::BOLD)
                    }
                )
            ];

            let line = Line::from(spans);
            ListItem::new(line).style(bg_style)
        })
        .collect();

    let border_style = if is_active {
        Style::default().fg(app.ctx.theme.primary)
    } else {
        Style::default()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
    );

    f.render_stateful_widget(list, area, &mut app.hosts.group_list_state);
}

fn draw_hosts_panel(f: &mut Frame, app: &mut App, area: Rect, is_search_mode: bool) {
    let is_active = app.hosts.active_panel == ActivePanel::Hosts;

    let (list_area, list_border_style, list_title) = if is_search_mode {
        let search_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input area
                Constraint::Min(0),    // Search results area
            ].as_ref())
            .split(area);

        let search_title = " 🔍 Search (Press 'Esc' to exit) ";
        let search_block = Block::default()
            .borders(Borders::ALL)
            .title(search_title)
            .border_style(Style::default().fg(app.ctx.theme.highlight));

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let cursor = if now % 1000 < 500 { "█" } else { " " };

        let search_text = format!("{} {}", app.search.query, cursor);
        let search_paragraph = Paragraph::new(search_text)
            .style(Style::default().fg(app.ctx.theme.text))
            .block(search_block);

        f.render_widget(search_paragraph, search_chunks[0]);

        let results_title = format!(
            " {} Results ({} matches) ",
            if is_active { ">" } else { " " },
            app.search.filtered.len()
        );

        (
            search_chunks[1],
            Style::default().fg(app.ctx.theme.highlight),
            results_title,
        )
    } else {
        (
            area,
            if is_active { Style::default().fg(app.ctx.theme.primary) } else { Style::default() },
            format!(" {} 👤 Hosts ", if is_active { ">" } else { " " }),
        )
    };

    let hosts_to_display = if is_search_mode {
        app.search.filtered
            .iter()
            .map(|fh| (fh.clone(), app.hosts.hosts.get(fh.original_index).unwrap().clone()))
            .collect::<Vec<_>>()
    } else {
        app.hosts.hosts_in_current_group
            .iter()
            .map(|&idx| {
                let host = app.hosts.hosts.get(idx).unwrap().clone();
                let filtered_host = crate::app::FilteredHost {
                    original_index: idx,
                    score: 0,
                    matched_indices: vec![],
                };
                (filtered_host, host)
            })
            .collect::<Vec<_>>()
    };

    let items: Vec<ListItem> = hosts_to_display
        .iter()
        .enumerate()
        .map(|(i, (filtered_host, host))| {
            let is_selected = if is_search_mode {
                i == app.search.selected && app.hosts.active_panel == ActivePanel::Hosts
            } else {
                i == app.hosts.selected_host && app.hosts.active_panel == ActivePanel::Hosts
            };

            let prefix = if is_selected { "> " } else { "  " };

            let (text_style, bg_style) = if is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(if is_search_mode { app.ctx.theme.highlight } else { app.ctx.theme.primary })
                        .add_modifier(Modifier::BOLD),
                    Style::default().bg(if is_search_mode { app.ctx.theme.highlight } else { app.ctx.theme.primary })
                )
            } else {
                (Style::default().fg(app.ctx.theme.text), Style::default())
            };

            let mut spans = vec![Span::styled(prefix, text_style)];

            spans.push(Span::styled(
                format!("[{}] ", i + 1),
                text_style.add_modifier(Modifier::BOLD).fg(if is_selected { Color::Black } else { app.ctx.theme.highlight })
            ));

            if is_search_mode && !app.search.query.is_empty() {
                let alias_chars: Vec<char> = host.alias.chars().collect();
                let mut last_idx = 0;
                for (idx, &ch) in alias_chars.iter().enumerate() {
                    if filtered_host.matched_indices.contains(&idx) {
                        if idx > last_idx {
                            let prefix: String = alias_chars[last_idx..idx].iter().collect();
                            spans.push(Span::styled(prefix, text_style));
                        }
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(app.ctx.theme.error).add_modifier(Modifier::BOLD),
                        ));
                        last_idx = idx + 1;
                    }
                }
                if last_idx < alias_chars.len() {
                    let suffix: String = alias_chars[last_idx..].iter().collect();
                    spans.push(Span::styled(suffix, text_style));
                }
            } else {
                spans.push(Span::styled(host.alias.clone(), text_style));
            }

            let details = format!(" ({}@{}:{})", host.user, host.host, host.port.unwrap_or(22));
            let details_style = if is_selected {
                text_style.add_modifier(Modifier::DIM)
            } else {
                text_style.fg(app.ctx.theme.secondary)
            };
            spans.push(Span::styled(details, details_style));

            let item_text = Line::from(spans);
            ListItem::new(item_text).style(bg_style)
        })
        .collect();

    let list = if items.is_empty() {
        let message = if is_search_mode {
            format!("No results for '{}'", app.search.query)
        } else {
            "No hosts in this group".to_string()
        };
        List::new(vec![ListItem::new(Span::styled(
            message,
            Style::default().fg(app.ctx.theme.secondary).not_italic()
        ))])
    } else {
        List::new(items)
    };

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(list_border_style)
        .title(list_title);

    let list_widget = list.block(list_block);

    f.render_stateful_widget(list_widget, list_area, &mut app.hosts.host_list_state);
}

fn draw_enhanced_loading_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 10, f.size());

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let dots_count = (now / 500) % 4;
    let dots = ".".repeat(dots_count as usize);
    let padding = " ".repeat(3 - dots_count as usize);

    let status_text = if let Some((msg, _)) = &app.ui.status_message {
        msg.clone()
    } else {
        "Connecting".to_string()
    };

    let theme = &app.ctx.theme;
    let accent_style = Style::default().fg(theme.highlight);
    let title_style = Style::default().fg(theme.text).add_modifier(Modifier::BOLD);
    let info_style = Style::default().fg(theme.secondary);
    let muted_style = Style::default().fg(theme.text).add_modifier(Modifier::DIM);
    let label_style = Style::default().fg(theme.secondary);

    let loading_content = if app.session.is_sftp_loading() {
        let status_text = if let Some((msg, _)) = &app.ui.status_message {
            msg.clone()
        } else {
            "Initializing SFTP".to_string()
        };
        vec![
            Line::from(vec![
                Span::styled("🔄 ", accent_style),
                Span::styled("SFTP Initialization", title_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📡 ", info_style),
                Span::styled(format!("{}{}", status_text, dots), info_style),
                Span::raw(padding),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", accent_style),
                Span::styled("Please wait...", muted_style),
            ]),
        ]
    } else if let Some(host) = &app.session.connecting_host() {
        vec![
            Line::from(vec![
                Span::styled("🔗 ", accent_style),
                Span::styled(format!("SSH Connection to {}", host.alias), title_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("📡 ", info_style),
                Span::styled(format!("{}{}", status_text, dots), info_style),
                Span::raw(padding),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Host: ", label_style),
                Span::styled(
                    format!("{}@{}:{}", host.user, host.host, host.port.unwrap_or(22)),
                    Style::default().fg(theme.success),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 ", accent_style),
                Span::styled("Press Ctrl+C to cancel", muted_style),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("🔗 ", accent_style),
                Span::styled("SSH Connection", title_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("{}{}", status_text, dots), info_style),
                Span::raw(padding),
            ]),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" SSH Manager ")
        .title_style(accent_style.add_modifier(Modifier::BOLD))
        .border_style(accent_style);

    let paragraph = Paragraph::new(loading_content)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

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
