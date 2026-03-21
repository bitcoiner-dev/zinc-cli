use crate::ui::widgets::glass_panel::GlassPanel;
use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::*;
use ratatui_image::protocol::Protocol;
use zinc_core::ordinals::Inscription;

pub struct InscriptionWidget<'a> {
    pub inscription: &'a Inscription,
    pub image: Option<&'a Protocol>,
    pub ascii: Option<&'a Vec<Line<'static>>>,
    pub is_failed: bool,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub _tick: u64,
    pub theme: &'a ZincTheme,
    pub ascii_mode: bool,
}

impl<'a> Widget for InscriptionWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let panel = GlassPanel::new(self.theme)
            .selected(self.is_selected || self.is_hovered)
            .title(Span::styled(
                format!(" #{} ", self.inscription.number),
                Style::default()
                    .fg(if self.is_selected {
                        self.theme.cream
                    } else if self.is_hovered {
                        self.theme.accent
                    } else {
                        self.theme.text_muted
                    })
                    .add_modifier(Modifier::BOLD),
            ));

        let inner_area = panel.render(area, buf);

        if inner_area.height < 4 {
            return;
        }

        let img_area = Rect {
            x: inner_area.x.saturating_add(1),
            y: inner_area.y.saturating_add(1),
            width: inner_area.width.saturating_sub(2),
            height: inner_area.height.saturating_sub(2),
        };

        let content_type = self
            .inscription
            .content_type
            .as_deref()
            .unwrap_or("unknown");
        let is_image = content_type.starts_with("image/");
        let is_text = content_type.starts_with("text/");

        if let Some(protocol) = self.image {
            if self.ascii_mode {
                self.render_ascii_fallback(img_area, buf);
            } else {
                let protocol_area = protocol.area();
                let render_width = img_area.width.min(protocol_area.width);
                let render_height = img_area.height.min(protocol_area.height);
                let render_area = Rect {
                    x: img_area.x + (img_area.width.saturating_sub(render_width) / 2),
                    y: img_area.y + (img_area.height.saturating_sub(render_height) / 2),
                    width: render_width,
                    height: render_height,
                };
                let image_widget = ratatui_image::Image::new(protocol);
                Widget::render(image_widget, render_area, buf);
            }
        } else if is_image && self.ascii_mode && self.ascii.is_some() {
            self.render_ascii_fallback(img_area, buf);
        } else {
            let (preview_lines, color) = if self.is_failed {
                (
                    vec!["   [ MEDIA ERROR ]   ", "   failed to decode   "],
                    self.theme.danger,
                )
            } else if is_image {
                (
                    vec![
                        "░░░░░░░░░░░░░░░░░░░",
                        "    ZINC MEDIA     ",
                        "    loading...     ",
                    ],
                    self.theme.text_muted,
                )
            } else if is_text {
                (
                    vec!["      TEXT/HTML      ", "   render preview n/a"],
                    self.theme.text_muted,
                )
            } else {
                (
                    vec!["    NON-IMAGE NFT    ", "   metadata preview  "],
                    self.theme.text_muted,
                )
            };

            let start_y =
                img_area.y + (img_area.height.saturating_sub(preview_lines.len() as u16) / 2);
            for (i, line) in preview_lines.iter().enumerate() {
                if i < img_area.height as usize {
                    buf.set_string(
                        img_area.x + (img_area.width.saturating_sub(line.len() as u16) / 2),
                        start_y + i as u16,
                        line,
                        Style::default().fg(color).bg(self.theme.surface_elevated),
                    );
                }
            }
        }
    }
}

impl<'a> InscriptionWidget<'a> {
    fn render_ascii_fallback(&self, area: Rect, buf: &mut Buffer) {
        if let Some(lines) = self.ascii {
            let p = Paragraph::new((*lines).clone()).alignment(Alignment::Center);
            Widget::render(p, area, buf);
        } else {
            let center_y = area.y + area.height / 2;
            buf.set_string(
                area.x + (area.width.saturating_sub(5) / 2),
                center_y,
                "[IMG]",
                Style::default().fg(self.theme.text_muted),
            );
        }
    }
}
