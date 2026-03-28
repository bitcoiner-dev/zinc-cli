pub mod events;
pub mod render;
pub mod state;
pub mod tasks;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};

use ratatui::prelude::*;
use serde_json::Value;
use zinc_core::ZincWallet;

use crate::cli::Cli;
use crate::config::load_persisted_config;
use crate::error::AppError;
use crate::ui::ZincTheme;

use self::events::handle_dashboard_event;
use self::render::{render_dashboard, render_locked, render_quitting};
use self::state::DashboardState;
use self::tasks::{cleanup_tui, setup_tui, spawn_sync_tasks};

pub async fn run(cli: &Cli) -> Result<Value, AppError> {
    let theme = ZincTheme::dark();

    let (tui, picker) = setup_tui(cli)?;
    let mut terminal = tui.terminal;
    let event_tx = tui.event_tx;
    let mut event_rx = tui.event_rx;
    let input_handle = tui.input_handle;

    let (sync_tx, _) = broadcast::channel(32);

    let ascii_mode = detect_ascii_mode(cli, &picker);

    let persisted = load_persisted_config().unwrap_or_default();
    let service_cfg = crate::service_config(cli);
    let resolver = crate::config_resolver::ConfigResolver::new(&persisted, &service_cfg);

    let mut state = DashboardState::new(sync_tx.clone(), picker, ascii_mode);

    let mut background_tasks = vec![input_handle];
    let mut wallet_mutex: Option<Arc<Mutex<ZincWallet>>> = None;
    let mut pending_session = check_pending_session(cli);

    if let Some(ref session) = pending_session {
        activate_session(&mut state, session, &resolver);
    }

    let mut skip_draw = pending_session.is_some();

    loop {
        if let Some(session) = pending_session.take() {
            activate_session(&mut state, &session, &resolver);

            let wallet: Arc<Mutex<ZincWallet>> = Arc::new(Mutex::new(session.wallet));
            let wallet_clone = Arc::clone(&wallet);

            let sync_tasks = spawn_sync_tasks(
                wallet_clone,
                event_tx.clone(),
                session.profile.esplora_url.clone(),
                session.profile.ord_url.clone(),
                state.sync_tx.clone(),
            );

            background_tasks.push(sync_tasks.balance_task);
            background_tasks.push(sync_tasks.inscription_task);

            wallet_mutex = Some(wallet);
            let _ = state.sync_tx.send(());
        }

        if !skip_draw {
            terminal.draw(|f: &mut Frame| {
                if state.is_locked {
                    f.render_widget(render_locked(&theme, &state), f.area());
                    return;
                }

                render_dashboard(f, f.area(), &mut state, &theme);

                if state.is_quitting {
                    f.render_widget(
                        render_quitting(&theme, state.tick_count, f.area()),
                        f.area(),
                    );
                }
            })?;
        }
        skip_draw = false;

        if state.is_quitting {
            terminal.draw(|f| {
                f.render_widget(
                    render_quitting(&theme, state.tick_count, f.area()),
                    f.area(),
                );
            })?;

            for task in background_tasks {
                task.abort();
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
            break;
        }

        if let Some(event) = event_rx.recv().await {
            handle_dashboard_event(
                event,
                &mut state,
                cli,
                &wallet_mutex,
                &mut pending_session,
                &event_tx,
            );

            if pending_session.is_some() {
                skip_draw = true;
            }
        }
    }

    cleanup_tui(&mut terminal).await?;
    Ok(serde_json::Value::Null)
}

fn detect_ascii_mode(cli: &Cli, picker: &ratatui_image::picker::Picker) -> bool {
    use ratatui_image::picker::ProtocolType;
    let protocol = picker.protocol_type();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let is_reliable_iterm = matches!(
        term_program.as_str(),
        "iTerm.app" | "WezTerm" | "com.mitchellh.ghostty"
    );

    cli.ascii
        || protocol == ProtocolType::Halfblocks
        || (protocol == ProtocolType::Iterm2 && !is_reliable_iterm)
}

fn check_pending_session(cli: &Cli) -> Option<crate::wallet_service::WalletSession> {
    let has_password = cli.password.is_some()
        || std::env::var(
            cli.password_env
                .as_deref()
                .unwrap_or("ZINC_WALLET_PASSWORD"),
        )
        .is_ok();

    if has_password {
        crate::load_wallet_session(cli).ok()
    } else {
        None
    }
}

fn activate_session(
    state: &mut DashboardState,
    session: &crate::wallet_service::WalletSession,
    resolver: &crate::config_resolver::ConfigResolver,
) {
    state.is_locked = false;
    let resolved_network = resolver.resolve_network(Some(&session.profile));
    let resolved_profile = session
        .profile_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_string();

    state.account_index = session.wallet.active_account_index();
    state.network = Some(resolved_network.value);
    state.profile_name = Some(resolved_profile);

    state.ordinals_address = Some(session.wallet.peek_taproot_address(0).to_string());
    state.payment_address = session
        .wallet
        .peek_payment_address(0)
        .map(|s| s.to_string());
}
