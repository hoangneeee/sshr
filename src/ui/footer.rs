use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Which footer hint set to show. SFTP has its own footer rendered inline by
/// [`crate::ui::sftp`], so it has no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterKind {
    Normal,
    Search,
    Connecting,
}

pub fn draw_footer(f: &mut Frame, app: &App, area: Rect, kind: FooterKind) {
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let key_style = Style::default()
        .fg(app.ctx.theme.secondary)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    let (nav_spans, action_spans) = match kind {
        FooterKind::Connecting => (
            Line::from(Span::styled(
                "Connecting to SSH host...",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                "[Ctrl+C] Cancel",
                Style::default().fg(Color::Red),
            )),
        ),
        FooterKind::Normal => (
            Line::from(vec![
                Span::styled("↑/k:", key_style),
                Span::styled(" Up  ", desc_style),
                Span::styled("↓/j:", key_style),
                Span::styled(" Down  ", desc_style),
                Span::styled("[Enter]", key_style),
                Span::styled(" Connect  ", desc_style),
                Span::styled("[s]", key_style),
                Span::styled(" Search  ", desc_style),
                Span::styled("[f]", key_style),
                Span::styled(" SFTP", desc_style),
            ]),
            Line::from(vec![
                Span::styled("[e]", key_style),
                Span::styled(" Edit  ", desc_style),
                Span::styled("[r]", key_style),
                Span::styled(" Reload  ", desc_style),
                Span::styled("[q]", key_style),
                Span::styled(" Quit", desc_style),
            ]),
        ),
        FooterKind::Search => (
            Line::from(vec![
                Span::styled("↑:", key_style),
                Span::styled(" Up  ", desc_style),
                Span::styled("↓:", key_style),
                Span::styled(" Down  ", desc_style),
                Span::styled("[Enter]", key_style),
                Span::styled(" Connect", desc_style),
            ]),
            Line::from(vec![
                Span::styled("[Esc]", key_style),
                Span::styled(" Exit Search  ", desc_style),
                Span::styled("Type to filter", desc_style),
            ]),
        ),
    };

    let nav_help = Paragraph::new(nav_spans);

    let action_help = Paragraph::new(action_spans)
        .alignment(ratatui::layout::Alignment::Right);

    f.render_widget(nav_help, footer[0]);
    f.render_widget(action_help, footer[1]);
}
