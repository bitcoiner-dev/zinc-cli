use crate::cli::{Cli, ScenarioAction, ScenarioArgs};
use crate::error::AppError;
use crate::utils::run_bitcoin_cli;
use crate::wallet_service::NetworkArg;
use crate::{confirm, load_wallet_session, persist_wallet_session, snapshot_dir};
use serde_json::{json, Value};
use std::fs;

pub async fn run(cli: &Cli, args: &ScenarioArgs) -> Result<Value, AppError> {
    let mut session = load_wallet_session(cli)?;
    if !matches!(session.profile.network, NetworkArg::Regtest) {
        return Err(AppError::Invalid(
            "scenario commands are only supported on regtest profiles".to_string(),
        ));
    }

    let mut should_persist = true;
    let result = match &args.action {
        ScenarioAction::Mine { blocks, address } => {
            if *blocks == 0 {
                return Err(AppError::Invalid(
                    "--blocks must be greater than 0".to_string(),
                ));
            }

            let mining_address = if let Some(address) = address {
                address.clone()
            } else {
                session.wallet.peek_taproot_address(0).to_string()
            };

            let generated = run_bitcoin_cli(
                &session.profile,
                &[
                    "generatetoaddress".to_string(),
                    blocks.to_string(),
                    mining_address.clone(),
                ],
            )?;

            json!({
                "action": "mine",
                "blocks": blocks,
                "address": mining_address,
                "raw": generated
            })
        }
        ScenarioAction::Fund {
            address,
            amount_btc,
            mine_blocks,
        } => {
            if *mine_blocks == 0 {
                return Err(AppError::Invalid(
                    "--mine-blocks must be greater than 0".to_string(),
                ));
            }
            let destination = if let Some(address) = address {
                address.clone()
            } else {
                session.wallet.peek_taproot_address(0).to_string()
            };

            let txid = run_bitcoin_cli(
                &session.profile,
                &[
                    "sendtoaddress".to_string(),
                    destination.clone(),
                    amount_btc.clone(),
                ],
            )?;

            let mine_address = session.wallet.peek_taproot_address(0).to_string();
            let generated = run_bitcoin_cli(
                &session.profile,
                &[
                    "generatetoaddress".to_string(),
                    mine_blocks.to_string(),
                    mine_address.clone(),
                ],
            )?;

            json!({
                "action": "fund",
                "address": destination,
                "amount_btc": amount_btc,
                "txid": txid.trim(),
                "mine_blocks": mine_blocks,
                "mine_address": mine_address,
                "generated_blocks": generated
            })
        }
        ScenarioAction::Reset {
            remove_profile,
            remove_snapshots,
        } => {
            if !confirm("Are you sure you want to reset the scenario? This may delete your profile and/or snapshots.", cli) {
                return Err(AppError::Internal("aborted by user".to_string()));
            }
            should_persist = false;
            let mut removed = Vec::new();
            if *remove_profile || (!remove_profile && !remove_snapshots) {
                if session.profile_path.exists() {
                    fs::remove_file(&session.profile_path).map_err(|e| {
                        AppError::Config(format!(
                            "failed to remove profile {}: {e}",
                            session.profile_path.display()
                        ))
                    })?;
                    removed.push(session.profile_path.display().to_string());
                }
            }
            if *remove_snapshots || (!remove_profile && !remove_snapshots) {
                let snap_dir = snapshot_dir(cli)?;
                if snap_dir.exists() {
                    fs::remove_dir_all(&snap_dir).map_err(|e| {
                        AppError::Config(format!(
                            "failed to remove snapshots {}: {e}",
                            snap_dir.display()
                        ))
                    })?;
                    removed.push(snap_dir.display().to_string());
                }
            }

            json!({
                "action": "reset",
                "removed": removed
            })
        }
    };

    if should_persist {
        persist_wallet_session(&mut session)?;
    }
    Ok(result)
}
