#![allow(dead_code)]
use crate::cli::{Cli, ListingAction, ListingArgs};
use crate::commands::psbt::{analyze_psbt_with_policy, enforce_policy_mode};
use crate::config::NetworkArg;
use crate::error::AppError;
use crate::network_retry::with_network_retry;
use crate::output::CommandOutput;
use crate::utils::maybe_write_text;
use crate::wallet_service::map_wallet_error;
use crate::{load_wallet_session, persist_wallet_session};
use bitcoin::{Address, Amount, TxOut};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use zinc_core::{
    create_listing, finalize_listing_sale, sign_listing_coordinator_psbt,
    CreateListingPurchaseRequest, CreateListingRequest, ListingEnvelopeV1,
    ListingRelayQueryOptions, NostrListingEvent, NostrListingRelayClient, OrdClient, SignOptions,
    LISTING_SALE_SIGHASH_U8,
};

pub async fn run(cli: &Cli, args: &ListingArgs) -> Result<CommandOutput, AppError> {
    if cli.password_stdin && action_reads_listing_stdin(&args.action) {
        return Err(AppError::Invalid(
            "--password-stdin cannot be combined with --listing-stdin".to_string(),
        ));
    }

    match &args.action {
        ListingAction::Sell { .. } => handle_sell(cli, &args.action).await,
        ListingAction::Create {
            inscription,
            amount,
            fee_rate,
            coordinator_pubkey_hex,
            expires_in_secs,
            created_at_unix,
            nonce,
            seller_payout_address,
            recovery_address,
            listing_out_file,
            tx1_out_file,
            sale_psbt_out_file,
            recovery_psbt_out_file,
        } => {
            let mut session = load_wallet_session(cli)?;
            session.require_seed_mode()?;
            if !session
                .wallet
                .inscriptions()
                .iter()
                .any(|ins| ins.id == *inscription)
            {
                return Err(AppError::Invalid(format!(
                    "inscription {} is not in wallet",
                    inscription
                )));
            }

            let ord_url = resolve_ord_url(cli)?;
            let client = OrdClient::new(ord_url.clone());
            let inscription_details = client
                .get_inscription_details(inscription)
                .await
                .map_err(map_listing_error)?;
            let output_details = client
                .get_output_details(&inscription_details.satpoint.outpoint)
                .await
                .map_err(map_listing_error)?;

            if !output_details
                .inscriptions
                .iter()
                .any(|id| id == inscription)
            {
                return Err(AppError::Invalid(format!(
                    "inscription {} is not present in output {}",
                    inscription, output_details.outpoint
                )));
            }

            let network = session.profile.network;
            let seller_prevout_script = address_to_script_pubkey(&output_details.address, network)?;
            let main_payment_address = session
                .wallet
                .peek_payment_address(0)
                .map(|address| address.to_string())
                .unwrap_or_else(|| session.wallet.peek_taproot_address(0).to_string());
            let main_taproot_address = session.wallet.peek_taproot_address(0).to_string();
            let seller_payout_script_pubkey = address_to_script_pubkey(
                seller_payout_address
                    .as_deref()
                    .unwrap_or(main_payment_address.as_str()),
                network,
            )?;
            let recovery_script_pubkey = address_to_script_pubkey(
                recovery_address
                    .as_deref()
                    .unwrap_or(main_taproot_address.as_str()),
                network,
            )?;
            let seller_pubkey_hex = session
                .wallet
                .get_taproot_public_key(0)
                .map_err(AppError::Internal)?;
            let created_unix_u64 = created_at_unix.unwrap_or_else(current_unix);
            let created_unix = i64::try_from(created_unix_u64)
                .map_err(|_| AppError::Invalid("created_at_unix is out of range".to_string()))?;
            let expires_u64 = created_unix_u64
                .checked_add(*expires_in_secs)
                .ok_or_else(|| {
                    AppError::Invalid("created_at_unix + expires_in_secs overflowed".to_string())
                })?;
            let expires_unix = i64::try_from(expires_u64)
                .map_err(|_| AppError::Invalid("expires_at_unix is out of range".to_string()))?;

            let request = CreateListingRequest {
                seller_pubkey_hex,
                coordinator_pubkey_hex: coordinator_pubkey_hex.clone(),
                network: network_label(network).to_string(),
                inscription_id: inscription.clone(),
                seller_outpoint: inscription_details.satpoint.outpoint,
                seller_prevout: TxOut {
                    value: Amount::from_sat(output_details.value),
                    script_pubkey: seller_prevout_script,
                },
                seller_payout_script_pubkey,
                recovery_script_pubkey,
                ask_sats: *amount,
                fee_rate_sat_vb: *fee_rate,
                created_at_unix: created_unix,
                expires_at_unix: expires_unix,
                nonce: nonce.unwrap_or_else(current_nanos),
            };
            let mut created = create_listing(&request).map_err(map_listing_error)?;

            let signed_sale_psbt = session
                .wallet
                .sign_psbt(
                    &created.listing.sale_psbt_base64,
                    Some(SignOptions {
                        sign_inputs: Some(vec![0]),
                        sighash: Some(LISTING_SALE_SIGHASH_U8),
                        finalize: false,
                    }),
                )
                .map_err(map_wallet_error)?;
            created.listing.sale_psbt_base64 = signed_sale_psbt;

            if let Some(path) = tx1_out_file {
                maybe_write_text(
                    Some(&path.display().to_string()),
                    &created.listing.tx1_base64,
                )?;
            }
            if let Some(path) = sale_psbt_out_file {
                maybe_write_text(
                    Some(&path.display().to_string()),
                    &created.listing.sale_psbt_base64,
                )?;
            }
            if let Some(path) = recovery_psbt_out_file {
                maybe_write_text(
                    Some(&path.display().to_string()),
                    &created.listing.recovery_psbt_base64,
                )?;
            }
            if let Some(path) = listing_out_file {
                let listing_json = serde_json::to_string_pretty(&created.listing)
                    .map_err(|e| AppError::Internal(format!("failed to serialize listing: {e}")))?;
                maybe_write_text(Some(&path.display().to_string()), &listing_json)?;
            }

            persist_wallet_session(&mut session)?;
            listing_create_output(json!({
                "inscription_id": inscription,
                "ask_sats": amount,
                "fee_rate_sat_vb": fee_rate,
                "seller_outpoint": created.listing.seller_outpoint,
                "passthrough_outpoint": created.listing.passthrough_outpoint,
                "seller_pubkey_hex": created.listing.seller_pubkey_hex,
                "coordinator_pubkey_hex": created.listing.coordinator_pubkey_hex,
                "expires_at_unix": created.listing.expires_at_unix,
                "listing": created.listing,
                "ord_url": ord_url,
            }))
        }
        ListingAction::Activate {
            listing_json,
            listing_file,
            listing_stdin,
            dry_run,
            signed_tx1_out_file,
        } => {
            let source = resolve_listing_source(
                listing_json.as_deref(),
                listing_file.as_deref(),
                *listing_stdin,
            )?;
            let listing: ListingEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?;

            let mut session = load_wallet_session(cli)?;
            session.require_seed_mode()?;
            assert_listing_network_matches_profile(&listing, session.profile.network)?;
            let (analysis, policy) =
                analyze_psbt_with_policy(&session.wallet, &listing.tx1_base64)?;
            enforce_policy_mode(cli, &policy)?;
            let signed = session
                .wallet
                .sign_psbt(
                    &listing.tx1_base64,
                    Some(SignOptions {
                        sign_inputs: None,
                        sighash: None,
                        finalize: true,
                    }),
                )
                .map_err(map_wallet_error)?;
            if let Some(path) = signed_tx1_out_file {
                maybe_write_text(Some(&path.display().to_string()), &signed)?;
            }

            if *dry_run {
                return listing_activate_output(json!({
                    "activated": true,
                    "dry_run": true,
                    "inscription_id": listing.inscription_id,
                    "txid": null,
                    "signed_tx1_psbt": signed,
                    "safe_to_send": policy.safe_to_send,
                    "inscription_risk": policy.inscription_risk,
                    "policy_reasons": policy.policy_reasons,
                    "analysis": analysis,
                }));
            }

            let esplora_url = session.profile.esplora_url.clone();
            let txid: String = with_network_retry(
                cli,
                "listing activate broadcast",
                &mut session.wallet,
                |wallet| {
                    let url = esplora_url.clone();
                    let psbt = signed.clone();
                    Box::pin(async move {
                        wallet
                            .broadcast(&psbt, &url)
                            .await
                            .map_err(map_wallet_error)
                    })
                },
            )
            .await?;
            persist_wallet_session(&mut session)?;
            listing_activate_output(json!({
                "activated": true,
                "dry_run": false,
                "inscription_id": listing.inscription_id,
                "txid": txid,
                "safe_to_send": policy.safe_to_send,
                "inscription_risk": policy.inscription_risk,
                "policy_reasons": policy.policy_reasons,
                "analysis": analysis,
            }))
        }
        ListingAction::Publish {
            listing_json,
            listing_file,
            listing_stdin,
            secret_key_hex,
            relay,
            created_at_unix,
            timeout_ms,
        } => {
            if relay.is_empty() {
                return Err(AppError::Invalid(
                    "at least one --relay is required".to_string(),
                ));
            }
            let source = resolve_listing_source(
                listing_json.as_deref(),
                listing_file.as_deref(),
                *listing_stdin,
            )?;
            let listing: ListingEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?;
            let created_at = created_at_unix.unwrap_or_else(current_unix);
            let event = NostrListingEvent::from_listing(&listing, secret_key_hex, created_at)
                .map_err(map_listing_error)?;
            let results =
                NostrListingRelayClient::publish_listing_multi(relay, &event, *timeout_ms).await;
            let accepted = results.iter().filter(|r| r.accepted).count();
            listing_publish_output(json!({
                "event": event,
                "publish_results": results,
                "accepted_relays": accepted,
                "total_relays": relay.len(),
            }))
        }
        ListingAction::Discover {
            relay,
            limit,
            timeout_ms,
        } => {
            if relay.is_empty() {
                return Err(AppError::Invalid(
                    "at least one --relay is required".to_string(),
                ));
            }
            let options = ListingRelayQueryOptions {
                limit: *limit,
                timeout_ms: *timeout_ms,
            };
            let events =
                NostrListingRelayClient::discover_listing_events_multi(relay, options).await;
            let listings: Vec<Value> = events
                .iter()
                .filter_map(|event| {
                    event.decode_listing().ok().map(|listing| {
                        json!({
                            "event_id": event.id,
                            "pubkey": event.pubkey,
                            "created_at": event.created_at,
                            "listing": listing
                        })
                    })
                })
                .collect();
            listing_discover_output(json!({
                "events": events,
                "listings": listings,
                "event_count": events.len(),
                "listing_count": listings.len(),
            }))
        }
        ListingAction::Buy {
            listing_json,
            listing_file,
            listing_stdin,
            expect_inscription,
            expect_ask_sats,
            listing_out_file,
            psbt_out_file,
        } => {
            let source = resolve_listing_source(
                listing_json.as_deref(),
                listing_file.as_deref(),
                *listing_stdin,
            )?;
            let listing: ListingEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?;
            assert_listing_expectations(&listing, expect_inscription.as_deref(), *expect_ask_sats)?;

            let mut session = load_wallet_session(cli)?;
            session.require_seed_mode()?;
            assert_listing_network_matches_profile(&listing, session.profile.network)?;
            let purchased = session
                .wallet
                .create_listing_purchase(&CreateListingPurchaseRequest {
                    listing: listing.clone(),
                    now_unix: i64::try_from(current_unix()).unwrap_or(i64::MAX),
                })
                .map_err(map_listing_error)?;

            if let Some(path) = psbt_out_file {
                maybe_write_text(Some(&path.display().to_string()), &purchased.psbt_base64)?;
            }
            if let Some(path) = listing_out_file {
                let listing_json = serde_json::to_string_pretty(&purchased.listing)
                    .map_err(|e| AppError::Internal(format!("failed to serialize listing: {e}")))?;
                maybe_write_text(Some(&path.display().to_string()), &listing_json)?;
            }

            persist_wallet_session(&mut session)?;
            listing_buy_output(json!({
                "inscription_id": purchased.listing.inscription_id,
                "ask_sats": purchased.listing.ask_sats,
                "fee_sats": purchased.fee_sats,
                "seller_input_index": purchased.seller_input_index,
                "buyer_input_count": purchased.buyer_input_count,
                "buyer_receive_output_index": purchased.buyer_receive_output_index,
                "psbt": purchased.psbt_base64,
                "listing": purchased.listing,
            }))
        }
        ListingAction::CoordinatorSign {
            listing_json,
            listing_file,
            listing_stdin,
            secret_key_hex,
            created_at_unix,
            listing_out_file,
            psbt_out_file,
        } => {
            let source = resolve_listing_source(
                listing_json.as_deref(),
                listing_file.as_deref(),
                *listing_stdin,
            )?;
            let mut listing: ListingEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?;
            let now_unix = i64::try_from(created_at_unix.unwrap_or_else(current_unix))
                .map_err(|_| AppError::Invalid("created_at_unix is out of range".to_string()))?;
            listing.sale_psbt_base64 =
                sign_listing_coordinator_psbt(&listing, secret_key_hex, now_unix)
                    .map_err(map_listing_error)?;

            if let Some(path) = psbt_out_file {
                maybe_write_text(Some(&path.display().to_string()), &listing.sale_psbt_base64)?;
            }
            if let Some(path) = listing_out_file {
                let listing_json = serde_json::to_string_pretty(&listing)
                    .map_err(|e| AppError::Internal(format!("failed to serialize listing: {e}")))?;
                maybe_write_text(Some(&path.display().to_string()), &listing_json)?;
            }
            listing_coordinator_sign_output(json!({
                "inscription_id": listing.inscription_id,
                "ask_sats": listing.ask_sats,
                "psbt": listing.sale_psbt_base64,
                "listing": listing,
            }))
        }
        ListingAction::Finalize {
            listing_json,
            listing_file,
            listing_stdin,
            broadcast,
            finalized_psbt_out_file,
            tx_hex_out_file,
        } => {
            let source = resolve_listing_source(
                listing_json.as_deref(),
                listing_file.as_deref(),
                *listing_stdin,
            )?;
            let listing: ListingEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?;
            let finalized =
                finalize_listing_sale(&listing, i64::try_from(current_unix()).unwrap_or(i64::MAX))
                    .map_err(map_listing_error)?;

            if let Some(path) = finalized_psbt_out_file {
                maybe_write_text(
                    Some(&path.display().to_string()),
                    &finalized.finalized_psbt_base64,
                )?;
            }
            if let Some(path) = tx_hex_out_file {
                maybe_write_text(Some(&path.display().to_string()), &finalized.tx_hex)?;
            }

            let broadcast_txid = if *broadcast {
                let mut session = load_wallet_session(cli)?;
                let esplora_url = session.profile.esplora_url.clone();
                let psbt = finalized.finalized_psbt_base64.clone();
                let txid: String = with_network_retry(
                    cli,
                    "listing finalize broadcast",
                    &mut session.wallet,
                    |wallet| {
                        let url = esplora_url.clone();
                        let psbt = psbt.clone();
                        Box::pin(async move {
                            wallet
                                .broadcast(&psbt, &url)
                                .await
                                .map_err(map_wallet_error)
                        })
                    },
                )
                .await?;
                persist_wallet_session(&mut session)?;
                Some(txid)
            } else {
                None
            };

            listing_finalize_output(json!({
                "inscription_id": listing.inscription_id,
                "ask_sats": listing.ask_sats,
                "txid": broadcast_txid.unwrap_or_else(|| finalized.txid.clone()),
                "broadcast": broadcast,
                "finalized_psbt": finalized.finalized_psbt_base64,
                "tx_hex": finalized.tx_hex,
                "seller_input_index": finalized.seller_input_index,
                "passthrough_witness_items": finalized.passthrough_witness_items,
            }))
        }
        ListingAction::Purchase { .. } => handle_purchase(cli, &args.action).await,
    }
}

async fn handle_sell(cli: &Cli, action: &ListingAction) -> Result<CommandOutput, AppError> {
    let ListingAction::Sell {
        inscription,
        amount,
        fee_rate,
        coordinator_pubkey_hex,
        expires_in_secs,
        created_at_unix,
        nonce,
        seller_payout_address,
        recovery_address,
        activate,
        dry_run,
        relay,
        secret_key_hex,
        listing_out_file,
        tx1_out_file,
        sale_psbt_out_file,
        recovery_psbt_out_file,
        signed_tx1_out_file,
        timeout_ms,
    } = action
    else {
        return Err(AppError::Internal(
            "handle_sell called with non-sell action".to_string(),
        ));
    };

    if *dry_run && !*activate {
        return Err(AppError::Invalid(
            "--dry-run only applies when --activate is set".to_string(),
        ));
    }
    if !relay.is_empty() && secret_key_hex.is_none() {
        return Err(AppError::Invalid(
            "--secret-key-hex is required when --relay is supplied".to_string(),
        ));
    }

    let mut session = load_wallet_session(cli)?;
    session.require_seed_mode()?;
    if !session
        .wallet
        .inscriptions()
        .iter()
        .any(|ins| ins.id == *inscription)
    {
        return Err(AppError::Invalid(format!(
            "inscription {} is not in wallet",
            inscription
        )));
    }

    let ord_url = resolve_ord_url(cli)?;
    let client = OrdClient::new(ord_url.clone());
    let inscription_details = client
        .get_inscription_details(inscription)
        .await
        .map_err(map_listing_error)?;
    let output_details = client
        .get_output_details(&inscription_details.satpoint.outpoint)
        .await
        .map_err(map_listing_error)?;

    if !output_details
        .inscriptions
        .iter()
        .any(|id| id == inscription)
    {
        return Err(AppError::Invalid(format!(
            "inscription {} is not present in output {}",
            inscription, output_details.outpoint
        )));
    }

    let network = session.profile.network;
    let seller_prevout_script = address_to_script_pubkey(&output_details.address, network)?;
    let main_payment_address = session
        .wallet
        .peek_payment_address(0)
        .map(|address| address.to_string())
        .unwrap_or_else(|| session.wallet.peek_taproot_address(0).to_string());
    let main_taproot_address = session.wallet.peek_taproot_address(0).to_string();
    let seller_payout_script_pubkey = address_to_script_pubkey(
        seller_payout_address
            .as_deref()
            .unwrap_or(main_payment_address.as_str()),
        network,
    )?;
    let recovery_script_pubkey = address_to_script_pubkey(
        recovery_address
            .as_deref()
            .unwrap_or(main_taproot_address.as_str()),
        network,
    )?;
    let seller_pubkey_hex = session
        .wallet
        .get_taproot_public_key(0)
        .map_err(AppError::Internal)?;
    let created_unix_u64 = created_at_unix.unwrap_or_else(current_unix);
    let created_unix = i64::try_from(created_unix_u64)
        .map_err(|_| AppError::Invalid("created_at_unix is out of range".to_string()))?;
    let expires_u64 = created_unix_u64
        .checked_add(*expires_in_secs)
        .ok_or_else(|| {
            AppError::Invalid("created_at_unix + expires_in_secs overflowed".to_string())
        })?;
    let expires_unix = i64::try_from(expires_u64)
        .map_err(|_| AppError::Invalid("expires_at_unix is out of range".to_string()))?;

    let request = CreateListingRequest {
        seller_pubkey_hex,
        coordinator_pubkey_hex: coordinator_pubkey_hex.clone(),
        network: network_label(network).to_string(),
        inscription_id: inscription.clone(),
        seller_outpoint: inscription_details.satpoint.outpoint,
        seller_prevout: TxOut {
            value: Amount::from_sat(output_details.value),
            script_pubkey: seller_prevout_script,
        },
        seller_payout_script_pubkey,
        recovery_script_pubkey,
        ask_sats: *amount,
        fee_rate_sat_vb: *fee_rate,
        created_at_unix: created_unix,
        expires_at_unix: expires_unix,
        nonce: nonce.unwrap_or_else(current_nanos),
    };
    let mut created = create_listing(&request).map_err(map_listing_error)?;

    let signed_sale_psbt = session
        .wallet
        .sign_psbt(
            &created.listing.sale_psbt_base64,
            Some(SignOptions {
                sign_inputs: Some(vec![0]),
                sighash: Some(LISTING_SALE_SIGHASH_U8),
                finalize: false,
            }),
        )
        .map_err(map_wallet_error)?;
    created.listing.sale_psbt_base64 = signed_sale_psbt;

    if let Some(path) = tx1_out_file {
        maybe_write_text(
            Some(&path.display().to_string()),
            &created.listing.tx1_base64,
        )?;
    }
    if let Some(path) = sale_psbt_out_file {
        maybe_write_text(
            Some(&path.display().to_string()),
            &created.listing.sale_psbt_base64,
        )?;
    }
    if let Some(path) = recovery_psbt_out_file {
        maybe_write_text(
            Some(&path.display().to_string()),
            &created.listing.recovery_psbt_base64,
        )?;
    }
    if let Some(path) = listing_out_file {
        let listing_json = serde_json::to_string_pretty(&created.listing)
            .map_err(|e| AppError::Internal(format!("failed to serialize listing: {e}")))?;
        maybe_write_text(Some(&path.display().to_string()), &listing_json)?;
    }

    let mut activation = None;
    if *activate {
        let (analysis, policy) =
            analyze_psbt_with_policy(&session.wallet, &created.listing.tx1_base64)?;
        enforce_policy_mode(cli, &policy)?;
        let signed_tx1 = session
            .wallet
            .sign_psbt(
                &created.listing.tx1_base64,
                Some(SignOptions {
                    sign_inputs: None,
                    sighash: None,
                    finalize: true,
                }),
            )
            .map_err(map_wallet_error)?;
        if let Some(path) = signed_tx1_out_file {
            maybe_write_text(Some(&path.display().to_string()), &signed_tx1)?;
        }
        let txid = if *dry_run {
            None
        } else {
            let esplora_url = session.profile.esplora_url.clone();
            let psbt = signed_tx1.clone();
            Some(
                with_network_retry(
                    cli,
                    "listing sell activate broadcast",
                    &mut session.wallet,
                    |wallet| {
                        let url = esplora_url.clone();
                        let psbt = psbt.clone();
                        Box::pin(async move {
                            wallet
                                .broadcast(&psbt, &url)
                                .await
                                .map_err(map_wallet_error)
                        })
                    },
                )
                .await?,
            )
        };
        activation = Some(json!({
            "dry_run": dry_run,
            "txid": txid,
            "signed_tx1_psbt": signed_tx1,
            "safe_to_send": policy.safe_to_send,
            "inscription_risk": policy.inscription_risk,
            "policy_reasons": policy.policy_reasons,
            "analysis": analysis,
        }));
    }

    let mut publish = None;
    if !relay.is_empty() {
        let created_at = created_at_unix.unwrap_or_else(current_unix);
        let event = NostrListingEvent::from_listing(
            &created.listing,
            secret_key_hex.as_deref().unwrap_or_default(),
            created_at,
        )
        .map_err(map_listing_error)?;
        let results =
            NostrListingRelayClient::publish_listing_multi(relay, &event, *timeout_ms).await;
        let accepted = results.iter().filter(|r| r.accepted).count();
        publish = Some(json!({
            "event": event,
            "publish_results": results,
            "accepted_relays": accepted,
            "total_relays": relay.len(),
        }));
    }

    persist_wallet_session(&mut session)?;
    Ok(CommandOutput::RawJson(json!({
        "action": "listing_sell",
        "inscription": inscription,
        "ask_sats": amount,
        "fee_rate_sat_vb": fee_rate,
        "seller_outpoint": created.listing.seller_outpoint,
        "passthrough_outpoint": created.listing.passthrough_outpoint,
        "seller_pubkey_hex": created.listing.seller_pubkey_hex,
        "coordinator_pubkey_hex": created.listing.coordinator_pubkey_hex,
        "expires_at_unix": created.listing.expires_at_unix,
        "listing": created.listing,
        "ord_url": ord_url,
        "activation": activation,
        "publish": publish,
    })))
}

async fn handle_purchase(cli: &Cli, action: &ListingAction) -> Result<CommandOutput, AppError> {
    let ListingAction::Purchase {
        listing_json,
        listing_file,
        listing_stdin,
        relay,
        expect_inscription,
        expect_ask_sats,
        limit,
        timeout_ms,
        coordinator_secret_key_hex,
        finalize,
        broadcast,
        listing_out_file,
        psbt_out_file,
        finalized_psbt_out_file,
        tx_hex_out_file,
    } = action
    else {
        return Err(AppError::Internal(
            "handle_purchase called with non-purchase action".to_string(),
        ));
    };

    if *broadcast && !*finalize {
        return Err(AppError::Invalid(
            "--broadcast requires --finalize".to_string(),
        ));
    }
    if *finalize && coordinator_secret_key_hex.is_none() {
        return Err(AppError::Invalid(
            "--finalize requires --coordinator-secret-key-hex".to_string(),
        ));
    }

    let source_count = u8::from(listing_json.is_some())
        + u8::from(listing_file.is_some())
        + u8::from(*listing_stdin);
    if source_count > 0 && !relay.is_empty() {
        return Err(AppError::Invalid(
            "listing purchase accepts either a listing source or --relay discovery, not both"
                .to_string(),
        ));
    }
    if source_count == 0 && relay.is_empty() {
        return Err(AppError::Invalid(
            "listing purchase requires a listing source or at least one --relay".to_string(),
        ));
    }
    if source_count == 0 && expect_inscription.is_none() {
        return Err(AppError::Invalid(
            "--expect-inscription is required when purchasing from relay discovery".to_string(),
        ));
    }

    let listing = if source_count > 0 {
        let source = resolve_listing_source(
            listing_json.as_deref(),
            listing_file.as_deref(),
            *listing_stdin,
        )?;
        serde_json::from_str::<ListingEnvelopeV1>(&source)
            .map_err(|e| AppError::Invalid(format!("invalid listing json: {e}")))?
    } else {
        let options = ListingRelayQueryOptions {
            limit: *limit,
            timeout_ms: *timeout_ms,
        };
        let events = NostrListingRelayClient::discover_listing_events_multi(relay, options).await;
        events
            .iter()
            .filter_map(|event| event.decode_listing().ok())
            .find(|listing| {
                expect_inscription
                    .as_deref()
                    .map(|expected| listing.inscription_id == expected)
                    .unwrap_or(true)
                    && expect_ask_sats
                        .map(|expected| listing.ask_sats == expected)
                        .unwrap_or(true)
            })
            .ok_or_else(|| {
                AppError::Invalid(
                    "no discovered listing matched the supplied expectations".to_string(),
                )
            })?
    };

    assert_listing_expectations(&listing, expect_inscription.as_deref(), *expect_ask_sats)?;

    let mut session = load_wallet_session(cli)?;
    session.require_seed_mode()?;
    assert_listing_network_matches_profile(&listing, session.profile.network)?;
    let purchased = session
        .wallet
        .create_listing_purchase(&CreateListingPurchaseRequest {
            listing: listing.clone(),
            now_unix: i64::try_from(current_unix()).unwrap_or(i64::MAX),
        })
        .map_err(map_listing_error)?;

    if let Some(path) = psbt_out_file {
        maybe_write_text(Some(&path.display().to_string()), &purchased.psbt_base64)?;
    }

    let mut latest_listing = purchased.listing.clone();
    let mut coordinator = None;
    if let Some(secret) = coordinator_secret_key_hex {
        latest_listing.sale_psbt_base64 = sign_listing_coordinator_psbt(
            &latest_listing,
            secret,
            i64::try_from(current_unix()).unwrap_or(i64::MAX),
        )
        .map_err(map_listing_error)?;
        coordinator = Some(json!({
            "signed": true,
            "psbt": latest_listing.sale_psbt_base64,
        }));
    }

    let mut finalized_value = None;
    if *finalize {
        let finalized = finalize_listing_sale(
            &latest_listing,
            i64::try_from(current_unix()).unwrap_or(i64::MAX),
        )
        .map_err(map_listing_error)?;
        if let Some(path) = finalized_psbt_out_file {
            maybe_write_text(
                Some(&path.display().to_string()),
                &finalized.finalized_psbt_base64,
            )?;
        }
        if let Some(path) = tx_hex_out_file {
            maybe_write_text(Some(&path.display().to_string()), &finalized.tx_hex)?;
        }
        let broadcast_txid = if *broadcast {
            let esplora_url = session.profile.esplora_url.clone();
            let psbt = finalized.finalized_psbt_base64.clone();
            Some(
                with_network_retry(
                    cli,
                    "listing purchase broadcast",
                    &mut session.wallet,
                    |wallet| {
                        let url = esplora_url.clone();
                        let psbt = psbt.clone();
                        Box::pin(async move {
                            wallet
                                .broadcast(&psbt, &url)
                                .await
                                .map_err(map_wallet_error)
                        })
                    },
                )
                .await?,
            )
        } else {
            None
        };
        finalized_value = Some(json!({
            "txid": broadcast_txid.unwrap_or_else(|| finalized.txid.clone()),
            "broadcast": broadcast,
            "finalized_psbt": finalized.finalized_psbt_base64,
            "tx_hex": finalized.tx_hex,
            "seller_input_index": finalized.seller_input_index,
            "passthrough_witness_items": finalized.passthrough_witness_items,
        }));
    }

    if let Some(path) = listing_out_file {
        let listing_json = serde_json::to_string_pretty(&latest_listing)
            .map_err(|e| AppError::Internal(format!("failed to serialize listing: {e}")))?;
        maybe_write_text(Some(&path.display().to_string()), &listing_json)?;
    }

    persist_wallet_session(&mut session)?;
    Ok(CommandOutput::RawJson(json!({
        "action": "listing_purchase",
        "inscription": latest_listing.inscription_id,
        "ask_sats": latest_listing.ask_sats,
        "fee_sats": purchased.fee_sats,
        "seller_input_index": purchased.seller_input_index,
        "buyer_input_count": purchased.buyer_input_count,
        "buyer_receive_output_index": purchased.buyer_receive_output_index,
        "psbt": purchased.psbt_base64,
        "listing": latest_listing,
        "coordinator": coordinator,
        "finalized": finalized_value,
    })))
}

fn listing_create_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingCreate {
        inscription: response
            .get("inscription_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ask_sats: response
            .get("ask_sats")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        fee_rate_sat_vb: response
            .get("fee_rate_sat_vb")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        seller_outpoint: response
            .get("seller_outpoint")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        passthrough_outpoint: response
            .get("passthrough_outpoint")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        seller_pubkey_hex: response
            .get("seller_pubkey_hex")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        coordinator_pubkey_hex: response
            .get("coordinator_pubkey_hex")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        expires_at_unix: response
            .get("expires_at_unix")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        raw_response: response,
    })
}

fn listing_activate_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingActivate {
        inscription: response
            .get("inscription_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        txid: response
            .get("txid")
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string(),
        dry_run: response
            .get("dry_run")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        inscription_risk: response
            .get("inscription_risk")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        raw_response: response,
    })
}

fn listing_publish_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingPublish {
        event_id: response
            .get("event")
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        accepted_relays: response
            .get("accepted_relays")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_relays: response
            .get("total_relays")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        publish_results: response
            .get("publish_results")
            .and_then(Value::as_array)
            .unwrap_or(&vec![])
            .clone(),
        raw_response: response,
    })
}

fn listing_discover_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingDiscover {
        event_count: response
            .get("event_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        listing_count: response
            .get("listing_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        listings: response
            .get("listings")
            .and_then(Value::as_array)
            .unwrap_or(&vec![])
            .clone(),
        raw_response: response,
    })
}

fn listing_buy_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingBuy {
        inscription: response
            .get("inscription_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ask_sats: response
            .get("ask_sats")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        fee_sats: response
            .get("fee_sats")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        buyer_input_count: response
            .get("buyer_input_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        raw_response: response,
    })
}

fn listing_coordinator_sign_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingCoordinatorSign {
        inscription: response
            .get("inscription_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ask_sats: response
            .get("ask_sats")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        raw_response: response,
    })
}

fn listing_finalize_output(response: Value) -> Result<CommandOutput, AppError> {
    Ok(CommandOutput::ListingFinalize {
        inscription: response
            .get("inscription_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ask_sats: response
            .get("ask_sats")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        txid: response
            .get("txid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        broadcast: response
            .get("broadcast")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        raw_response: response,
    })
}

fn action_reads_listing_stdin(action: &ListingAction) -> bool {
    match action {
        ListingAction::Activate { listing_stdin, .. }
        | ListingAction::Publish { listing_stdin, .. }
        | ListingAction::Buy { listing_stdin, .. }
        | ListingAction::CoordinatorSign { listing_stdin, .. }
        | ListingAction::Finalize { listing_stdin, .. }
        | ListingAction::Purchase { listing_stdin, .. } => *listing_stdin,
        ListingAction::Sell { .. }
        | ListingAction::Create { .. }
        | ListingAction::Discover { .. } => false,
    }
}

fn resolve_ord_url(cli: &Cli) -> Result<String, AppError> {
    cli.ord_url.clone().ok_or_else(|| {
        AppError::Config(
            "ord url is not configured; pass --ord-url or run setup/config".to_string(),
        )
    })
}

pub fn resolve_listing_source(
    listing_json: Option<&str>,
    listing_file: Option<&Path>,
    listing_stdin: bool,
) -> Result<String, AppError> {
    let count = u8::from(listing_json.is_some())
        + u8::from(listing_file.is_some())
        + u8::from(listing_stdin);
    if count > 1 {
        return Err(AppError::Invalid(
            "accepts only one of --listing-json, --listing-file, --listing-stdin".to_string(),
        ));
    }
    if let Some(source) = listing_json {
        return Ok(source.to_string());
    }
    if let Some(path) = listing_file {
        return std::fs::read_to_string(path).map_err(|e| {
            AppError::Io(format!(
                "failed to read listing file {}: {e}",
                path.display()
            ))
        });
    }
    if listing_stdin {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| AppError::Io(format!("failed to read listing json from stdin: {e}")))?;
        if source.trim().is_empty() {
            return Err(AppError::Invalid("listing stdin was empty".to_string()));
        }
        return Ok(source);
    }

    Err(AppError::Invalid(
        "requires one of --listing-json, --listing-file, --listing-stdin".to_string(),
    ))
}

pub fn assert_listing_expectations(
    listing: &ListingEnvelopeV1,
    expect_inscription: Option<&str>,
    expect_ask_sats: Option<u64>,
) -> Result<(), AppError> {
    if let Some(expected) = expect_inscription {
        if listing.inscription_id != expected {
            return Err(AppError::Invalid(format!(
                "listing inscription_id mismatch: expected {}, got {}",
                expected, listing.inscription_id
            )));
        }
    }
    if let Some(expected) = expect_ask_sats {
        if listing.ask_sats != expected {
            return Err(AppError::Invalid(format!(
                "listing ask_sats mismatch: expected {}, got {}",
                expected, listing.ask_sats
            )));
        }
    }
    Ok(())
}

fn assert_listing_network_matches_profile(
    listing: &ListingEnvelopeV1,
    network: NetworkArg,
) -> Result<(), AppError> {
    let profile_network = network_label(network);
    let lower_listing_network = listing.network.trim().to_ascii_lowercase();
    let matches = lower_listing_network == profile_network
        || (profile_network == "bitcoin" && lower_listing_network == "mainnet");
    if !matches {
        return Err(AppError::Invalid(format!(
            "listing network mismatch: listing={}, profile={}",
            listing.network, profile_network
        )));
    }
    Ok(())
}

fn address_to_script_pubkey(
    address: &str,
    network: NetworkArg,
) -> Result<bitcoin::ScriptBuf, AppError> {
    let parsed = Address::from_str(address)
        .map_err(|e| AppError::Invalid(format!("invalid address {address}: {e}")))?;
    let checked = parsed
        .require_network(bitcoin_network(network))
        .map_err(|e| AppError::Invalid(format!("address network mismatch for {address}: {e}")))?;
    Ok(checked.script_pubkey())
}

fn bitcoin_network(network: NetworkArg) -> bitcoin::Network {
    match network {
        NetworkArg::Bitcoin => bitcoin::Network::Bitcoin,
        NetworkArg::Signet => bitcoin::Network::Signet,
        NetworkArg::Testnet => bitcoin::Network::Testnet,
        NetworkArg::Regtest => bitcoin::Network::Regtest,
    }
}

fn network_label(network: NetworkArg) -> &'static str {
    match network {
        NetworkArg::Bitcoin => "bitcoin",
        NetworkArg::Signet => "signet",
        NetworkArg::Testnet => "testnet",
        NetworkArg::Regtest => "regtest",
    }
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn current_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub fn map_listing_error<E: ToString>(err: E) -> AppError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("policy") || lower.contains("safety lock") || lower.contains("security") {
        return AppError::Policy(message);
    }
    if lower.contains("network")
        || lower.contains("request")
        || lower.contains("relay")
        || lower.contains("connect")
        || lower.contains("timed out")
        || lower.contains("status")
    {
        return AppError::Network(message);
    }
    if lower.contains("invalid") || lower.contains("missing") || lower.contains("mismatch") {
        return AppError::Invalid(message);
    }
    AppError::Internal(message)
}

#[cfg(test)]
mod tests {
    use super::{
        assert_listing_expectations, map_listing_error, resolve_listing_source, ListingEnvelopeV1,
    };
    use crate::error::AppError;
    use std::path::Path;

    fn sample_listing() -> ListingEnvelopeV1 {
        ListingEnvelopeV1 {
            version: 1,
            seller_pubkey_hex: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            coordinator_pubkey_hex:
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
            network: "regtest".to_string(),
            inscription_id: "inscription-123".to_string(),
            seller_outpoint: "6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799:0"
                .to_string(),
            passthrough_outpoint:
                "25f976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799:0".to_string(),
            seller_payout_script_pubkey_hex: "0014751e76e8199196d454941c45d1b3a323f1433bd6"
                .to_string(),
            ask_sats: 42_000,
            postage_sats: 10_000,
            fee_rate_sat_vb: 1,
            tx1_base64: "cHNidP8BAAoCAAAAAQAAAAA=".to_string(),
            sale_psbt_base64: "cHNidP8BAAoCAAAAAQAAAAA=".to_string(),
            recovery_psbt_base64: "cHNidP8BAAoCAAAAAQAAAAA=".to_string(),
            created_at_unix: 1_710_000_000,
            expires_at_unix: 1_710_086_400,
            nonce: 1,
        }
    }

    #[test]
    fn resolve_listing_source_prefers_inline_json() {
        let source = resolve_listing_source(Some("{\"version\":1}"), None, false).expect("source");
        assert_eq!(source, "{\"version\":1}");
    }

    #[test]
    fn resolve_listing_source_rejects_multiple_sources() {
        let err = resolve_listing_source(Some("{}"), Some(Path::new("/tmp/listing.json")), false)
            .expect_err("must reject");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn resolve_listing_source_requires_one_source() {
        let err = resolve_listing_source(None, None, false).expect_err("must reject");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn assert_listing_expectations_accepts_matching_values() {
        let listing = sample_listing();
        assert_listing_expectations(&listing, Some("inscription-123"), Some(42_000))
            .expect("matching values");
    }

    #[test]
    fn assert_listing_expectations_rejects_mismatches() {
        let listing = sample_listing();
        let err = assert_listing_expectations(&listing, Some("other"), Some(42_000))
            .expect_err("inscription mismatch");
        assert!(matches!(err, AppError::Invalid(_)));

        let err = assert_listing_expectations(&listing, Some("inscription-123"), Some(1))
            .expect_err("ask mismatch");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn map_listing_error_classifies_network_and_policy_errors() {
        assert!(matches!(
            map_listing_error("relay timed out"),
            AppError::Network(_)
        ));
        assert!(matches!(
            map_listing_error("policy blocked"),
            AppError::Policy(_)
        ));
        assert!(matches!(
            map_listing_error("invalid listing"),
            AppError::Invalid(_)
        ));
    }
}
