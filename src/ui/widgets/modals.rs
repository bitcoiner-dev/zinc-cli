use crate::ui::widgets::shared::centered_rect;
use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::Span;
use ratatui::widgets::*;

pub struct PasswordModal<'a> {
    pub input: &'a str,
    pub theme: &'a ZincTheme,
    pub error: Option<&'a str>,
}

impl<'a> Widget for PasswordModal<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(self.theme.surface_base))
            .render(area, buf);

        let modal_area = centered_rect(44, 14, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(self.theme.surface_glass))
            .border_style(Style::default().fg(self.theme.border));

        let inner_area = block.inner(modal_area);
        block.render(modal_area, buf);

        let title = Line::from(vec![
            Span::styled(
                " ZINC ",
                Style::default()
                    .fg(self.theme.charcoal)
                    .bg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  ·  "),
            Span::styled(
                "UNLOCK WALLET",
                Style::default()
                    .fg(self.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        Paragraph::new(title).alignment(Alignment::Center).render(
            Rect::new(modal_area.x + 1, modal_area.y + 1, modal_area.width - 2, 1),
            buf,
        );

        let hint = "Enter your wallet password";
        Paragraph::new(Span::styled(
            hint,
            Style::default()
                .fg(self.theme.text_muted)
                .add_modifier(Modifier::ITALIC),
        ))
        .alignment(Alignment::Center)
        .render(
            Rect::new(modal_area.x + 1, modal_area.y + 4, modal_area.width - 2, 1),
            buf,
        );

        let input_width = 24.min(inner_area.width.saturating_sub(4));
        let input_area = Rect::new(
            inner_area.x + (inner_area.width - input_width) / 2,
            inner_area.y + 6,
            input_width,
            3,
        );

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .style(Style::default().bg(self.theme.charcoal))
            .border_style(Style::default().fg(self.theme.border));

        let input_inner = input_block.inner(input_area);
        input_block.render(input_area, buf);

        if !self.input.is_empty() {
            let masked = "*".repeat(self.input.len());
            let display_text = format!("{:^width$}", masked, width = input_inner.width as usize);
            Paragraph::new(Span::styled(
                display_text,
                Style::default()
                    .fg(self.theme.cream)
                    .bg(self.theme.charcoal)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center)
            .render(input_inner, buf);
        } else {
            let cursor_hint = "·";
            Paragraph::new(Span::styled(
                cursor_hint,
                Style::default()
                    .fg(self.theme.text_muted)
                    .add_modifier(Modifier::DIM),
            ))
            .alignment(Alignment::Center)
            .render(input_inner, buf);
        }

        let key_hint = Line::from(vec![
            Span::styled("[", Style::default().fg(self.theme.border)),
            Span::styled(
                "ENTER",
                Style::default()
                    .fg(self.theme.cream)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("]", Style::default().fg(self.theme.border)),
            Span::styled(" to unlock   ", Style::default().fg(self.theme.text_muted)),
            Span::styled("[", Style::default().fg(self.theme.border)),
            Span::styled(
                "ESC",
                Style::default()
                    .fg(self.theme.cream)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("]", Style::default().fg(self.theme.border)),
            Span::styled(" to quit", Style::default().fg(self.theme.text_muted)),
        ]);
        Paragraph::new(key_hint)
            .alignment(Alignment::Center)
            .render(
                Rect::new(modal_area.x + 1, modal_area.y + 11, modal_area.width - 2, 1),
                buf,
            );

        if let Some(err) = self.error {
            let err_style = Style::default()
                .fg(self.theme.danger)
                .add_modifier(Modifier::BOLD);
            Paragraph::new(Line::from(vec![
                Span::styled("✗ ", err_style),
                Span::styled(err, err_style),
            ]))
            .alignment(Alignment::Center)
            .render(
                Rect::new(modal_area.x + 1, modal_area.y + 13, modal_area.width - 2, 1),
                buf,
            );
        }
    }
}

pub struct ExitOverlay<'a> {
    pub theme: &'a ZincTheme,
    pub tick: u64,
}

impl<'a> Widget for ExitOverlay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(self.theme.charcoal))
            .render(area, buf);

        let area = centered_rect(42, 22, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(self.theme.border))
            .style(Style::default().bg(self.theme.surface_elevated));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let spinner = create_loading_spinner(self.tick, self.theme);
        Paragraph::new(spinner).alignment(Alignment::Center).render(
            Rect::new(
                inner_area.x,
                inner_area.y + (inner_area.height / 2),
                inner_area.width,
                1,
            ),
            buf,
        );

        let subtext = "Cleaning up sessions and tasks";
        Paragraph::new(subtext)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(self.theme.text_muted)
                    .bg(self.theme.surface_elevated)
                    .add_modifier(Modifier::DIM),
            )
            .render(
                Rect::new(
                    inner_area.x,
                    inner_area.y + (inner_area.height / 2) + 1,
                    inner_area.width,
                    1,
                ),
                buf,
            );
    }
}

fn create_loading_spinner(tick: u64, theme: &ZincTheme) -> Line<'_> {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame = frames[(tick as usize) % frames.len()];

    let track_width: usize = 12;
    let head_pos = (tick % (track_width as u64 * 2)) as usize;
    let pos = if head_pos < track_width {
        head_pos
    } else {
        track_width * 2 - head_pos - 1
    };

    let mut spans = vec![Span::styled(
        frame,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )];

    spans.push(Span::raw("  "));

    for i in 0..track_width {
        let ch = if i == pos { "█" } else { "░" };
        let color = if i == pos {
            theme.selection
        } else if i > pos && i <= pos + 3 {
            Color::Rgb(245, 158 - ((i - pos) as u8 * 30), 11)
        } else {
            theme.border
        };
        spans.push(Span::styled(ch, Style::default().fg(color)));
    }

    spans.push(Span::styled(
        "  exiting",
        Style::default()
            .fg(theme.text_muted)
            .add_modifier(Modifier::ITALIC),
    ));

    Line::from(spans)
}
