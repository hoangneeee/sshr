use crate::sftp_logic::types::{
    AppSftpState, DownloadProgress, FileItem, PanelSide, UploadProgress,
};
use crate::theme::ResolvedTheme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn draw_sftp(f: &mut Frame, sftp_state: &mut AppSftpState, theme: &ResolvedTheme) {
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
    draw_file_panel(
        f,
        panels[0],
        &mut sftp_state.local_list_state,
        &sftp_state.local_files,
        sftp_state.local_selected,
        &format!("Local: {}", sftp_state.local_current_path.display()),
        sftp_state.active_panel == PanelSide::Local,
        theme,
    );

    // Draw remote panel (right)
    draw_file_panel(
        f,
        panels[1],
        &mut sftp_state.remote_list_state,
        &sftp_state.remote_files,
        sftp_state.remote_selected,
        &format!("Remote: {}", sftp_state.remote_current_path),
        sftp_state.active_panel == PanelSide::Remote,
        theme,
    );

    // Draw footer with controls
    draw_sftp_footer(f, main_chunks[1], sftp_state, theme);

    // Draw status message if exists
    if let Some(ref message) = sftp_state.status_message {
        draw_status_overlay(f, message, theme);
    }

    // Draw upload progress if active
    if let Some(ref progress) = sftp_state.upload_progress {
        draw_upload_progress(f, progress, theme);
    }

    // Draw download progress if active
    if let Some(ref progress) = sftp_state.download_progress {
        draw_download_progress(f, progress, theme);
    }
}

fn draw_file_panel(
    f: &mut Frame,
    area: Rect,
    list_state: &mut ListState,
    files: &[FileItem],
    selected: usize,
    title: &str,
    is_active: bool,
    theme: &ResolvedTheme,
) {
    let border_style = if is_active {
        Style::default().fg(theme.primary)
    } else {
        Style::default().fg(Color::Gray)
    };

    let title_style = if is_active {
        Style::default()
            .fg(theme.primary)
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
                Style::default().fg(theme.highlight),
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
                FileItem::File {
                    name: _name,
                    size: _,
                } => ("📄 ", Color::White),
            };

            spans.push(Span::styled(icon, Style::default().fg(theme.highlight)));

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
                    .bg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    let list = List::new(list_items).block(block);
    f.render_stateful_widget(list, area, list_state);
}

fn draw_sftp_footer(f: &mut Frame, area: Rect, sftp_state: &AppSftpState, theme: &ResolvedTheme) {
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Navigation help
    let nav_text = "↑/↓: Navigate  [Enter]: Open  [Backspace]: Back  [Tab]: Switch Panel";
    let nav_help = Paragraph::new(nav_text).style(Style::default().fg(Color::Gray));

    // Action help
    let action_text = "[u]: Upload  [d]: Download  [r]: Refresh  [q]: Quit SFTP";
    let action_help = Paragraph::new(action_text).style(Style::default().fg(theme.highlight));

    // Status/Info
    let active_panel_text = format!(
        "Active: {} Panel",
        match sftp_state.active_panel {
            PanelSide::Local => "Local",
            PanelSide::Remote => "Remote",
        }
    );
    let status_help = Paragraph::new(active_panel_text)
        .style(Style::default().fg(theme.secondary))
        .alignment(ratatui::layout::Alignment::Right);

    f.render_widget(nav_help, footer_chunks[0]);
    f.render_widget(action_help, footer_chunks[1]);
    f.render_widget(status_help, footer_chunks[2]);
}

fn draw_status_overlay(f: &mut Frame, message: &str, theme: &ResolvedTheme) {
    let area = centered_rect(60, 5, f.size());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Status ")
        .title_style(
            Style::default()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.highlight));

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(theme.text))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_upload_progress(f: &mut Frame, progress: &UploadProgress, theme: &ResolvedTheme) {
    // Use a wider area to accommodate the file name
    let area = bottom_right_rect(40, 6, f.size());

    // Truncate the file name if it's too long
    let max_name_width = 30;
    let truncated_name = if progress.file_name.len() > max_name_width {
        format!(
            "..{}",
            &progress.file_name[progress.file_name.len() - (max_name_width - 2)..]
        )
    } else {
        progress.file_name.clone()
    };

    // Calculate percentage
    let percent = if progress.total_size > 0 {
        ((progress.uploaded_size as f64 / progress.total_size as f64) * 100.0) as u16
    } else {
        0
    };

    // Create a block with the file name as title
    let block = Block::default()
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.highlight))
        .title(format!(" {} ", truncated_name));

    // Create the gauge with file sizes as label
    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::default().fg(theme.primary).bg(Color::Black))
        .label(format!(
            "{} / {}",
            format_file_size(progress.uploaded_size),
            format_file_size(progress.total_size)
        ))
        .percent(percent);

    f.render_widget(Clear, area);
    f.render_widget(gauge, area);
}

fn draw_download_progress(f: &mut Frame, progress: &DownloadProgress, theme: &ResolvedTheme) {
    // Use a wider area to accommodate the file name
    let area = bottom_right_rect(40, 6, f.size());

    // Truncate the file name if it's too long
    let max_name_width = 30;
    let truncated_name = if progress.file_name.len() > max_name_width {
        format!(
            "..{}",
            &progress.file_name[progress.file_name.len() - (max_name_width - 2)..]
        )
    } else {
        progress.file_name.clone()
    };

    // Calculate percentage
    let percent = if progress.total_size > 0 {
        ((progress.downloaded_size as f64 / progress.total_size as f64) * 100.0) as u16
    } else {
        0
    };

    // Create a block with the file name as title
    let block = Block::default()
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.highlight))
        .title(format!(" Download Progress {} ", truncated_name));

    // Create the gauge with file sizes as label
    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::default().fg(theme.primary).bg(Color::Black))
        .label(format!(
            "{} / {}",
            format_file_size(progress.downloaded_size),
            format_file_size(progress.total_size)
        ))
        .percent(percent);

    f.render_widget(Clear, area);
    f.render_widget(gauge, area);
}

fn bottom_right_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(height)])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - percent_x),
            Constraint::Percentage(percent_x),
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
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
