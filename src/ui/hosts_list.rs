use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
    let title = format!(
        " {} Groups ",
        if app.active_panel == ActivePanel::Groups { ">" } else { " " }
    );
    
    let items: Vec<ListItem> = app.groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let is_selected = i == app.selected_group && app.active_panel == ActivePanel::Groups;
            let prefix = if is_selected { "> " } else { "  " };
            
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("[{}] {}", i + 1, group), style),
            ]))
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));
    
    f.render_stateful_widget(list, area, &mut app.group_list_state);
}

fn draw_hosts_panel<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        " {} Hosts ",
        if app.active_panel == ActivePanel::Hosts { ">" } else { " " }
    );
    
    let items: Vec<ListItem> = app.hosts_in_current_group
        .iter()
        .enumerate()
        .filter_map(|(i, &host_idx)| {
            app.hosts.get(host_idx).map(|host| {
                let is_selected = i == app.selected_host && app.active_panel == ActivePanel::Hosts;
                let prefix = if is_selected { "> " } else { "  " };
                
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                
                let host_display = format!("[{}] {} ({}@{}:{})", i + 1, host.alias, host.user, host.host, host.port.unwrap_or(22));
                
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(host_display, style),
                ]))
            })
        })
        .collect();
    
    let list = if items.is_empty() {
        List::new(vec![ListItem::new("No hosts in this group")])
    } else {
        List::new(items)
    };
    
    let list = list.block(Block::default().borders(Borders::ALL).title(title));
    f.render_stateful_widget(list, area, &mut app.host_list_state);
}

fn draw_enhanced_loading_overlay<B: Backend>(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 10, f.size());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Connecting... ")
        .style(Style::default().bg(Color::Black).fg(Color::White));

    let text = vec![
        Line::from("Establishing connection..."),
        Line::from(""),
        Line::from(format!("Host: {}", app.get_current_host().map(|h| h.alias.clone()).unwrap_or_default())),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

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
