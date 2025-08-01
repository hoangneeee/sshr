use ratatui::{
    backend::Backend,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::app::App;

pub fn render_help_popup<B: Backend>(f: &mut Frame, app: &mut App) {
    let block = Block::default()
        .title("Keyboard Shortcuts")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White));

    let area = centered_rect(80, 80, f.size());
    f.render_widget(Clear, area); // this clears the background
    f.render_widget(block, area);

    let text = get_help_text();
    let line_count = text.lines.len();

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .scroll((app.help_scroll_position, 0));

    let inner_area = area.inner(&Margin {
        vertical: 1,
        horizontal: 1,
    }); // Get area inside the block borders

    f.render_widget(paragraph, inner_area);

    // Make scrollbar only appear if there is overflow
    if line_count > inner_area.height as usize {
        let mut scrollbar_state =
            ratatui::widgets::ScrollbarState::new(line_count).position(app.help_scroll_position as usize);

        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            inner_area,
            &mut scrollbar_state,
        );
    }
}

fn get_help_text<'a>() -> Text<'a> {
    Text::from(vec![
        Line::from(Span::styled("Normal Mode", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(vec![Span::styled("  q", Style::default().fg(Color::Green)), Span::raw("         - Quit")]),
        Line::from(vec![Span::styled("  Enter", Style::default().fg(Color::Green)), Span::raw("     - Connect to selected host")]),
        Line::from(vec![Span::styled("  s", Style::default().fg(Color::Green)), Span::raw("         - Switch to SEARCH mode")]),
        Line::from(vec![Span::styled("  f", Style::default().fg(Color::Green)), Span::raw("         - Switch to SFTP mode")]),
        Line::from(vec![Span::styled("  e", Style::default().fg(Color::Green)), Span::raw("         - Edit file config custom hosts")]),
        Line::from(vec![Span::styled("  r", Style::default().fg(Color::Green)), Span::raw("         - Reload")]),
        Line::from(vec![Span::styled("  j, ↓", Style::default().fg(Color::Green)), Span::raw("    - Move down")]),
        Line::from(vec![Span::styled("  k, ↑", Style::default().fg(Color::Green)), Span::raw("    - Move up")]),
        Line::from(""),
        Line::from(Span::styled("Search Mode", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(vec![Span::styled("  Esc", Style::default().fg(Color::Green)), Span::raw("       - Switch to Normal mode")]),
        Line::from(vec![Span::styled("  Enter", Style::default().fg(Color::Green)), Span::raw("     - Connect to selected host")]),
        Line::from(vec![Span::styled("  ↓", Style::default().fg(Color::Green)), Span::raw("         - Move down")]),
        Line::from(vec![Span::styled("  ↑", Style::default().fg(Color::Green)), Span::raw("         - Move up")]),
        Line::from(vec![Span::styled("  Backspace", Style::default().fg(Color::Green)), Span::raw(" - Clear search input")]),
        Line::from(""),
        Line::from(Span::styled("SFTP Mode", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(vec![Span::styled("  q", Style::default().fg(Color::Green)), Span::raw("         - Switch to Normal mode")]),
        Line::from(vec![Span::styled("  Enter", Style::default().fg(Color::Green)), Span::raw("     - Open directory")]),
        Line::from(vec![Span::styled("  ↓", Style::default().fg(Color::Green)), Span::raw("         - Move down")]),
        Line::from(vec![Span::styled("  ↑", Style::default().fg(Color::Green)), Span::raw("         - Move up")]),
        Line::from(vec![Span::styled("  Backspace", Style::default().fg(Color::Green)), Span::raw(" - Go back to parent directory")]),
        Line::from(vec![Span::styled("  Tab", Style::default().fg(Color::Green)), Span::raw("       - Switch between local and remote directory")]),
        Line::from(vec![Span::styled("  u", Style::default().fg(Color::Green)), Span::raw("         - Upload file")]),
        Line::from(vec![Span::styled("  d", Style::default().fg(Color::Green)), Span::raw("         - Download file")]),
        Line::from(vec![Span::styled("  r", Style::default().fg(Color::Green)), Span::raw("         - Reload")]),
        Line::from(""),
        Line::from(Span::styled("Help Popup", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
        Line::from(vec![Span::styled("  ?, Esc", Style::default().fg(Color::Green)), Span::raw("    - Toggle/Close help")]),
        Line::from(vec![Span::styled("  ↑, k", Style::default().fg(Color::Green)), Span::raw("      - Scroll up")]),
        Line::from(vec![Span::styled("  ↓, j", Style::default().fg(Color::Green)), Span::raw("      - Scroll down")]),
    ])
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
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