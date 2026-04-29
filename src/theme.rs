use crate::config::ThemeColors;
use ratatui::style::Color;

/// Resolved theme colors ready for use with ratatui
#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    /// Active/selected items (was Color::Green)
    pub primary: Color,
    /// Search/accent highlights (was Color::Yellow)
    pub highlight: Color,
    /// Info and secondary text (was Color::Cyan/LightCyan)
    pub secondary: Color,
    /// Normal text (was Color::White)
    pub text: Color,
    /// Errors and match highlights (was Color::Red)
    pub error: Color,
    /// Warnings (was Color::Yellow). Reserved for future use; surfaced from config.
    #[allow(dead_code)]
    pub warning: Color,
    /// Success messages (was Color::Green)
    pub success: Color,
}

impl ResolvedTheme {
    pub fn from_config(colors: &ThemeColors) -> Self {
        Self {
            primary: parse_hex(&colors.primary).unwrap_or(Color::Green),
            highlight: parse_hex(&colors.highlight).unwrap_or(Color::Yellow),
            secondary: parse_hex(&colors.secondary).unwrap_or(Color::Cyan),
            text: parse_hex(&colors.text).unwrap_or(Color::White),
            error: parse_hex(&colors.error).unwrap_or(Color::Red),
            warning: parse_hex(&colors.warning).unwrap_or(Color::Yellow),
            success: parse_hex(&colors.success).unwrap_or(Color::Green),
        }
    }
}

impl Default for ResolvedTheme {
    fn default() -> Self {
        Self {
            primary: Color::Green,
            highlight: Color::Yellow,
            secondary: Color::Cyan,
            text: Color::White,
            error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
        }
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
