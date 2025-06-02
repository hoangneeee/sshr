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
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());


    draw_header::<B>(f, app, chunks[0]);
    draw_hosts_list::<B>(f, app, chunks[1]);
    draw_footer::<B>(f, app, chunks[2]);
}

fn draw_header<B: Backend>(f: &mut Frame, _app: &App, area: Rect) {
    let title = "SSH Host Manager";
    let title = Paragraph::new(Line::from(vec![Span::styled(
        title,
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )]));

    f.render_widget(title, area);
}

fn draw_hosts_list<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = app
        .hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
            } else {
                Style::default()
            };

            let display_text = format!(
                "{}@{}:{} {}",
                host.user,
                host.host,
                host.port.unwrap_or(22),
                host.description.as_deref().unwrap_or("")
            );

            ListItem::new(Line::from(Span::styled(display_text, style)))
        })
        .collect();

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Hosts"));

    f.render_widget(list, area);
}

fn draw_footer<B: Backend>(f: &mut Frame, _app: &App, area: Rect) {
    let help = "↑/k: Up  ↓/j: Down  Enter: Connect  a: Add  e: Edit  d: Delete  q: Quit";
    let help = Paragraph::new(Line::from(Span::styled(
        help,
        Style::default().add_modifier(Modifier::DIM),
    )));

    f.render_widget(help, area);
}