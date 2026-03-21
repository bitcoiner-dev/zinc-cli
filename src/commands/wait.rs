use crate::cli::{Cli, WaitAction, WaitArgs};
use crate::error::AppError;
use crate::load_wallet_session;
use crate::network_retry::with_network_retry;
use crate::wallet_service::map_wallet_error;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

pub async fn run(cli: &Cli, args: &WaitArgs) -> Result<Value, AppError> {
    let mut session = load_wallet_session(cli)?;

    let spinner = if !cli.json && !cli.quiet {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    match &args.action {
        WaitAction::TxConfirmed {
            txid,
            timeout_secs,
            poll_secs,
        } => {
            if let Some(ref pb) = spinner {
                pb.set_message(format!("Waiting for tx {}...", txid));
            }
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(*timeout_secs);
            let poll = Duration::from_secs(*poll_secs);
            let esplora_url = session.profile.esplora_url.clone();

            loop {
                let _: Vec<String> = with_network_retry(
                    cli,
                    "wait tx-confirmed sync",
                    &mut session.wallet,
                    |wallet| {
                        let url = esplora_url.clone();
                        Box::pin(async move { wallet.sync(&url).await.map_err(map_wallet_error) })
                    },
                )
                .await?;

                let txs = session.wallet.get_transactions(1000);
                let confirmation_time = txs
                    .iter()
                    .find(|t| t.txid == **txid)
                    .and_then(|t| t.confirmation_time);

                if confirmation_time.is_some() {
                    if let Some(pb) = spinner {
                        pb.finish_with_message("Transaction confirmed!");
                    }
                    return Ok(json!({
                        "txid": txid,
                        "confirmation_time": confirmation_time,
                        "confirmed": true,
                        "waited_secs": start.elapsed().as_secs()
                    }));
                }

                if start.elapsed() >= timeout {
                    return Err(AppError::Internal(format!(
                        "timed out waiting for tx {txid} after {timeout_secs}s"
                    )));
                }

                sleep(poll).await;
            }
        }
        WaitAction::Balance {
            confirmed_at_least,
            timeout_secs,
            poll_secs,
        } => {
            if *confirmed_at_least == 0 {
                return Ok(json!({
                    "confirmed": 0,
                    "confirmed_balance": 0,
                    "target": confirmed_at_least,
                    "waited_secs": 0
                }));
            }

            if let Some(ref pb) = spinner {
                pb.set_message(format!("Waiting for balance {}...", confirmed_at_least));
            }
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(*timeout_secs);
            let poll = Duration::from_secs(*poll_secs);
            let esplora_url = session.profile.esplora_url.clone();

            loop {
                let _: Vec<String> =
                    with_network_retry(cli, "wait balance sync", &mut session.wallet, |wallet| {
                        let url = esplora_url.clone();
                        Box::pin(async move { wallet.sync(&url).await.map_err(map_wallet_error) })
                    })
                    .await?;

                let confirmed = session.wallet.get_balance().total.confirmed.to_sat();
                if confirmed >= *confirmed_at_least {
                    if let Some(pb) = spinner {
                        pb.finish_with_message("Balance reached!");
                    }
                    return Ok(json!({
                        "confirmed": confirmed,
                        "confirmed_balance": confirmed,
                        "target": confirmed_at_least,
                        "waited_secs": start.elapsed().as_secs()
                    }));
                }

                if start.elapsed() >= timeout {
                    return Err(AppError::Internal(format!(
                        "timed out waiting for balance {confirmed_at_least} after {timeout_secs}s"
                    )));
                }

                sleep(poll).await;
            }
        }
    }
}
