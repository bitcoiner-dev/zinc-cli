use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::*;
use zinc_core::Network;

pub struct BrandedHeader<'a> {
    pub title: &'a str,
    pub profile_name: &'a str,
    pub theme: &'a ZincTheme,
    pub network: Network,
    pub account_index: u32,
    pub is_loading: bool,
    pub tick: u64,
    pub _ascii_mode: bool,
}

impl<'a> Widget for BrandedHeader<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(self.theme.surface_glass))
            .border_style(
                Style::default()
                    .fg(self.theme.border)
                    .add_modifier(Modifier::DIM),
            );

        block.render(area, buf);

        let content_area = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: 1,
        };

        if content_area.width == 0 {
            return;
        }

        let logo = Span::styled(
            " ZINC ",
            Style::default()
                .fg(self.theme.charcoal)
                .bg(self.theme.accent)
                .add_modifier(Modifier::BOLD),
        );
        let title = Span::styled(
            self.title.to_uppercase(),
            Style::default()
                .fg(self.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );

        let network_str = match self.network {
            Network::Bitcoin => "MAINNET",
            Network::Testnet => "TESTNET",
            Network::Regtest => "REGTEST",
            Network::Signet => "SIGNET",
            _ => "UNKNOWN",
        };

        let profile_label = if self.profile_name == "default" {
            String::new()
        } else {
            format!("{}  ·  ", self.profile_name.to_uppercase())
        };

        let dot_color = if self.tick % 2 == 0 {
            self.theme.selection
        } else {
            self.theme.accent
        };

        let status_spans: Vec<Span<'static>> = if self.is_loading {
            let track_width: usize = 10;
            let head_pos = (self.tick % 40) as usize;

            let head = if head_pos < track_width {
                head_pos
            } else if head_pos < track_width + 4 {
                track_width - 1
            } else {
                39 - head_pos
            };

            let trail_len: usize = 4;

            let mut spans = vec![Span::raw(" ")];

            for i in 0..track_width {
                let dist = if i <= head { head - i } else { 0 };

                let (ch, color) = if dist == 0 {
                    ("█", self.theme.accent)
                } else if dist <= trail_len {
                    ("█", Color::Rgb(245, 158 - (dist * 30) as u8, 11))
                } else {
                    ("░", self.theme.border)
                };

                spans.push(Span::styled(ch, Style::default().fg(color)));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "syncing",
                Style::default()
                    .fg(self.theme.text_muted)
                    .add_modifier(Modifier::ITALIC),
            ));

            spans
        } else {
            vec![
                Span::styled("●", Style::default().fg(dot_color)),
                Span::styled(
                    "  LIVE",
                    Style::default()
                        .fg(self.theme.text_muted)
                        .add_modifier(Modifier::BOLD),
                ),
            ]
        };

        let context_info = vec![
            Span::styled(
                &profile_label[..],
                Style::default().fg(self.theme.text_muted),
            ),
            Span::raw("    "),
            Span::styled(
                "ACCOUNT",
                Style::default()
                    .fg(self.theme.text_muted)
                    .add_modifier(Modifier::DIM),
            ),
            Span::raw(" "),
            Span::styled(
                format!("#{}", self.account_index + 1),
                Style::default()
                    .fg(self.theme.cream)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("      "),
            Span::styled(
                network_str,
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        let left_content = Line::from(vec![logo, Span::raw("  "), title]);
        let right_content = Line::from([status_spans, context_info].concat());

        Paragraph::new(left_content).render(content_area, buf);

        let right_width = right_content.width() as u16;
        if content_area.width > right_width {
            let right_area = Rect {
                x: content_area.right().saturating_sub(right_width),
                y: content_area.y,
                width: right_width,
                height: 1,
            };
            Paragraph::new(right_content).render(right_area, buf);
        }
    }
}
