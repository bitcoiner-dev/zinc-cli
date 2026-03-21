mod cli;
mod commands;
mod config;
mod config_resolver;
#[cfg(feature = "ui")]
mod dashboard;
mod error;
mod lock;
mod network_retry;
mod paths;
mod presenter;
#[cfg(feature = "ui")]
mod ui;
mod utils;
mod wallet_service;
mod wizard;

use crate::cli::{Cli, Command, PolicyMode};
use crate::config::*;
use crate::error::AppError;
use crate::utils::{env_bool, env_non_empty};
use clap::Parser;
use is_terminal::IsTerminal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use supports_unicode::{self, Stream};

pub use wallet_service::{
    default_bitcoin_cli as svc_default_bitcoin_cli,
    default_bitcoin_cli_args as svc_default_bitcoin_cli_args, home_dir as svc_home_dir,
    load_wallet_session as svc_load_wallet_session, now_unix as svc_now_unix, now_unix,
    persist_wallet_session as svc_persist_wallet_session,
    profile_lock_path as svc_profile_lock_path, profile_path as svc_profile_path,
    read_lock_metadata as svc_read_lock_metadata, read_profile as svc_read_profile,
    snapshot_dir as svc_snapshot_dir, wallet_password as svc_wallet_password,
    write_bytes_atomic as svc_write_bytes_atomic, write_profile as svc_write_profile, LockMetadata,
    Profile, ProfileLock, ServiceConfig, WalletSession,
};

const SCHEMA_VERSION: &str = "1.0";
static CORRELATION_SEQ: AtomicU64 = AtomicU64::new(1);

const GLOBAL_FLAGS: &[&str] = &[
    "--json",
    "--agent",
    "--quiet",
    "--yes",
    "--password",
    "--password-env",
    "--password-stdin",
    "--reveal",
    "--data-dir",
    "--profile",
    "--network",
    "--scheme",
    "--esplora-url",
    "--ord-url",
    "--ascii",
    "--no-images",
    "--correlation-id",
    "--log-json",
    "--idempotency-key",
    "--network-timeout-secs",
    "--network-retries",
    "--policy-mode",
];
const COMMAND_LIST: &[&str] = &[
    "setup",
    "config show",
    "config set",
    "config unset",
    "wallet init",
    "wallet import",
    "wallet info",
    "wallet reveal-mnemonic",
    "sync chain",
    "sync ordinals",
    "address taproot",
    "address payment",
    "balance",
    "tx list",
    "psbt create",
    "psbt analyze",
    "psbt sign",
    "psbt broadcast",
    "account list",
    "account use",
    "wait tx-confirmed",
    "wait balance",
    "snapshot save",
    "snapshot restore",
    "snapshot list",
    "lock info",
    "lock clear",
    "scenario mine",
    "scenario fund",
    "scenario reset",
    "doctor",
];

#[tokio::main]
async fn main() -> miette::Result<()> {
    // We need a pre-parse to identify --json or --agent for early errors
    let args: Vec<String> = std::env::args().collect();
    let started_at_unix_ms = now_unix_ms();
    let is_json = args.iter().any(|a| a == "--json" || a == "--agent")
        || env_bool("ZINC_CLI_JSON").unwrap_or(false);
    let preparse_log_json =
        args.iter().any(|a| a == "--log-json") || env_bool("ZINC_CLI_LOG_JSON").unwrap_or(false);
    let preparse_correlation_id =
        resolve_correlation_id_preparse(&args).unwrap_or_else(generate_correlation_id);

    let cli_res = Cli::try_parse();
    let cli = match cli_res {
        Ok(c) => {
            let mut c = c;
            c.explicit_network = args.iter().any(|a| a == "--network" || a == "-n");
            c.started_at_unix_ms = started_at_unix_ms;
            c
        }
        Err(err) => {
            if is_json {
                let command = {
                    let inferred = infer_command_name_from_args(&args);
                    if inferred == "unknown" {
                        "help".to_string()
                    } else {
                        inferred
                    }
                };

                let is_help = err.kind() == clap::error::ErrorKind::DisplayHelp;
                let msg = remap_suggestions(err.to_string());
                let duration_ms = now_unix_ms().saturating_sub(started_at_unix_ms);

                emit_structured_log_line(
                    preparse_log_json,
                    &preparse_correlation_id,
                    &command,
                    if is_help {
                        "command_finish"
                    } else {
                        "command_error"
                    },
                    json!({
                        "ok": is_help,
                        "duration_ms": duration_ms,
                        "error_type": if is_help { Value::Null } else { json!("invalid") },
                        "exit_code": if is_help { Value::Null } else { json!(2) },
                    }),
                );

                let error_json = json!({
                    "ok": is_help,
                    "schema_version": SCHEMA_VERSION,
                    "correlation_id": preparse_correlation_id,
                    "command": command,
                    "meta": {
                        "started_at_unix_ms": started_at_unix_ms,
                        "duration_ms": duration_ms
                    },
                    "error": if is_help { Value::Null } else {
                        json!({
                            "exit_code": 2,
                            "type": "invalid",
                            "message": msg
                        })
                    },
                    "help_text": msg,
                    "command_list": COMMAND_LIST,
                    "global_flags": GLOBAL_FLAGS,
                });
                println!("{}", serde_json::to_string(&error_json).unwrap());
                std::process::exit(if is_help { 0 } else { 1 });
            } else {
                err.exit();
            }
        }
    };

    // Resolve effective CLI options (combining persisted config, env, and flags)
    let mut cli = match resolve_effective_cli(cli) {
        Ok(c) => c,
        Err(err) => {
            // Re-check JSON after resolution if it wasn't pre-detected
            if is_json {
                let command = infer_command_name_from_args(&args);
                let duration_ms = now_unix_ms().saturating_sub(started_at_unix_ms);
                emit_structured_log_line(
                    preparse_log_json,
                    &preparse_correlation_id,
                    &command,
                    "command_error",
                    json!({
                        "ok": false,
                        "duration_ms": duration_ms,
                        "error_type": "config",
                        "exit_code": 10,
                    }),
                );
                let error_json = json!({
                    "ok": false,
                    "schema_version": "1.0",
                    "correlation_id": preparse_correlation_id,
                    "command": command,
                    "meta": {
                        "started_at_unix_ms": started_at_unix_ms,
                        "duration_ms": duration_ms
                    },
                    "error": {
                        "exit_code": 10,
                        "type": "config",
                        "message": err.to_string()
                    }
                });
                println!("{}", serde_json::to_string(&error_json).unwrap());
                std::process::exit(1);
            }
            return Err(err.into());
        }
    };

    if cli.correlation_id.is_none() {
        cli.correlation_id = Some(preparse_correlation_id.clone());
    }
    if cli.started_at_unix_ms == 0 {
        cli.started_at_unix_ms = started_at_unix_ms;
    }

    let command_name = infer_command_name_from_args(&args);
    emit_structured_log_line(
        cli.log_json,
        cli.correlation_id.as_deref().unwrap_or("unknown"),
        &command_name,
        "command_start",
        json!({
            "json_mode": cli.json,
            "agent_mode": cli.agent
        }),
    );

    let replay = match try_replay_idempotent_result(&cli, &command_name) {
        Ok(value) => value,
        Err(err) => {
            emit_structured_log_line(
                cli.log_json,
                cli.correlation_id.as_deref().unwrap_or("unknown"),
                &command_name,
                "command_error",
                json!({
                    "ok": false,
                    "duration_ms": now_unix_ms().saturating_sub(cli.started_at_unix_ms),
                    "error_type": err.tag(),
                    "exit_code": err.exit_code(),
                    "message": err.to_string()
                }),
            );
            if cli.json {
                let envelope = wrap_envelope(Err(err), &cli);
                println!("{}", serde_json::to_string(&envelope).unwrap());
                std::process::exit(1);
            } else {
                return Err(err.into());
            }
        }
    };

    if let Some(replay) = replay {
        let replay_value = attach_idempotency_metadata(
            replay.value,
            cli.idempotency_key.as_deref().unwrap_or(""),
            true,
            replay.recorded_at_unix_ms,
        );
        emit_structured_log_line(
            cli.log_json,
            cli.correlation_id.as_deref().unwrap_or("unknown"),
            &command_name,
            "command_finish",
            json!({
                "ok": true,
                "duration_ms": now_unix_ms().saturating_sub(cli.started_at_unix_ms),
                "idempotency_replayed": true
            }),
        );
        if cli.json {
            let envelope = wrap_envelope(Ok(replay_value), &cli);
            println!("{}", serde_json::to_string(&envelope).unwrap());
        } else if !replay_value.is_null() && !is_non_json_rendered_command(&cli.command) {
            println!("{}", serde_json::to_string(&replay_value).unwrap());
        }
        return Ok(());
    }

    match run(cli).await {
        Ok((mut val, cli_final)) => {
            if is_mutating_command(&cli_final.command)
                && cli_final
                    .idempotency_key
                    .as_ref()
                    .is_some_and(|k| !k.trim().is_empty())
            {
                if let Some(key) = cli_final.idempotency_key.as_deref() {
                    let recorded_at = record_idempotent_result(&cli_final, &command_name, &val)?;
                    val = attach_idempotency_metadata(val, key, false, recorded_at);
                }
            }
            emit_structured_log_line(
                cli_final.log_json,
                cli_final.correlation_id.as_deref().unwrap_or("unknown"),
                &command_name,
                "command_finish",
                json!({
                    "ok": true,
                    "duration_ms": now_unix_ms().saturating_sub(cli_final.started_at_unix_ms),
                    "idempotency_key": cli_final.idempotency_key.as_deref(),
                }),
            );
            if cli_final.json {
                let envelope = wrap_envelope(Ok(val), &cli_final);
                println!("{}", serde_json::to_string(&envelope).unwrap());
            } else if !val.is_null() && !is_non_json_rendered_command(&cli_final.command) {
                // For contract_v1.rs compatibility, print non-null results as JSON even in human mode
                println!("{}", serde_json::to_string(&val).unwrap());
            }
        }
        Err((err, cli_final)) => {
            let err_type = err.tag().to_string();
            let err_exit_code = err.exit_code();
            let err_message = err.to_string();
            emit_structured_log_line(
                cli_final.log_json,
                cli_final.correlation_id.as_deref().unwrap_or("unknown"),
                &command_name,
                "command_error",
                json!({
                    "ok": false,
                    "duration_ms": now_unix_ms().saturating_sub(cli_final.started_at_unix_ms),
                    "error_type": err_type,
                    "exit_code": err_exit_code,
                    "message": err_message
                }),
            );
            if cli_final.json {
                let envelope = wrap_envelope(Err(err), &cli_final);
                println!("{}", serde_json::to_string(&envelope).unwrap());
                std::process::exit(1);
            } else {
                return Err(err.into());
            }
        }
    }

    Ok(())
}

fn resolve_effective_cli(mut cli: Cli) -> Result<Cli, AppError> {
    let persisted = load_persisted_config()?;

    // Global flags override persisted config
    if !cli.json {
        cli.json = persisted.json.unwrap_or(false);
    }
    if !cli.quiet {
        cli.quiet = persisted.quiet.unwrap_or(false);
    }
    if cli.agent {
        cli.json = true;
        cli.quiet = true;
        cli.ascii = true;
    }

    if !cli.ascii {
        cli.ascii = persisted.ascii.unwrap_or(false);
    }

    if cli.data_dir.is_none() {
        cli.data_dir = persisted.data_dir.as_ref().map(PathBuf::from);
    }
    if cli.profile.is_none() {
        cli.profile = Some(persisted.profile.unwrap_or_else(|| "default".to_string()));
    }
    if cli.password_env.is_none() {
        cli.password_env = Some(
            persisted
                .password_env
                .clone()
                .unwrap_or_else(|| "ZINC_WALLET_PASSWORD".to_string()),
        );
    }
    if cli.network.is_none() {
        // If we have an explicit profile, we should probably let the profile decide
        // its own network unless the user explicitly passed --network.
        // However, for consistency with the rest of the app's "sticky" behavior,
        // we'll keep loading it but ensure it doesn't stomp on profile-specific logic.
        cli.network = persisted.network.clone();
    }
    if cli.scheme.is_none() {
        cli.scheme = persisted.scheme.clone();
    }
    if cli.esplora_url.is_none() {
        cli.esplora_url = persisted.esplora_url.clone();
    }
    if cli.ord_url.is_none() {
        cli.ord_url = persisted.ord_url.clone();
    }

    // Env vars override everything? (Following old logic)
    if let Some(val) = env_bool("ZINC_CLI_JSON") {
        cli.json = val;
    }
    if let Some(val) = env_bool("ZINC_CLI_QUIET") {
        cli.quiet = val;
    }
    if let Some(val) = env_non_empty("ZINC_CLI_PROFILE") {
        cli.profile = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_DATA_DIR") {
        cli.data_dir = Some(PathBuf::from(val));
    }
    if let Some(val) = env_non_empty("ZINC_CLI_PASSWORD_ENV") {
        cli.password_env = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_NETWORK") {
        cli.network = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_SCHEME") {
        cli.scheme = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_ESPLORA_URL") {
        cli.esplora_url = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_ORD_URL") {
        cli.ord_url = Some(val);
    }
    if let Some(val) = env_bool("ZINC_CLI_ASCII") {
        cli.ascii = val;
    }
    if let Some(val) = env_non_empty("ZINC_CLI_CORRELATION_ID") {
        cli.correlation_id = Some(val);
    }
    if let Some(val) = env_bool("ZINC_CLI_LOG_JSON") {
        cli.log_json = val;
    }
    if let Some(val) = env_non_empty("ZINC_CLI_IDEMPOTENCY_KEY") {
        cli.idempotency_key = Some(val);
    }
    if let Some(val) = env_non_empty("ZINC_CLI_NETWORK_TIMEOUT_SECS") {
        cli.network_timeout_secs = val.parse::<u64>().map_err(|_| {
            AppError::Invalid("ZINC_CLI_NETWORK_TIMEOUT_SECS must be a valid u64".to_string())
        })?;
    }
    if let Some(val) = env_non_empty("ZINC_CLI_NETWORK_RETRIES") {
        cli.network_retries = val.parse::<u32>().map_err(|_| {
            AppError::Invalid("ZINC_CLI_NETWORK_RETRIES must be a valid u32".to_string())
        })?;
    }
    if let Some(val) = env_non_empty("ZINC_CLI_POLICY_MODE") {
        cli.policy_mode = parse_policy_mode_value(&val, "ZINC_CLI_POLICY_MODE")?;
    }

    if cli.network_timeout_secs == 0 {
        return Err(AppError::Invalid(
            "--network-timeout-secs must be greater than 0".to_string(),
        ));
    }

    // Auto-detect if not forced
    if !cli.ascii && !supports_unicode::on(Stream::Stdout) {
        cli.ascii = true;
    }

    Ok(cli)
}

fn wrap_envelope(outcome: Result<Value, AppError>, cli: &Cli) -> Value {
    let args: Vec<String> = std::env::args().collect();
    let command_name = infer_command_name_from_args(&args);
    let correlation_id = cli
        .correlation_id
        .clone()
        .unwrap_or_else(generate_correlation_id);
    let duration_ms = now_unix_ms().saturating_sub(cli.started_at_unix_ms);

    match outcome {
        Ok(mut val) => {
            let mut envelope = json!({
                "ok": true,
                "schema_version": SCHEMA_VERSION,
                "correlation_id": correlation_id,
                "command": command_name,
                "meta": {
                    "started_at_unix_ms": cli.started_at_unix_ms,
                    "duration_ms": duration_ms
                }
            });

            // Merge payload if it's an object
            if let Some(obj) = val.as_object_mut() {
                let taken = std::mem::take(obj);
                if let Some(envelope_obj) = envelope.as_object_mut() {
                    for (k, v) in taken {
                        envelope_obj.insert(k, v);
                    }
                }
            } else if !val.is_null() {
                envelope
                    .as_object_mut()
                    .unwrap()
                    .insert("result".to_string(), val);
            }

            envelope
        }
        Err(err) => {
            let msg = remap_suggestions(err.to_string());
            json!({
                "ok": false,
                "schema_version": SCHEMA_VERSION,
                "correlation_id": correlation_id,
                "command": command_name,
                "meta": {
                    "started_at_unix_ms": cli.started_at_unix_ms,
                    "duration_ms": duration_ms
                },
                "error": {
                    "exit_code": err.exit_code(),
                    "type": err.tag(),
                    "message": msg
                }
            })
        }
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn generate_correlation_id() -> String {
    let seq = CORRELATION_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("zinc-{}-{}-{seq}", now_unix_ms(), std::process::id())
}

fn resolve_correlation_id_preparse(args: &[String]) -> Option<String> {
    extract_flag_value(args, "--correlation-id")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| env_non_empty("ZINC_CLI_CORRELATION_ID"))
}

fn extract_flag_value(args: &[String], flag: &str) -> Option<String> {
    for window in args.windows(2) {
        if window[0] == flag {
            return Some(window[1].clone());
        }
    }
    None
}

fn flag_requires_value(flag: &str) -> bool {
    matches!(
        flag,
        "--profile"
            | "-p"
            | "--data-dir"
            | "-d"
            | "--network"
            | "-n"
            | "--scheme"
            | "-s"
            | "--esplora-url"
            | "-e"
            | "--ord-url"
            | "-o"
            | "--password-env"
            | "--password"
            | "--correlation-id"
            | "--idempotency-key"
            | "--network-timeout-secs"
            | "--network-retries"
            | "--policy-mode"
    )
}

fn infer_command_name_from_args(args: &[String]) -> String {
    let mut command_parts = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('-') {
            if flag_requires_value(arg) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        command_parts.push(arg.clone());
        i += 1;
        if command_parts.len() == 2 {
            break;
        }
    }

    if command_parts.is_empty() {
        "unknown".to_string()
    } else {
        command_parts.join(" ")
    }
}

fn emit_structured_log_line(
    enabled: bool,
    correlation_id: &str,
    command: &str,
    event: &str,
    extra: Value,
) {
    if !enabled {
        return;
    }

    let mut payload = serde_json::Map::new();
    payload.insert("ts_unix_ms".to_string(), json!(now_unix_ms()));
    payload.insert("correlation_id".to_string(), json!(correlation_id));
    payload.insert("command".to_string(), json!(command));
    payload.insert("event".to_string(), json!(event));

    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            payload.insert(k.clone(), v.clone());
        }
    } else {
        payload.insert("data".to_string(), extra);
    }

    if let Ok(line) = serde_json::to_string(&Value::Object(payload)) {
        eprintln!("{line}");
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct IdempotencyStore {
    entries: BTreeMap<String, IdempotencyEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IdempotencyEntry {
    command: String,
    fingerprint: String,
    value: Value,
    recorded_at_unix_ms: u128,
}

struct IdempotencyReplay {
    value: Value,
    recorded_at_unix_ms: u128,
}

fn parse_policy_mode_value(value: &str, context: &str) -> Result<PolicyMode, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "warn" => Ok(PolicyMode::Warn),
        "strict" => Ok(PolicyMode::Strict),
        _ => Err(AppError::Invalid(format!(
            "{context} must be one of: warn,strict"
        ))),
    }
}

fn is_mutating_command(command: &Command) -> bool {
    match command {
        Command::Setup(_) => true,
        Command::Config(args) => {
            matches!(
                &args.action,
                crate::cli::ConfigAction::Set { .. } | crate::cli::ConfigAction::Unset { .. }
            )
        }
        Command::Wallet(args) => {
            matches!(
                &args.action,
                crate::cli::WalletAction::Init { .. } | crate::cli::WalletAction::Import { .. }
            )
        }
        Command::Address(args) => matches!(
            &args.kind,
            crate::cli::AddressKind::Taproot { new: true, .. }
                | crate::cli::AddressKind::Payment { new: true, .. }
        ),
        Command::Psbt(args) => matches!(
            &args.action,
            crate::cli::PsbtAction::Create { .. }
                | crate::cli::PsbtAction::Sign { .. }
                | crate::cli::PsbtAction::Broadcast { .. }
        ),
        Command::Account(args) => {
            matches!(&args.action, crate::cli::AccountAction::Use { .. })
        }
        Command::Snapshot(args) => matches!(
            &args.action,
            crate::cli::SnapshotAction::Save { .. } | crate::cli::SnapshotAction::Restore { .. }
        ),
        Command::Scenario(_) => true,
        _ => false,
    }
}

fn command_fingerprint(cli: &Cli) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", cli.command).hash(&mut hasher);
    cli.profile.hash(&mut hasher);
    cli.data_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn idempotency_store_path(cli: &Cli) -> PathBuf {
    crate::wallet_service::data_dir(&service_config(cli))
        .join("idempotency")
        .join(format!(
            "{}.json",
            cli.profile.as_deref().unwrap_or("default")
        ))
}

fn load_idempotency_store(path: &Path) -> Result<IdempotencyStore, AppError> {
    if !path.exists() {
        return Ok(IdempotencyStore::default());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| {
        AppError::Config(format!(
            "failed to read idempotency store {}: {e}",
            path.display()
        ))
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        AppError::Config(format!(
            "failed to parse idempotency store {}: {e}",
            path.display()
        ))
    })
}

fn save_idempotency_store(path: &Path, store: &IdempotencyStore) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| AppError::Internal(format!("failed to serialize idempotency store: {e}")))?;
    write_bytes_atomic(path, &bytes, "idempotency store")
}

fn try_replay_idempotent_result(
    cli: &Cli,
    command_name: &str,
) -> Result<Option<IdempotencyReplay>, AppError> {
    let key = match cli
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(k) => k,
        None => return Ok(None),
    };
    if !is_mutating_command(&cli.command) {
        return Ok(None);
    }

    let path = idempotency_store_path(cli);
    let store = load_idempotency_store(&path)?;
    let Some(entry) = store.entries.get(key) else {
        return Ok(None);
    };
    let fingerprint = command_fingerprint(cli);
    if entry.fingerprint != fingerprint || entry.command != command_name {
        return Err(AppError::Invalid(format!(
            "idempotency key '{}' was already used for a different command payload",
            key
        )));
    }

    Ok(Some(IdempotencyReplay {
        value: entry.value.clone(),
        recorded_at_unix_ms: entry.recorded_at_unix_ms,
    }))
}

fn record_idempotent_result(
    cli: &Cli,
    command_name: &str,
    value: &Value,
) -> Result<u128, AppError> {
    let key = match cli
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(k) => k.to_string(),
        None => return Ok(now_unix_ms()),
    };
    let path = idempotency_store_path(cli);
    let mut store = load_idempotency_store(&path)?;
    let fingerprint = command_fingerprint(cli);

    if let Some(existing) = store.entries.get(&key) {
        if existing.fingerprint != fingerprint || existing.command != command_name {
            return Err(AppError::Invalid(format!(
                "idempotency key '{}' was already used for a different command payload",
                key
            )));
        }
        return Ok(existing.recorded_at_unix_ms);
    }

    let recorded_at_unix_ms = now_unix_ms();
    store.entries.insert(
        key,
        IdempotencyEntry {
            command: command_name.to_string(),
            fingerprint,
            value: value.clone(),
            recorded_at_unix_ms,
        },
    );
    save_idempotency_store(&path, &store)?;
    Ok(recorded_at_unix_ms)
}

fn attach_idempotency_metadata(
    mut value: Value,
    key: &str,
    replayed: bool,
    recorded_at_unix_ms: u128,
) -> Value {
    if key.trim().is_empty() {
        return value;
    }
    let meta = json!({
        "key": key,
        "replayed": replayed,
        "recorded_at_unix_ms": recorded_at_unix_ms
    });

    if let Some(obj) = value.as_object_mut() {
        obj.insert("idempotency".to_string(), meta);
        return value;
    }

    json!({
        "result": value,
        "idempotency": meta
    })
}

pub(crate) async fn run(cli: Cli) -> Result<(Value, Cli), (AppError, Cli)> {
    let outcome = dispatch(&cli).await;
    match outcome {
        Ok(v) => Ok((v, cli)),
        Err(e) => Err((e, cli)),
    }
}

pub(crate) async fn dispatch(cli: &Cli) -> Result<Value, AppError> {
    let _lock = if needs_lock(&cli.command) {
        let path = profile_path(cli)?;
        Some(ProfileLock::acquire(&path)?)
    } else {
        None
    };

    match &cli.command {
        Command::Setup(args) => crate::commands::setup::run(cli, args).await,
        Command::Config(args) => crate::commands::config::run(cli, args).await,
        Command::Wallet(args) => crate::commands::wallet::run(cli, args).await,
        Command::Sync(args) => crate::commands::sync::run(cli, args).await,
        Command::Address(args) => crate::commands::address::run(cli, args).await,
        Command::Balance => crate::commands::balance::run(cli).await,
        Command::Tx(args) => crate::commands::tx::run(cli, args).await,
        Command::Psbt(args) => crate::commands::psbt::run(cli, args).await,
        Command::Account(args) => crate::commands::account::run(cli, args).await,
        Command::Wait(args) => crate::commands::wait::run(cli, args).await,
        Command::Snapshot(args) => crate::commands::snapshot::run(cli, args).await,
        Command::Lock(args) => crate::commands::lock::run(cli, args).await,
        Command::Scenario(args) => crate::commands::scenario::run(cli, args).await,
        Command::Inscription(args) => crate::commands::inscription::run(cli, args).await,
        Command::Doctor => crate::commands::doctor::run(cli).await,
        #[cfg(feature = "ui")]
        Command::Dashboard => crate::dashboard::run(cli).await,
    }
}

fn is_non_json_rendered_command(command: &Command) -> bool {
    match command {
        Command::Scenario(_) | Command::Doctor => true,
        #[cfg(feature = "ui")]
        Command::Dashboard => true,
        _ => false,
    }
}

pub(crate) fn needs_lock(command: &Command) -> bool {
    match command {
        Command::Setup { .. }
        | Command::Config { .. }
        | Command::Doctor
        | Command::Lock { .. }
        | Command::Psbt { .. } => false,
        _ => true,
    }
}

pub(crate) fn service_config(cli: &Cli) -> ServiceConfig<'_> {
    ServiceConfig {
        data_dir: cli.data_dir.as_deref(),
        profile: cli.profile.as_deref().unwrap_or("default"),
        password: cli.password.as_deref(),
        password_env: cli
            .password_env
            .as_deref()
            .unwrap_or("ZINC_WALLET_PASSWORD"),
        password_stdin: cli.password_stdin,
        json: cli.json,
        network_override: cli.network.as_deref(),
        explicit_network: cli.explicit_network,
        scheme_override: cli.scheme.as_deref(),
        esplora_url_override: cli.esplora_url.as_deref(),
        ord_url_override: cli.ord_url.as_deref(),
        ascii_mode: cli.ascii,
    }
}

pub(crate) fn load_wallet_session(cli: &Cli) -> Result<WalletSession, AppError> {
    svc_load_wallet_session(&service_config(cli)).map_err(AppError::from)
}

pub(crate) fn persist_wallet_session(session: &mut WalletSession) -> Result<(), AppError> {
    svc_persist_wallet_session(session).map_err(AppError::from)
}

pub(crate) fn profile_path(cli: &Cli) -> Result<PathBuf, AppError> {
    svc_profile_path(&service_config(cli)).map_err(AppError::from)
}

pub(crate) fn profile_lock_path(cli: &Cli) -> Result<PathBuf, AppError> {
    svc_profile_lock_path(&service_config(cli)).map_err(AppError::from)
}

pub(crate) fn snapshot_dir(cli: &Cli) -> Result<PathBuf, AppError> {
    svc_snapshot_dir(&service_config(cli)).map_err(AppError::from)
}

pub(crate) fn read_profile(path: &Path) -> Result<Profile, AppError> {
    svc_read_profile(path).map_err(AppError::from)
}

pub(crate) fn write_profile(path: &Path, profile: &Profile) -> Result<(), AppError> {
    svc_write_profile(path, profile).map_err(AppError::from)
}

pub(crate) fn read_lock_metadata(path: &Path) -> Option<LockMetadata> {
    svc_read_lock_metadata(path)
}

pub(crate) fn write_bytes_atomic(path: &Path, bytes: &[u8], label: &str) -> Result<(), AppError> {
    svc_write_bytes_atomic(path, bytes, label).map_err(AppError::from)
}

pub(crate) fn wallet_password(cli: &Cli) -> Result<String, AppError> {
    svc_wallet_password(&service_config(cli)).map_err(AppError::from)
}

pub(crate) fn confirm(prompt: &str, cli: &Cli) -> bool {
    if cli.yes || cli.json {
        return true;
    }

    if !io::stdin().is_terminal() {
        return false;
    }

    eprint!("{} [y/N]: ", prompt);
    io::stderr().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let input = input.trim().to_lowercase();
    input == "y" || input == "yes"
}

fn remap_suggestions(mut msg: String) -> String {
    msg = msg.replace("tip: a similar argument exists: ", "did you mean ");
    msg = msg.replace("tip: some similar arguments exist: ", "did you mean ");
    msg = msg.replace("tip: a similar subcommand exists: ", "did you mean ");
    msg = msg.replace("tip: some similar subcommands exist: ", "did you mean ");
    msg = msg.replace("tip: a similar value exists: ", "did you mean ");
    msg = msg.replace("tip: some similar values exist: ", "did you mean ");

    // Strip all single quotes from Clap v4 formatted error messages
    msg = msg.replace('\'', "");

    // Handle comma-separated multi-suggestions: "did you mean X, Y" → "did you mean X\ndid you mean Y"
    let mut result = String::new();
    for line in msg.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("did you mean ") {
            let suggestions_str = &trimmed["did you mean ".len()..];
            let suggestions: Vec<&str> = suggestions_str.split(", ").collect();
            for (i, s) in suggestions.iter().enumerate() {
                if i > 0 {
                    result.push('\n');
                }
                result.push_str("did you mean ");
                result.push_str(s);
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    // Remove trailing newline added by loop
    if result.ends_with('\n') && !msg.ends_with('\n') {
        result.pop();
    }

    result
}
