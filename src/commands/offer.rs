use crate::cli::{Cli, OfferAction, OfferArgs, ThumbMode};
use crate::commands::psbt::{analyze_psbt_with_policy, enforce_policy_mode};
use crate::config::NetworkArg;
use crate::error::AppError;
use crate::network_retry::with_network_retry;
use crate::presenter::thumbnail::{render_non_image_badge, render_thumbnail_from_bytes};
use crate::utils::{maybe_write_text, resolve_psbt_source};
use crate::wallet_service::map_wallet_error;
use crate::{load_wallet_session, persist_wallet_session};
use crate::output::CommandOutput;
use serde_json::{json, Value};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use zinc_core::{
    prepare_offer_acceptance, CreateOfferRequest, NostrOfferEvent, NostrRelayClient,
    OfferEnvelopeV1, OrdClient, RelayQueryOptions, SignOptions,
};

pub async fn run(cli: &Cli, args: &OfferArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        OfferAction::Create {
            inscription,
            amount,
            fee_rate,
            expires_in_secs,
            created_at_unix,
            nonce,
            publisher_pubkey_hex,
            seller_payout_address,
            submit_ord,
            offer_out_file,
            psbt_out_file,
        } => {
            let mut session = load_wallet_session(cli)?;
            if session
                .wallet
                .inscriptions()
                .iter()
                .any(|ins| ins.id == *inscription)
            {
                return Err(AppError::Invalid(format!(
                    "inscription {} already in wallet",
                    inscription
                )));
            }

            let ord_url = resolve_ord_url(cli)?;
            let client = OrdClient::new(ord_url.clone());
            let inscription_details = client
                .get_inscription_details(inscription)
                .await
                .map_err(map_offer_error)?;
            let output_details = client
                .get_output_details(&inscription_details.satpoint.outpoint)
                .await
                .map_err(map_offer_error)?;

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

            let postage_sats = inscription_details.value.ok_or_else(|| {
                AppError::Invalid(format!("inscription {} is unbound", inscription))
            })?;
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

            let request = CreateOfferRequest {
                inscription_id: inscription.clone(),
                seller_outpoint: inscription_details.satpoint.outpoint,
                seller_input_address: output_details.address.clone(),
                seller_payout_address: seller_payout_address
                    .clone()
                    .unwrap_or_else(|| output_details.address.clone()),
                seller_output_value_sats: postage_sats,
                ask_sats: *amount,
                fee_rate_sat_vb: *fee_rate,
                created_at_unix: created_unix,
                expires_at_unix: expires_unix,
                nonce: nonce.unwrap_or_else(current_nanos),
                publisher_pubkey_hex: publisher_pubkey_hex.clone(),
            };
            let created = session
                .wallet
                .create_offer(&request)
                .map_err(map_offer_error)?;

            if let Some(path) = psbt_out_file {
                maybe_write_text(Some(&path.display().to_string()), &created.psbt)?;
            }
            if let Some(path) = offer_out_file {
                let offer_json = serde_json::to_string_pretty(&created.offer)
                    .map_err(|e| AppError::Internal(format!("failed to serialize offer: {e}")))?;
                maybe_write_text(Some(&path.display().to_string()), &offer_json)?;
            }

            if *submit_ord {
                client
                    .submit_offer_psbt(&created.psbt)
                    .await
                    .map_err(map_offer_error)?;
            }

            persist_wallet_session(&mut session)?;
            let response = json!({
                "inscription": created.inscription,
                "seller_address": created.seller_address,
                "seller_outpoint": created.seller_outpoint,
                "postage_sats": created.postage_sats,
                "ask_sats": created.ask_sats,
                "fee_rate_sat_vb": created.fee_rate_sat_vb,
                "seller_input_index": created.seller_input_index,
                "buyer_input_count": created.buyer_input_count,
                "psbt": created.psbt,
                "offer": created.offer,
                "submitted_ord": submit_ord,
                "ord_url": ord_url,
            });
            finalize_offer_output(cli, &args.action, response).await
        }
        OfferAction::Publish {
            offer_json,
            offer_file,
            offer_stdin,
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

            let source = resolve_offer_source(
                offer_json.as_deref(),
                offer_file.as_ref().map(|p| p.to_str().unwrap()),
                *offer_stdin,
            )?;
            let offer: OfferEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid offer json: {e}")))?;

            let created_at = created_at_unix.unwrap_or_else(current_unix);
            let event = NostrOfferEvent::from_offer(&offer, secret_key_hex, created_at)
                .map_err(map_offer_error)?;

            let results = NostrRelayClient::publish_offer_multi(relay, &event, *timeout_ms).await;
            let accepted = results.iter().filter(|r| r.accepted).count();

            let response = json!({
                "event": event,
                "publish_results": results,
                "accepted_relays": accepted,
                "total_relays": relay.len(),
            });
            finalize_offer_output(cli, &args.action, response).await
        }
        OfferAction::Discover {
            relay,
            limit,
            timeout_ms,
        } => {
            if relay.is_empty() {
                return Err(AppError::Invalid(
                    "at least one --relay is required".to_string(),
                ));
            }
            let options = RelayQueryOptions {
                limit: *limit,
                timeout_ms: *timeout_ms,
            };
            let events = NostrRelayClient::discover_offer_events_multi(relay, options).await;
            let offers: Vec<Value> = events
                .iter()
                .filter_map(|event| {
                    event.decode_offer().ok().map(|offer| {
                        json!({
                            "event_id": event.id,
                            "pubkey": event.pubkey,
                            "created_at": event.created_at,
                            "offer": offer
                        })
                    })
                })
                .collect();

            let response = json!({
                "events": events,
                "offers": offers,
                "event_count": events.len(),
                "offer_count": offers.len(),
            });
            finalize_offer_output(cli, &args.action, response).await
        }
        OfferAction::SubmitOrd {
            psbt,
            psbt_file,
            psbt_stdin,
        } => {
            let psbt_source = resolve_psbt_source(
                psbt.as_deref(),
                psbt_file.as_ref().map(|p| p.to_str().unwrap()),
                *psbt_stdin,
            )?;
            let ord_url = resolve_ord_url(cli)?;
            let client = OrdClient::new(ord_url.clone());
            client
                .submit_offer_psbt(&psbt_source)
                .await
                .map_err(map_offer_error)?;

            let response = json!({
                "submitted": true,
                "ord_url": ord_url,
            });
            finalize_offer_output(cli, &args.action, response).await
        }
        OfferAction::ListOrd => {
            let ord_url = resolve_ord_url(cli)?;
            let client = OrdClient::new(ord_url.clone());
            let offers = client.get_offer_psbts().await.map_err(map_offer_error)?;
            let response = json!({
                "ord_url": ord_url,
                "offers": offers,
                "count": offers.len(),
            });
            finalize_offer_output(cli, &args.action, response).await
        }
        OfferAction::Accept {
            offer_json,
            offer_file,
            offer_stdin,
            expect_inscription,
            expect_ask_sats,
            dry_run,
        } => {
            let source = resolve_offer_source(
                offer_json.as_deref(),
                offer_file.as_ref().map(|p| p.to_str().unwrap()),
                *offer_stdin,
            )?;
            let offer: OfferEnvelopeV1 = serde_json::from_str(&source)
                .map_err(|e| AppError::Invalid(format!("invalid offer json: {e}")))?;
            assert_offer_expectations(&offer, expect_inscription.as_deref(), *expect_ask_sats)?;

            let mut session = load_wallet_session(cli)?;
            assert_offer_network_matches_profile(&offer, session.profile.network)?;
            let now_unix = i64::try_from(current_unix()).unwrap_or(i64::MAX);
            let plan = prepare_offer_acceptance(&offer, now_unix).map_err(map_offer_error)?;
            let (analysis, policy) = analyze_psbt_with_policy(&session.wallet, &offer.psbt_base64)?;
            enforce_policy_mode(cli, &policy)?;

            // Attempt local signing on the seller input to ensure this wallet can accept
            // the offer before optionally broadcasting it.
            let signed = session
                .wallet
                .sign_psbt(
                    &offer.psbt_base64,
                    Some(SignOptions {
                        sign_inputs: Some(vec![plan.seller_input_index]),
                        sighash: None,
                        finalize: !*dry_run,
                    }),
                )
                .map_err(map_wallet_error)?;

            if *dry_run {
                let response = json!({
                    "accepted": true,
                    "dry_run": true,
                    "offer_id": plan.offer_id,
                    "seller_input_index": plan.seller_input_index,
                    "input_count": plan.input_count,
                    "inscription_id": offer.inscription_id,
                    "ask_sats": offer.ask_sats,
                    "safe_to_send": policy.safe_to_send,
                    "inscription_risk": policy.inscription_risk,
                    "policy_reasons": policy.policy_reasons,
                    "analysis": analysis
                });
                return finalize_offer_output(cli, &args.action, response).await;
            }

            let esplora_url = session.profile.esplora_url.clone();
            let txid: String = with_network_retry(
                cli,
                "offer accept broadcast",
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

            let response = json!({
                "accepted": true,
                "dry_run": false,
                "offer_id": plan.offer_id,
                "seller_input_index": plan.seller_input_index,
                "input_count": plan.input_count,
                "inscription_id": offer.inscription_id,
                "ask_sats": offer.ask_sats,
                "txid": txid,
                "safe_to_send": policy.safe_to_send,
                "inscription_risk": policy.inscription_risk,
                "policy_reasons": policy.policy_reasons,
                "analysis": analysis
            });
            finalize_offer_output(cli, &args.action, response).await
        }
    }
}

async fn finalize_offer_output(
    cli: &Cli,
    action: &OfferAction,
    response: Value,
) -> Result<CommandOutput, AppError> {
    let thumbnail_lines = maybe_offer_thumbnail_lines(cli, action, &response).await;
    let hide_inscription_ids = cli.thumb != ThumbMode::None;

    match action {
        OfferAction::Create { inscription, .. } => {
            Ok(CommandOutput::OfferCreate {
                inscription: inscription.clone(),
                ask_sats: response.get("ask_sats").and_then(Value::as_u64).unwrap_or(0),
                fee_rate_sat_vb: response.get("fee_rate_sat_vb").and_then(Value::as_u64).unwrap_or(0),
                seller_address: response.get("seller_address").and_then(Value::as_str).unwrap_or("").to_string(),
                seller_outpoint: response.get("seller_outpoint").and_then(Value::as_str).unwrap_or("").to_string(),
                seller_pubkey_hex: response.get("offer").and_then(|o| o.get("seller_pubkey_hex")).and_then(Value::as_str).unwrap_or("").to_string(),
                expires_at_unix: response.get("offer").and_then(|o| o.get("expires_at_unix")).and_then(Value::as_i64).unwrap_or(0),
                thumbnail_lines,
                hide_inscription_ids,
                raw_response: response,
            })
        }
        OfferAction::Publish { .. } => {
            Ok(CommandOutput::OfferPublish {
                event_id: response.get("event").and_then(|v| v.get("id")).and_then(Value::as_str).unwrap_or("").to_string(),
                accepted_relays: response.get("accepted_relays").and_then(Value::as_u64).unwrap_or(0),
                total_relays: response.get("total_relays").and_then(Value::as_u64).unwrap_or(0),
                publish_results: response.get("publish_results").and_then(Value::as_array).unwrap_or(&vec![]).clone(),
                raw_response: response,
            })
        }
        OfferAction::Discover { .. } => {
            Ok(CommandOutput::OfferDiscover {
                event_count: response.get("event_count").and_then(Value::as_u64).unwrap_or(0),
                offer_count: response.get("offer_count").and_then(Value::as_u64).unwrap_or(0),
                offers: response.get("offers").and_then(Value::as_array).unwrap_or(&vec![]).clone(),
                thumbnail_lines,
                hide_inscription_ids,
                raw_response: response,
            })
        }
        OfferAction::SubmitOrd { .. } => {
            Ok(CommandOutput::OfferSubmitOrd {
                ord_url: response.get("ord_url").and_then(Value::as_str).unwrap_or("").to_string(),
                submitted: true,
                raw_response: response,
            })
        }
        OfferAction::ListOrd => {
            Ok(CommandOutput::OfferListOrd {
                ord_url: response.get("ord_url").and_then(Value::as_str).unwrap_or("").to_string(),
                count: response.get("count").and_then(Value::as_u64).unwrap_or(0),
                offers: response.get("offers").and_then(Value::as_array).unwrap_or(&vec![]).clone(),
                raw_response: response,
            })
        }
        OfferAction::Accept { .. } => {
            Ok(CommandOutput::OfferAccept {
                inscription: response.get("inscription_id").and_then(Value::as_str).unwrap_or("").to_string(),
                ask_sats: response.get("ask_sats").and_then(Value::as_u64).unwrap_or(0),
                txid: response.get("txid").and_then(Value::as_str).unwrap_or("-").to_string(),
                dry_run: response.get("dry_run").and_then(Value::as_bool).unwrap_or(false),
                inscription_risk: response.get("inscription_risk").and_then(Value::as_str).unwrap_or("").to_string(),
                thumbnail_lines,
                hide_inscription_ids,
                raw_response: response,
            })
        }
    }
}

async fn maybe_offer_thumbnail_lines(
    cli: &Cli,
    action: &OfferAction,
    response: &Value,
) -> Option<Vec<String>> {
    if cli.thumb == ThumbMode::None {
        return None;
    }

    let inscription_id = offer_thumbnail_inscription_id(action, response)?;
    let ord_url = resolve_ord_url(cli)
        .ok()
        .or_else(|| response.get("ord_url").and_then(Value::as_str).map(ToString::to_string))?;

    let client = OrdClient::new(ord_url);
    let details = client.get_inscription_details(&inscription_id).await.ok()?;
    let content_type = details.content_type.clone();
    if !content_type
        .as_deref()
        .is_some_and(|kind| kind.starts_with("image/"))
    {
        let mut badge = vec![format!(
            "thumbnail ({}):",
            abbreviate(&inscription_id, 12, 8)
        )];
        badge.extend(render_non_image_badge(content_type.as_deref()));
        return Some(badge);
    }

    let content = client.get_inscription_content(&inscription_id).await.ok()?;
    let rendered = render_thumbnail_from_bytes(&content.bytes, cli.thumb, 48).ok()?;
    let mut lines = vec![format!(
        "thumbnail ({}):",
        abbreviate(&inscription_id, 12, 8)
    )];
    lines.extend(rendered);
    Some(lines)
}

fn offer_thumbnail_inscription_id(action: &OfferAction, response: &Value) -> Option<String> {
    match action {
        OfferAction::Create { inscription, .. } => Some(inscription.clone()),
        OfferAction::Accept { .. } => response
            .get("inscription_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        OfferAction::Discover { .. } => response
            .get("offers")
            .and_then(Value::as_array)
            .and_then(|offers| offers.first())
            .and_then(|entry| entry.get("offer"))
            .and_then(|offer| offer.get("inscription_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        OfferAction::Publish { .. } | OfferAction::SubmitOrd { .. } | OfferAction::ListOrd => None,
    }
}



pub fn abbreviate(value: &str, prefix: usize, suffix: usize) -> String {
    if value.chars().count() <= prefix + suffix + 3 {
        return value.to_string();
    }
    let start: String = value.chars().take(prefix).collect();
    let end: String = value
        .chars()
        .rev()
        .take(suffix)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{start}...{end}")
}

fn resolve_ord_url(cli: &Cli) -> Result<String, AppError> {
    cli.ord_url.clone().ok_or_else(|| {
        AppError::Config(
            "ord url is not configured; pass --ord-url or run setup/config".to_string(),
        )
    })
}

fn resolve_offer_source(
    offer_json: Option<&str>,
    offer_file: Option<&str>,
    offer_stdin: bool,
) -> Result<String, AppError> {
    let count = (offer_json.is_some() as u8) + (offer_file.is_some() as u8) + (offer_stdin as u8);
    if count > 1 {
        return Err(AppError::Invalid(
            "accepts only one of --offer-json, --offer-file, --offer-stdin".to_string(),
        ));
    }
    if let Some(source) = offer_json {
        return Ok(source.to_string());
    }
    if let Some(path) = offer_file {
        return std::fs::read_to_string(path)
            .map_err(|e| AppError::Io(format!("failed to read offer file {path}: {e}")));
    }
    if offer_stdin {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| AppError::Io(format!("failed to read offer json from stdin: {e}")))?;
        if source.trim().is_empty() {
            return Err(AppError::Invalid("offer stdin was empty".to_string()));
        }
        return Ok(source);
    }

    Err(AppError::Invalid(
        "requires one of --offer-json, --offer-file, --offer-stdin".to_string(),
    ))
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

fn assert_offer_expectations(
    offer: &OfferEnvelopeV1,
    expect_inscription: Option<&str>,
    expect_ask_sats: Option<u64>,
) -> Result<(), AppError> {
    if let Some(expected) = expect_inscription {
        if offer.inscription_id != expected {
            return Err(AppError::Invalid(format!(
                "offer inscription_id mismatch: expected {}, got {}",
                expected, offer.inscription_id
            )));
        }
    }
    if let Some(expected) = expect_ask_sats {
        if offer.ask_sats != expected {
            return Err(AppError::Invalid(format!(
                "offer ask_sats mismatch: expected {}, got {}",
                expected, offer.ask_sats
            )));
        }
    }
    Ok(())
}

fn assert_offer_network_matches_profile(
    offer: &OfferEnvelopeV1,
    network: NetworkArg,
) -> Result<(), AppError> {
    let profile_network = match network {
        NetworkArg::Bitcoin => "bitcoin",
        NetworkArg::Signet => "signet",
        NetworkArg::Testnet => "testnet",
        NetworkArg::Regtest => "regtest",
    };
    let lower_offer_network = offer.network.trim().to_ascii_lowercase();

    let matches = lower_offer_network == profile_network
        || (profile_network == "bitcoin" && lower_offer_network == "mainnet");
    if !matches {
        return Err(AppError::Invalid(format!(
            "offer network mismatch: offer={}, profile={}",
            offer.network, profile_network
        )));
    }
    Ok(())
}

fn map_offer_error<E: ToString>(err: E) -> AppError {
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
    if lower.contains("invalid") || lower.contains("missing") {
        return AppError::Invalid(message);
    }
    AppError::Internal(message)
}

#[cfg(test)]
mod tests {
    use super::{
        abbreviate, assert_offer_expectations, map_offer_error, resolve_offer_source,
    };
    use crate::error::AppError;
    use zinc_core::OfferEnvelopeV1;

    fn sample_offer() -> OfferEnvelopeV1 {
        OfferEnvelopeV1 {
            version: 1,
            seller_pubkey_hex: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            network: "regtest".to_string(),
            inscription_id: "inscription-123".to_string(),
            seller_outpoint: "6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799:0"
                .to_string(),
            ask_sats: 42_000,
            fee_rate_sat_vb: 1,
            psbt_base64: "cHNidP8BAAoCAAAAAQAAAAA=".to_string(),
            created_at_unix: 1_710_000_000,
            expires_at_unix: 1_710_086_400,
            nonce: 1,
        }
    }

    #[test]
    fn resolve_offer_source_prefers_inline_json() {
        let source = resolve_offer_source(Some("{\"version\":1}"), None, false).expect("source");
        assert_eq!(source, "{\"version\":1}");
    }

    #[test]
    fn resolve_offer_source_rejects_multiple_sources() {
        let err = resolve_offer_source(Some("{}"), Some("/tmp/offer.json"), false)
            .expect_err("must reject");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn resolve_offer_source_requires_one_source() {
        let err = resolve_offer_source(None, None, false).expect_err("must reject");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn map_offer_error_classifies_network_issues() {
        let err = map_offer_error("relay timed out while publishing");
        assert!(matches!(err, AppError::Network(_)));
    }

    #[test]
    fn map_offer_error_classifies_policy_issues() {
        let err = map_offer_error("Policy error: safety lock engaged");
        assert!(matches!(err, AppError::Policy(_)));
    }

    #[test]
    fn assert_offer_expectations_rejects_inscription_mismatch() {
        let offer = sample_offer();
        let err = assert_offer_expectations(&offer, Some("wrong-inscription"), Some(42_000))
            .expect_err("must reject mismatch");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn assert_offer_expectations_rejects_ask_mismatch() {
        let offer = sample_offer();
        let err = assert_offer_expectations(&offer, Some("inscription-123"), Some(43_000))
            .expect_err("must reject mismatch");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn abbreviate_shortens_long_identifiers() {
        let value = "1234567890abcdef1234567890abcdef";
        let short = abbreviate(value, 6, 4);
        assert_eq!(short, "123456...cdef");
    }


}
