use ratatui::{
  backend::Backend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
  Frame,
};
use crate::sftp_logic::{AppSftpState, FileItem, PanelSide};

pub fn draw_sftp<B: Backend>(f: &mut Frame, sftp_state: &AppSftpState) {
  let main_chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([
          Constraint::Min(3),    // Main SFTP content
          Constraint::Length(3), // Footer with controls
      ])
      .split(f.size());

  // Split main area into two panels
  let panels = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
      .split(main_chunks[0]);

  // Draw local panel (left)
  draw_file_panel::<B>(
      f,
      panels[0],
      &sftp_state.local_files,
      sftp_state.local_selected,
      &format!("Local: {}", sftp_state.local_current_path.display()),
      sftp_state.active_panel == PanelSide::Local,
  );

  // Draw remote panel (right)
  draw_file_panel::<B>(
      f,
      panels[1],
      &sftp_state.remote_files,
      sftp_state.remote_selected,
      &format!("Remote: {}", sftp_state.remote_current_path),
      sftp_state.active_panel == PanelSide::Remote,
  );

  // Draw footer with controls
  draw_sftp_footer::<B>(f, main_chunks[1], sftp_state);

  // Draw status message if exists
  if let Some(ref message) = sftp_state.status_message {
      draw_status_overlay::<B>(f, message);
  }
}

fn draw_file_panel<B: Backend>(
  f: &mut Frame,
  area: Rect,
  files: &[FileItem],
  selected: usize,
  title: &str,
  is_active: bool,
) {
  let border_style = if is_active {
      Style::default().fg(Color::Green)
  } else {
      Style::default().fg(Color::Gray)
  };

  let title_style = if is_active {
      Style::default()
          .fg(Color::Green)
          .add_modifier(Modifier::BOLD)
  } else {
      Style::default()
          .fg(Color::Gray)
          .add_modifier(Modifier::BOLD)
  };

  let block = Block::default()
      .borders(Borders::ALL)
      .border_style(border_style)
      .title(title)
      .title_style(title_style);

  let list_items: Vec<ListItem> = files
      .iter()
      .enumerate()
      .map(|(i, file)| {
          let is_selected = i == selected && is_active;
          let mut spans = vec![];

          // Selection indicator
          spans.push(Span::styled(
              if is_selected { "> " } else { "  " },
              Style::default().fg(Color::Yellow),
          ));

          // File type icon and name
          let (icon, name_color) = match file {
              FileItem::Directory { name } => {
                  if name == ".." {
                      ("↰ ", Color::Cyan)
                  } else {
                      ("📁 ", Color::Blue)
                  }
              }
              FileItem::File { name, size: _ } => ("📄 ", Color::White),
          };

          spans.push(Span::styled(icon, Style::default().fg(Color::Yellow)));

          let name = match file {
              FileItem::Directory { name } => name,
              FileItem::File { name, size: _ } => name,
          };

          spans.push(Span::styled(
              name,
              Style::default().fg(if is_selected {
                  Color::Black
              } else {
                  name_color
              }),
          ));

          // File size for files
          if let FileItem::File { size, .. } = file {
              spans.push(Span::styled(
                  format!(" ({})", format_file_size(*size)),
                  Style::default().fg(if is_selected {
                      Color::Black
                  } else {
                      Color::Gray
                  }),
              ));
          }

          let style = if is_selected {
              Style::default()
                  .bg(Color::Green)
                  .add_modifier(Modifier::BOLD)
          } else {
              Style::default()
          };

          ListItem::new(Line::from(spans)).style(style)
      })
      .collect();

  let list = List::new(list_items).block(block);
  f.render_widget(list, area);
}

fn draw_sftp_footer<B: Backend>(f: &mut Frame, area: Rect, sftp_state: &AppSftpState) {
  let footer_chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
      .split(area);

  // Navigation help
  let nav_text = "↑/↓: Navigate  [Enter]: Open  [Backspace]: Back  [Tab]: Switch Panel";
  let nav_help = Paragraph::new(nav_text).style(Style::default().fg(Color::Gray));

  // Action help
  let action_text = "[u]: Upload  [d]: Download  [r]: Refresh  [q]: Quit SFTP";
  let action_help = Paragraph::new(action_text).style(Style::default().fg(Color::Yellow));

  // Status/Info
  let active_panel_text = format!(
      "Active: {} Panel",
      match sftp_state.active_panel {
          PanelSide::Local => "Local",
          PanelSide::Remote => "Remote",
      }
  );
  let status_help = Paragraph::new(active_panel_text)
      .style(Style::default().fg(Color::Cyan))
      .alignment(ratatui::layout::Alignment::Right);

  f.render_widget(nav_help, footer_chunks[0]);
  f.render_widget(action_help, footer_chunks[1]);
  f.render_widget(status_help, footer_chunks[2]);
}

fn draw_status_overlay<B: Backend>(f: &mut Frame, message: &str) {
  let area = centered_rect(60, 5, f.size());

  let block = Block::default()
      .borders(Borders::ALL)
      .title(" Status ")
      .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
      .border_style(Style::default().fg(Color::Yellow));

  let paragraph = Paragraph::new(message)
      .block(block)
      .style(Style::default().fg(Color::White))
      .alignment(ratatui::layout::Alignment::Center);

  f.render_widget(Clear, area);
  f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints([
          Constraint::Length((r.height.saturating_sub(height)) / 2),
          Constraint::Length(height),
          Constraint::Length((r.height.saturating_sub(height)) / 2),
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

fn format_file_size(size: u64) -> String {
  const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
  let mut size = size as f64;
  let mut unit_index = 0;

  while size >= 1024.0 && unit_index < UNITS.len() - 1 {
      size /= 1024.0;
      unit_index += 1;
  }

  if unit_index == 0 {
      format!("{} {}", size as u64, UNITS[unit_index])
  } else {
      format!("{:.1} {}", size, UNITS[unit_index])
  }
}