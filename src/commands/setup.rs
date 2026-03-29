use crate::cli::{Cli, SetupArgs};
use crate::config::{load_persisted_config, save_persisted_config};
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::utils::parse_network;
use crate::utils::parse_scheme;
use crate::wallet_service::{
    default_bitcoin_cli, default_bitcoin_cli_args, default_esplora_url, default_ord_url,
    validate_mnemonic_internal, Profile,
};
use crate::wizard::{resolve_setup_values, run_tui_setup_wizard, should_run_setup_wizard};
use crate::{now_unix, profile_path, wallet_password, write_profile};
use serde_json::json;
use std::io::IsTerminal;
use zinc_core::{encrypt_wallet_internal, generate_wallet_internal};

pub async fn run(cli: &Cli, args: &SetupArgs) -> Result<CommandOutput, AppError> {
    let seed = resolve_setup_values(cli, args)?;

    let is_tui = should_run_setup_wizard(
        args,
        cli,
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    );
    let values = if is_tui {
        run_tui_setup_wizard(seed).await?
    } else {
        seed
    };

    // ── Persist config ──────────────────────────────────────────────────
    let mut config = load_persisted_config()?;
    config.profile = Some(values.profile.clone());
    config.data_dir = values.data_dir.clone();
    config.password_env = Some(values.password_env.clone());
    config.network = values.default_network.clone();
    config.scheme = values.default_scheme.clone();
    config.esplora_url = values.default_esplora_url.clone();
    config.ord_url = values.default_ord_url.clone();
    save_persisted_config(&config)?;

    // ── Optionally initialize wallet ────────────────────────────────────
    let wallet_result = if values.initialize_wallet {
        let network_str = values
            .default_network
            .as_deref()
            .or(cli.network.as_deref())
            .unwrap_or("regtest");
        let network = parse_network(network_str)?;
        let scheme_str = values
            .default_scheme
            .as_deref()
            .or(cli.scheme.as_deref())
            .unwrap_or("dual");
        let scheme = parse_scheme(scheme_str)?;

        // Build a temporary Cli for wallet operations
        let wallet_cli = Cli {
            profile: Some(values.profile.clone()),
            data_dir: values
                .data_dir
                .as_ref()
                .map(|s| std::path::PathBuf::from(s)),
            password: values.password.clone().or(cli.password.clone()),
            password_env: Some(values.password_env.clone()),
            password_stdin: cli.password_stdin,
            reveal: cli.reveal,
            yes: cli.yes,
            agent: cli.agent,
            no_images: cli.no_images,
            ascii: false,
            thumb: cli.thumb_enabled(),
            no_thumb: cli.no_thumb,
            correlation_id: cli.correlation_id.clone(),
            log_json: cli.log_json,
            idempotency_key: cli.idempotency_key.clone(),
            network_timeout_secs: cli.network_timeout_secs,
            network_retries: cli.network_retries,
            policy_mode: cli.policy_mode,
            explicit_network: false,
            started_at_unix_ms: cli.started_at_unix_ms,
            network: values.default_network.clone(),
            scheme: values.default_scheme.clone(),
            esplora_url: values.default_esplora_url.clone(),
            ord_url: values.default_ord_url.clone(),
            command: crate::cli::Command::Doctor, // placeholder, not used
        };

        let password = if let Some(ref p) = values.password {
            p.clone()
        } else {
            wallet_password(&wallet_cli)?
        };

        let words = values.words.unwrap_or(12);
        if words != 12 && words != 24 {
            return Err(AppError::Invalid(
                "setup --words must be 12 or 24".to_string(),
            ));
        }

        let (phrase, word_count) = if let Some(ref mnemonic) = values.restore_mnemonic {
            let p = mnemonic.trim().to_string();
            if !validate_mnemonic_internal(&p) {
                return Err(AppError::Invalid(
                    "setup restore mnemonic is invalid".to_string(),
                ));
            }
            let count = p.split_whitespace().count();
            (p, count)
        } else {
            let result = generate_wallet_internal(words)
                .map_err(|e| AppError::Internal(format!("failed to generate wallet: {e}")))?;
            (result.phrase, result.words.len())
        };

        let encrypted = encrypt_wallet_internal(&phrase, &password)
            .map_err(|e| AppError::Internal(format!("failed to encrypt mnemonic: {e}")))?;

        let esplora_url = values
            .default_esplora_url
            .clone()
            .unwrap_or_else(|| default_esplora_url(network).to_string());
        let ord_url = values
            .default_ord_url
            .clone()
            .unwrap_or_else(|| default_ord_url(network).to_string());

        let profile = Profile {
            version: 1,
            scan_policy_version: crate::config::SCAN_POLICY_VERSION_MAIN_ONLY,
            network,
            scheme,
            account_index: 0,
            esplora_url,
            ord_url,
            bitcoin_cli: default_bitcoin_cli(),
            bitcoin_cli_args: default_bitcoin_cli_args(),
            encrypted_mnemonic: encrypted,
            accounts: std::collections::BTreeMap::new(),
            updated_at_unix: now_unix(),
        };
        let profile_path = profile_path(&wallet_cli)?;
        write_profile(&profile_path, &profile)?;

        let display_phrase = if cli.reveal {
            phrase.to_string()
        } else {
            "<hidden; use --reveal to show>".to_string()
        };

        Some(json!({
            "wallet_initialized": true,
            "mode": if values.restore_mnemonic.is_some() { "restore" } else { "generate" },
            "phrase": display_phrase,
            "word_count": word_count,
        }))
    } else {
        None
    };

    #[cfg(feature = "ui")]
    if is_tui {
        // If we ran the TUI, transition directly to the dashboard
        return crate::dashboard::run(cli).await.map(CommandOutput::Generic);
    }

    let mut wallet_initialized = false;
    let mut wallet_mode = None;
    let mut wallet_phrase = None;
    let mut wallet_word_count = None;
    if let Some(wallet_info) = wallet_result {
        wallet_initialized = wallet_info["wallet_initialized"].as_bool().unwrap_or(false);
        wallet_mode = wallet_info["mode"].as_str().map(|s: &str| s.to_string());
        wallet_phrase = wallet_info["phrase"].as_str().map(|s: &str| s.to_string());
        wallet_word_count = wallet_info["word_count"].as_u64().map(|v| v as usize);
    }

    Ok(CommandOutput::Setup {
        config_saved: true,
        wizard_used: is_tui,
        profile: Some(values.profile),
        data_dir: crate::wallet_service::data_dir(&crate::service_config(cli))
            .display()
            .to_string(),
        default_network: values
            .default_network
            .as_deref()
            .or(cli.network.as_deref())
            .unwrap_or("regtest")
            .to_string(),
        default_scheme: values
            .default_scheme
            .as_deref()
            .or(cli.scheme.as_deref())
            .unwrap_or("dual")
            .to_string(),
        default_esplora_url: values
            .default_esplora_url
            .as_deref()
            .or(cli.esplora_url.as_deref())
            .unwrap_or_else(|| default_esplora_url(parse_network("regtest").unwrap()))
            .to_string(),
        default_ord_url: values
            .default_ord_url
            .as_deref()
            .or(cli.ord_url.as_deref())
            .unwrap_or_else(|| default_ord_url(parse_network("regtest").unwrap()))
            .to_string(),
        password_env: values.password_env.clone(),
        wallet_requested: values.initialize_wallet,
        wallet_initialized,
        wallet_mode,
        wallet_phrase,
        wallet_word_count,
    })
}
