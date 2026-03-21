use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(Debug, Clone)]
pub struct GlassPanel<'a> {
    pub theme: &'a ZincTheme,
    pub title: Option<Span<'a>>,
    pub is_selected: bool,
}

impl<'a> GlassPanel<'a> {
    pub fn new(theme: &'a ZincTheme) -> Self {
        Self {
            theme,
            title: None,
            is_selected: false,
        }
    }

    pub fn title(mut self, title: Span<'a>) -> Self {
        self.title = Some(title);
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) -> Rect {
        let border_color = if self.is_selected {
            self.theme.selection
        } else {
            self.theme.border
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(self.theme.surface_glass))
            .border_style(Style::default().fg(border_color));

        if let Some(t) = self.title {
            block = block.title(t);
        }

        let inner = block.inner(area);
        block.render(area, buf);

        inner
    }
}
