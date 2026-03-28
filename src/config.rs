use crate::error::AppError;
use crate::paths::write_bytes_atomic;
use crate::utils::{parse_bool_value, parse_network, parse_scheme, unknown_with_hint};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use zinc_core::{AddressScheme, Network};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedConfig {
    pub profile: Option<String>,
    pub data_dir: Option<String>,
    pub password_env: Option<String>,
    pub network: Option<String>,
    pub scheme: Option<String>,
    pub esplora_url: Option<String>,
    pub ord_url: Option<String>,
    pub ascii: Option<bool>,
}

impl Default for PersistedConfig {
    fn default() -> Self {
        Self {
            profile: None,
            data_dir: None,
            password_env: None,
            network: None,
            scheme: None,
            esplora_url: None,
            ord_url: None,
            ascii: None,
        }
    }
}

pub(crate) fn persisted_config_path() -> PathBuf {
    crate::utils::home_dir()
        .join(".zinc-cli")
        .join("config.json")
}

pub(crate) fn load_persisted_config() -> Result<PersistedConfig, AppError> {
    let path = persisted_config_path();
    if !path.exists() {
        return Ok(PersistedConfig::default());
    }

    let data = fs::read_to_string(&path)
        .map_err(|e| AppError::Config(format!("failed to read config {}: {e}", path.display())))?;
    serde_json::from_str::<PersistedConfig>(&data)
        .map_err(|e| AppError::Config(format!("failed to parse config {}: {e}", path.display())))
}

pub(crate) fn save_persisted_config(config: &PersistedConfig) -> Result<(), AppError> {
    let path = persisted_config_path();
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| AppError::Internal(format!("failed to serialize config: {e}")))?;
    Ok(write_bytes_atomic(&path, &bytes, "config")?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Profile,
    DataDir,
    PasswordEnv,
    Network,
    Scheme,
    EsploraUrl,
    OrdUrl,
    Ascii,
}

pub const CONFIG_KEYS: &[&str] = &[
    "profile",
    "data-dir",
    "password-env",
    "network",
    "scheme",
    "esplora-url",
    "ord-url",
    "ascii",
];

impl ConfigField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::DataDir => "data-dir",
            Self::PasswordEnv => "password-env",
            Self::Network => "network",
            Self::Scheme => "scheme",
            Self::EsploraUrl => "esplora-url",
            Self::OrdUrl => "ord-url",
            Self::Ascii => "ascii",
        }
    }

    pub fn parse(key: &str) -> Result<Self, AppError> {
        match key {
            "profile" => Ok(Self::Profile),
            "data-dir" | "data_dir" => Ok(Self::DataDir),
            "password-env" | "password_env" => Ok(Self::PasswordEnv),
            "network" => Ok(Self::Network),
            "scheme" => Ok(Self::Scheme),
            "esplora-url" | "esplora_url" => Ok(Self::EsploraUrl),
            "ord-url" | "ord_url" => Ok(Self::OrdUrl),
            "ascii" => Ok(Self::Ascii),
            other => Err(AppError::Invalid(unknown_with_hint(
                "config key",
                other,
                CONFIG_KEYS,
            ))),
        }
    }
}

pub(crate) fn set_config_field(
    config: &mut PersistedConfig,
    key: ConfigField,
    raw_value: &str,
) -> Result<Value, AppError> {
    let value = raw_value.trim();
    if value.is_empty() {
        return Err(AppError::Invalid(format!(
            "config value for {} cannot be empty",
            key.as_str()
        )));
    }

    match key {
        ConfigField::Profile => {
            config.profile = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::DataDir => {
            config.data_dir = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::PasswordEnv => {
            config.password_env = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::Network => {
            let parsed = parse_network(value)?;
            let canonical = parsed.to_string();
            config.network = Some(canonical.clone());
            Ok(Value::String(canonical))
        }
        ConfigField::Scheme => {
            let parsed = parse_scheme(value)?;
            let canonical = parsed.to_string();
            config.scheme = Some(canonical.clone());
            Ok(Value::String(canonical))
        }
        ConfigField::EsploraUrl => {
            config.esplora_url = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::OrdUrl => {
            config.ord_url = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::Ascii => {
            let parsed = parse_bool_value(value, "config ascii").map_err(AppError::Invalid)?;
            config.ascii = Some(parsed);
            Ok(Value::Bool(parsed))
        }
    }
}

pub(crate) fn unset_config_field(config: &mut PersistedConfig, key: ConfigField) -> bool {
    match key {
        ConfigField::Profile => config.profile.take().is_some(),
        ConfigField::DataDir => config.data_dir.take().is_some(),
        ConfigField::PasswordEnv => config.password_env.take().is_some(),
        ConfigField::Network => config.network.take().is_some(),
        ConfigField::Scheme => config.scheme.take().is_some(),
        ConfigField::EsploraUrl => config.esplora_url.take().is_some(),
        ConfigField::OrdUrl => config.ord_url.take().is_some(),
        ConfigField::Ascii => config.ascii.take().is_some(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceConfig<'a> {
    pub data_dir: Option<&'a Path>,
    pub profile: &'a str,
    pub password: Option<&'a str>,
    pub password_env: &'a str,
    pub password_stdin: bool,
    pub agent: bool,
    pub network_override: Option<&'a str>,
    pub explicit_network: bool,
    pub scheme_override: Option<&'a str>,
    pub esplora_url_override: Option<&'a str>,
    pub ord_url_override: Option<&'a str>,
    pub ascii_mode: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkArg {
    Bitcoin,
    Signet,
    Testnet,
    Regtest,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemeArg {
    Unified,
    Dual,
}

impl std::fmt::Display for NetworkArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NetworkArg::Bitcoin => "bitcoin",
            NetworkArg::Signet => "signet",
            NetworkArg::Testnet => "testnet",
            NetworkArg::Regtest => "regtest",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for SchemeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SchemeArg::Unified => "unified",
            SchemeArg::Dual => "dual",
        };
        write!(f, "{}", s)
    }
}

impl From<NetworkArg> for Network {
    fn from(value: NetworkArg) -> Self {
        match value {
            NetworkArg::Bitcoin => Network::Bitcoin,
            NetworkArg::Signet => Network::Signet,
            NetworkArg::Testnet => Network::Testnet,
            NetworkArg::Regtest => Network::Regtest,
        }
    }
}

impl From<Network> for NetworkArg {
    fn from(value: Network) -> Self {
        match value {
            Network::Bitcoin => NetworkArg::Bitcoin,
            Network::Signet => NetworkArg::Signet,
            Network::Testnet => NetworkArg::Testnet,
            Network::Regtest => NetworkArg::Regtest,
            _ => NetworkArg::Bitcoin, // Fallback for other variants if any
        }
    }
}

impl From<SchemeArg> for AddressScheme {
    fn from(value: SchemeArg) -> Self {
        match value {
            SchemeArg::Unified => AddressScheme::Unified,
            SchemeArg::Dual => AddressScheme::Dual,
        }
    }
}

impl From<AddressScheme> for SchemeArg {
    fn from(value: AddressScheme) -> Self {
        match value {
            AddressScheme::Unified => SchemeArg::Unified,
            AddressScheme::Dual => SchemeArg::Dual,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountState {
    pub persistence_json: Option<String>,
    pub inscriptions_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    pub version: u32,
    pub network: NetworkArg,
    pub scheme: SchemeArg,
    pub account_index: u32,
    pub esplora_url: String,
    pub ord_url: String,
    #[serde(default = "default_bitcoin_cli")]
    pub bitcoin_cli: String,
    #[serde(default = "default_bitcoin_cli_args")]
    pub bitcoin_cli_args: Vec<String>,
    pub encrypted_mnemonic: String,
    pub accounts: BTreeMap<u32, AccountState>,
    pub updated_at_unix: u64,
}

impl Profile {
    #[must_use]
    pub fn account_state(&self) -> AccountState {
        self.accounts
            .get(&self.account_index)
            .cloned()
            .unwrap_or(AccountState {
                persistence_json: None,
                inscriptions_json: None,
            })
    }

    pub fn set_account_state(&mut self, state: AccountState) {
        self.accounts.insert(self.account_index, state);
        self.updated_at_unix = crate::lock::now_unix();
    }
}

#[must_use]
pub fn default_esplora_url(network: NetworkArg) -> &'static str {
    match network {
        NetworkArg::Bitcoin => "https://m.exittheloop.com/api",
        NetworkArg::Signet => "https://mutinynet.com/api",
        NetworkArg::Testnet => "https://blockstream.info/testnet/api",
        NetworkArg::Regtest => "https://regtest.exittheloop.com/api",
    }
}

#[must_use]
pub fn default_ord_url(network: NetworkArg) -> &'static str {
    match network {
        NetworkArg::Bitcoin => "https://o.exittheloop.com",
        NetworkArg::Signet => "https://signet.ordinals.com",
        NetworkArg::Testnet => "https://testnet.ordinals.com",
        NetworkArg::Regtest => "https://ord-regtest.exittheloop.com",
    }
}

#[must_use]
pub fn default_bitcoin_cli() -> String {
    "bitcoin-cli".to_string()
}

#[must_use]
pub fn default_bitcoin_cli_args() -> Vec<String> {
    vec!["-regtest".to_string()]
}

pub fn write_profile(path: &Path, profile: &Profile) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(profile)
        .map_err(|e| AppError::Internal(format!("failed to serialize profile: {e}")))?;
    write_bytes_atomic(path, &bytes, "profile")
}

pub fn read_profile(path: &Path) -> Result<Profile, AppError> {
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "profile not found: {}",
            path.display()
        )));
    }
    let data = fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("failed to read profile: {e}")))?;
    serde_json::from_str::<Profile>(&data)
        .map_err(|e| AppError::Config(format!("failed to parse profile: {e}")))
}

#[cfg(test)]
mod tests {
    use super::{set_config_field, ConfigField, PersistedConfig};
    use crate::error::AppError;

    #[test]
    fn set_config_network_validates_and_canonicalizes() {
        let mut cfg = PersistedConfig::default();
        let value = set_config_field(&mut cfg, ConfigField::Network, "mainnet")
            .expect("mainnet should parse");
        assert_eq!(value.as_str(), Some("bitcoin"));
        assert_eq!(cfg.network.as_deref(), Some("bitcoin"));
    }

    #[test]
    fn set_config_scheme_validates() {
        let mut cfg = PersistedConfig::default();
        let err = set_config_field(&mut cfg, ConfigField::Scheme, "legacy")
            .expect_err("invalid scheme should be rejected");
        assert!(matches!(err, AppError::Invalid(_)));
    }
}
