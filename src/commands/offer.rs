use crate::cli::{Cli, OfferAction, OfferArgs};
use crate::error::AppError;
use crate::utils::resolve_psbt_source;
use serde_json::{json, Value};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use zinc_core::{
    NostrOfferEvent, NostrRelayClient, OfferEnvelopeV1, OrdClient, RelayQueryOptions,
};

pub async fn run(cli: &Cli, args: &OfferArgs) -> Result<Value, AppError> {
    match &args.action {
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

            Ok(json!({
                "event": event,
                "publish_results": results,
                "accepted_relays": accepted,
                "total_relays": relay.len(),
            }))
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

            Ok(json!({
                "events": events,
                "offers": offers,
                "event_count": events.len(),
                "offer_count": offers.len(),
            }))
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

            Ok(json!({
                "submitted": true,
                "ord_url": ord_url,
            }))
        }
        OfferAction::ListOrd => {
            let ord_url = resolve_ord_url(cli)?;
            let client = OrdClient::new(ord_url.clone());
            let offers = client.get_offer_psbts().await.map_err(map_offer_error)?;
            Ok(json!({
                "ord_url": ord_url,
                "offers": offers,
                "count": offers.len(),
            }))
        }
    }
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

fn map_offer_error<E: ToString>(err: E) -> AppError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
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
    use super::{map_offer_error, resolve_offer_source};
    use crate::error::AppError;

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
}
