use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    pub fn bg() -> Color {
        Color::Rgb(12, 16, 26)
    }

    pub fn surface() -> Color {
        Color::Rgb(24, 31, 45)
    }

    pub fn text() -> Color {
        Color::Rgb(227, 235, 248)
    }

    pub fn muted() -> Color {
        Color::Rgb(139, 151, 173)
    }

    pub fn accent() -> Color {
        Color::Rgb(54, 164, 255)
    }

    pub fn success() -> Color {
        Color::Rgb(81, 214, 154)
    }

    pub fn warning() -> Color {
        Color::Rgb(247, 191, 84)
    }

    pub fn error() -> Color {
        Color::Rgb(241, 104, 122)
    }

    pub fn base() -> Style {
        Style::default().fg(Self::text()).bg(Self::bg())
    }

    pub fn panel() -> Style {
        Style::default().fg(Self::text()).bg(Self::surface())
    }

    pub fn muted_text() -> Style {
        Style::default().fg(Self::muted())
    }

    pub fn accent_text() -> Style {
        Style::default().fg(Self::accent()).add_modifier(Modifier::BOLD)
    }

    pub fn selected_row() -> Style {
        Style::default().bg(Color::Rgb(35, 56, 80)).fg(Self::text())
    }
}
