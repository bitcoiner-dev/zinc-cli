use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::ui::widgets::{
    BalanceWidget, BrandedHeader, ControlsBar, ExitOverlay, InscriptionWidget, PasswordModal,
};
use crate::ui::{ZincTheme, INSCRIPTION_TILE_HEIGHT, INSCRIPTION_TILE_WIDTH};

use super::state::{DashboardLayout, DashboardState};

pub fn render_locked<'a>(theme: &'a ZincTheme, state: &'a DashboardState) -> impl Widget + 'a {
    PasswordModal {
        input: &state.password_input,
        theme,
        error: state.auth_error.as_deref(),
    }
}

pub fn render_dashboard(
    f: &mut ratatui::Frame,
    area: Rect,
    state: &mut DashboardState,
    theme: &ZincTheme,
) {
    let layout = DashboardLayout::new(area);

    Block::default()
        .style(Style::default().bg(theme.surface_base))
        .render(area, f.buffer_mut());

    if let (Some(profile_name), Some(network)) = (&state.profile_name, state.network) {
        BrandedHeader {
            title: "DASHBOARD",
            profile_name,
            theme,
            network,
            account_index: state.account_index,
            is_loading: state.is_syncing_balance || state.is_syncing_inscriptions,
            tick: state.tick_count,
            _ascii_mode: state.ascii_mode,
        }
        .render(layout.header, f.buffer_mut());
    }

    BalanceWidget {
        confirmed: state.confirmed_balance,
        theme,
        ascii_mode: state.ascii_mode,
        is_hovered: state.hover_balance,
        ordinals_address: state.ordinals_address.clone(),
        payment_address: state.payment_address.clone(),
    }
    .render(layout.hero, f.buffer_mut());

    let inner_area = layout.main;
    let max_cols = if inner_area.width >= 110 {
        6
    } else if inner_area.width >= 80 {
        3
    } else {
        1
    };
    let gallery_cols = if max_cols == 1 {
        1
    } else {
        (inner_area.width / INSCRIPTION_TILE_WIDTH)
            .max(1)
            .min(max_cols as u16) as usize
    };

    draw_gallery(f, layout.main, state, theme, gallery_cols);
    state.gallery_cols = gallery_cols;

    ControlsBar {
        theme,
        is_syncing: state.is_syncing_balance || state.is_syncing_inscriptions,
    }
    .render(layout.footer, f.buffer_mut());

    if state.is_quitting {
        ExitOverlay {
            theme,
            tick: state.tick_count,
        }
        .render(area, f.buffer_mut());
    }
}

pub fn render_quitting<'a>(theme: &'a ZincTheme, tick_count: u64, _area: Rect) -> impl Widget + 'a {
    ExitOverlay {
        theme,
        tick: tick_count,
    }
}

fn draw_gallery(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &DashboardState,
    theme: &ZincTheme,
    gallery_cols: usize,
) {
    let title = if state.inscriptions.is_empty() {
        " INSCRIPTIONS ".to_string()
    } else {
        format!(" INSCRIPTIONS ({}) ", state.inscriptions.len())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.surface_base))
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(theme.text_primary)
                .bg(theme.surface_base)
                .add_modifier(Modifier::BOLD),
        ));

    let inner_area = block.inner(area);
    block.render(area, frame.buffer_mut());

    if state.inscriptions.is_empty() {
        Paragraph::new("No inscriptions found in this wallet.")
            .style(
                Style::default()
                    .fg(theme.text_muted)
                    .bg(theme.surface_base)
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center)
            .render(inner_area, frame.buffer_mut());
        return;
    }

    let num_cols = gallery_cols;
    let num_rows = (state.inscriptions.len() as u16 + num_cols as u16 - 1) / num_cols as u16;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(INSCRIPTION_TILE_HEIGHT);
            num_rows as usize
        ])
        .split(inner_area);

    for (r, row_area) in rows.iter().enumerate() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, num_cols as u32);
                num_cols as usize
            ])
            .split(*row_area);

        for (c, chunk) in cols.iter().enumerate() {
            let idx = (r * num_cols as usize) + c;
            if let Some(inscription) = state.inscriptions.get(idx) {
                InscriptionWidget {
                    inscription,
                    image: state.image_cache.get(&inscription.id),
                    ascii: state.ascii_cache.get(&inscription.id),
                    is_failed: state.failed_images.contains(&inscription.id),
                    is_selected: idx == state.inscription_index,
                    is_hovered: state.hover_inscription_index == Some(idx),
                    _tick: state.tick_count,
                    theme,
                    ascii_mode: state.ascii_mode,
                }
                .render(*chunk, frame.buffer_mut());
            }
        }
    }
}
