use crate::cli::{Cli, SyncArgs, SyncTarget};
use crate::error::AppError;
use crate::network_retry::with_network_retry;
use crate::output::CommandOutput;
use crate::wallet_service::map_wallet_error;
use crate::{load_wallet_session, persist_wallet_session};
use indicatif::{ProgressBar, ProgressStyle};

pub async fn run(cli: &Cli, args: &SyncArgs) -> Result<CommandOutput, AppError> {
    let spinner = if !cli.agent {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Syncing wallet...");
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    let result = match &args.target {
        SyncTarget::Chain => {
            let mut session = load_wallet_session(cli)?;
            let esplora_url = session.profile.esplora_url.clone();
            let events = with_network_retry(cli, "sync chain", &mut session.wallet, |wallet| {
                let url = esplora_url.clone();
                Box::pin(async move { wallet.sync(&url).await.map_err(map_wallet_error) })
            })
            .await?;
            persist_wallet_session(&mut session)?;
            CommandOutput::SyncChain { events }
        }
        SyncTarget::Ordinals => {
            let mut session = load_wallet_session(cli)?;
            if let Some(ref pb) = spinner {
                pb.set_message("Syncing ordinals...");
            }
            let ord_url = session.profile.ord_url.clone();
            let count = with_network_retry(cli, "sync ordinals", &mut session.wallet, |wallet| {
                let url = ord_url.clone();
                Box::pin(async move { wallet.sync_ordinals(&url).await.map_err(map_wallet_error) })
            })
            .await?;
            persist_wallet_session(&mut session)?;
            CommandOutput::SyncOrdinals {
                inscriptions: count,
            }
        }
    };

    if let Some(pb) = spinner {
        pb.finish_with_message("Sync complete!");
    }

    Ok(result)
}
