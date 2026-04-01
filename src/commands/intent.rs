use crate::cli::{Cli, IntentAction, IntentArgs, IntentPairAction, IntentPairArgs};
use crate::error::AppError;
use crate::output::{CommandOutput, IntentPairLinkEntry};
use crate::wallet_password;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::future::Future;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use zinc_core::{
    build_signed_pairing_complete_receipt, decode_pairing_ack_envelope_event_with_secret,
    decode_signed_sign_intent_receipt_event_with_secret, decrypt_secret_internal,
    encrypt_pairing_transport_content, encrypt_secret_internal, generate_secret_key_hex,
    pairing_tag_hash_hex, pairing_transport_tags, pubkey_hex_from_secret_key,
    verify_pairing_approval, BuildBuyerOfferIntentV1, CapabilityPolicyV1, NostrTransportEventV1,
    PairingAckDecisionV1, PairingAckV1, PairingLinkApprovalV1, PairingRequestV1,
    SignIntentActionV1, SignIntentPayloadV1, SignIntentReceiptStatusV1, SignIntentReceiptV1,
    SignIntentV1, SignedPairingAckV1, SignedPairingCompleteReceiptV1, SignedPairingRequestV1,
    SignedSignIntentReceiptV1, SignedSignIntentV1, NOSTR_PAIRING_ACK_TYPE_TAG_VALUE,
    NOSTR_PAIRING_COMPLETE_RECEIPT_TYPE_TAG_VALUE, NOSTR_SIGN_INTENT_APP_TAG_VALUE,
    NOSTR_SIGN_INTENT_RECEIPT_TYPE_TAG_VALUE, NOSTR_SIGN_INTENT_TYPE_TAG_VALUE, NOSTR_TAG_APP_KEY,
    NOSTR_TAG_PAIRING_HASH_KEY, NOSTR_TAG_RECIPIENT_PUBKEY_KEY, NOSTR_TAG_TYPE_KEY,
    PAIRING_TRANSPORT_EVENT_KIND,
};

const DEFAULT_AGENT_SECRET_KEY_HEX: &str =
    "0001020304050607080900010203040506070809000102030405060708090001";
const DEFAULT_WALLET_SECRET_KEY_HEX: &str =
    "0102030405060708090001020304050607080900010203040506070809000102";
const INTENT_AGENT_KEY_STORE_SCHEMA_VERSION: &str = "intent-agent-key-v1";
const DEFAULT_PAIR_EXPIRES_IN_SECS: u64 = 600;
const MAX_PAIRING_URI_CHARS: usize = 8 * 1024;
const MAX_PAIRING_REQUEST_JSON_BYTES: usize = 16 * 1024;
const MAX_PAIRING_ACK_JSON_BYTES: usize = 16 * 1024;
const PAIRING_ACK_CODE_PREFIX: &str = "zincack1_";
const DEFAULT_PAIRING_RELAY_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_PAIRING_RELAY_LIMIT: usize = 32;
const DEFAULT_PAIR_START_AWAIT_ACK_WINDOW_MS: u64 = 30_000;
const DEFAULT_PAIR_START_AWAIT_ACK_POLL_MS: u64 = 900;
const DEFAULT_SIGN_INTENT_EXPIRES_IN_SECS: u64 = 180;
const DEFAULT_WAIT_RECEIPT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_WAIT_RECEIPT_POLL_MS: u64 = 1_000;
const DEFAULT_PAIRING_RELAYS: &[&str] = &["wss://relay.damus.io", "wss://nos.lol"];
const REGTEST_DEFAULT_PAIRING_RELAYS: &[&str] = &["wss://nostr-regtest.exittheloop.com"];
const LINK_STATUS_ACTIVE: &str = "active";
const LINK_STATUS_PAUSED: &str = "paused";
const LINK_STATUS_REVOKED: &str = "revoked";
const LINK_STATUS_ROTATED: &str = "rotated";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentFixtureBundleV1 {
    schema_version: String,
    signed_pairing_request: SignedPairingRequestV1,
    signed_pairing_ack: SignedPairingAckV1,
    signed_sign_intent: SignedSignIntentV1,
    signed_sign_intent_receipt: SignedSignIntentReceiptV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentFixtureEnvelopeV1 {
    fixtures: IntentFixtureBundleV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentLinkStoreV1 {
    schema_version: String,
    links: Vec<LinkedWalletV1>,
}

impl Default for IntentLinkStoreV1 {
    fn default() -> Self {
        Self {
            schema_version: "intent-link-store-v1".to_string(),
            links: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkedWalletV1 {
    pairing_id: String,
    agent_pubkey_hex: String,
    wallet_pubkey_hex: String,
    granted_capabilities: CapabilityPolicyV1,
    request_expires_at_unix: i64,
    ack_expires_at_unix: i64,
    linked_at_unix: i64,
    #[serde(default = "default_link_status")]
    status: String,
    #[serde(default)]
    status_updated_at_unix: Option<i64>,
    #[serde(default)]
    paused_at_unix: Option<i64>,
    #[serde(default)]
    revoked_at_unix: Option<i64>,
    #[serde(default)]
    rotated_by_pairing_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentAgentKeyStoreV1 {
    schema_version: String,
    agent_pubkey_hex: String,
    encrypted_agent_secret: String,
    created_at_unix: i64,
    updated_at_unix: i64,
}

fn default_link_status() -> String {
    LINK_STATUS_ACTIVE.to_string()
}

pub async fn run(cli: &Cli, args: &IntentArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        IntentAction::Pair(pair_args) => run_pair_action(cli, &pair_args.action),
        IntentAction::Send {
            pairing_id,
            payload_json,
            now_unix,
            expires_in_secs,
            nonce,
            agent_secret_key_hex,
            relay,
            network,
        } => run_intent_send(
            cli,
            pairing_id,
            payload_json,
            *now_unix,
            *expires_in_secs,
            *nonce,
            agent_secret_key_hex.as_deref(),
            relay,
            network,
        ),
        IntentAction::WaitReceipt {
            pairing_id,
            intent_id,
            timeout_ms,
            agent_secret_key_hex,
            relay,
        } => run_intent_wait_receipt(
            cli,
            pairing_id,
            intent_id,
            *timeout_ms,
            agent_secret_key_hex.as_deref(),
            relay,
        ),
        IntentAction::FixtureGenerate {
            now_unix,
            agent_secret_key_hex,
            wallet_secret_key_hex,
        } => {
            let now_unix = i64::try_from(now_unix.unwrap_or_else(current_unix))
                .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;
            let agent_secret = agent_secret_key_hex
                .as_deref()
                .unwrap_or(DEFAULT_AGENT_SECRET_KEY_HEX);
            let wallet_secret = wallet_secret_key_hex
                .as_deref()
                .unwrap_or(DEFAULT_WALLET_SECRET_KEY_HEX);

            let bundle = build_fixture_bundle(now_unix, agent_secret, wallet_secret)?;
            let pairing_id = bundle
                .signed_pairing_request
                .pairing_id_hex()
                .map_err(map_intent_error)?;
            let intent_id = bundle
                .signed_sign_intent
                .intent_id_hex()
                .map_err(map_intent_error)?;

            if cli.agent {
                Ok(CommandOutput::Generic(json!({
                    "schemaVersion": bundle.schema_version,
                    "pairingId": pairing_id,
                    "intentId": intent_id,
                    "signedPairingRequest": bundle.signed_pairing_request,
                    "signedPairingAck": bundle.signed_pairing_ack,
                    "signedSignIntent": bundle.signed_sign_intent,
                    "signedSignIntentReceipt": bundle.signed_sign_intent_receipt
                })))
            } else {
                Ok(CommandOutput::IntentFixtureGenerate {
                    schema_version: bundle.schema_version,
                    pairing_id,
                    intent_id,
                    signed_pairing_request: to_json_value(
                        &bundle.signed_pairing_request,
                        "signed pairing request",
                    )?,
                    signed_pairing_ack: to_json_value(
                        &bundle.signed_pairing_ack,
                        "signed pairing ack",
                    )?,
                    signed_sign_intent: to_json_value(
                        &bundle.signed_sign_intent,
                        "signed sign intent",
                    )?,
                    signed_sign_intent_receipt: to_json_value(
                        &bundle.signed_sign_intent_receipt,
                        "signed sign intent receipt",
                    )?,
                })
            }
        }
        IntentAction::FixtureVerify {
            fixture_json,
            fixture_file,
            fixture_stdin,
        } => {
            let source = resolve_fixture_source(
                fixture_json.as_deref(),
                fixture_file.as_deref(),
                *fixture_stdin,
            )?;
            let bundle = parse_fixture_bundle(&source)?;

            let pairing_id = bundle
                .signed_pairing_request
                .pairing_id_hex()
                .map_err(map_intent_error)?;
            let intent_id = bundle
                .signed_sign_intent
                .intent_id_hex()
                .map_err(map_intent_error)?;

            bundle
                .signed_pairing_request
                .verify()
                .map_err(map_intent_error)?;
            bundle
                .signed_pairing_ack
                .verify()
                .map_err(map_intent_error)?;
            bundle
                .signed_sign_intent
                .verify()
                .map_err(map_intent_error)?;
            bundle
                .signed_sign_intent_receipt
                .verify()
                .map_err(map_intent_error)?;

            if bundle.signed_pairing_ack.ack.pairing_id != pairing_id {
                return Err(AppError::Invalid(
                    "pairing ack pairing_id does not match signed pairing request".to_string(),
                ));
            }
            if bundle.signed_sign_intent.intent.pairing_id != pairing_id {
                return Err(AppError::Invalid(
                    "sign intent pairing_id does not match signed pairing request".to_string(),
                ));
            }
            if bundle.signed_sign_intent_receipt.receipt.intent_id != intent_id {
                return Err(AppError::Invalid(
                    "sign intent receipt intent_id does not match signed sign intent".to_string(),
                ));
            }

            if cli.agent {
                Ok(CommandOutput::Generic(json!({
                    "schemaVersion": bundle.schema_version,
                    "valid": true,
                    "pairingId": pairing_id,
                    "intentId": intent_id
                })))
            } else {
                Ok(CommandOutput::IntentFixtureVerify {
                    schema_version: bundle.schema_version,
                    valid: true,
                    pairing_id,
                    intent_id,
                })
            }
        }
    }
}

pub async fn run_pair(cli: &Cli, args: &IntentPairArgs) -> Result<CommandOutput, AppError> {
    run_pair_action(cli, &args.action)
}

fn run_pair_action(cli: &Cli, action: &IntentPairAction) -> Result<CommandOutput, AppError> {
    match action {
        IntentPairAction::Start {
            now_unix,
            expires_in_secs,
            agent_secret_key_hex,
            relay,
            network,
            show_json,
            no_wait,
        } => run_pair_start(
            cli,
            *now_unix,
            *expires_in_secs,
            agent_secret_key_hex.as_deref(),
            relay,
            network,
            *show_json,
            *no_wait,
        ),
        IntentPairAction::Finish {
            now_unix,
            request_json,
            request_file,
            agent_secret_key_hex,
            ack_json,
            ack_file,
            ack_code,
        } => run_pair_finish(
            cli,
            *now_unix,
            request_json.as_deref(),
            request_file.as_deref(),
            agent_secret_key_hex.as_deref(),
            ack_json.as_deref(),
            ack_file.as_deref(),
            ack_code.as_deref(),
        ),
        IntentPairAction::List {} => run_pair_list(cli),
        IntentPairAction::Show { pairing_id } => run_pair_show(cli, pairing_id),
        IntentPairAction::Pause {
            pairing_id,
            now_unix,
        } => run_pair_set_status(cli, pairing_id, LINK_STATUS_PAUSED, *now_unix),
        IntentPairAction::Resume {
            pairing_id,
            now_unix,
        } => run_pair_set_status(cli, pairing_id, LINK_STATUS_ACTIVE, *now_unix),
        IntentPairAction::Revoke {
            pairing_id,
            now_unix,
        } => run_pair_set_status(cli, pairing_id, LINK_STATUS_REVOKED, *now_unix),
    }
}

fn run_intent_send(
    cli: &Cli,
    pairing_id_selector: &str,
    payload_json: &str,
    now_unix: Option<u64>,
    expires_in_secs: Option<u64>,
    nonce: Option<u64>,
    agent_secret_key_hex: Option<&str>,
    relay_urls: &[String],
    network: &str,
) -> Result<CommandOutput, AppError> {
    let now_unix = i64::try_from(now_unix.unwrap_or_else(current_unix))
        .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;
    let expires_delta =
        i64::try_from(expires_in_secs.unwrap_or(DEFAULT_SIGN_INTENT_EXPIRES_IN_SECS))
            .map_err(|_| AppError::Invalid("expires_in_secs is out of range".to_string()))?;
    if expires_delta <= 0 {
        return Err(AppError::Invalid(
            "expires_in_secs must be greater than zero".to_string(),
        ));
    }

    let links_path = intent_links_path(cli)?;
    let store = load_link_store(&links_path)?;
    let link_index = find_link_index(&store.links, pairing_id_selector)?;
    let link = store
        .links
        .get(link_index)
        .ok_or_else(|| AppError::NotFound("linked agent not found".to_string()))?;
    ensure_link_allows_intents(link)?;

    let payload: SignIntentPayloadV1 = serde_json::from_str(payload_json)
        .map_err(|e| AppError::Invalid(format!("invalid sign intent payload json: {e}")))?;
    enforce_payload_against_capabilities(&payload, &link.granted_capabilities, network)?;

    let agent_secret_key_hex = resolve_agent_secret_key_hex(
        cli,
        agent_secret_key_hex,
        Some(&link.agent_pubkey_hex),
        false,
    )?;
    let relays = normalize_pairing_relays(relay_urls, network_hint_from_link(link));
    let intent = SignIntentV1 {
        version: 1,
        pairing_id: link.pairing_id.clone(),
        agent_pubkey_hex: link.agent_pubkey_hex.clone(),
        wallet_pubkey_hex: link.wallet_pubkey_hex.clone(),
        network: network.trim().to_string(),
        created_at_unix: now_unix,
        expires_at_unix: now_unix + expires_delta,
        nonce: nonce.unwrap_or_else(current_unix),
        payload,
    };
    let signed_intent =
        SignedSignIntentV1::new(intent, &agent_secret_key_hex).map_err(map_intent_error)?;
    let intent_id = signed_intent.intent_id_hex().map_err(map_intent_error)?;
    let signed_intent_json = serde_json::to_string(&signed_intent)
        .map_err(|e| AppError::Internal(format!("failed to serialize signed sign intent: {e}")))?;
    let tags = pairing_transport_tags(
        NOSTR_SIGN_INTENT_TYPE_TAG_VALUE,
        &link.pairing_id,
        &link.wallet_pubkey_hex,
    )
    .map_err(map_intent_error)?;
    let content = encrypt_pairing_transport_content(
        &signed_intent_json,
        &agent_secret_key_hex,
        &link.wallet_pubkey_hex,
    )
    .map_err(map_intent_error)?;
    let event = NostrTransportEventV1::new(
        PAIRING_TRANSPORT_EVENT_KIND,
        tags,
        content,
        u64::try_from(now_unix).unwrap_or_else(|_| current_unix()),
        &agent_secret_key_hex,
    )
    .map_err(map_intent_error)?;

    let stats = run_async_with_runtime(publish_transport_event_multi_stats(
        &relays,
        &event,
        DEFAULT_PAIRING_RELAY_TIMEOUT_MS,
    ))?;
    if stats.accepted == 0 {
        return Err(AppError::Network(
            "failed to publish sign intent to any relay".to_string(),
        ));
    }

    if cli.agent {
        return Ok(CommandOutput::Generic(json!({
            "schemaVersion": "sign-intent-send-v1",
            "pairingId": link.pairing_id,
            "intentId": intent_id,
            "eventId": event.id,
            "acceptedRelays": stats.accepted,
            "totalRelays": stats.total,
            "signedSignIntent": signed_intent
        })));
    }

    Ok(CommandOutput::IntentSend {
        pairing_id: link.pairing_id.clone(),
        fingerprint: pairing_fingerprint(&link.pairing_id),
        intent_id,
        action: format!("{:?}", signed_intent.intent.payload.action()),
        accepted_relays: u64::try_from(stats.accepted)
            .map_err(|_| AppError::Internal("accepted relay count is out of range".to_string()))?,
        total_relays: u64::try_from(stats.total)
            .map_err(|_| AppError::Internal("total relay count is out of range".to_string()))?,
    })
}

fn run_intent_wait_receipt(
    cli: &Cli,
    pairing_id_selector: &str,
    intent_id: &str,
    timeout_ms: u64,
    agent_secret_key_hex: Option<&str>,
    relay_urls: &[String],
) -> Result<CommandOutput, AppError> {
    let timeout_ms = if timeout_ms == 0 {
        DEFAULT_WAIT_RECEIPT_TIMEOUT_MS
    } else {
        timeout_ms
    };
    let links_path = intent_links_path(cli)?;
    let store = load_link_store(&links_path)?;
    let link_index = find_link_index(&store.links, pairing_id_selector)?;
    let link = store
        .links
        .get(link_index)
        .ok_or_else(|| AppError::NotFound("linked agent not found".to_string()))?;
    let agent_secret_key_hex = resolve_agent_secret_key_hex(
        cli,
        agent_secret_key_hex,
        Some(&link.agent_pubkey_hex),
        false,
    )?;
    let relays = normalize_pairing_relays(relay_urls, network_hint_from_link(link));
    let signed_receipt = wait_for_sign_intent_receipt_from_relays(
        &relays,
        &link.pairing_id,
        intent_id,
        &link.agent_pubkey_hex,
        &agent_secret_key_hex,
        timeout_ms,
    )?
    .ok_or_else(|| {
        AppError::Invalid(format!(
            "no sign intent receipt found for intent_id `{intent_id}` before timeout"
        ))
    })?;

    if !signed_receipt
        .receipt
        .pairing_id
        .eq_ignore_ascii_case(&link.pairing_id)
    {
        return Err(AppError::Invalid(
            "sign intent receipt pairing_id did not match linked pairing".to_string(),
        ));
    }
    if !signed_receipt
        .receipt
        .signer_pubkey_hex
        .eq_ignore_ascii_case(&link.wallet_pubkey_hex)
    {
        return Err(AppError::Invalid(
            "sign intent receipt signer did not match linked wallet".to_string(),
        ));
    }

    let receipt_id = signed_receipt.receipt_id_hex().map_err(map_intent_error)?;
    if cli.agent {
        return Ok(CommandOutput::Generic(json!({
            "schemaVersion": "sign-intent-receipt-v1",
            "pairingId": link.pairing_id,
            "intentId": signed_receipt.receipt.intent_id,
            "receiptId": receipt_id,
            "status": format!("{:?}", signed_receipt.receipt.status),
            "signedSignIntentReceipt": signed_receipt,
        })));
    }

    Ok(CommandOutput::IntentWaitReceipt {
        pairing_id: link.pairing_id.clone(),
        fingerprint: pairing_fingerprint(&link.pairing_id),
        intent_id: signed_receipt.receipt.intent_id.clone(),
        receipt_id,
        status: format!("{:?}", signed_receipt.receipt.status),
        signer_pubkey_hex: signed_receipt.receipt.signer_pubkey_hex.clone(),
        error_message: signed_receipt.receipt.error_message.clone(),
    })
}

fn run_pair_start(
    cli: &Cli,
    now_unix: Option<u64>,
    expires_in_secs: Option<u64>,
    agent_secret_key_hex: Option<&str>,
    relay_urls: &[String],
    network: &str,
    show_json: bool,
    no_wait: bool,
) -> Result<CommandOutput, AppError> {
    let now_unix = i64::try_from(now_unix.unwrap_or_else(current_unix))
        .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;
    let expires_in_secs = expires_in_secs.unwrap_or(DEFAULT_PAIR_EXPIRES_IN_SECS);
    let expires_delta = i64::try_from(expires_in_secs)
        .map_err(|_| AppError::Invalid("expires_in_secs is out of range".to_string()))?;
    if expires_delta <= 0 {
        return Err(AppError::Invalid(
            "expires_in_secs must be greater than zero".to_string(),
        ));
    }

    let agent_secret_key_hex = resolve_agent_secret_key_hex(cli, agent_secret_key_hex, None, true)?;
    let agent_pubkey_hex =
        pubkey_hex_from_secret_key(&agent_secret_key_hex).map_err(map_intent_error)?;

    let requested_capabilities = default_pairing_capabilities(network);
    let relays = normalize_pairing_relays(relay_urls, network);
    let request = PairingRequestV1 {
        version: 1,
        agent_pubkey_hex: agent_pubkey_hex.clone(),
        challenge_nonce: format!("pair-{}-{}", now_unix, std::process::id()),
        created_at_unix: now_unix,
        expires_at_unix: now_unix + expires_delta,
        relays,
        requested_capabilities,
    };
    let signed_request =
        SignedPairingRequestV1::new(request, &agent_secret_key_hex).map_err(map_intent_error)?;
    let pairing_id = signed_request.pairing_id_hex().map_err(map_intent_error)?;

    let request_json = serde_json::to_string(&signed_request).map_err(|e| {
        AppError::Internal(format!("failed to serialize signed pairing request: {e}"))
    })?;
    let pairing_uri = pairing_uri_from_request_json(&request_json)?;
    let request_path = pairing_request_path(cli, &pairing_id)?;
    crate::write_bytes_atomic(
        &request_path,
        request_json.as_bytes(),
        "pairing request payload",
    )?;
    let latest_request_path = latest_pairing_request_path(cli)?;
    crate::write_bytes_atomic(
        &latest_request_path,
        request_json.as_bytes(),
        "latest pairing request payload",
    )?;

    let links_path = intent_links_path(cli)?;

    if cli.agent {
        return Ok(CommandOutput::Generic(json!({
            "schemaVersion": "pairing-request-v1",
            "pairingId": pairing_id,
            "agentPubkeyHex": agent_pubkey_hex,
            "fingerprint": pairing_fingerprint(&pairing_id),
            "pairingUri": pairing_uri,
            "signedPairingRequest": signed_request,
            "pairingRequestJson": request_json,
            "requestPath": request_path.display().to_string(),
            "linksPath": links_path.display().to_string()
        })));
    }

    let await_ack = !no_wait;
    if await_ack {
        use crate::output::Presenter;
        let start_output = CommandOutput::IntentPairStart {
            schema_version: "pairing-request-v1".to_string(),
            pairing_id: pairing_id.clone(),
            agent_pubkey_hex: agent_pubkey_hex.clone(),
            fingerprint: pairing_fingerprint(
                &signed_request.pairing_id_hex().map_err(map_intent_error)?,
            ),
            pairing_uri: pairing_uri.clone(),
            signed_pairing_request: to_json_value(&signed_request, "signed pairing request")?,
            pairing_request_json: if show_json {
                Some(request_json.clone())
            } else {
                None
            },
            request_path: request_path.display().to_string(),
            links_path: links_path.display().to_string(),
            await_ack,
        };
        let preview = crate::output::HumanPresenter::new(true).render(&start_output);
        let _ = std::io::stdout().write_all(preview.as_bytes());
        let _ = std::io::stdout().flush();

        if let Some(signed_ack) = wait_for_pairing_ack_from_relays(
            &signed_request,
            &agent_secret_key_hex,
            DEFAULT_PAIR_START_AWAIT_ACK_WINDOW_MS,
        ) {
            let finish_now_unix = i64::try_from(current_unix())
                .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;
            return finalize_pairing_link(
                cli,
                &signed_request,
                &signed_ack,
                &agent_secret_key_hex,
                finish_now_unix,
                "relay".to_string(),
            );
        }
        return Ok(CommandOutput::IntentPairAwaitTimeout {
            pairing_id,
            fingerprint: pairing_fingerprint(
                &signed_request.pairing_id_hex().map_err(map_intent_error)?,
            ),
            request_path: request_path.display().to_string(),
            links_path: links_path.display().to_string(),
        });
    }

    Ok(CommandOutput::IntentPairStart {
        schema_version: "pairing-request-v1".to_string(),
        pairing_id,
        agent_pubkey_hex,
        fingerprint: pairing_fingerprint(
            &signed_request.pairing_id_hex().map_err(map_intent_error)?,
        ),
        pairing_uri,
        signed_pairing_request: to_json_value(&signed_request, "signed pairing request")?,
        pairing_request_json: if show_json { Some(request_json) } else { None },
        request_path: request_path.display().to_string(),
        links_path: links_path.display().to_string(),
        await_ack,
    })
}

fn run_pair_finish(
    cli: &Cli,
    now_unix: Option<u64>,
    request_json: Option<&str>,
    request_file: Option<&Path>,
    agent_secret_key_hex: Option<&str>,
    ack_json: Option<&str>,
    ack_file: Option<&Path>,
    ack_code: Option<&str>,
) -> Result<CommandOutput, AppError> {
    let now_unix = i64::try_from(now_unix.unwrap_or_else(current_unix))
        .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;

    let request_source = resolve_pairing_request_source(cli, request_json, request_file)?;
    let request_source = if request_source.trim_start().starts_with("zinc://") {
        pairing_request_json_from_uri(&request_source)?
    } else {
        request_source
    };
    let signed_request: SignedPairingRequestV1 = serde_json::from_str(&request_source)
        .map_err(|e| AppError::Invalid(format!("invalid signed pairing request json: {e}")))?;
    let resolved_ack_source = resolve_pairing_ack_source_optional(ack_json, ack_file, ack_code)?;
    let agent_secret_key_hex = resolve_agent_secret_key_hex(
        cli,
        agent_secret_key_hex,
        Some(&signed_request.request.agent_pubkey_hex),
        false,
    )?;
    let relay_ack =
        match fetch_signed_pairing_ack_from_relays(&signed_request, &agent_secret_key_hex) {
            Ok(found) => found,
            Err(_) if resolved_ack_source.is_some() => None,
            Err(err) => return Err(err),
        };
    let (signed_ack, ack_source_label) = if let Some(signed_ack) = relay_ack {
        (signed_ack, "relay".to_string())
    } else if let Some(ack_source) = resolved_ack_source {
        let signed_ack: SignedPairingAckV1 = serde_json::from_str(&ack_source)
            .map_err(|e| AppError::Invalid(format!("invalid signed pairing ack json: {e}")))?;
        let source_label = if ack_code.is_some() {
            "ack-code".to_string()
        } else if ack_json.is_some() {
            "ack-json".to_string()
        } else {
            "ack-file".to_string()
        };
        (signed_ack, source_label)
    } else {
        return Err(AppError::Invalid(
            "no relay ack found and no ack source provided; provide --ack-code, --ack-json, or --ack-file"
                .to_string(),
        ));
    };
    finalize_pairing_link(
        cli,
        &signed_request,
        &signed_ack,
        &agent_secret_key_hex,
        now_unix,
        ack_source_label,
    )
}

fn finalize_pairing_link(
    cli: &Cli,
    signed_request: &SignedPairingRequestV1,
    signed_ack: &SignedPairingAckV1,
    agent_secret_key_hex: &str,
    now_unix: i64,
    ack_source_label: String,
) -> Result<CommandOutput, AppError> {
    let approval =
        verify_pairing_approval(signed_request, signed_ack, now_unix).map_err(map_intent_error)?;
    let pairing_id = approval.pairing_id.clone();

    let links_path = intent_links_path(cli)?;
    let mut store = load_link_store(&links_path)?;
    upsert_link(&mut store, build_link_from_approval(&approval, now_unix));
    save_link_store(&links_path, &store)?;

    let completion_receipt_published = publish_pairing_complete_receipt_best_effort(
        signed_request,
        signed_ack,
        agent_secret_key_hex,
        now_unix,
    );

    if cli.agent {
        Ok(CommandOutput::Generic(json!({
            "paired": true,
            "pairingId": pairing_id,
            "fingerprint": pairing_fingerprint(&approval.pairing_id),
            "agentPubkeyHex": approval.agent_pubkey_hex,
            "walletPubkeyHex": approval.wallet_pubkey_hex,
            "grantedCapabilities": approval.granted_capabilities,
            "linkedAtUnix": now_unix,
            "ackSource": ack_source_label,
            "completionReceiptPublished": completion_receipt_published,
            "linksPath": links_path.display().to_string()
        })))
    } else {
        Ok(CommandOutput::IntentPairFinish {
            paired: true,
            pairing_id,
            fingerprint: pairing_fingerprint(&approval.pairing_id),
            agent_pubkey_hex: approval.agent_pubkey_hex,
            wallet_pubkey_hex: approval.wallet_pubkey_hex,
            granted_capabilities: to_json_value(
                &approval.granted_capabilities,
                "granted capabilities",
            )?,
            linked_at_unix: now_unix,
            ack_source: ack_source_label,
            completion_receipt_published,
            links_path: links_path.display().to_string(),
        })
    }
}

fn run_pair_list(cli: &Cli) -> Result<CommandOutput, AppError> {
    let links_path = intent_links_path(cli)?;
    let store = load_link_store(&links_path)?;
    let entries: Vec<IntentPairLinkEntry> = store.links.iter().map(link_to_entry).collect();

    let active = entries
        .iter()
        .filter(|entry| entry.status == LINK_STATUS_ACTIVE)
        .count();
    let paused = entries
        .iter()
        .filter(|entry| entry.status == LINK_STATUS_PAUSED)
        .count();
    let revoked = entries
        .iter()
        .filter(|entry| entry.status == LINK_STATUS_REVOKED)
        .count();
    let rotated = entries
        .iter()
        .filter(|entry| entry.status == LINK_STATUS_ROTATED)
        .count();

    if cli.agent {
        Ok(CommandOutput::Generic(json!({
            "schemaVersion": "intent-links-v1",
            "linksPath": links_path.display().to_string(),
            "total": entries.len(),
            "statusCounts": {
                "active": active,
                "paused": paused,
                "revoked": revoked,
                "rotated": rotated
            },
            "links": entries
        })))
    } else {
        Ok(CommandOutput::IntentPairList {
            links_path: links_path.display().to_string(),
            total: entries.len(),
            active,
            paused,
            revoked,
            rotated,
            links: entries,
        })
    }
}

fn run_pair_show(cli: &Cli, pairing_id_selector: &str) -> Result<CommandOutput, AppError> {
    let links_path = intent_links_path(cli)?;
    let store = load_link_store(&links_path)?;
    let index = find_link_index(&store.links, pairing_id_selector)?;
    let entry = link_to_entry(&store.links[index]);

    if cli.agent {
        Ok(CommandOutput::Generic(json!({
            "schemaVersion": "intent-link-v1",
            "linksPath": links_path.display().to_string(),
            "link": entry
        })))
    } else {
        Ok(CommandOutput::IntentPairShow {
            links_path: links_path.display().to_string(),
            link: entry,
        })
    }
}

fn run_pair_set_status(
    cli: &Cli,
    pairing_id_selector: &str,
    requested_status: &str,
    now_unix: Option<u64>,
) -> Result<CommandOutput, AppError> {
    let now_unix = i64::try_from(now_unix.unwrap_or_else(current_unix))
        .map_err(|_| AppError::Invalid("now_unix is out of range".to_string()))?;
    let target_status = normalize_link_status(requested_status).to_string();

    let links_path = intent_links_path(cli)?;
    let mut store = load_link_store(&links_path)?;
    let index = find_link_index(&store.links, pairing_id_selector)?;
    let link = store
        .links
        .get_mut(index)
        .ok_or_else(|| AppError::Internal("link index out of bounds".to_string()))?;

    let current_status = normalize_link_status(&link.status).to_string();
    validate_status_transition(&current_status, &target_status)?;

    link.status = target_status.clone();
    link.status_updated_at_unix = Some(now_unix);
    if target_status == LINK_STATUS_PAUSED {
        link.paused_at_unix = Some(now_unix);
    }
    if target_status == LINK_STATUS_ACTIVE {
        link.paused_at_unix = None;
    }
    if target_status == LINK_STATUS_REVOKED {
        link.revoked_at_unix = Some(now_unix);
    }

    let updated_entry = link_to_entry(link);
    save_link_store(&links_path, &store)?;

    if cli.agent {
        Ok(CommandOutput::Generic(json!({
            "schemaVersion": "intent-link-status-v1",
            "updated": true,
            "linksPath": links_path.display().to_string(),
            "link": updated_entry
        })))
    } else {
        Ok(CommandOutput::IntentPairStatusUpdate {
            links_path: links_path.display().to_string(),
            pairing_id: updated_entry.pairing_id.clone(),
            fingerprint: updated_entry.fingerprint.clone(),
            status: updated_entry.status.clone(),
            updated_at_unix: now_unix,
        })
    }
}

fn link_to_entry(link: &LinkedWalletV1) -> IntentPairLinkEntry {
    IntentPairLinkEntry {
        pairing_id: link.pairing_id.clone(),
        fingerprint: pairing_fingerprint(&link.pairing_id),
        agent_pubkey_hex: link.agent_pubkey_hex.clone(),
        wallet_pubkey_hex: link.wallet_pubkey_hex.clone(),
        status: normalize_link_status(&link.status).to_string(),
        send_allowed: normalize_link_status(&link.status) == LINK_STATUS_ACTIVE,
        linked_at_unix: link.linked_at_unix,
        status_updated_at_unix: link.status_updated_at_unix,
        request_expires_at_unix: link.request_expires_at_unix,
        ack_expires_at_unix: link.ack_expires_at_unix,
        granted_capabilities: serde_json::to_value(&link.granted_capabilities)
            .unwrap_or_else(|_| json!({})),
    }
}

fn normalize_link_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        LINK_STATUS_ACTIVE => LINK_STATUS_ACTIVE,
        LINK_STATUS_PAUSED => LINK_STATUS_PAUSED,
        LINK_STATUS_REVOKED => LINK_STATUS_REVOKED,
        LINK_STATUS_ROTATED => LINK_STATUS_ROTATED,
        _ => LINK_STATUS_PAUSED,
    }
}

fn validate_status_transition(current: &str, next: &str) -> Result<(), AppError> {
    if current == next {
        return Ok(());
    }

    match (current, next) {
        (LINK_STATUS_ACTIVE, LINK_STATUS_PAUSED)
        | (LINK_STATUS_ACTIVE, LINK_STATUS_REVOKED)
        | (LINK_STATUS_PAUSED, LINK_STATUS_ACTIVE)
        | (LINK_STATUS_PAUSED, LINK_STATUS_REVOKED)
        | (LINK_STATUS_ROTATED, LINK_STATUS_REVOKED) => Ok(()),
        (LINK_STATUS_REVOKED, _) => Err(AppError::Invalid(
            "revoked links cannot change state".to_string(),
        )),
        _ => Err(AppError::Invalid(format!(
            "invalid status transition from `{current}` to `{next}`"
        ))),
    }
}

fn ensure_link_allows_intents(link: &LinkedWalletV1) -> Result<(), AppError> {
    match normalize_link_status(&link.status) {
        LINK_STATUS_ACTIVE => Ok(()),
        LINK_STATUS_PAUSED => Err(AppError::Policy(
            "linked agent is paused; resume it before sending intents".to_string(),
        )),
        LINK_STATUS_REVOKED => Err(AppError::Policy(
            "linked agent is revoked; create a new pairing before sending intents".to_string(),
        )),
        LINK_STATUS_ROTATED => Err(AppError::Policy(
            "linked agent was rotated by a newer pairing; use the latest active pairing"
                .to_string(),
        )),
        _ => Err(AppError::Policy(
            "linked agent is not in an active state".to_string(),
        )),
    }
}

fn find_link_index(links: &[LinkedWalletV1], pairing_id_selector: &str) -> Result<usize, AppError> {
    let selector = pairing_id_selector.trim().to_ascii_lowercase();
    if selector.is_empty() {
        return Err(AppError::Invalid(
            "--pairing-id must be a non-empty pairing id or prefix".to_string(),
        ));
    }

    if let Some((index, _)) = links
        .iter()
        .enumerate()
        .find(|(_, link)| link.pairing_id.eq_ignore_ascii_case(&selector))
    {
        return Ok(index);
    }

    let mut matches = links
        .iter()
        .enumerate()
        .filter(|(_, link)| link.pairing_id.to_ascii_lowercase().starts_with(&selector))
        .map(|(index, _)| index);

    let first = matches.next().ok_or_else(|| {
        AppError::NotFound(format!(
            "no link found for pairing id selector `{pairing_id_selector}`"
        ))
    })?;
    if matches.next().is_some() {
        return Err(AppError::Invalid(format!(
            "pairing id selector `{pairing_id_selector}` is ambiguous; provide more characters"
        )));
    }
    Ok(first)
}

fn to_json_value<T: Serialize>(value: &T, label: &str) -> Result<Value, AppError> {
    serde_json::to_value(value)
        .map_err(|e| AppError::Internal(format!("failed to serialize {label}: {e}")))
}

fn parse_fixture_bundle(source: &str) -> Result<IntentFixtureBundleV1, AppError> {
    match serde_json::from_str::<IntentFixtureBundleV1>(source) {
        Ok(bundle) => Ok(bundle),
        Err(bundle_err) => match serde_json::from_str::<IntentFixtureEnvelopeV1>(source) {
            Ok(envelope) => Ok(envelope.fixtures),
            Err(_) => Err(AppError::Invalid(format!(
                "invalid fixture json: {bundle_err}"
            ))),
        },
    }
}

fn default_pairing_capabilities(network: &str) -> CapabilityPolicyV1 {
    CapabilityPolicyV1 {
        allowed_actions: vec![
            SignIntentActionV1::BuildBuyerOffer,
            SignIntentActionV1::SignSellerInput,
        ],
        max_sats_per_intent: Some(300_000),
        daily_spend_limit_sats: Some(900_000),
        max_fee_rate_sat_vb: Some(20),
        allowed_networks: vec![network.to_string()],
    }
}

fn network_hint_from_link(link: &LinkedWalletV1) -> &str {
    link.granted_capabilities
        .allowed_networks
        .first()
        .map(String::as_str)
        .unwrap_or("mainnet")
}

fn capability_allows_network(capabilities: &CapabilityPolicyV1, network: &str) -> bool {
    capabilities
        .allowed_networks
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(network))
}

fn enforce_payload_against_capabilities(
    payload: &SignIntentPayloadV1,
    capabilities: &CapabilityPolicyV1,
    network: &str,
) -> Result<(), AppError> {
    if !capability_allows_network(capabilities, network) {
        return Err(AppError::Policy(format!(
            "network `{network}` is not allowed by linked-agent policy"
        )));
    }

    let action = payload.action();
    if !capabilities.allowed_actions.contains(&action) {
        return Err(AppError::Policy(format!(
            "action `{action:?}` is not allowed by linked-agent policy"
        )));
    }

    let intent_sats = match payload {
        SignIntentPayloadV1::BuildBuyerOffer(details) => {
            if let Some(max_fee_rate) = capabilities.max_fee_rate_sat_vb {
                if details.fee_rate_sat_vb > max_fee_rate {
                    return Err(AppError::Policy(format!(
                        "requested fee rate {} sat/vB exceeds policy max {} sat/vB",
                        details.fee_rate_sat_vb, max_fee_rate
                    )));
                }
            }
            details.ask_sats
        }
        SignIntentPayloadV1::SignSellerInput(details) => details.expected_ask_sats,
    };

    if let Some(max_sats_per_intent) = capabilities.max_sats_per_intent {
        if intent_sats > max_sats_per_intent {
            return Err(AppError::Policy(format!(
                "intent amount {intent_sats} sats exceeds policy max_sats_per_intent {max_sats_per_intent}"
            )));
        }
    }
    if let Some(daily_spend_limit_sats) = capabilities.daily_spend_limit_sats {
        if intent_sats > daily_spend_limit_sats {
            return Err(AppError::Policy(format!(
                "intent amount {intent_sats} sats exceeds policy daily_spend_limit_sats {daily_spend_limit_sats}"
            )));
        }
    }

    Ok(())
}

fn default_pairing_relays_for_network(network: &str) -> &'static [&'static str] {
    if network.eq_ignore_ascii_case("regtest") {
        REGTEST_DEFAULT_PAIRING_RELAYS
    } else {
        DEFAULT_PAIRING_RELAYS
    }
}

fn canonicalize_relay_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("https://") {
        return format!("wss://{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        return format!("ws://{rest}");
    }
    trimmed.to_string()
}

fn pairing_request_network_hint(signed_request: &SignedPairingRequestV1) -> &str {
    signed_request
        .request
        .requested_capabilities
        .allowed_networks
        .first()
        .map(String::as_str)
        .unwrap_or("mainnet")
}

fn normalize_pairing_relays(relays: &[String], default_network: &str) -> Vec<String> {
    let defaults = default_pairing_relays_for_network(default_network);
    let source: Vec<String> = if relays.is_empty() {
        defaults.iter().map(|relay| relay.to_string()).collect()
    } else {
        relays.to_vec()
    };

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for relay in source {
        let trimmed = relay.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = canonicalize_relay_url(trimmed);
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            deduped.push(normalized);
        }
    }

    if deduped.is_empty() {
        defaults.iter().map(|relay| relay.to_string()).collect()
    } else {
        deduped
    }
}

fn build_fixture_bundle(
    now_unix: i64,
    agent_secret_key_hex: &str,
    wallet_secret_key_hex: &str,
) -> Result<IntentFixtureBundleV1, AppError> {
    let agent_pubkey_hex =
        pubkey_hex_from_secret_key(agent_secret_key_hex).map_err(map_intent_error)?;
    let wallet_pubkey_hex =
        pubkey_hex_from_secret_key(wallet_secret_key_hex).map_err(map_intent_error)?;

    let capabilities = default_pairing_capabilities("regtest");

    let pairing_request = PairingRequestV1 {
        version: 1,
        agent_pubkey_hex: agent_pubkey_hex.clone(),
        challenge_nonce: format!("pairing-{}", now_unix),
        created_at_unix: now_unix,
        expires_at_unix: now_unix + 600,
        relays: vec!["wss://relay.example".to_string()],
        requested_capabilities: capabilities.clone(),
    };
    let signed_pairing_request = SignedPairingRequestV1::new(pairing_request, agent_secret_key_hex)
        .map_err(map_intent_error)?;
    let pairing_id = signed_pairing_request
        .pairing_id_hex()
        .map_err(map_intent_error)?;

    let pairing_ack = PairingAckV1 {
        version: 1,
        pairing_id: pairing_id.clone(),
        challenge_nonce: signed_pairing_request.request.challenge_nonce.clone(),
        agent_pubkey_hex: agent_pubkey_hex.clone(),
        wallet_pubkey_hex: wallet_pubkey_hex.clone(),
        created_at_unix: now_unix + 1,
        expires_at_unix: now_unix + 601,
        decision: PairingAckDecisionV1::Approved,
        granted_capabilities: Some(capabilities),
        rejection_reason: None,
    };
    let signed_pairing_ack =
        SignedPairingAckV1::new(pairing_ack, wallet_secret_key_hex).map_err(map_intent_error)?;

    let sign_intent = SignIntentV1 {
        version: 1,
        pairing_id: pairing_id.clone(),
        agent_pubkey_hex,
        wallet_pubkey_hex: wallet_pubkey_hex.clone(),
        network: "regtest".to_string(),
        created_at_unix: now_unix + 2,
        expires_at_unix: now_unix + 602,
        nonce: 1,
        payload: SignIntentPayloadV1::BuildBuyerOffer(BuildBuyerOfferIntentV1 {
            inscription_id: "inscription-123".to_string(),
            seller_outpoint: "6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799:0"
                .to_string(),
            ask_sats: 100_000,
            fee_rate_sat_vb: 2,
        }),
    };
    let signed_sign_intent =
        SignedSignIntentV1::new(sign_intent, agent_secret_key_hex).map_err(map_intent_error)?;
    let intent_id = signed_sign_intent
        .intent_id_hex()
        .map_err(map_intent_error)?;

    let sign_intent_receipt = SignIntentReceiptV1 {
        version: 1,
        intent_id,
        pairing_id,
        signer_pubkey_hex: wallet_pubkey_hex,
        created_at_unix: now_unix + 3,
        status: SignIntentReceiptStatusV1::Approved,
        signed_psbt_base64: Some(
            "cHNidP8BAHECAAAAAf//////////////////////////////////////////AAAAAAD9////AqCGAQAAAAAAIgAgx0Jv4z2frfr6f3Ff9rR9lSxDgP3UzrA1n6g0bHTqfQAAAAAAAAAA"
                .to_string(),
        ),
        artifact_json: None,
        error_message: None,
    };
    let signed_sign_intent_receipt =
        SignedSignIntentReceiptV1::new(sign_intent_receipt, wallet_secret_key_hex)
            .map_err(map_intent_error)?;

    Ok(IntentFixtureBundleV1 {
        schema_version: "intent-fixture-v1".to_string(),
        signed_pairing_request,
        signed_pairing_ack,
        signed_sign_intent,
        signed_sign_intent_receipt,
    })
}

fn resolve_json_source(
    inline_json: Option<&str>,
    file_path: Option<&Path>,
    label: &str,
    inline_flag: &str,
    file_flag: &str,
) -> Result<String, AppError> {
    match (inline_json, file_path) {
        (Some(_), Some(_)) => Err(AppError::Invalid(format!(
            "provide exactly one {label} source: {inline_flag} or {file_flag}"
        ))),
        (None, None) => Err(AppError::Invalid(format!(
            "missing {label} source: provide {inline_flag} or {file_flag}"
        ))),
        (Some(inline), None) => Ok(inline.to_string()),
        (None, Some(path)) => std::fs::read_to_string(path).map_err(|e| {
            AppError::Io(format!(
                "failed to read {label} file {}: {e}",
                path.display()
            ))
        }),
    }
}

fn resolve_pairing_request_source(
    cli: &Cli,
    request_json: Option<&str>,
    request_file: Option<&Path>,
) -> Result<String, AppError> {
    if request_json.is_some() || request_file.is_some() {
        return resolve_json_source(
            request_json,
            request_file,
            "request",
            "--request-json",
            "--request-file",
        );
    }

    let latest_path = latest_pairing_request_path(cli)?;
    std::fs::read_to_string(&latest_path).map_err(|e| {
        AppError::Invalid(format!(
            "missing request source: provide --request-json/--request-file, or run `pair start` first ({})",
            e
        ))
    })
}

fn resolve_pairing_ack_source(
    ack_json: Option<&str>,
    ack_file: Option<&Path>,
    ack_code: Option<&str>,
) -> Result<String, AppError> {
    let source_count = usize::from(ack_json.is_some())
        + usize::from(ack_file.is_some())
        + usize::from(ack_code.is_some());
    if source_count != 1 {
        return Err(AppError::Invalid(
            "provide exactly one ack source: --ack-code, --ack-json, or --ack-file".to_string(),
        ));
    }

    if let Some(code) = ack_code {
        return pairing_ack_json_from_code(code);
    }
    if let Some(inline_json) = ack_json {
        let trimmed = inline_json.trim();
        if trimmed.starts_with(PAIRING_ACK_CODE_PREFIX) {
            return pairing_ack_json_from_code(trimmed);
        }
        return Ok(inline_json.to_string());
    }
    let path = ack_file.expect("ack_file must be present when source_count == 1");
    let file_contents = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(format!("failed to read ack file {}: {e}", path.display())))?;
    let trimmed = file_contents.trim();
    if trimmed.starts_with(PAIRING_ACK_CODE_PREFIX) {
        return pairing_ack_json_from_code(trimmed);
    }
    Ok(file_contents)
}

fn resolve_pairing_ack_source_optional(
    ack_json: Option<&str>,
    ack_file: Option<&Path>,
    ack_code: Option<&str>,
) -> Result<Option<String>, AppError> {
    let source_count = usize::from(ack_json.is_some())
        + usize::from(ack_file.is_some())
        + usize::from(ack_code.is_some());
    if source_count == 0 {
        return Ok(None);
    }
    resolve_pairing_ack_source(ack_json, ack_file, ack_code).map(Some)
}

fn resolve_fixture_source(
    fixture_json: Option<&str>,
    fixture_file: Option<&Path>,
    fixture_stdin: bool,
) -> Result<String, AppError> {
    let source_count = usize::from(fixture_json.is_some())
        + usize::from(fixture_file.is_some())
        + usize::from(fixture_stdin);

    if source_count != 1 {
        return Err(AppError::Invalid(
            "provide exactly one source: --fixture-json, --fixture-file, or --fixture-stdin"
                .to_string(),
        ));
    }

    if let Some(inline) = fixture_json {
        return Ok(inline.to_string());
    }

    if let Some(path) = fixture_file {
        return std::fs::read_to_string(path).map_err(|e| {
            AppError::Io(format!(
                "failed to read fixture file {}: {e}",
                path.display()
            ))
        });
    }

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| AppError::Io(format!("failed to read fixture json from stdin: {e}")))?;
    Ok(input)
}

fn resolve_agent_secret_key_hex(
    cli: &Cli,
    explicit_secret_key_hex: Option<&str>,
    expected_agent_pubkey_hex: Option<&str>,
    allow_generate_if_missing: bool,
) -> Result<String, AppError> {
    if let Some(secret_key_hex) = explicit_secret_key_hex {
        let trimmed = secret_key_hex.trim();
        if trimmed.is_empty() {
            return Err(AppError::Invalid(
                "--agent-secret-key-hex cannot be empty".to_string(),
            ));
        }
        let derived_pubkey_hex = pubkey_hex_from_secret_key(trimmed).map_err(map_intent_error)?;
        ensure_agent_pubkey_matches_expected(&derived_pubkey_hex, expected_agent_pubkey_hex)?;
        return Ok(trimmed.to_string());
    }

    let store_path = intent_agent_key_store_path(cli)?;
    if let Some(store) = load_agent_key_store(&store_path)? {
        let password = wallet_password(cli)?;
        let decrypted = decrypt_secret_internal(&store.encrypted_agent_secret, &password)
            .map_err(map_secret_decrypt_error)?;
        let secret_key_hex = decrypted.trim();
        if secret_key_hex.is_empty() {
            return Err(AppError::Config(
                "intent agent key store decrypted to an empty secret".to_string(),
            ));
        }
        let derived_pubkey_hex =
            pubkey_hex_from_secret_key(secret_key_hex).map_err(map_intent_error)?;
        if !derived_pubkey_hex.eq_ignore_ascii_case(&store.agent_pubkey_hex) {
            return Err(AppError::Config(
                "intent agent key store failed integrity check (pubkey mismatch)".to_string(),
            ));
        }
        ensure_agent_pubkey_matches_expected(&derived_pubkey_hex, expected_agent_pubkey_hex)?;
        return Ok(secret_key_hex.to_string());
    }

    if !allow_generate_if_missing {
        return Err(AppError::NotFound(
            "intent agent key is not initialized; pass --agent-secret-key-hex or run `pair start` once to bootstrap secure key storage".to_string(),
        ));
    }

    let password = wallet_password(cli)?;
    let generated_secret = generate_secret_key_hex().map_err(map_intent_error)?;
    let generated_pubkey =
        pubkey_hex_from_secret_key(&generated_secret).map_err(map_intent_error)?;
    ensure_agent_pubkey_matches_expected(&generated_pubkey, expected_agent_pubkey_hex)?;

    let encrypted_agent_secret =
        encrypt_secret_internal(&generated_secret, &password).map_err(map_intent_error)?;
    let now_unix = i64::try_from(current_unix())
        .map_err(|_| AppError::Internal("current time is out of range".to_string()))?;
    let key_store = IntentAgentKeyStoreV1 {
        schema_version: INTENT_AGENT_KEY_STORE_SCHEMA_VERSION.to_string(),
        agent_pubkey_hex: generated_pubkey,
        encrypted_agent_secret,
        created_at_unix: now_unix,
        updated_at_unix: now_unix,
    };
    save_agent_key_store(&store_path, &key_store)?;
    Ok(generated_secret)
}

fn ensure_agent_pubkey_matches_expected(
    actual_agent_pubkey_hex: &str,
    expected_agent_pubkey_hex: Option<&str>,
) -> Result<(), AppError> {
    let Some(expected) = expected_agent_pubkey_hex else {
        return Ok(());
    };
    if actual_agent_pubkey_hex.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    Err(AppError::Invalid(format!(
        "agent key mismatch: resolved pubkey {} does not match expected {}",
        actual_agent_pubkey_hex, expected
    )))
}

fn map_secret_decrypt_error<E: ToString>(err: E) -> AppError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("decryption failed") || lower.contains("wrong password") {
        return AppError::Auth(
            "failed to decrypt intent agent key (wrong password or corrupted key store)"
                .to_string(),
        );
    }
    AppError::Config(format!("failed to decrypt intent agent key: {message}"))
}

fn intent_agent_key_store_path(cli: &Cli) -> Result<PathBuf, AppError> {
    Ok(crate::profile_path(cli)?.with_extension("intent-agent-key.json"))
}

fn load_agent_key_store(path: &Path) -> Result<Option<IntentAgentKeyStoreV1>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("failed to read intent agent key store: {e}")))?;
    let mut store: IntentAgentKeyStoreV1 = serde_json::from_str(&raw)
        .map_err(|e| AppError::Config(format!("failed to parse intent agent key store: {e}")))?;
    if store.schema_version.trim().is_empty() {
        store.schema_version = INTENT_AGENT_KEY_STORE_SCHEMA_VERSION.to_string();
    }
    Ok(Some(store))
}

fn save_agent_key_store(path: &Path, store: &IntentAgentKeyStoreV1) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(store).map_err(|e| {
        AppError::Internal(format!("failed to serialize intent agent key store: {e}"))
    })?;
    crate::write_bytes_atomic(path, &bytes, "intent agent key store")
}

fn intent_links_path(cli: &Cli) -> Result<PathBuf, AppError> {
    Ok(crate::profile_path(cli)?.with_extension("intent-links.json"))
}

fn pairing_request_path(cli: &Cli, pairing_id: &str) -> Result<PathBuf, AppError> {
    let profile_path = crate::profile_path(cli)?;
    Ok(profile_path.with_extension(format!(
        "pair-request-{}.json",
        pairing_fingerprint(pairing_id)
    )))
}

fn latest_pairing_request_path(cli: &Cli) -> Result<PathBuf, AppError> {
    Ok(crate::profile_path(cli)?.with_extension("pair-request-latest.json"))
}

fn pairing_uri_from_request_json(request_json: &str) -> Result<String, AppError> {
    if request_json.len() > MAX_PAIRING_REQUEST_JSON_BYTES {
        return Err(AppError::Invalid(format!(
            "pairing request json exceeds {} bytes",
            MAX_PAIRING_REQUEST_JSON_BYTES
        )));
    }
    let encoded = URL_SAFE_NO_PAD.encode(request_json.as_bytes());
    let uri = format!("zinc://pair?request={encoded}");
    if uri.len() > MAX_PAIRING_URI_CHARS {
        return Err(AppError::Invalid(format!(
            "pairing uri exceeds {} characters",
            MAX_PAIRING_URI_CHARS
        )));
    }
    Ok(uri)
}

fn pairing_request_json_from_uri(pairing_uri: &str) -> Result<String, AppError> {
    if pairing_uri.len() > MAX_PAIRING_URI_CHARS {
        return Err(AppError::Invalid(format!(
            "pairing uri exceeds {} characters",
            MAX_PAIRING_URI_CHARS
        )));
    }
    let query = pairing_uri
        .strip_prefix("zinc://pair?")
        .ok_or_else(|| AppError::Invalid("pairing uri must start with zinc://pair?".to_string()))?;
    let request_b64 = query
        .split('&')
        .find_map(|item| {
            let (key, value) = item.split_once('=')?;
            if key == "request" {
                Some(value)
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Invalid("pairing uri is missing request param".to_string()))?;

    let decoded = URL_SAFE_NO_PAD
        .decode(request_b64.as_bytes())
        .map_err(|e| {
            AppError::Invalid(format!("pairing uri request param is not base64url: {e}"))
        })?;
    if decoded.len() > MAX_PAIRING_REQUEST_JSON_BYTES {
        return Err(AppError::Invalid(format!(
            "decoded pairing request exceeds {} bytes",
            MAX_PAIRING_REQUEST_JSON_BYTES
        )));
    }

    String::from_utf8(decoded)
        .map_err(|e| AppError::Invalid(format!("decoded pairing request is not utf-8: {e}")))
}

#[cfg(test)]
fn pairing_ack_code_from_json(signed_ack_json: &str) -> Result<String, AppError> {
    let ack_bytes = signed_ack_json.as_bytes();
    if ack_bytes.len() > MAX_PAIRING_ACK_JSON_BYTES {
        return Err(AppError::Invalid(format!(
            "signed pairing ack exceeds {} bytes",
            MAX_PAIRING_ACK_JSON_BYTES
        )));
    }
    Ok(format!(
        "{PAIRING_ACK_CODE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(ack_bytes)
    ))
}

fn pairing_ack_json_from_code(ack_code: &str) -> Result<String, AppError> {
    let encoded = ack_code
        .trim()
        .strip_prefix(PAIRING_ACK_CODE_PREFIX)
        .ok_or_else(|| {
            AppError::Invalid(format!(
                "ack code must start with {PAIRING_ACK_CODE_PREFIX}"
            ))
        })?;
    if encoded.is_empty() {
        return Err(AppError::Invalid(
            "ack code is missing encoded payload".to_string(),
        ));
    }

    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|e| AppError::Invalid(format!("ack code is not valid base64url: {e}")))?;
    if decoded.len() > MAX_PAIRING_ACK_JSON_BYTES {
        return Err(AppError::Invalid(format!(
            "decoded pairing ack exceeds {} bytes",
            MAX_PAIRING_ACK_JSON_BYTES
        )));
    }
    let ack_json = String::from_utf8(decoded)
        .map_err(|e| AppError::Invalid(format!("decoded pairing ack is not utf-8: {e}")))?;
    serde_json::from_str::<serde_json::Value>(&ack_json)
        .map_err(|e| AppError::Invalid(format!("decoded pairing ack is not valid json: {e}")))?;
    Ok(ack_json)
}

fn run_async_with_runtime<T, F>(future: F) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| AppError::Internal(format!("failed to build async runtime: {e}")))?;
        runtime.block_on(future)
    }
}

fn fetch_signed_pairing_ack_from_relays(
    signed_request: &SignedPairingRequestV1,
    recipient_secret_key_hex: &str,
) -> Result<Option<SignedPairingAckV1>, AppError> {
    run_async_with_runtime(fetch_signed_pairing_ack_from_relays_async(
        signed_request,
        recipient_secret_key_hex,
    ))
}

fn wait_for_pairing_ack_from_relays(
    signed_request: &SignedPairingRequestV1,
    recipient_secret_key_hex: &str,
    wait_window_ms: u64,
) -> Option<SignedPairingAckV1> {
    let deadline = Instant::now() + Duration::from_millis(wait_window_ms);
    loop {
        if let Ok(Some(signed_ack)) =
            fetch_signed_pairing_ack_from_relays(signed_request, recipient_secret_key_hex)
        {
            return Some(signed_ack);
        }

        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        let remaining = deadline.saturating_duration_since(now);
        let sleep_for = remaining.min(Duration::from_millis(DEFAULT_PAIR_START_AWAIT_ACK_POLL_MS));
        if sleep_for.is_zero() {
            return None;
        }
        std::thread::sleep(sleep_for);
    }
}

fn wait_for_sign_intent_receipt_from_relays(
    relays: &[String],
    pairing_id: &str,
    intent_id: &str,
    recipient_pubkey_hex: &str,
    recipient_secret_key_hex: &str,
    wait_window_ms: u64,
) -> Result<Option<SignedSignIntentReceiptV1>, AppError> {
    let deadline = Instant::now() + Duration::from_millis(wait_window_ms);
    loop {
        if let Some(signed_receipt) = fetch_sign_intent_receipt_from_relays(
            relays,
            pairing_id,
            intent_id,
            recipient_pubkey_hex,
            recipient_secret_key_hex,
        )? {
            return Ok(Some(signed_receipt));
        }

        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        let remaining = deadline.saturating_duration_since(now);
        let sleep_for = remaining.min(Duration::from_millis(DEFAULT_WAIT_RECEIPT_POLL_MS));
        if sleep_for.is_zero() {
            return Ok(None);
        }
        std::thread::sleep(sleep_for);
    }
}

fn fetch_sign_intent_receipt_from_relays(
    relays: &[String],
    pairing_id: &str,
    intent_id: &str,
    recipient_pubkey_hex: &str,
    recipient_secret_key_hex: &str,
) -> Result<Option<SignedSignIntentReceiptV1>, AppError> {
    run_async_with_runtime(fetch_sign_intent_receipt_from_relays_async(
        relays,
        pairing_id,
        intent_id,
        recipient_pubkey_hex,
        recipient_secret_key_hex,
    ))
}

async fn fetch_sign_intent_receipt_from_relays_async(
    relays: &[String],
    pairing_id: &str,
    intent_id: &str,
    recipient_pubkey_hex: &str,
    recipient_secret_key_hex: &str,
) -> Result<Option<SignedSignIntentReceiptV1>, AppError> {
    if relays.is_empty() {
        return Ok(None);
    }
    let pairing_hash = pairing_tag_hash_hex(pairing_id).map_err(map_intent_error)?;
    let mut newest: Option<SignedSignIntentReceiptV1> = None;
    let mut seen_receipt_ids = HashSet::new();

    for relay_url in relays {
        let candidate = discover_sign_intent_receipt_from_relay(
            relay_url,
            &pairing_hash,
            pairing_id,
            intent_id,
            recipient_pubkey_hex,
            recipient_secret_key_hex,
            DEFAULT_PAIRING_RELAY_TIMEOUT_MS,
        )
        .await?;
        if let Some(signed_receipt) = candidate {
            let receipt_id = signed_receipt.receipt_id_hex().map_err(map_intent_error)?;
            if !seen_receipt_ids.insert(receipt_id) {
                continue;
            }
            let replace = newest
                .as_ref()
                .map(|existing| {
                    signed_receipt.receipt.created_at_unix > existing.receipt.created_at_unix
                })
                .unwrap_or(true);
            if replace {
                newest = Some(signed_receipt);
            }
        }
    }

    Ok(newest)
}

async fn discover_sign_intent_receipt_from_relay(
    relay_url: &str,
    pairing_hash: &str,
    pairing_id: &str,
    intent_id: &str,
    recipient_pubkey_hex: &str,
    recipient_secret_key_hex: &str,
    timeout_ms: u64,
) -> Result<Option<SignedSignIntentReceiptV1>, AppError> {
    let (mut socket, _) = connect_async(relay_url)
        .await
        .map_err(|e| AppError::Network(format!("failed to connect relay {relay_url}: {e}")))?;

    let subscription_id = format!(
        "zinc-intent-receipt-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let req = sign_intent_receipt_req_frame(
        &subscription_id,
        pairing_hash,
        recipient_pubkey_hex,
        DEFAULT_PAIRING_RELAY_LIMIT,
    )?;
    socket
        .send(Message::Text(req.into()))
        .await
        .map_err(|e| AppError::Network(format!("failed to send relay req frame: {e}")))?;

    let mut newest: Option<SignedSignIntentReceiptV1> = None;
    let sid = subscription_id.clone();
    timeout(Duration::from_millis(timeout_ms), async {
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Some(event) = parse_transport_event_frame(text.as_ref(), &sid) {
                        if let Ok(signed_receipt) =
                            decode_signed_sign_intent_receipt_event_with_secret(
                                &event,
                                recipient_secret_key_hex,
                            )
                        {
                            if !signed_receipt
                                .receipt
                                .pairing_id
                                .eq_ignore_ascii_case(pairing_id)
                            {
                                continue;
                            }
                            if !signed_receipt
                                .receipt
                                .intent_id
                                .eq_ignore_ascii_case(intent_id)
                            {
                                continue;
                            }
                            let replace = newest
                                .as_ref()
                                .map(|existing| {
                                    signed_receipt.receipt.created_at_unix
                                        > existing.receipt.created_at_unix
                                })
                                .unwrap_or(true);
                            if replace {
                                newest = Some(signed_receipt);
                            }
                        }
                        continue;
                    }
                    if is_eose_frame(text.as_ref(), &sid) {
                        break;
                    }
                }
                Ok(Message::Binary(bin)) => {
                    if let Ok(text) = std::str::from_utf8(&bin) {
                        if let Some(event) = parse_transport_event_frame(text, &sid) {
                            if let Ok(signed_receipt) =
                                decode_signed_sign_intent_receipt_event_with_secret(
                                    &event,
                                    recipient_secret_key_hex,
                                )
                            {
                                if !signed_receipt
                                    .receipt
                                    .pairing_id
                                    .eq_ignore_ascii_case(pairing_id)
                                {
                                    continue;
                                }
                                if !signed_receipt
                                    .receipt
                                    .intent_id
                                    .eq_ignore_ascii_case(intent_id)
                                {
                                    continue;
                                }
                                let replace = newest
                                    .as_ref()
                                    .map(|existing| {
                                        signed_receipt.receipt.created_at_unix
                                            > existing.receipt.created_at_unix
                                    })
                                    .unwrap_or(true);
                                if replace {
                                    newest = Some(signed_receipt);
                                }
                            }
                            continue;
                        }
                        if is_eose_frame(text, &sid) {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(AppError::Network(format!(
                        "relay read error for {relay_url}: {e}"
                    )));
                }
            }
        }
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|_| {
        AppError::Network(format!(
            "relay {relay_url} timed out reading sign intent receipts"
        ))
    })??;

    let close = close_frame(&subscription_id)?;
    let _ = socket.send(Message::Text(close.into())).await;
    Ok(newest)
}

async fn fetch_signed_pairing_ack_from_relays_async(
    signed_request: &SignedPairingRequestV1,
    recipient_secret_key_hex: &str,
) -> Result<Option<SignedPairingAckV1>, AppError> {
    let relays = normalize_pairing_relays(
        &signed_request.request.relays,
        pairing_request_network_hint(signed_request),
    );
    if relays.is_empty() {
        return Ok(None);
    }
    let pairing_hash =
        pairing_tag_hash_hex(&signed_request.pairing_id_hex().map_err(map_intent_error)?)
            .map_err(map_intent_error)?;
    let mut seen_ack_ids = HashSet::new();
    let mut newest: Option<SignedPairingAckV1> = None;

    for relay_url in relays {
        let candidate = discover_pairing_ack_from_relay(
            &relay_url,
            signed_request,
            &pairing_hash,
            recipient_secret_key_hex,
            DEFAULT_PAIRING_RELAY_TIMEOUT_MS,
        )
        .await?;
        if let Some(signed_ack) = candidate {
            let ack_id = signed_ack.ack_id_hex().map_err(map_intent_error)?;
            if !seen_ack_ids.insert(ack_id) {
                continue;
            }
            let replace = newest
                .as_ref()
                .map(|existing| signed_ack.ack.created_at_unix > existing.ack.created_at_unix)
                .unwrap_or(true);
            if replace {
                newest = Some(signed_ack);
            }
        }
    }

    Ok(newest)
}

async fn discover_pairing_ack_from_relay(
    relay_url: &str,
    signed_request: &SignedPairingRequestV1,
    pairing_hash: &str,
    recipient_secret_key_hex: &str,
    timeout_ms: u64,
) -> Result<Option<SignedPairingAckV1>, AppError> {
    let (mut socket, _) = connect_async(relay_url)
        .await
        .map_err(|e| AppError::Network(format!("failed to connect relay {relay_url}: {e}")))?;

    let subscription_id = format!(
        "zinc-pair-ack-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let req = pairing_ack_req_frame(
        &subscription_id,
        pairing_hash,
        &signed_request.request.agent_pubkey_hex,
        DEFAULT_PAIRING_RELAY_LIMIT,
    )?;
    socket
        .send(Message::Text(req.into()))
        .await
        .map_err(|e| AppError::Network(format!("failed to send relay req frame: {e}")))?;

    let mut newest: Option<SignedPairingAckV1> = None;
    let sid = subscription_id.clone();
    timeout(Duration::from_millis(timeout_ms), async {
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Some(event) = parse_transport_event_frame(text.as_ref(), &sid) {
                        if let Ok(envelope) = decode_pairing_ack_envelope_event_with_secret(
                            &event,
                            recipient_secret_key_hex,
                        ) {
                            let replace = newest
                                .as_ref()
                                .map(|existing| {
                                    envelope.signed_ack.ack.created_at_unix
                                        > existing.ack.created_at_unix
                                })
                                .unwrap_or(true);
                            if replace {
                                newest = Some(envelope.signed_ack);
                            }
                        }
                        continue;
                    }
                    if is_eose_frame(text.as_ref(), &sid) {
                        break;
                    }
                }
                Ok(Message::Binary(bin)) => {
                    if let Ok(text) = std::str::from_utf8(&bin) {
                        if let Some(event) = parse_transport_event_frame(text, &sid) {
                            if let Ok(envelope) = decode_pairing_ack_envelope_event_with_secret(
                                &event,
                                recipient_secret_key_hex,
                            ) {
                                let replace = newest
                                    .as_ref()
                                    .map(|existing| {
                                        envelope.signed_ack.ack.created_at_unix
                                            > existing.ack.created_at_unix
                                    })
                                    .unwrap_or(true);
                                if replace {
                                    newest = Some(envelope.signed_ack);
                                }
                            }
                            continue;
                        }
                        if is_eose_frame(text, &sid) {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(AppError::Network(format!(
                        "relay read error for {relay_url}: {e}"
                    )));
                }
            }
        }
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|_| AppError::Network(format!("relay {relay_url} timed out reading ack events")))??;

    let close = close_frame(&subscription_id)?;
    let _ = socket.send(Message::Text(close.into())).await;
    Ok(newest)
}

fn publish_pairing_complete_receipt_best_effort(
    signed_request: &SignedPairingRequestV1,
    signed_ack: &SignedPairingAckV1,
    agent_secret_key_hex: &str,
    now_unix: i64,
) -> bool {
    let relays = normalize_pairing_relays(
        &signed_request.request.relays,
        pairing_request_network_hint(signed_request),
    );
    if relays.is_empty() {
        return false;
    }
    let created_at_unix = u64::try_from(now_unix).unwrap_or_else(|_| current_unix());

    let signed_receipt: SignedPairingCompleteReceiptV1 = match build_signed_pairing_complete_receipt(
        signed_request,
        signed_ack,
        agent_secret_key_hex,
        now_unix,
    ) {
        Ok(receipt) => receipt,
        Err(_) => return false,
    };

    let tags = match pairing_transport_tags(
        NOSTR_PAIRING_COMPLETE_RECEIPT_TYPE_TAG_VALUE,
        &signed_receipt.receipt.pairing_id,
        &signed_receipt.receipt.wallet_pubkey_hex,
    ) {
        Ok(tags) => tags,
        Err(_) => return false,
    };
    let receipt_json = match serde_json::to_string(&signed_receipt) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let content = match encrypt_pairing_transport_content(
        &receipt_json,
        agent_secret_key_hex,
        &signed_receipt.receipt.wallet_pubkey_hex,
    ) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let event = match NostrTransportEventV1::new(
        PAIRING_TRANSPORT_EVENT_KIND,
        tags,
        content,
        created_at_unix,
        agent_secret_key_hex,
    ) {
        Ok(event) => event,
        Err(_) => return false,
    };

    run_async_with_runtime(publish_transport_event_multi(
        &relays,
        &event,
        DEFAULT_PAIRING_RELAY_TIMEOUT_MS,
    ))
    .unwrap_or_default()
}

async fn publish_transport_event_multi(
    relay_urls: &[String],
    event: &NostrTransportEventV1,
    timeout_ms: u64,
) -> Result<bool, AppError> {
    let stats = publish_transport_event_multi_stats(relay_urls, event, timeout_ms).await?;
    Ok(stats.accepted > 0)
}

#[derive(Debug, Clone, Copy)]
struct RelayPublishStats {
    accepted: usize,
    total: usize,
}

async fn publish_transport_event_multi_stats(
    relay_urls: &[String],
    event: &NostrTransportEventV1,
    timeout_ms: u64,
) -> Result<RelayPublishStats, AppError> {
    let mut accepted = 0usize;
    for relay in relay_urls {
        if publish_transport_event(relay, event, timeout_ms).await? {
            accepted += 1;
        }
    }
    Ok(RelayPublishStats {
        accepted,
        total: relay_urls.len(),
    })
}

async fn publish_transport_event(
    relay_url: &str,
    event: &NostrTransportEventV1,
    timeout_ms: u64,
) -> Result<bool, AppError> {
    let (mut socket, _) = connect_async(relay_url)
        .await
        .map_err(|e| AppError::Network(format!("failed to connect relay {relay_url}: {e}")))?;
    let frame = event_frame(event)?;
    socket
        .send(Message::Text(frame.into()))
        .await
        .map_err(|e| AppError::Network(format!("failed to send relay event frame: {e}")))?;

    let event_id = event.id.clone();
    let accepted = timeout(Duration::from_millis(timeout_ms), async {
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Some((ok, _msg)) = parse_ok_frame(text.as_ref(), &event_id) {
                        return Ok(ok);
                    }
                }
                Ok(Message::Binary(bin)) => {
                    if let Ok(text) = std::str::from_utf8(&bin) {
                        if let Some((ok, _msg)) = parse_ok_frame(text, &event_id) {
                            return Ok(ok);
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(AppError::Network(format!(
                        "relay read error for {relay_url}: {e}"
                    )));
                }
            }
        }
        Ok(false)
    })
    .await
    .map_err(|_| AppError::Network(format!("relay {relay_url} timed out waiting for OK")))??;

    Ok(accepted)
}

fn event_frame(event: &NostrTransportEventV1) -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!(["EVENT", event]))
        .map_err(|e| AppError::Internal(format!("failed to encode relay event frame: {e}")))
}

fn close_frame(subscription_id: &str) -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!(["CLOSE", subscription_id]))
        .map_err(|e| AppError::Internal(format!("failed to encode relay close frame: {e}")))
}

fn pairing_ack_req_frame(
    subscription_id: &str,
    pairing_hash: &str,
    recipient_pubkey_hex: &str,
    limit: usize,
) -> Result<String, AppError> {
    let mut filter = serde_json::Map::new();
    filter.insert("kinds".to_string(), json!([PAIRING_TRANSPORT_EVENT_KIND]));
    filter.insert(
        format!("#{}", NOSTR_TAG_APP_KEY),
        json!([NOSTR_SIGN_INTENT_APP_TAG_VALUE]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_TYPE_KEY),
        json!([NOSTR_PAIRING_ACK_TYPE_TAG_VALUE]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_PAIRING_HASH_KEY),
        json!([pairing_hash]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_RECIPIENT_PUBKEY_KEY),
        json!([recipient_pubkey_hex]),
    );
    filter.insert("limit".to_string(), json!(limit));

    serde_json::to_string(&serde_json::json!(["REQ", subscription_id, filter]))
        .map_err(|e| AppError::Internal(format!("failed to encode relay req frame: {e}")))
}

fn sign_intent_receipt_req_frame(
    subscription_id: &str,
    pairing_hash: &str,
    recipient_pubkey_hex: &str,
    limit: usize,
) -> Result<String, AppError> {
    let mut filter = serde_json::Map::new();
    filter.insert("kinds".to_string(), json!([PAIRING_TRANSPORT_EVENT_KIND]));
    filter.insert(
        format!("#{}", NOSTR_TAG_APP_KEY),
        json!([NOSTR_SIGN_INTENT_APP_TAG_VALUE]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_TYPE_KEY),
        json!([NOSTR_SIGN_INTENT_RECEIPT_TYPE_TAG_VALUE]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_PAIRING_HASH_KEY),
        json!([pairing_hash]),
    );
    filter.insert(
        format!("#{}", NOSTR_TAG_RECIPIENT_PUBKEY_KEY),
        json!([recipient_pubkey_hex]),
    );
    filter.insert("limit".to_string(), json!(limit));

    serde_json::to_string(&serde_json::json!(["REQ", subscription_id, filter]))
        .map_err(|e| AppError::Internal(format!("failed to encode relay req frame: {e}")))
}

fn parse_ok_frame(frame: &str, event_id: &str) -> Option<(bool, String)> {
    let value: Value = serde_json::from_str(frame).ok()?;
    let arr = value.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    if arr.first()?.as_str()? != "OK" {
        return None;
    }
    if arr.get(1)?.as_str()? != event_id {
        return None;
    }
    let accepted = arr.get(2)?.as_bool()?;
    let message = arr.get(3)?.as_str()?.to_string();
    Some((accepted, message))
}

fn parse_transport_event_frame(
    frame: &str,
    subscription_id: &str,
) -> Option<NostrTransportEventV1> {
    let value: Value = serde_json::from_str(frame).ok()?;
    let arr = value.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    if arr.first()?.as_str()? != "EVENT" {
        return None;
    }
    if arr.get(1)?.as_str()? != subscription_id {
        return None;
    }
    let event: NostrTransportEventV1 = serde_json::from_value(arr.get(2)?.clone()).ok()?;
    event.verify().ok()?;
    Some(event)
}

fn is_eose_frame(frame: &str, subscription_id: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(frame) else {
        return false;
    };
    let Some(arr) = value.as_array() else {
        return false;
    };
    arr.len() == 2
        && arr.first().and_then(Value::as_str) == Some("EOSE")
        && arr.get(1).and_then(Value::as_str) == Some(subscription_id)
}

fn load_link_store(path: &Path) -> Result<IntentLinkStoreV1, AppError> {
    if !path.exists() {
        return Ok(IntentLinkStoreV1::default());
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("failed to read intent links: {e}")))?;
    let mut store: IntentLinkStoreV1 = serde_json::from_str(&raw)
        .map_err(|e| AppError::Config(format!("failed to parse intent links json: {e}")))?;
    if store.schema_version.trim().is_empty() {
        store.schema_version = "intent-link-store-v1".to_string();
    }
    for link in &mut store.links {
        link.status = normalize_link_status(&link.status).to_string();
    }
    Ok(store)
}

fn save_link_store(path: &Path, store: &IntentLinkStoreV1) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| AppError::Internal(format!("failed to serialize intent links: {e}")))?;
    crate::write_bytes_atomic(path, &bytes, "intent links")
}

fn build_link_from_approval(
    approval: &PairingLinkApprovalV1,
    linked_at_unix: i64,
) -> LinkedWalletV1 {
    LinkedWalletV1 {
        pairing_id: approval.pairing_id.clone(),
        agent_pubkey_hex: approval.agent_pubkey_hex.clone(),
        wallet_pubkey_hex: approval.wallet_pubkey_hex.clone(),
        granted_capabilities: approval.granted_capabilities.clone(),
        request_expires_at_unix: approval.request_expires_at_unix,
        ack_expires_at_unix: approval.ack_expires_at_unix,
        linked_at_unix,
        status: LINK_STATUS_ACTIVE.to_string(),
        status_updated_at_unix: Some(linked_at_unix),
        paused_at_unix: None,
        revoked_at_unix: None,
        rotated_by_pairing_id: None,
    }
}

fn upsert_link(store: &mut IntentLinkStoreV1, new_link: LinkedWalletV1) {
    for existing in &mut store.links {
        if existing.pairing_id == new_link.pairing_id {
            continue;
        }
        let existing_status = normalize_link_status(&existing.status);
        if existing.agent_pubkey_hex == new_link.agent_pubkey_hex
            && existing.wallet_pubkey_hex == new_link.wallet_pubkey_hex
            && existing_status != LINK_STATUS_REVOKED
        {
            existing.status = LINK_STATUS_ROTATED.to_string();
            existing.status_updated_at_unix = Some(new_link.linked_at_unix);
            existing.rotated_by_pairing_id = Some(new_link.pairing_id.clone());
        }
    }

    store
        .links
        .retain(|link| link.pairing_id != new_link.pairing_id);
    store.links.push(new_link);
}

fn pairing_fingerprint(pairing_id: &str) -> String {
    pairing_id.chars().take(12).collect()
}

fn current_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn map_intent_error<E: ToString>(err: E) -> AppError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("invalid")
        || lower.contains("unsupported")
        || lower.contains("duplicate")
        || lower.contains("expired")
        || lower.contains("exceeds")
        || lower.contains("verification")
        || lower.contains("signature")
        || lower.contains("missing")
        || lower.contains("must")
    {
        return AppError::Invalid(message);
    }
    AppError::Internal(message)
}

#[cfg(test)]
mod tests {
    use super::{
        build_fixture_bundle, default_pairing_capabilities, ensure_link_allows_intents,
        intent_agent_key_store_path, latest_pairing_request_path, map_intent_error,
        normalize_pairing_relays, pairing_ack_code_from_json, pairing_ack_json_from_code,
        pairing_request_json_from_uri, pairing_uri_from_request_json, parse_fixture_bundle,
        resolve_agent_secret_key_hex, resolve_fixture_source, resolve_json_source, run_pair_finish,
        run_pair_list, run_pair_set_status, run_pair_show, run_pair_start, upsert_link,
        IntentLinkStoreV1, LinkedWalletV1, DEFAULT_AGENT_SECRET_KEY_HEX,
        DEFAULT_WALLET_SECRET_KEY_HEX, LINK_STATUS_ACTIVE, LINK_STATUS_PAUSED, LINK_STATUS_REVOKED,
        LINK_STATUS_ROTATED, MAX_PAIRING_URI_CHARS,
    };
    use crate::cli::Cli;
    use crate::error::AppError;
    use crate::output::CommandOutput;
    use clap::Parser;
    use std::path::Path;
    use zinc_core::{
        pubkey_hex_from_secret_key, PairingAckDecisionV1, PairingAckV1, SignedPairingAckV1,
        SignedPairingRequestV1,
    };

    fn ensure_profile_parent_exists(cli: &Cli) {
        let profile_path = crate::profile_path(cli).expect("profile path");
        let parent = profile_path.parent().expect("profile parent");
        std::fs::create_dir_all(parent).expect("create profile parent");
    }

    fn unique_data_dir(prefix: &str) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        format!("/tmp/zinc-cli-{prefix}-{}-{now}", std::process::id())
    }

    #[test]
    fn resolve_agent_secret_key_bootstraps_and_reuses_store() {
        let data_dir = unique_data_dir("agent-key-bootstrap");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--password".to_string(),
            "test-password".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "pair".to_string(),
            "list".to_string(),
        ])
        .expect("cli parse");

        let first = resolve_agent_secret_key_hex(&cli, None, None, true).expect("first resolve");
        let second = resolve_agent_secret_key_hex(&cli, None, None, false).expect("second resolve");
        assert_eq!(first, second);

        let store_path = intent_agent_key_store_path(&cli).expect("store path");
        assert!(store_path.exists(), "agent key store should be written");
    }

    #[test]
    fn resolve_agent_secret_key_without_store_requires_explicit_or_bootstrap() {
        let data_dir = unique_data_dir("agent-key-missing");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "pair".to_string(),
            "list".to_string(),
        ])
        .expect("cli parse");

        let err = resolve_agent_secret_key_hex(&cli, None, None, false)
            .expect_err("missing store must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn resolve_agent_secret_key_rejects_expected_pubkey_mismatch() {
        let data_dir = unique_data_dir("agent-key-mismatch");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "pair".to_string(),
            "list".to_string(),
        ])
        .expect("cli parse");
        let unexpected_pubkey =
            pubkey_hex_from_secret_key(DEFAULT_WALLET_SECRET_KEY_HEX).expect("wallet pubkey");
        let err = resolve_agent_secret_key_hex(
            &cli,
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            Some(&unexpected_pubkey),
            false,
        )
        .expect_err("expected mismatch should fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn resolve_fixture_source_requires_exactly_one() {
        let err = resolve_fixture_source(None, None, false).expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn resolve_fixture_source_prefers_inline_json() {
        let source = resolve_fixture_source(Some("{\"x\":1}"), None, false).expect("source");
        assert_eq!(source, "{\"x\":1}");
    }

    #[test]
    fn resolve_fixture_source_rejects_multiple_sources() {
        let err = resolve_fixture_source(Some("{}"), Some(Path::new("/tmp/a.json")), false)
            .expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn map_intent_error_classifies_validation_as_invalid() {
        let err = map_intent_error("invalid schema");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn parse_fixture_bundle_accepts_raw_bundle() {
        let bundle = build_fixture_bundle(
            1_710_000_000,
            DEFAULT_AGENT_SECRET_KEY_HEX,
            DEFAULT_WALLET_SECRET_KEY_HEX,
        )
        .expect("fixture bundle");
        let raw = serde_json::to_string(&bundle).expect("fixture json");
        let bundle = parse_fixture_bundle(&raw).expect("parsed bundle");
        assert_eq!(bundle.schema_version, "intent-fixture-v1");
    }

    #[test]
    fn parse_fixture_bundle_accepts_envelope() {
        let bundle = build_fixture_bundle(
            1_710_000_000,
            DEFAULT_AGENT_SECRET_KEY_HEX,
            DEFAULT_WALLET_SECRET_KEY_HEX,
        )
        .expect("fixture bundle");
        let envelope = serde_json::json!({ "fixtures": bundle }).to_string();
        let parsed = parse_fixture_bundle(&envelope).expect("parsed");
        assert_eq!(parsed.schema_version, "intent-fixture-v1");
    }

    #[test]
    fn normalize_pairing_relays_uses_regtest_default() {
        let normalized = normalize_pairing_relays(&[], "regtest");
        assert_eq!(
            normalized,
            vec!["wss://nostr-regtest.exittheloop.com".to_string()]
        );
    }

    #[test]
    fn normalize_pairing_relays_canonicalizes_http_schemes() {
        let normalized = normalize_pairing_relays(
            &["https://nostr-regtest.exittheloop.com".to_string()],
            "regtest",
        );
        assert_eq!(
            normalized,
            vec!["wss://nostr-regtest.exittheloop.com".to_string()]
        );
    }

    #[test]
    fn resolve_json_source_requires_exactly_one_source() {
        let err = resolve_json_source(None, None, "request", "--request-json", "--request-file")
            .expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn resolve_json_source_rejects_multiple_sources() {
        let err = resolve_json_source(
            Some("{}"),
            Some(Path::new("/tmp/request.json")),
            "request",
            "--request-json",
            "--request-file",
        )
        .expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn upsert_link_replaces_existing_pairing() {
        let capabilities = default_pairing_capabilities("regtest");
        let mut store = IntentLinkStoreV1 {
            schema_version: "intent-link-store-v1".to_string(),
            links: vec![LinkedWalletV1 {
                pairing_id: "a".repeat(64),
                agent_pubkey_hex: "1".repeat(64),
                wallet_pubkey_hex: "2".repeat(64),
                granted_capabilities: capabilities.clone(),
                request_expires_at_unix: 10,
                ack_expires_at_unix: 11,
                linked_at_unix: 1,
                status: LINK_STATUS_ACTIVE.to_string(),
                status_updated_at_unix: Some(1),
                paused_at_unix: None,
                revoked_at_unix: None,
                rotated_by_pairing_id: None,
            }],
        };

        upsert_link(
            &mut store,
            LinkedWalletV1 {
                pairing_id: "a".repeat(64),
                agent_pubkey_hex: "3".repeat(64),
                wallet_pubkey_hex: "4".repeat(64),
                granted_capabilities: capabilities,
                request_expires_at_unix: 20,
                ack_expires_at_unix: 21,
                linked_at_unix: 2,
                status: LINK_STATUS_ACTIVE.to_string(),
                status_updated_at_unix: Some(2),
                paused_at_unix: None,
                revoked_at_unix: None,
                rotated_by_pairing_id: None,
            },
        );

        assert_eq!(store.links.len(), 1);
        assert_eq!(store.links[0].agent_pubkey_hex, "3".repeat(64));
        assert_eq!(store.links[0].linked_at_unix, 2);
    }

    #[test]
    fn upsert_link_rotates_prior_active_link_for_same_agent_wallet() {
        let capabilities = default_pairing_capabilities("regtest");
        let mut store = IntentLinkStoreV1 {
            schema_version: "intent-link-store-v1".to_string(),
            links: vec![LinkedWalletV1 {
                pairing_id: "a".repeat(64),
                agent_pubkey_hex: "1".repeat(64),
                wallet_pubkey_hex: "2".repeat(64),
                granted_capabilities: capabilities.clone(),
                request_expires_at_unix: 10,
                ack_expires_at_unix: 11,
                linked_at_unix: 1,
                status: LINK_STATUS_ACTIVE.to_string(),
                status_updated_at_unix: Some(1),
                paused_at_unix: None,
                revoked_at_unix: None,
                rotated_by_pairing_id: None,
            }],
        };

        upsert_link(
            &mut store,
            LinkedWalletV1 {
                pairing_id: "b".repeat(64),
                agent_pubkey_hex: "1".repeat(64),
                wallet_pubkey_hex: "2".repeat(64),
                granted_capabilities: capabilities,
                request_expires_at_unix: 20,
                ack_expires_at_unix: 21,
                linked_at_unix: 2,
                status: LINK_STATUS_ACTIVE.to_string(),
                status_updated_at_unix: Some(2),
                paused_at_unix: None,
                revoked_at_unix: None,
                rotated_by_pairing_id: None,
            },
        );

        assert_eq!(store.links.len(), 2);
        let rotated = store
            .links
            .iter()
            .find(|link| link.pairing_id == "a".repeat(64))
            .expect("rotated link");
        assert_eq!(rotated.status, LINK_STATUS_ROTATED);
        assert_eq!(
            rotated.rotated_by_pairing_id.as_deref(),
            Some("b".repeat(64).as_str())
        );
    }

    #[test]
    fn paused_links_are_blocked_for_future_intents() {
        let capabilities = default_pairing_capabilities("regtest");
        let link = LinkedWalletV1 {
            pairing_id: "a".repeat(64),
            agent_pubkey_hex: "1".repeat(64),
            wallet_pubkey_hex: "2".repeat(64),
            granted_capabilities: capabilities,
            request_expires_at_unix: 10,
            ack_expires_at_unix: 11,
            linked_at_unix: 1,
            status: LINK_STATUS_PAUSED.to_string(),
            status_updated_at_unix: Some(1),
            paused_at_unix: Some(1),
            revoked_at_unix: None,
            rotated_by_pairing_id: None,
        };
        let err = ensure_link_allows_intents(&link).expect_err("paused link should fail");
        assert!(matches!(err, AppError::Policy(_)));
    }

    #[test]
    fn pairing_uri_roundtrip_is_lossless() {
        let request_json = r#"{"request":{"version":1,"challengeNonce":"n1"}}"#;
        let uri = pairing_uri_from_request_json(request_json).expect("pairing uri");
        let decoded = pairing_request_json_from_uri(&uri).expect("decoded");
        assert_eq!(decoded, request_json);
    }

    #[test]
    fn pairing_uri_rejects_invalid_scheme() {
        let err = pairing_request_json_from_uri("https://pair?request=abc").expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn pairing_uri_rejects_missing_request_param() {
        let err = pairing_request_json_from_uri("zinc://pair?x=1").expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn pairing_uri_rejects_malformed_base64() {
        let err = pairing_request_json_from_uri("zinc://pair?request=****").expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn pairing_uri_rejects_oversized_uri() {
        let oversized = format!(
            "zinc://pair?request={}",
            "a".repeat(MAX_PAIRING_URI_CHARS + 1)
        );
        let err = pairing_request_json_from_uri(&oversized).expect_err("must fail");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn pairing_ack_code_roundtrip_is_lossless() {
        let ack_json = r#"{"ack":{"version":1},"signatureHex":"aa"}"#;
        let code = pairing_ack_code_from_json(ack_json).expect("ack code");
        let decoded = pairing_ack_json_from_code(&code).expect("decoded");
        assert_eq!(decoded, ack_json);
    }

    #[test]
    fn pair_start_agent_output_includes_pairing_uri_and_legacy_json() {
        let data_dir = unique_data_dir("pair-agent");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--agent".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "intent".to_string(),
            "pair".to_string(),
            "start".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);
        let output = run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            false,
            true,
        )
        .expect("pair start");

        match output {
            CommandOutput::Generic(value) => {
                assert!(value.get("pairingUri").is_some());
                assert!(value.get("pairingRequestJson").is_some());
            }
            _ => panic!("expected generic output"),
        }
    }

    #[test]
    fn pair_start_human_output_hides_json_by_default() {
        let data_dir = unique_data_dir("pair-human-hide");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "intent".to_string(),
            "pair".to_string(),
            "start".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);
        let output = run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            false,
            true,
        )
        .expect("pair start");

        match output {
            CommandOutput::IntentPairStart {
                pairing_request_json,
                ..
            } => assert!(pairing_request_json.is_none()),
            _ => panic!("expected human pair start output"),
        }
    }

    #[test]
    fn pair_start_human_output_shows_json_when_flag_is_set() {
        let data_dir = unique_data_dir("pair-human-show");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "intent".to_string(),
            "pair".to_string(),
            "start".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);
        let output = run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            true,
            true,
        )
        .expect("pair start");

        match output {
            CommandOutput::IntentPairStart {
                pairing_request_json,
                ..
            } => assert!(pairing_request_json.is_some()),
            _ => panic!("expected human pair start output"),
        }
    }

    #[test]
    fn pair_start_human_output_sets_await_ack_false_when_no_wait() {
        let data_dir = unique_data_dir("pair-human-no-wait");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "pair".to_string(),
            "start".to_string(),
            "--no-wait".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);
        let output = run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            false,
            true,
        )
        .expect("pair start");

        match output {
            CommandOutput::IntentPairStart { await_ack, .. } => assert!(!await_ack),
            _ => panic!("expected human pair start output"),
        }
    }

    #[test]
    fn pair_finish_accepts_ack_code_and_uses_latest_request_file() {
        let data_dir = unique_data_dir("pair-finish-ack-code");
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "intent".to_string(),
            "pair".to_string(),
            "start".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);

        run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            false,
            true,
        )
        .expect("pair start");

        let request_json = std::fs::read_to_string(
            latest_pairing_request_path(&cli).expect("latest request path"),
        )
        .expect("read latest request");
        let signed_request: SignedPairingRequestV1 =
            serde_json::from_str(&request_json).expect("signed request");

        let pairing_id = signed_request.pairing_id_hex().expect("pairing id");
        let wallet_pubkey_hex =
            pubkey_hex_from_secret_key(DEFAULT_WALLET_SECRET_KEY_HEX).expect("wallet pubkey");
        let ack = PairingAckV1 {
            version: 1,
            pairing_id,
            challenge_nonce: signed_request.request.challenge_nonce.clone(),
            agent_pubkey_hex: signed_request.request.agent_pubkey_hex.clone(),
            wallet_pubkey_hex,
            created_at_unix: 1_710_000_010,
            expires_at_unix: 1_710_000_300,
            decision: PairingAckDecisionV1::Approved,
            granted_capabilities: Some(signed_request.request.requested_capabilities.clone()),
            rejection_reason: None,
        };
        let signed_ack =
            SignedPairingAckV1::new(ack, DEFAULT_WALLET_SECRET_KEY_HEX).expect("signed ack");
        let ack_json = serde_json::to_string(&signed_ack).expect("ack json");
        let ack_code = pairing_ack_code_from_json(&ack_json).expect("ack code");

        let output = run_pair_finish(
            &cli,
            Some(1_710_000_020),
            None,
            None,
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            None,
            None,
            Some(&ack_code),
        )
        .expect("pair finish");

        match output {
            CommandOutput::IntentPairFinish { paired, .. } => assert!(paired),
            _ => panic!("expected intent pair finish output"),
        }
    }

    fn create_linked_profile(prefix: &str) -> Cli {
        let data_dir = unique_data_dir(prefix);
        let cli = Cli::try_parse_from(vec![
            "zinc-cli".to_string(),
            "--data-dir".to_string(),
            data_dir,
            "pair".to_string(),
            "start".to_string(),
        ])
        .expect("cli parse");
        ensure_profile_parent_exists(&cli);

        run_pair_start(
            &cli,
            Some(1_710_000_000),
            Some(600),
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            &[],
            "regtest",
            false,
            true,
        )
        .expect("pair start");

        let request_json = std::fs::read_to_string(
            latest_pairing_request_path(&cli).expect("latest request path"),
        )
        .expect("read latest request");
        let signed_request: SignedPairingRequestV1 =
            serde_json::from_str(&request_json).expect("signed request");

        let pairing_id = signed_request.pairing_id_hex().expect("pairing id");
        let wallet_pubkey_hex =
            pubkey_hex_from_secret_key(DEFAULT_WALLET_SECRET_KEY_HEX).expect("wallet pubkey");
        let ack = PairingAckV1 {
            version: 1,
            pairing_id,
            challenge_nonce: signed_request.request.challenge_nonce.clone(),
            agent_pubkey_hex: signed_request.request.agent_pubkey_hex.clone(),
            wallet_pubkey_hex,
            created_at_unix: 1_710_000_010,
            expires_at_unix: 1_710_000_300,
            decision: PairingAckDecisionV1::Approved,
            granted_capabilities: Some(signed_request.request.requested_capabilities.clone()),
            rejection_reason: None,
        };
        let signed_ack =
            SignedPairingAckV1::new(ack, DEFAULT_WALLET_SECRET_KEY_HEX).expect("signed ack");
        let ack_json = serde_json::to_string(&signed_ack).expect("ack json");
        let ack_code = pairing_ack_code_from_json(&ack_json).expect("ack code");

        run_pair_finish(
            &cli,
            Some(1_710_000_020),
            None,
            None,
            Some(DEFAULT_AGENT_SECRET_KEY_HEX),
            None,
            None,
            Some(&ack_code),
        )
        .expect("pair finish");

        cli
    }

    #[test]
    fn pair_list_and_show_include_link_details() {
        let cli = create_linked_profile("pair-list-show");

        let list_output = run_pair_list(&cli).expect("pair list");
        let pairing_id = match list_output {
            CommandOutput::IntentPairList { total, links, .. } => {
                assert_eq!(total, 1);
                assert_eq!(links.len(), 1);
                assert_eq!(links[0].status, LINK_STATUS_ACTIVE);
                links[0].pairing_id.clone()
            }
            _ => panic!("expected intent pair list output"),
        };

        let show_output = run_pair_show(&cli, &pairing_id[..12]).expect("pair show");
        match show_output {
            CommandOutput::IntentPairShow { link, .. } => {
                assert_eq!(link.pairing_id, pairing_id);
                assert_eq!(link.status, LINK_STATUS_ACTIVE);
            }
            _ => panic!("expected intent pair show output"),
        }
    }

    #[test]
    fn pair_status_transitions_pause_resume_and_revoke() {
        let cli = create_linked_profile("pair-status");
        let pairing_id = match run_pair_list(&cli).expect("pair list") {
            CommandOutput::IntentPairList { links, .. } => links[0].pairing_id.clone(),
            _ => panic!("expected intent pair list output"),
        };

        let pause_output =
            run_pair_set_status(&cli, &pairing_id, LINK_STATUS_PAUSED, Some(1_710_000_030))
                .expect("pause");
        match pause_output {
            CommandOutput::IntentPairStatusUpdate { status, .. } => {
                assert_eq!(status, LINK_STATUS_PAUSED);
            }
            _ => panic!("expected status update output"),
        }

        let resume_output =
            run_pair_set_status(&cli, &pairing_id, LINK_STATUS_ACTIVE, Some(1_710_000_040))
                .expect("resume");
        match resume_output {
            CommandOutput::IntentPairStatusUpdate { status, .. } => {
                assert_eq!(status, LINK_STATUS_ACTIVE);
            }
            _ => panic!("expected status update output"),
        }

        let revoke_output =
            run_pair_set_status(&cli, &pairing_id, LINK_STATUS_REVOKED, Some(1_710_000_050))
                .expect("revoke");
        match revoke_output {
            CommandOutput::IntentPairStatusUpdate { status, .. } => {
                assert_eq!(status, LINK_STATUS_REVOKED);
            }
            _ => panic!("expected status update output"),
        }

        let err =
            match run_pair_set_status(&cli, &pairing_id, LINK_STATUS_ACTIVE, Some(1_710_000_060)) {
                Ok(_) => panic!("revoked cannot resume"),
                Err(err) => err,
            };
        assert!(matches!(err, AppError::Invalid(_)));
    }
}
