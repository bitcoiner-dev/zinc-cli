use crossterm::event::{Event, KeyCode, MouseEvent};
use ratatui::layout::Rect;
use std::sync::Arc;
use tokio::sync::mpsc;

use zinc_core::ZincWallet;

use crate::cli::Cli;
use crate::ui::{INSCRIPTION_TILE_HEIGHT, INSCRIPTION_TILE_WIDTH};

use super::state::{DashboardEvent, DashboardState, SyncType};

pub fn handle_dashboard_event(
    event: DashboardEvent,
    state: &mut DashboardState,
    cli: &Cli,
    wallet_mutex: &Option<Arc<tokio::sync::Mutex<ZincWallet>>>,
    pending_session: &mut Option<crate::wallet_service::WalletSession>,
    event_tx: &mpsc::Sender<DashboardEvent>,
) {
    match event {
        DashboardEvent::Input(Event::Key(key)) => {
            if state.is_locked {
                match key.code {
                    KeyCode::Enter => {
                        let mut cli_auth = cli.clone();
                        cli_auth.password = Some(state.password_input.clone());
                        if let Ok(s) = crate::load_wallet_session(&cli_auth) {
                            *pending_session = Some(s);
                        } else {
                            state.auth_error = Some("Invalid password. Try again.".to_string());
                            state.password_input.clear();
                        }
                    }
                    KeyCode::Char(c) => state.password_input.push(c),
                    KeyCode::Backspace => {
                        state.password_input.pop();
                    }
                    KeyCode::Esc => {
                        state.is_quitting = true;
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    state.is_quitting = true;
                }
                KeyCode::Char('s') => {
                    let _ = state.sync_tx.send(());
                }
                KeyCode::Tab => {
                    switch_account(state, wallet_mutex, (state.account_index + 1) % 5);
                }
                KeyCode::BackTab => {
                    let prev_account = if state.account_index == 0 {
                        4
                    } else {
                        state.account_index - 1
                    };
                    switch_account(state, wallet_mutex, prev_account);
                    let _ = state.sync_tx.send(());
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    navigate_inscriptions(state, key.code);
                }
                _ => {}
            }
        }
        DashboardEvent::Input(Event::Mouse(mouse_event)) => {
            handle_mouse_event(mouse_event, state);
        }
        DashboardEvent::Tick => {
            state.tick_count += 1;
        }
        DashboardEvent::BalanceUpdated { confirmed, pending } => {
            state.confirmed_balance = confirmed;
            state.pending_balance = pending;

            if state.ordinals_address.is_none() || state.payment_address.is_none() {
                if let Some(ref w_mutex) = wallet_mutex {
                    let w_clone = Arc::clone(w_mutex);
                    let event_tx_clone = event_tx.clone();
                    tokio::spawn(async move {
                        let w = w_clone.lock().await;
                        let ordinals = w.peek_taproot_address(0).to_string();
                        let payment = w.peek_payment_address(0).map(|s| s.to_string());
                        let _ = event_tx_clone
                            .send(DashboardEvent::AddressesUpdated { ordinals, payment })
                            .await;
                    });
                }
            }
        }
        DashboardEvent::AddressesUpdated { ordinals, payment } => {
            state.ordinals_address = Some(ordinals);
            state.payment_address = payment;
        }
        DashboardEvent::InscriptionsUpdated(inscriptions) => {
            state.inscriptions = inscriptions;
        }
        DashboardEvent::SyncStarted(stype) => match stype {
            SyncType::Balance => state.is_syncing_balance = true,
            SyncType::Inscriptions => state.is_syncing_inscriptions = true,
        },
        DashboardEvent::SyncFinished(stype) => match stype {
            SyncType::Balance => state.is_syncing_balance = false,
            SyncType::Inscriptions => state.is_syncing_inscriptions = false,
        },
        _ => {}
    }
}

fn switch_account(
    state: &mut DashboardState,
    wallet_mutex: &Option<Arc<tokio::sync::Mutex<ZincWallet>>>,
    new_account: u32,
) {
    state.account_index = new_account;
    state.confirmed_balance = 0;
    state.pending_balance = 0;
    state.inscriptions.clear();
    state.image_cache.clear();
    state.failed_images.clear();
    state.ordinals_address = None;
    state.payment_address = None;

    if let Some(ref w_mutex) = wallet_mutex {
        let w_switch: Arc<tokio::sync::Mutex<zinc_core::ZincWallet>> = Arc::clone(w_mutex);
        let sync_tx = state.sync_tx.clone();
        tokio::spawn(async move {
            let mut w = w_switch.lock().await;
            let _ = w.set_active_account(new_account);
            let _ = sync_tx.send(());
        });
    }
}

fn navigate_inscriptions(state: &mut DashboardState, key_code: KeyCode) {
    if state.inscriptions.is_empty() {
        return;
    }

    let cols = state.gallery_cols.max(1);
    let current_idx = state.inscription_index;
    let (row, col) = (current_idx / cols, current_idx % cols);
    let total_rows = (state.inscriptions.len() + cols - 1) / cols;

    let (new_row, new_col) = match key_code {
        KeyCode::Left if col > 0 => (row, col - 1),
        KeyCode::Right if col < cols - 1 && current_idx + 1 < state.inscriptions.len() => {
            (row, col + 1)
        }
        KeyCode::Up if row > 0 => (row - 1, col),
        KeyCode::Down if row < total_rows - 1 && current_idx + cols < state.inscriptions.len() => {
            (row + 1, col)
        }
        _ => (row, col),
    };

    let new_idx = new_row * cols + new_col;
    if new_idx < state.inscriptions.len() {
        state.inscription_index = new_idx;
    }
}

fn handle_mouse_event(mouse_event: MouseEvent, state: &mut DashboardState) {
    if state.is_locked {
        return;
    }

    let (x, y) = (mouse_event.column, mouse_event.row);
    state.mouse_pos = Some((x, y));

    let layout = super::state::DashboardLayout::new(Rect::new(0, 0, 120, 30));

    state.hover_balance = y >= layout.hero.y && y < layout.hero.y + layout.hero.height;

    state.hover_inscription_index = None;
    if y >= layout.main.y && y < layout.main.y + layout.main.height {
        let cols = state.gallery_cols.max(1);
        let tile_width = INSCRIPTION_TILE_WIDTH;
        let tile_height = INSCRIPTION_TILE_HEIGHT;

        let rel_y = y.saturating_sub(layout.main.y + 1);
        let rel_x = x.saturating_sub(layout.main.x + 1);

        let col = rel_x / tile_width;
        let row = rel_y / tile_height;

        let idx = (row as usize * cols) + col as usize;
        if idx < state.inscriptions.len() {
            state.hover_inscription_index = Some(idx);
        }
    }
}
