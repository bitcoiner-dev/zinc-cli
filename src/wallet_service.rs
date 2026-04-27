pub use crate::config::*;
pub use crate::lock::*;
pub use crate::paths::*;
use is_terminal::IsTerminal;
use rpassword::read_password;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ShellCommand;

pub use zinc_core::{
    decrypt_wallet_internal, encrypt_wallet_internal, generate_wallet_internal,
    validate_mnemonic_internal, Inscription, WalletBuilder, ZincMnemonic, ZincWallet,
};

use crate::error::AppError;

pub struct WalletSession {
    pub wallet: ZincWallet,
    pub profile: Profile,
    pub profile_path: PathBuf,
}

#[must_use]
pub fn map_wallet_error(message: String) -> AppError {
    let lower = message.to_ascii_lowercase();
    if lower.contains("wrong password") || lower.contains("decryption failed") {
        return AppError::Auth(message);
    }
    if lower.contains("insufficient") || lower.contains("not enough") {
        return AppError::InsufficientFunds(message);
    }
    if lower.contains("security violation")
        || lower.contains("safety lock")
        || lower.contains("ordinal shield")
    {
        return AppError::Policy(message);
    }
    if lower.contains("http")
        || lower.contains("network")
        || lower.contains("esplora")
        || lower.contains("request")
    {
        return AppError::Network(message);
    }
    if lower.contains("not found") {
        return AppError::NotFound(message);
    }
    AppError::Internal(message)
}

#[must_use]
pub fn read_lock_metadata(path: &Path) -> Option<LockMetadata> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str::<LockMetadata>(&data).ok()
}

pub fn wallet_password(config: &ServiceConfig<'_>) -> Result<String, AppError> {
    if let Some(pass) = config.password {
        if pass.is_empty() {
            return Err(AppError::Auth("--password cannot be empty".to_string()));
        }
        return Ok(pass.to_string());
    }

    if config.password_stdin {
        let mut buffer = String::new();
        io::stdin()
            .read_line(&mut buffer)
            .map_err(|e| AppError::Auth(format!("failed to read password from stdin: {e}")))?;
        let pass = buffer.trim().to_string();
        if pass.is_empty() {
            return Err(AppError::Auth("password from stdin is empty".to_string()));
        }
        return Ok(pass);
    }

    if let Some(pass) = std::env::var_os(config.password_env) {
        let pass = pass.to_string_lossy().to_string();
        if pass.is_empty() {
            return Err(AppError::Auth(format!(
                "environment variable {} is empty",
                config.password_env
            )));
        }
        return Ok(pass);
    }

    if config.agent {
        return Err(AppError::Auth(format!(
            "wallet password missing (use --password, --password-stdin, or set {})",
            config.password_env
        )));
    }

    if io::stdin().is_terminal() {
        eprint!("Enter wallet password: ");
        io::stderr().flush().ok();
        let pass =
            read_password().map_err(|e| AppError::Auth(format!("failed to read password: {e}")))?;

        return Ok(pass);
    }

    Err(AppError::Auth(format!(
        "wallet password missing (use --password, --password-stdin, set {}, or run in interactive terminal)",
        config.password_env
    )))
}

pub fn load_wallet_session(config: &ServiceConfig<'_>) -> Result<WalletSession, AppError> {
    let path = profile_path(config)?;
    let mut profile = read_profile(&path)?;
    if apply_scan_policy_migration(&mut profile) {
        write_profile(&path, &profile)?;
    }

    // Use ConfigResolver to apply overrides correctly
    let persisted = load_persisted_config().unwrap_or_default();
    let resolver = crate::config_resolver::ConfigResolver::new(&persisted, config);

    let new_network: crate::config::NetworkArg =
        resolver.resolve_network(Some(&profile)).value.into();
    let new_scheme: crate::config::SchemeArg = resolver.resolve_scheme(Some(&profile)).value.into();

    let network_changed = profile.network.to_string() != new_network.to_string();
    let scheme_changed = profile.scheme.to_string() != new_scheme.to_string();

    if network_changed || scheme_changed {
        for state in profile.accounts.values_mut() {
            state.persistence_json = None;
            state.inscriptions_json = None;
        }
    }

    if network_changed {
        profile.esplora_url = crate::config::default_esplora_url(new_network).to_string();
        profile.ord_url = crate::config::default_ord_url(new_network).to_string();
    }

    profile.network = new_network;
    profile.scheme = new_scheme;

    if let Some(e) = config.esplora_url_override {
        profile.esplora_url = e.to_string();
    }
    if let Some(url) = config.ord_url_override {
        profile.ord_url = url.to_string();
    }

    let password = wallet_password(config)?;

    let wallet_phrase = decrypt_wallet_internal(&profile.encrypted_mnemonic, &password)
        .map_err(|_| {
            AppError::Auth("wallet decrypt failed (wrong password or corrupted vault)".to_string())
        })?
        .phrase;

    let mnemonic = ZincMnemonic::parse(&wallet_phrase)
        .map_err(|e| AppError::Internal(format!("invalid mnemonic in vault: {e}")))?;

    let mut builder = WalletBuilder::from_mnemonic(profile.network.into(), &mnemonic)
        .with_scheme(profile.scheme.into())
        .with_account_index(profile.account_index);

    let state = profile.account_state();
    if let Some(persistence_json) = &state.persistence_json {
        builder = builder
            .with_persistence(persistence_json)
            .map_err(|e| AppError::Config(format!("failed to load persistence: {e}")))?;
    }

    let mut wallet = builder
        .build()
        .map_err(|e| AppError::Internal(format!("wallet build failed: {e}")))?;

    if let Some(inscriptions_json) = &state.inscriptions_json {
        let inscriptions: Vec<Inscription> = serde_json::from_str(inscriptions_json)
            .map_err(|e| AppError::Config(format!("invalid inscription cache: {e}")))?;
        let protected_outpoints = inscriptions
            .iter()
            .map(|inscription| inscription.satpoint.outpoint)
            .collect();
        wallet.apply_verified_ordinals_update(inscriptions, protected_outpoints);
    }

    Ok(WalletSession {
        wallet,
        profile,
        profile_path: path,
    })
}

fn apply_scan_policy_migration(profile: &mut Profile) -> bool {
    if profile.scan_policy_version >= SCAN_POLICY_VERSION_MAIN_ONLY {
        return false;
    }

    for state in profile.accounts.values_mut() {
        state.persistence_json = None;
        state.inscriptions_json = None;
    }
    profile.scan_policy_version = SCAN_POLICY_VERSION_MAIN_ONLY;
    profile.updated_at_unix = now_unix();
    true
}

pub fn persist_wallet_session(session: &mut WalletSession) -> Result<(), AppError> {
    let persistence = session
        .wallet
        .export_changeset()
        .map_err(map_wallet_error)
        .and_then(|persist| {
            serde_json::to_string(&persist)
                .map_err(|e| AppError::Internal(format!("persistence serialize failed: {e}")))
        })?;

    let inscriptions = serde_json::to_string(session.wallet.inscriptions())
        .map_err(|e| AppError::Internal(format!("inscription cache serialize failed: {e}")))?;

    session.profile.set_account_state(AccountState {
        persistence_json: Some(persistence),
        inscriptions_json: Some(inscriptions),
    });

    write_profile(&session.profile_path, &session.profile)
}

#[allow(dead_code)]
pub fn run_bitcoin_cli(profile: &Profile, args: &[String]) -> Result<String, AppError> {
    crate::utils::validate_bitcoin_cli_path(&profile.bitcoin_cli)?;

    let output = ShellCommand::new(&profile.bitcoin_cli)
        .args(&profile.bitcoin_cli_args)
        .args(args)
        .output()
        .map_err(|e| AppError::Config(format!("failed to launch {}: {e}", profile.bitcoin_cli)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(AppError::Network(format!(
            "bitcoin-cli command failed: {} {}",
            profile.bitcoin_cli, details
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AccountState, NetworkArg, Profile, SchemeArg};
    use std::collections::BTreeMap;

    #[test]
    fn scan_policy_migration_clears_cached_account_state_once() {
        let mut accounts = BTreeMap::new();
        accounts.insert(
            0,
            AccountState {
                persistence_json: Some("{\"mock\":true}".to_string()),
                inscriptions_json: Some("[{\"id\":\"i0\"}]".to_string()),
            },
        );
        accounts.insert(
            1,
            AccountState {
                persistence_json: Some("{\"mock\":true}".to_string()),
                inscriptions_json: None,
            },
        );

        let mut profile = Profile {
            version: 1,
            scan_policy_version: 0,
            network: NetworkArg::Regtest,
            scheme: SchemeArg::Dual,
            account_index: 0,
            esplora_url: "https://regtest.exittheloop.com/api".to_string(),
            ord_url: "https://ord-regtest.exittheloop.com".to_string(),
            bitcoin_cli: "bitcoin-cli".to_string(),
            bitcoin_cli_args: vec!["-regtest".to_string()],
            encrypted_mnemonic: "encrypted".to_string(),
            accounts,
            updated_at_unix: 123,
        };

        assert!(apply_scan_policy_migration(&mut profile));
        assert_eq!(profile.scan_policy_version, SCAN_POLICY_VERSION_MAIN_ONLY);
        assert!(profile
            .accounts
            .values()
            .all(|state| state.persistence_json.is_none() && state.inscriptions_json.is_none()));

        let migrated_updated_at = profile.updated_at_unix;
        assert!(!apply_scan_policy_migration(&mut profile));
        assert_eq!(profile.scan_policy_version, SCAN_POLICY_VERSION_MAIN_ONLY);
        assert_eq!(profile.updated_at_unix, migrated_updated_at);
    }
}
