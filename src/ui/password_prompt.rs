use crate::models::SshHost;
use crate::theme::ResolvedTheme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Centered modal for password entry.
pub fn draw(
    f: &mut Frame,
    host: &SshHost,
    input: &str,
    retry: bool,
    theme: &ResolvedTheme,
) {
    let area = centered_rect(60, 9, f.size());

    let title = format!(" SFTP password — {} ", host.alias);
    let masked: String = "•".repeat(input.chars().count());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Host: ", Style::default().fg(theme.secondary)),
            Span::styled(
                format!("{}@{}:{}", host.user, host.host, host.port.unwrap_or(22)),
                Style::default().fg(theme.highlight),
            ),
        ]),
        Line::from(""),
    ];
    if retry {
        lines.push(Line::from(Span::styled(
            "Previous password was rejected — try again or press Esc to cancel.",
            Style::default().fg(theme.error),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(vec![
        Span::styled("Password: ", Style::default().fg(theme.text)),
        Span::styled(
            masked,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(theme.highlight)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Enter] submit   [Esc] cancel",
        Style::default()
            .fg(theme.secondary)
            .add_modifier(Modifier::DIM),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.highlight))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        );
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);

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
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
