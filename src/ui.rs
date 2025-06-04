use crate::app::{App};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn draw<B: Backend>(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(3),  // Main content
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_hosts_list::<B>(f, app, chunks[0]);
    draw_footer::<B>(f, app, chunks[1]);
}

fn draw_hosts_list<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    // TODO: Get version from Cargo.toml or version file
    let title = format!("SSHr v{}", "0.2.0");
    let block = Block::default()
        .borders(Borders::ALL)
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .title(title);

    let list_items: Vec<ListItem> = app
        .hosts
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
                format!("{} ({}@{}:{})", 
                    host.alias, 
                    host.user, 
                    host.host, 
                    host.port.unwrap_or(22)
                ),
                Style::default().fg(if is_selected { Color::Black } else { Color::White }),
            ));
            
            // Add group if exists
            if let Some(group) = &host.group {
                text.push(Span::raw(" "));
                text.push(Span::styled(
                    format!("[Group: {}]", group),
                    Style::default()
                        .fg(if is_selected { Color::Black } else { Color::Gray })
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
        .collect();

    let list = List::new(list_items)
        .block(block)
        .highlight_symbol(">")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

fn draw_footer<B: Backend>(f: &mut Frame, _app: &App, area: Rect) {
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Left side: Navigation help
    let nav_help = Paragraph::new("↑/k: Up  ↓/j: Down  [Enter] Connect")
        .style(Style::default().fg(Color::Gray));
    
    // Right side: Action help
    let action_help = Paragraph::new("[e] Edit [r] Reload [q] Quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(ratatui::layout::Alignment::Right);

    f.render_widget(nav_help, footer[0]);
    f.render_widget(action_help, footer[1]);
}