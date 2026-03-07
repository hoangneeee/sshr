
use ratatui::{
    backend::Backend,
    layout::{Rect},
    style::{Color, Style},
    widgets::{Paragraph},
    Frame,
};
use crate::app::{App};

pub fn draw_status_bar<B: Backend>(f: &mut Frame, app: &mut App, area: Rect) {
  if let Some((message, timestamp)) = &app.status_message {
      // Clear messages older than 5 seconds (except when connecting)
      let should_show = if app.is_connecting {
          true // Always show status during connection
      } else {
          timestamp.elapsed().as_secs() < 5
      };

      if should_show {
          let style = if message.to_lowercase().contains("error")
              || message.to_lowercase().contains("failed")
          {
              Style::default().fg(app.theme.error)
          } else if message.to_lowercase().contains("success")
              || message.to_lowercase().contains("successful")
              || message.to_lowercase().contains("ended")
          {
              Style::default().fg(app.theme.success)
          } else if message.to_lowercase().contains("connecting")
              || message.to_lowercase().contains("testing")
          {
              Style::default().fg(app.theme.secondary)
          } else {
              Style::default().fg(app.theme.highlight)
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
