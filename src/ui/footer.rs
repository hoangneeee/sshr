use crate::app::{App, InputMode};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph},
    Frame,
};

pub fn draw_footer<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (nav_text, action_text) = match app.input_mode {
        InputMode::Normal if app.is_connecting => ("Connecting to SSH host...", "[Ctrl+C] Cancel"),
        InputMode::Normal => (
            "↑/k: Up  ↓/j: Down  [Enter] Connect  [s] Search [f] SFTP",
            "[e] Edit [r] Reload [q] Quit",
        ),
        InputMode::Search => (
            "↑: Up  ↓: Down  [Enter] Connect",
            "[Esc] Exit Search  Type to filter",
        ),
        InputMode::Sftp => (
            "↑: Up  ↓: Down  [Enter] Connect",
            "[Esc] Exit Search  Type to filter",
        ),
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
