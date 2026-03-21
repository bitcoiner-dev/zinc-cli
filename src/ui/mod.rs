#[allow(dead_code)]
pub mod ascii_art;
pub mod widgets;

pub const INSCRIPTION_TILE_WIDTH: u16 = 26;
pub const INSCRIPTION_TILE_HEIGHT: u16 = 13;
#[allow(dead_code)]
pub const INSCRIPTION_IMAGE_WIDTH: u16 = 22;
#[allow(dead_code)]
pub const INSCRIPTION_IMAGE_HEIGHT: u16 = 9;

use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZincTheme {
    pub primary: Color,
    pub cream: Color,
    pub charcoal: Color,
    pub mud: Color,
    pub surface_base: Color,
    pub surface_elevated: Color,
    pub surface_glass: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub border: Color,
    pub glass_border_light: Color,
    pub glass_border_dark: Color,
    pub selection: Color,
    pub danger: Color,
}

impl ZincTheme {
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(245, 158, 11),
            cream: Color::Rgb(226, 232, 240),
            charcoal: Color::Rgb(11, 14, 20),
            mud: Color::Rgb(21, 24, 30),
            surface_base: Color::Rgb(11, 14, 20),
            surface_elevated: Color::Rgb(26, 30, 38),
            surface_glass: Color::Rgb(38, 44, 56),
            text_primary: Color::Rgb(226, 232, 240),
            text_muted: Color::Rgb(170, 160, 145),
            accent: Color::Rgb(245, 158, 11),
            border: Color::Rgb(70, 80, 95),
            glass_border_light: Color::Rgb(80, 100, 120),
            glass_border_dark: Color::Rgb(5, 5, 10),
            selection: Color::Rgb(245, 177, 74),
            danger: Color::Rgb(231, 91, 99),
        }
    }
}
