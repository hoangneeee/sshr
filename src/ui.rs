use crate::app::App;
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

    // Draw enhanced loading overlay if connecting
    if app.is_connecting {
        draw_loading_overlay::<B>(f, app);
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
    let title = format!("SSHr - SSH Manager: Easy control your SSH hosts");
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
        .collect();

    let list = List::new(list_items)
        .block(block)
        .highlight_symbol(">")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

fn draw_status_bar<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some((message, timestamp)) = &app.status_message {
        // Clear messages older than 5 seconds
        if timestamp.elapsed().as_secs() < 5 {
            let style = if message.to_lowercase().contains("error") {
                Style::default().fg(Color::Red)
            } else if message.to_lowercase().contains("success") {
                Style::default().fg(Color::Green)
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

fn draw_footer<B: Backend>(f: &mut Frame, _app: &App, area: Rect) {
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
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

fn draw_loading_overlay<B: Backend>(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 8, f.size());

    // Create loading content with animation
    let loading_text = if let Some(host) = app.get_selected_host() {
        format!("🔗 Connecting to {}...\n\n⏳ Please wait...", host.alias)
    } else {
        "🔗 Connecting...\n\n⏳ Please wait...".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("SSH Connection")
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(loading_text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::White));

    // Clear the area và render loading overlay
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}
