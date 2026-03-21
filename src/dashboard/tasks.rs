use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};

use crossterm::{
    event::{self},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui_image::picker::Picker;
use ratatui_image::picker::ProtocolType;

use zinc_core::ZincWallet;

use crate::cli::Cli;
use crate::error::AppError;

use super::state::{DashboardEvent, SyncType};

pub struct TuiSetup {
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
    pub event_tx: mpsc::Sender<DashboardEvent>,
    pub event_rx: mpsc::Receiver<DashboardEvent>,
    pub input_handle: tokio::task::JoinHandle<()>,
}

pub fn setup_tui(cli: &Cli) -> Result<(TuiSetup, Picker), AppError> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let (event_tx, event_rx) = mpsc::channel(1024);

    let picker = if cli.no_images {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or(Picker::halfblocks())
    };

    let _ascii_mode = detect_ascii_mode(cli, &picker);

    let event_tx_clone = event_tx.clone();
    let input_handle = tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap() {
                if let Ok(ev) = event::read() {
                    let _ = event_tx_clone.send(DashboardEvent::Input(ev)).await;
                }
            }
            let _ = event_tx_clone.send(DashboardEvent::Tick).await;
        }
    });

    Ok((
        TuiSetup {
            terminal,
            event_tx,
            event_rx,
            input_handle,
        },
        picker,
    ))
}

fn detect_ascii_mode(cli: &Cli, picker: &Picker) -> bool {
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

pub struct SyncTasks {
    pub balance_task: tokio::task::JoinHandle<()>,
    pub inscription_task: tokio::task::JoinHandle<()>,
}

pub fn spawn_sync_tasks(
    wallet: Arc<Mutex<ZincWallet>>,
    event_tx: mpsc::Sender<DashboardEvent>,
    esplora_url: String,
    ord_url: String,
    sync_tx: broadcast::Sender<()>,
) -> SyncTasks {
    let balance_task = spawn_balance_task(
        Arc::clone(&wallet),
        event_tx.clone(),
        esplora_url,
        sync_tx.subscribe(),
    );

    let inscription_task =
        spawn_inscription_task(Arc::clone(&wallet), event_tx, ord_url, sync_tx.subscribe());

    SyncTasks {
        balance_task,
        inscription_task,
    }
}

fn spawn_balance_task(
    wallet: Arc<Mutex<ZincWallet>>,
    event_tx: mpsc::Sender<DashboardEvent>,
    esplora_url: String,
    mut sync_rx: broadcast::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let _ = event_tx
                .send(DashboardEvent::SyncStarted(SyncType::Balance))
                .await;
            {
                let mut w = wallet.lock().await;
                if let Ok(Result::Ok(_)) =
                    tokio::time::timeout(Duration::from_secs(30), w.sync(&esplora_url)).await
                {
                    let balance = w.get_balance();
                    let confirmed = balance.total.confirmed.to_sat();
                    let pending = balance.total.trusted_pending.to_sat()
                        + balance.total.untrusted_pending.to_sat();
                    let _ = event_tx
                        .send(DashboardEvent::BalanceUpdated { confirmed, pending })
                        .await;
                }
            }
            let _ = event_tx
                .send(DashboardEvent::SyncFinished(SyncType::Balance))
                .await;

            match sync_rx.recv().await {
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

fn spawn_inscription_task(
    wallet: Arc<Mutex<ZincWallet>>,
    event_tx: mpsc::Sender<DashboardEvent>,
    ord_url: String,
    mut sync_rx: broadcast::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let _ = event_tx
                .send(DashboardEvent::SyncStarted(SyncType::Inscriptions))
                .await;
            let mut w = wallet.lock().await;
            if let Ok(Result::Ok(_)) =
                tokio::time::timeout(Duration::from_secs(30), w.sync_ordinals(&ord_url)).await
            {
                let inscriptions = w.inscriptions().to_vec();
                let _ = event_tx
                    .send(DashboardEvent::InscriptionsUpdated(inscriptions.clone()))
                    .await;
            }
            drop(w);
            let _ = event_tx
                .send(DashboardEvent::SyncFinished(SyncType::Inscriptions))
                .await;

            tokio::select! {
                res = sync_rx.recv() => {
                    match res {
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                },
                _ = tokio::time::sleep(Duration::from_secs(60)) => {},
            }
        }
    })
}

pub async fn cleanup_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), AppError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
