use crate::cli::{Cli, WalletAction, WalletArgs};
use crate::error::AppError;
use crate::utils::{parse_network, parse_scheme};
use crate::wallet_service::{
    decrypt_wallet_internal, default_bitcoin_cli, default_bitcoin_cli_args, default_esplora_url,
    default_ord_url, encrypt_wallet_internal, generate_wallet_internal, validate_mnemonic_internal,
    Profile,
};
use crate::{now_unix, profile_path, read_profile, wallet_password, write_profile};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub async fn run(cli: &Cli, args: &WalletArgs) -> Result<Value, AppError> {
    match &args.action {
        WalletAction::Init {
            words,
            network,
            scheme,
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

            let password = wallet_password(cli)?;
            let wallet = generate_wallet_internal(words)
                .map_err(|e| AppError::Internal(format!("failed to generate wallet: {e}")))?;
            let encrypted = encrypt_wallet_internal(&wallet.phrase, &password)
                .map_err(|e| AppError::Internal(format!("failed to encrypt mnemonic: {e}")))?;

            let profile = Profile {
                version: 1,
                network: network_arg,
                scheme: scheme_arg,
                account_index: 0,
                esplora_url: default_esplora_url(network_arg).to_string(),
                ord_url: default_ord_url(network_arg).to_string(),
                bitcoin_cli: default_bitcoin_cli(),
                bitcoin_cli_args: default_bitcoin_cli_args(),
                encrypted_mnemonic: encrypted,
                accounts: BTreeMap::new(),
                updated_at_unix: now_unix(),
            };
            write_profile(&profile_path, &profile)?;

            let phrase = if cli.reveal || (!cli.json && !cli.agent) {
                wallet.phrase.clone()
            } else {
                "<hidden; use --reveal to show>".to_string()
            };

            let mut res = json!({
                "profile": cli.profile,
                "version": 1,
                "network": network_arg,
                "scheme": scheme_arg,
                "account_index": 0,
                "esplora_url": default_esplora_url(network_arg).to_string(),
                "ord_url": default_ord_url(network_arg).to_string(),
                "bitcoin_cli": default_bitcoin_cli(),
                "bitcoin_cli_args": default_bitcoin_cli_args(),
                "phrase": phrase,
            });

            if cli.reveal || (!cli.json && !cli.agent) {
                res["words"] = json!(wallet.words);
            }

            Ok(res)
        }
        WalletAction::Import {
            mnemonic,
            network,
            scheme,
            overwrite,
        } => {
            if !validate_mnemonic_internal(mnemonic) {
                return Err(AppError::Invalid("invalid mnemonic".to_string()));
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
                None => crate::utils::parse_scheme("dual")?, // fallback
            };

            let password = wallet_password(cli)?;
            let encrypted = encrypt_wallet_internal(mnemonic, &password)
                .map_err(|e| AppError::Internal(format!("failed to encrypt mnemonic: {e}")))?;
            let profile = Profile {
                version: 1,
                network: network_arg,
                scheme: scheme_arg,
                account_index: 0,
                esplora_url: default_esplora_url(network_arg).to_string(),
                ord_url: default_ord_url(network_arg).to_string(),
                bitcoin_cli: default_bitcoin_cli(),
                bitcoin_cli_args: default_bitcoin_cli_args(),
                encrypted_mnemonic: encrypted,
                accounts: BTreeMap::new(),
                updated_at_unix: now_unix(),
            };
            write_profile(&profile_path, &profile)?;

            let mut res = json!({
                "profile": cli.profile,
                "network": network_arg,
                "scheme": scheme_arg,
                "account_index": 0,
                "imported": true
            });
            if cli.reveal || (!cli.json && !cli.agent) {
                res["phrase"] = json!(mnemonic.to_string());
            }

            Ok(res)
        }
        WalletAction::Info => {
            let profile = read_profile(&profile_path(cli)?)?;
            let state = profile.account_state();
            let res = json!({
                "profile": cli.profile,
                "version": profile.version,
                "network": profile.network,
                "scheme": profile.scheme,
                "account_index": profile.account_index,
                "esplora_url": profile.esplora_url,
                "ord_url": profile.ord_url,
                "bitcoin_cli": profile.bitcoin_cli,
                "bitcoin_cli_args": profile.bitcoin_cli_args,
                "has_persistence": state.persistence_json.is_some(),
                "has_inscriptions": state.inscriptions_json.is_some(),
                "updated_at_unix": profile.updated_at_unix
            });
            if cli.json {
                Ok(res)
            } else {
                let table = crate::presenter::wallet::format_wallet_info(&res);
                println!("{table}");
                Ok(Value::Null)
            }
        }
        WalletAction::RevealMnemonic => {
            let profile = read_profile(&profile_path(cli)?)?;
            let password = wallet_password(cli)?;
            let result = decrypt_wallet_internal(&profile.encrypted_mnemonic, &password)
                .map_err(|e| crate::wallet_service::map_wallet_error(e.to_string()))?;
            Ok(json!({
                "phrase": result.phrase,
                "words": result.phrase.split_whitespace().count()
            }))
        }
    }
}
