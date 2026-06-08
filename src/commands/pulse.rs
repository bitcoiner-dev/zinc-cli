use crate::cli::{Cli, PulseAction, PulseArgs, PulseOrdnetAction};
use crate::config::{load_persisted_config, save_persisted_config, PulseSession};
use crate::config_resolver::ConfigResolver;
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::CommandOutput;
use crate::pulse_auth_client::PulseAuthClient;
use crate::pulse_auth_resolver::PulseAuthResolver;
use crate::{now_unix, profile_path, read_profile, service_config, write_profile};
use reqwest::StatusCode;
use serde_json::json;

pub async fn run(cli: &Cli, args: &PulseArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        PulseAction::Login {
            legacy_token,
            token,
            global,
            no_open,
        } => {
            handle_login(
                cli,
                legacy_token.as_deref(),
                token.as_deref(),
                *global,
                *no_open,
            )
            .await
        }
        PulseAction::Whoami { global } => handle_whoami(cli, *global).await,
        PulseAction::Logout { global } => handle_logout(cli, *global).await,
        PulseAction::Ordnet { action } => match action {
            PulseOrdnetAction::Bind => handle_ordnet_bind(cli).await,
        },
    }
}

async fn handle_ordnet_bind(cli: &Cli) -> Result<CommandOutput, AppError> {
    let mut session = load_wallet_session(cli)?;
    session.require_seed_mode()?;
    let persisted = load_persisted_config().unwrap_or_default();
    let service = service_config(cli);
    let resolver = ConfigResolver::new(&persisted, &service);
    let auth_resolver = PulseAuthResolver::new(&persisted, &service);
    let path = profile_path(cli)?;
    let token = auth_resolver
        .resolve_token(Some(&mut session.profile), Some(&path))
        .await?
        .ok_or_else(|| {
            AppError::Auth("Pulse authentication required. Run 'zinc pulse login'.".to_string())
        })?;
    let pulse_url = resolver.resolve_pulse_url(Some(&session.profile)).value;
    if pulse_url.trim().is_empty() {
        return Err(AppError::Config(
            "Pulse URL not found. Use --pulse-url or zinc setup.".to_string(),
        ));
    }

    let ordinals_address = session.wallet.peek_taproot_address(0).to_string();
    let payment_address = session
        .wallet
        .peek_payment_address(0)
        .map(|address| address.to_string())
        .unwrap_or_else(|| ordinals_address.clone());
    let http = reqwest::Client::new();
    let challenge_url = format!(
        "{}/v1/ordnet/auth/challenge",
        pulse_url.trim_end_matches('/')
    );
    let challenge: serde_json::Value = http
        .post(challenge_url)
        .bearer_auth(&token)
        .json(&json!({
            "ordinalsAddress": ordinals_address,
            "paymentAddress": payment_address,
        }))
        .send()
        .await
        .map_err(|e| AppError::Network(format!("ord.net challenge request failed: {e}")))?
        .error_for_status()
        .map_err(|e| map_ordnet_http_error("ord.net challenge", e))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("failed to parse ord.net challenge: {e}")))?;

    let auth_request_id = challenge
        .get("authRequestId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::Internal("ord.net challenge missing authRequestId".to_string()))?;
    let challenges = challenge
        .get("challenges")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::Internal("ord.net challenge missing challenges".to_string()))?;
    let mut verifications = Vec::new();
    for challenge in challenges {
        let challenge_id = challenge
            .get("challengeId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                AppError::Internal("ord.net challenge missing challengeId".to_string())
            })?;
        let message = challenge
            .get("message")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::Internal("ord.net challenge missing message".to_string()))?;
        let address = challenge
            .get("address")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::Internal("ord.net challenge missing address".to_string()))?;
        let signature = session
            .wallet
            .sign_bip322_simple_hex(address, message)
            .map_err(crate::wallet_service::map_wallet_error)?;
        verifications.push(json!({
            "challengeId": challenge_id,
            "address": address,
            "signature": signature,
        }));
    }

    let verify_url = format!("{}/v1/ordnet/auth/verify", pulse_url.trim_end_matches('/'));
    let bound: serde_json::Value = http
        .post(verify_url)
        .bearer_auth(&token)
        .json(&json!({
            "authRequestId": auth_request_id,
            "verifications": verifications,
        }))
        .send()
        .await
        .map_err(|e| AppError::Network(format!("ord.net verify request failed: {e}")))?
        .error_for_status()
        .map_err(|e| map_ordnet_http_error("ord.net verify", e))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("failed to parse ord.net verify response: {e}")))?;

    Ok(CommandOutput::Generic(json!({
        "bound": true,
        "provider": "ord.net",
        "ordinals_address": ordinals_address,
        "payment_address": payment_address,
        "requires_confirmed_payment_balance_btc": "0.01",
        "raw_response": bound,
    })))
}

fn map_ordnet_http_error(context: &str, err: reqwest::Error) -> AppError {
    match err.status() {
        Some(StatusCode::UNAUTHORIZED) => AppError::Auth(format!("{context} unauthorized")),
        Some(StatusCode::PAYMENT_REQUIRED) | Some(StatusCode::FORBIDDEN) => {
            AppError::Capability(format!(
                "{context} rejected; ord.net requires an eligible wallet binding, including 0.01 BTC confirmed on the payment address"
            ))
        }
        Some(StatusCode::TOO_MANY_REQUESTS) => AppError::Network(format!("{context} rate limited")),
        Some(status) => AppError::Network(format!("{context} returned {status}")),
        None => AppError::Network(format!("{context} failed: {err}")),
    }
}

async fn handle_login(
    cli: &Cli,
    legacy_token: Option<&str>,
    token: Option<&str>,
    global: bool,
    no_open: bool,
) -> Result<CommandOutput, AppError> {
    // 1. Handle token-based login (legacy or flag)
    if let Some(t) = token.or(legacy_token) {
        let session = PulseSession {
            access_token: t.to_string(),
            refresh_token: None,
            expires_at_unix: now_unix() + 365 * 24 * 3600, // 1 year fallback
            metadata: Some(json!({ "manual": true })),
        };
        save_pulse_session(cli, session, global)?;
        if let Ok(pulse_url) = resolve_pulse_url_for_scope(cli, global) {
            persist_pulse_url(cli, &pulse_url, global)?;
        }
        return Ok(CommandOutput::Message(
            "Pulse session saved successfully via token.".into(),
        ));
    }

    // 2. OAuth Device Code Flow
    let pulse_url = resolve_pulse_url_for_scope(cli, global)?;
    let client = PulseAuthClient::new(pulse_url.clone());
    let client_id = "zinc-cli";

    let device_resp = client.start_device_authorization(client_id).await?;

    println!("Please visit: {}", device_resp.verification_uri);
    println!("And enter code: {}", device_resp.user_code);

    if !no_open {
        let _ = webbrowser::open(&device_resp.verification_uri);
    }

    println!("Waiting for authorization...");

    let token_resp = client
        .poll_for_token(
            client_id,
            &device_resp.device_code,
            device_resp.interval,
            device_resp.expires_in,
        )
        .await?;

    let session = PulseSession {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at_unix: now_unix() + token_resp.expires_in,
        metadata: None,
    };

    save_pulse_session(cli, session, global)?;
    persist_pulse_url(cli, &pulse_url, global)?;

    Ok(CommandOutput::Message(
        "Logged in successfully via OAuth.".to_string(),
    ))
}

async fn handle_whoami(cli: &Cli, global: bool) -> Result<CommandOutput, AppError> {
    let session = load_pulse_session(cli, global)?;
    let Some(session) = session else {
        return Ok(CommandOutput::Message("Not logged in.".into()));
    };

    let pulse_url = resolve_pulse_url_for_scope(cli, global)?;

    let client = PulseAuthClient::new(pulse_url);
    match client.whoami(&session.access_token).await {
        Ok(info) => Ok(CommandOutput::Generic(json!({
            "logged_in": true,
            "sub": info.sub,
            "client_id": info.client_id,
            "expires_at": info.expires_at,
            "scopes": info.scopes,
            "scope": if global { "global" } else { "profile" }
        }))),
        Err(_) => Ok(CommandOutput::Message(
            "Session found but server rejected token (expired or revoked).".into(),
        )),
    }
}

async fn handle_logout(cli: &Cli, global: bool) -> Result<CommandOutput, AppError> {
    let session = load_pulse_session(cli, global)?;
    let Some(session) = session else {
        return Ok(CommandOutput::Message("Already logged out.".into()));
    };

    if let Ok(pulse_url) = resolve_pulse_url_for_scope(cli, global) {
        let client = PulseAuthClient::new(pulse_url);
        let _ = client.revoke_token(&session.access_token).await;
    }

    clear_pulse_session(cli, global)?;

    Ok(CommandOutput::Message("Logged out successfully.".into()))
}

fn resolve_pulse_url_for_scope(cli: &Cli, global: bool) -> Result<String, AppError> {
    let persisted = load_persisted_config()?;
    let svc_cfg = service_config(cli);
    let resolver = ConfigResolver::new(&persisted, &svc_cfg);
    let profile = if global {
        None
    } else {
        let path = profile_path(cli)?;
        if path.exists() {
            Some(read_profile(&path)?)
        } else {
            None
        }
    };
    let pulse_url = resolver.resolve_pulse_url(profile.as_ref()).value;
    let trimmed = pulse_url.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config(
            "Pulse URL not found. Use --pulse-url or zinc setup.".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn persist_pulse_url(cli: &Cli, pulse_url: &str, global: bool) -> Result<(), AppError> {
    let trimmed = pulse_url.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if global {
        let mut config = load_persisted_config()?;
        config.pulse_url = Some(trimmed.to_string());
        save_persisted_config(&config)?;
    } else {
        let path = profile_path(cli)?;
        let mut profile = read_profile(&path)?;
        profile.pulse_url = trimmed.to_string();
        write_profile(&path, &profile)?;
    }
    Ok(())
}

fn save_pulse_session(cli: &Cli, session: PulseSession, global: bool) -> Result<(), AppError> {
    if global {
        let mut config = load_persisted_config()?;
        config.pulse_session = Some(session);
        save_persisted_config(&config)?;
    } else {
        let path = profile_path(cli)?;
        let mut profile = read_profile(&path)?;
        profile.pulse_session = Some(session);
        write_profile(&path, &profile)?;
    }
    Ok(())
}

fn load_pulse_session(cli: &Cli, global: bool) -> Result<Option<PulseSession>, AppError> {
    if global {
        let config = load_persisted_config()?;
        Ok(config.pulse_session)
    } else {
        let path = profile_path(cli)?;
        if !path.exists() {
            return Ok(None);
        }
        let profile = read_profile(&path)?;
        Ok(profile.pulse_session)
    }
}

fn clear_pulse_session(cli: &Cli, global: bool) -> Result<(), AppError> {
    if global {
        let mut config = load_persisted_config()?;
        config.pulse_session = None;
        save_persisted_config(&config)?;
    } else {
        let path = profile_path(cli)?;
        let mut profile = read_profile(&path)?;
        profile.pulse_session = None;
        write_profile(&path, &profile)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use crate::config::{
        default_gap_limit, default_scan_depth, load_persisted_config, write_profile, NetworkArg,
        PaymentAddressTypeArg, Profile, ProfileModeArg, SchemeArg,
    };
    use clap::Parser;
    use httpmock::prelude::*;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn pulse_args(cli: &Cli) -> &PulseArgs {
        match &cli.command {
            Command::Pulse(args) => args,
            _ => panic!("expected pulse command"),
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("zinc-cli-{prefix}-{}-{nanos}", std::process::id()));
        crate::paths::create_secure_dir_all(&dir).expect("failed to create temp directory");
        dir
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct HomeGuard {
        old_home: Option<OsString>,
    }

    impl HomeGuard {
        fn set(path: &std::path::Path) -> Self {
            let old_home = std::env::var_os("HOME");
            std::env::set_var("HOME", path);
            Self { old_home }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            if let Some(old_home) = self.old_home.take() {
                std::env::set_var("HOME", old_home);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    fn base_profile() -> Profile {
        Profile {
            version: 1,
            scan_policy_version: 1,
            network: NetworkArg::Bitcoin,
            scheme: SchemeArg::Dual,
            payment_address_type: PaymentAddressTypeArg::Native,
            account_index: 0,
            esplora_url: "https://m.exittheloop.com/api".to_string(),
            ord_url: "https://o.exittheloop.com".to_string(),
            pulse_url: String::new(),
            bitcoin_cli: "bitcoin-cli".to_string(),
            bitcoin_cli_args: vec!["-regtest".to_string()],
            encrypted_mnemonic: Some("encrypted".to_string()),
            mode: ProfileModeArg::Seed,
            taproot_xpub: None,
            payment_xpub: None,
            watch_address: None,
            account_gap_limit: default_gap_limit(),
            address_scan_depth: default_scan_depth(),
            accounts: BTreeMap::new(),
            updated_at_unix: 1,
            pulse_session: None,
        }
    }

    fn seed_profile(cli: &Cli, pulse_url: &str) {
        let path = profile_path(cli).expect("profile path");
        if let Some(parent) = path.parent() {
            crate::paths::create_secure_dir_all(parent).expect("create profile parent");
        }
        let mut profile = base_profile();
        profile.pulse_url = pulse_url.to_string();
        write_profile(&path, &profile).expect("write profile");
    }

    #[tokio::test]
    async fn profile_login_with_pulse_url_persists_and_whoami_resolves_without_override() {
        let _lock = env_lock().lock().expect("env lock");
        let temp_home = unique_temp_dir("pulse-profile-home");
        let _home_guard = HomeGuard::set(&temp_home);
        let data_dir = unique_temp_dir("pulse-profile-data");
        let data_dir_str = data_dir.to_string_lossy().to_string();

        let server = MockServer::start();
        let whoami_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/auth/whoami")
                .header("authorization", "Bearer profile-token");
            then.status(200).json_body(json!({
                "sub": "user-profile",
                "client_id": "zinc-cli",
                "expires_at": "2026-04-09T00:00:00Z",
                "scopes": ["read"]
            }));
        });

        let cli_login = Cli::try_parse_from([
            "zinc-cli",
            "--data-dir",
            &data_dir_str,
            "--profile",
            "alice",
            "--pulse-url",
            &server.base_url(),
            "pulse",
            "login",
            "--token",
            "profile-token",
        ])
        .expect("parse login cli");
        seed_profile(&cli_login, "");
        run(&cli_login, pulse_args(&cli_login))
            .await
            .expect("profile token login should succeed");

        let persisted_profile =
            read_profile(&profile_path(&cli_login).expect("profile path")).expect("read profile");
        assert_eq!(persisted_profile.pulse_url, server.base_url());

        let cli_whoami = Cli::try_parse_from([
            "zinc-cli",
            "--data-dir",
            &data_dir_str,
            "--profile",
            "alice",
            "pulse",
            "whoami",
        ])
        .expect("parse whoami cli");
        let output = run(&cli_whoami, pulse_args(&cli_whoami))
            .await
            .expect("whoami should resolve persisted profile pulse url");
        match output {
            CommandOutput::Generic(value) => {
                assert_eq!(value.get("logged_in").and_then(|v| v.as_bool()), Some(true));
                assert_eq!(value.get("scope").and_then(|v| v.as_str()), Some("profile"));
            }
            _ => panic!("expected generic whoami output"),
        }
        whoami_mock.assert();
    }

    #[tokio::test]
    async fn global_login_persists_global_url_and_whoami_global_ignores_profile_url() {
        let _lock = env_lock().lock().expect("env lock");
        let temp_home = unique_temp_dir("pulse-global-home");
        let _home_guard = HomeGuard::set(&temp_home);
        let data_dir = unique_temp_dir("pulse-global-data");
        let data_dir_str = data_dir.to_string_lossy().to_string();

        let server = MockServer::start();
        let whoami_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/auth/whoami")
                .header("authorization", "Bearer global-token");
            then.status(200).json_body(json!({
                "sub": "user-global",
                "client_id": "zinc-cli",
                "expires_at": "2026-04-09T00:00:00Z",
                "scopes": ["read"]
            }));
        });

        let cli_profile = Cli::try_parse_from([
            "zinc-cli",
            "--data-dir",
            &data_dir_str,
            "--profile",
            "alice",
            "pulse",
            "whoami",
        ])
        .expect("parse profile cli");
        seed_profile(&cli_profile, "http://127.0.0.1:9");

        let cli_login = Cli::try_parse_from([
            "zinc-cli",
            "--data-dir",
            &data_dir_str,
            "--profile",
            "alice",
            "--pulse-url",
            &server.base_url(),
            "pulse",
            "login",
            "--token",
            "global-token",
            "--global",
        ])
        .expect("parse global login cli");
        run(&cli_login, pulse_args(&cli_login))
            .await
            .expect("global token login should succeed");

        let persisted = load_persisted_config().expect("load persisted config");
        assert_eq!(
            persisted.pulse_url.as_deref(),
            Some(server.base_url().as_str())
        );
        assert!(persisted.pulse_session.is_some());

        let cli_whoami = Cli::try_parse_from([
            "zinc-cli",
            "--data-dir",
            &data_dir_str,
            "--profile",
            "alice",
            "pulse",
            "whoami",
            "--global",
        ])
        .expect("parse global whoami cli");
        let output = run(&cli_whoami, pulse_args(&cli_whoami))
            .await
            .expect("global whoami should use global pulse url");
        match output {
            CommandOutput::Generic(value) => {
                assert_eq!(value.get("logged_in").and_then(|v| v.as_bool()), Some(true));
                assert_eq!(value.get("scope").and_then(|v| v.as_str()), Some("global"));
            }
            _ => panic!("expected generic whoami output"),
        }
        whoami_mock.assert();
    }
}
