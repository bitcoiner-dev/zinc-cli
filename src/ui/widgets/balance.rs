use crate::ui::widgets::shared::format_sats;
use crate::ui::ZincTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::Span;
use tui_big_text::{BigText, PixelSize};

pub struct BalanceDisplay {
    pub hero_btc: String,
    pub _precise_btc: String,
    pub _sats: String,
}

impl BalanceDisplay {
    pub(crate) fn from_sats(confirmed: u64) -> Self {
        let btc = confirmed as f64 / 100_000_000.0;
        Self {
            hero_btc: format!("{btc:.8}"),
            _precise_btc: format!("{btc:.8}"),
            _sats: format!("{} sats", format_sats(confirmed)),
        }
    }
}

pub struct BalanceWidget<'a> {
    pub confirmed: u64,
    pub theme: &'a ZincTheme,
    pub ascii_mode: bool,
    pub is_hovered: bool,
    pub ordinals_address: Option<String>,
    pub payment_address: Option<String>,
}

impl<'a> Widget for BalanceWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let display = BalanceDisplay::from_sats(self.confirmed);

        let panel = crate::ui::widgets::GlassPanel::new(self.theme).title(Span::styled(
            " BALANCE ",
            Style::default()
                .fg(if self.is_hovered {
                    self.theme.accent
                } else {
                    self.theme.text_primary
                })
                .add_modifier(Modifier::BOLD),
        ));

        let inner = panel.render(area, buf);

        if inner.height < 2 || inner.width < 8 {
            return;
        }

        let balance_height = if self.ascii_mode { 4 } else { 5 };
        let address_height = if self.ordinals_address.is_some() || self.payment_address.is_some() {
            2
        } else {
            0
        };
        let total_content = balance_height + address_height;

        if inner.height < total_content {
            self.render_balance_only(area, buf);
            return;
        }

        let vertical_margin = inner.height.saturating_sub(total_content) / 2;
        let balance_area = Rect {
            x: inner.x,
            y: inner.y + vertical_margin,
            width: inner.width,
            height: balance_height,
        };
        let address_area = Rect {
            x: inner.x,
            y: balance_area.y + balance_area.height,
            width: inner.width,
            height: address_height,
        };

        self.render_balance(display, balance_area, buf);

        if address_height > 0 {
            self.render_addresses(address_area, buf);
        }
    }
}

impl<'a> BalanceWidget<'a> {
    fn render_balance(&self, display: BalanceDisplay, area: Rect, buf: &mut Buffer) {
        if self.ascii_mode {
            let amount_str = format!("{} BTC", display.hero_btc);

            let mut hero_builder = BigText::builder();
            hero_builder
                .pixel_size(PixelSize::Quadrant)
                .centered()
                .style(Style::default().fg(self.theme.cream))
                .lines(vec![amount_str.into()]);

            hero_builder.build().render(area, buf);
        } else {
            let pixel_size = if area.width >= 120 {
                PixelSize::Quadrant
            } else {
                PixelSize::Sextant
            };

            let content_height = if pixel_size == PixelSize::Quadrant {
                4
            } else {
                5
            };
            let mut render_area = area;
            render_area.height = content_height.min(area.height);

            let mut hero_builder = BigText::builder();
            hero_builder
                .pixel_size(pixel_size)
                .centered()
                .style(
                    Style::default()
                        .fg(self.theme.cream)
                        .add_modifier(Modifier::BOLD),
                )
                .lines(vec![format!("{} BTC", display.hero_btc).into()]);
            hero_builder.build().render(render_area, buf);
        }
    }

    fn render_balance_only(&self, area: Rect, buf: &mut Buffer) {
        let display = BalanceDisplay::from_sats(self.confirmed);
        if self.ascii_mode {
            let mut hero_builder = BigText::builder();
            hero_builder
                .pixel_size(PixelSize::Quadrant)
                .centered()
                .style(Style::default().fg(self.theme.cream))
                .lines(vec![format!("{} BTC", display.hero_btc).into()]);
            hero_builder.build().render(area, buf);
        } else {
            let mut hero_builder = BigText::builder();
            hero_builder
                .pixel_size(PixelSize::Sextant)
                .centered()
                .style(
                    Style::default()
                        .fg(self.theme.cream)
                        .add_modifier(Modifier::BOLD),
                )
                .lines(vec![format!("{} BTC", display.hero_btc).into()]);
            hero_builder.build().render(area, buf);
        }
    }

    fn render_addresses(&self, area: Rect, buf: &mut Buffer) {
        let taproot_addr = self.ordinals_address.as_ref().map(|a| shorten_address(a));
        let payment_addr = self.payment_address.as_ref().map(|a| shorten_address(a));

        let taproot_str = taproot_addr.map(|a| format!("Taproot: {}", a));
        let payment_str = payment_addr.map(|a| format!("Payment: {}", a));

        let (line1, line2) = match (taproot_str, payment_str) {
            (Some(t), Some(p)) => {
                if area.width < 80 {
                    (Some(t), Some(p))
                } else {
                    (Some(format!("{}   ·   {}", t, p)), None)
                }
            }
            (Some(t), None) => (Some(t), None),
            (None, Some(p)) => (Some(p), None),
            (None, None) => return,
        };

        let y_center = area.y + area.height / 2;

        if let Some(line) = line1 {
            let x = area.x + (area.width.saturating_sub(line.len() as u16)) / 2;
            buf.set_string(
                x,
                y_center.saturating_sub(if line2.is_some() { 0 } else { 1 }),
                &line,
                Style::default()
                    .fg(self.theme.text_muted)
                    .add_modifier(Modifier::DIM),
            );
        }

        if let Some(line) = line2 {
            let x = area.x + (area.width.saturating_sub(line.len() as u16)) / 2;
            buf.set_string(
                x,
                y_center + 1,
                &line,
                Style::default()
                    .fg(self.theme.text_muted)
                    .add_modifier(Modifier::DIM),
            );
        }
    }
}

fn shorten_address(addr: &str) -> String {
    if addr.len() <= 16 {
        addr.to_string()
    } else {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    }
}
