use crate::cli::{Cli, WalletAction, WalletArgs};
use crate::config::load_persisted_config;
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::utils::{parse_network, parse_payment_address_type, parse_scheme};
use crate::wallet_service::{
    decrypt_wallet_internal, default_bitcoin_cli, default_bitcoin_cli_args, default_esplora_url,
    default_ord_url, default_pulse_url, encrypt_wallet_internal, generate_wallet_internal,
    validate_mnemonic_internal, Profile, WalletBuilder,
};
use crate::{now_unix, profile_path, read_profile, wallet_password, write_profile};
use std::collections::BTreeMap;

pub async fn run(cli: &Cli, args: &WalletArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        WalletAction::Init {
            words,
            network,
            scheme,
            payment_address_type,
            overwrite,
        } => {
            let words = words.unwrap_or(12);
            if words != 12 && words != 24 {
                return Err(AppError::Invalid("--words must be 12 or 24".to_string()));
            }

            let profile_path = profile_path(cli)?;
            if profile_path.exists() && !overwrite {
                return Err(AppError::Invalid(format!(
                    "profile '{}' already exists. Use --overwrite to replace it.",
                    cli.profile.as_deref().unwrap_or("default")
                )));
            }

            let network_arg = match network.as_deref().or(cli.network.as_deref()) {
                Some(n) => parse_network(n)?,
                None => parse_network("regtest")?, // fallback
            };
            let scheme_arg = match scheme.as_deref().or(cli.scheme.as_deref()) {
                Some(s) => parse_scheme(s)?,
                None => parse_scheme("dual")?, // fallback
            };
            let payment_address_type_arg = match payment_address_type
                .as_deref()
                .or(cli.payment_address_type.as_deref())
            {
                Some(value) => parse_payment_address_type(value)?,
                None => parse_payment_address_type("native")?,
            };

            let password = wallet_password(cli)?;
            let wallet = generate_wallet_internal(words)
                .map_err(|e| AppError::Internal(format!("failed to generate wallet: {e}")))?;
            let encrypted = encrypt_wallet_internal(&wallet.phrase, &password)
                .map_err(|e| AppError::Internal(format!("failed to encrypt mnemonic: {e}")))?;

            let profile = Profile {
                version: 1,
                scan_policy_version: crate::config::SCAN_POLICY_VERSION_MAIN_ONLY,
                network: network_arg,
                scheme: scheme_arg,
                payment_address_type: payment_address_type_arg,
                account_index: 0,
                esplora_url: default_esplora_url(network_arg).to_string(),
                ord_url: default_ord_url(network_arg).to_string(),
                pulse_url: default_pulse_url(network_arg).to_string(),
                bitcoin_cli: default_bitcoin_cli(),
                bitcoin_cli_args: default_bitcoin_cli_args(),
                encrypted_mnemonic: Some(encrypted),
                mode: crate::config::ProfileModeArg::Seed,
                taproot_xpub: None,
                payment_xpub: None,
                watch_address: None,
                account_gap_limit: crate::config::default_gap_limit(),
                address_scan_depth: crate::config::default_scan_depth(),
                accounts: BTreeMap::new(),
                updated_at_unix: now_unix(),
            };
            write_profile(&profile_path, &profile)?;

            let phrase = if cli.reveal || !cli.agent {
                wallet.phrase.clone()
            } else {
                "<hidden; use --reveal to show>".to_string()
            };

            Ok(CommandOutput::WalletInit {
                profile: cli.profile.clone(),
                version: 1,
                network: network_arg.to_string(),
                scheme: scheme_arg.to_string(),
                payment_address_type: payment_address_type_arg.to_string(),
                account_index: 0,
                esplora_url: default_esplora_url(network_arg).to_string(),
                ord_url: default_ord_url(network_arg).to_string(),
                pulse_url: default_pulse_url(network_arg).to_string(),
                bitcoin_cli: default_bitcoin_cli(),
                bitcoin_cli_args: default_bitcoin_cli_args().join(" "),
                phrase,
                words: if cli.reveal || !cli.agent {
                    Some(wallet.words.len())
                } else {
                    None
                },
            })
        }
        WalletAction::Import {
            mnemonic,
            taproot_xpub,
            payment_xpub,
            address,
            network,
            scheme,
            payment_address_type,
            overwrite,
        } => {
            if let Some(m) = mnemonic {
                if !validate_mnemonic_internal(m) {
                    return Err(AppError::Invalid("invalid mnemonic".to_string()));
                }
            }

            let is_seed_import = mnemonic.is_some();
            let has_taproot_xpub = taproot_xpub.is_some();
            let has_payment_xpub = payment_xpub.is_some();
            let is_address_watch_import = address.is_some();

            if is_seed_import && (has_taproot_xpub || has_payment_xpub || is_address_watch_import) {
                return Err(AppError::Invalid(
                    "Seed import cannot be combined with --taproot-xpub, --payment-xpub, or --address"
                        .to_string(),
                ));
            }
            if !is_seed_import && is_address_watch_import && (has_taproot_xpub || has_payment_xpub)
            {
                return Err(AppError::Invalid(
                    "Address watch import cannot be combined with xpub flags".to_string(),
                ));
            }
            if !is_seed_import && !is_address_watch_import && !has_taproot_xpub {
                return Err(AppError::Invalid(
                    "Provide one of: --mnemonic, --taproot-xpub (or --xpub), or --address"
                        .to_string(),
                ));
            }
            if has_payment_xpub && !has_taproot_xpub {
                return Err(AppError::Invalid(
                    "--payment-xpub requires --taproot-xpub".to_string(),
                ));
            }

            let profile_path = profile_path(cli)?;
            if profile_path.exists() && !overwrite {
                return Err(AppError::Invalid(format!(
                    "profile '{}' already exists. Use --overwrite to replace it.",
                    cli.profile.as_deref().unwrap_or("default")
                )));
            }

            let network_arg = match network.as_deref().or(cli.network.as_deref()) {
                Some(n) => crate::utils::parse_network(n)?,
                None => crate::utils::parse_network("regtest")?, // fallback
            };
            let scheme_arg = match scheme.as_deref().or(cli.scheme.as_deref()) {
                Some(s) => crate::utils::parse_scheme(s)?,
                None => {
                    if is_address_watch_import {
                        crate::utils::parse_scheme("unified")?
                    } else {
                        crate::utils::parse_scheme("dual")?
                    }
                }
            };
            let payment_address_type_arg = match payment_address_type
                .as_deref()
                .or(cli.payment_address_type.as_deref())
            {
                Some(value) => parse_payment_address_type(value)?,
                None => parse_payment_address_type("native")?,
            };

            if is_address_watch_import && scheme_arg != crate::config::SchemeArg::Unified {
                return Err(AppError::Invalid(
                    "Address watch import supports unified scheme only".to_string(),
                ));
            }

            let (encrypted, mode, taproot_xpub_field, payment_xpub_field, watch_address_field) =
                if let Some(m) = mnemonic {
                    let password = wallet_password(cli)?;
                    let enc = encrypt_wallet_internal(m, &password).map_err(|e| {
                        AppError::Internal(format!("failed to encrypt mnemonic: {e}"))
                    })?;
                    (
                        Some(enc),
                        crate::config::ProfileModeArg::Seed,
                        None,
                        None,
                        None,
                    )
                } else if let Some(addr) = address {
                    WalletBuilder::from_watch_only(network_arg.into())
                        .with_watch_address(addr)
                        .map_err(|e| AppError::Invalid(format!("invalid watch address: {e}")))?;

                    (
                        None,
                        crate::config::ProfileModeArg::WatchAddress,
                        None,
                        None,
                        Some(addr.clone()),
                    )
                } else {
                    (
                        None,
                        crate::config::ProfileModeArg::Watch,
                        taproot_xpub.clone(),
                        payment_xpub.clone(),
                        None,
                    )
                };

            let profile = Profile {
                version: 1,
                scan_policy_version: crate::config::SCAN_POLICY_VERSION_MAIN_ONLY,
                network: network_arg,
                scheme: scheme_arg,
                payment_address_type: payment_address_type_arg,
                account_index: 0,
                esplora_url: default_esplora_url(network_arg).to_string(),
                ord_url: default_ord_url(network_arg).to_string(),
                pulse_url: default_pulse_url(network_arg).to_string(),
                bitcoin_cli: default_bitcoin_cli(),
                bitcoin_cli_args: default_bitcoin_cli_args(),
                encrypted_mnemonic: encrypted,
                mode,
                taproot_xpub: taproot_xpub_field,
                payment_xpub: payment_xpub_field,
                watch_address: watch_address_field,
                account_gap_limit: crate::config::default_gap_limit(),
                address_scan_depth: crate::config::default_scan_depth(),
                accounts: BTreeMap::new(),
                updated_at_unix: now_unix(),
            };
            write_profile(&profile_path, &profile)?;

            Ok(CommandOutput::WalletImport {
                profile: cli.profile.clone(),
                network: network_arg.to_string(),
                scheme: scheme_arg.to_string(),
                payment_address_type: payment_address_type_arg.to_string(),
                account_index: 0,
                pulse_url: profile.pulse_url.clone(),
                imported: true,
                phrase: if mnemonic.is_some() && (cli.reveal || !cli.agent) {
                    mnemonic.clone()
                } else {
                    None
                },
            })
        }
        WalletAction::Info => {
            let mut profile = read_profile(&profile_path(cli)?)?;

            // Match runtime resolution used by wallet session loading so
            // wallet info reflects effective config/default overrides.
            let service_cfg = crate::service_config(cli);
            let persisted = load_persisted_config().unwrap_or_default();
            let resolver = crate::config_resolver::ConfigResolver::new(&persisted, &service_cfg);

            let resolved_network: crate::config::NetworkArg =
                resolver.resolve_network(Some(&profile)).value.into();
            let resolved_scheme: crate::config::SchemeArg =
                resolver.resolve_scheme(Some(&profile)).value.into();
            let resolved_payment_address_type: crate::config::PaymentAddressTypeArg = resolver
                .resolve_payment_address_type(Some(&profile))
                .value
                .into();

            let network_changed = profile.network.to_string() != resolved_network.to_string();
            if network_changed {
                profile.esplora_url = default_esplora_url(resolved_network).to_string();
                profile.ord_url = default_ord_url(resolved_network).to_string();
            }

            profile.network = resolved_network;
            profile.scheme = resolved_scheme;
            profile.payment_address_type = resolved_payment_address_type;

            if let Some(e) = service_cfg.esplora_url_override {
                profile.esplora_url = e.to_string();
            }
            if let Some(url) = service_cfg.ord_url_override {
                profile.ord_url = url.to_string();
            }
            if let Some(url) = service_cfg.pulse_url_override {
                profile.pulse_url = url.to_string();
            }

            let state = profile.account_state();
            Ok(CommandOutput::WalletInfo {
                profile: cli.profile.clone(),
                version: profile.version,
                network: profile.network.to_string(),
                scheme: profile.scheme.to_string(),
                payment_address_type: profile.payment_address_type.to_string(),
                account_index: profile.account_index,
                esplora_url: profile.esplora_url.clone(),
                ord_url: profile.ord_url.clone(),
                pulse_url: profile.pulse_url.clone(),
                bitcoin_cli: profile.bitcoin_cli.clone(),
                bitcoin_cli_args: profile.bitcoin_cli_args.join(" "),
                has_persistence: state.persistence_json.is_some(),
                has_inscriptions: state.inscriptions_json.is_some(),
                updated_at_unix: profile.updated_at_unix,
            })
        }
        WalletAction::RevealMnemonic => {
            let profile = read_profile(&profile_path(cli)?)?;
            if profile.mode != crate::config::ProfileModeArg::Seed {
                return Err(AppError::Capability(
                    "This command requires a 'Seed' profile and cannot be performed in 'Watch' mode."
                        .to_string(),
                ));
            }
            let password = wallet_password(cli)?;
            let encrypted = profile.encrypted_mnemonic.as_ref().ok_or_else(|| {
                AppError::Config("Seed profile missing encrypted mnemonic".to_string())
            })?;
            let result = decrypt_wallet_internal(encrypted, &password)
                .map_err(|e| crate::wallet_service::map_wallet_error(e.to_string()))?;
            Ok(CommandOutput::RevealMnemonic {
                phrase: result.phrase.clone(),
                words: result.phrase.split_whitespace().count(),
            })
        }
    }
}
