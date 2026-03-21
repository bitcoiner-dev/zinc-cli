use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub struct InfoCard<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub theme: &'a ZincTheme,
}

impl<'a> Widget for InfoCard<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!(" {} ", self.title),
                Style::default()
                    .fg(self.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(self.theme.border))
            .style(Style::default().bg(self.theme.surface_base));

        Paragraph::new(self.content)
            .style(
                Style::default()
                    .fg(self.theme.text_muted)
                    .bg(self.theme.surface_base),
            )
            .block(block)
            .render(area, buf);
    }
}

pub struct ControlsBar<'a> {
    pub theme: &'a ZincTheme,
    pub is_syncing: bool,
}

impl<'a> Widget for ControlsBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(self.theme.surface_base))
            .render(area, buf);

        let controls = vec![
            ("Q", "quit"),
            ("S", "sync"),
            ("Tab/ShiftTab", "account"),
            ("←→↑↓", "navigate"),
        ];

        let start_y = area.y + (area.height / 2);

        let sep = "  ";
        let total_width: usize = controls
            .iter()
            .map(|(k, a)| format!("[{}] {}", k, a).len())
            .sum::<usize>()
            + sep.len() * (controls.len() - 1);
        let start_x = area.x + (area.width.saturating_sub(total_width as u16)) / 2;

        let mut x = start_x;
        for (i, (key, action)) in controls.iter().enumerate() {
            if i > 0 {
                buf.set_string(x, start_y, sep, Style::default().fg(self.theme.border));
                x += sep.len() as u16;
            }

            let key_str = format!("[{}]", key);
            buf.set_string(
                x,
                start_y,
                &key_str,
                Style::default()
                    .fg(self.theme.cream)
                    .bg(self.theme.surface_elevated)
                    .add_modifier(Modifier::BOLD),
            );
            x += key_str.len() as u16;

            let action_str = format!(" {}", action);
            buf.set_string(
                x,
                start_y,
                &action_str,
                Style::default().fg(self.theme.text_muted),
            );
            x += action_str.len() as u16;
        }

        if self.is_syncing {
            let syncing_text = "syncing...";
            let syncing_x = area.right().saturating_sub(syncing_text.len() as u16 + 2);
            buf.set_string(
                syncing_x,
                start_y,
                syncing_text,
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::ITALIC),
            );
        }
    }
}
