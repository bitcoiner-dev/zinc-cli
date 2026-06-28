use crate::error::AppError;
use crate::paths::write_bytes_atomic;
use crate::utils::{
    parse_bool_value, parse_network, parse_payment_address_type, parse_scheme, unknown_with_hint,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use zinc_core::{AddressScheme, Network, PaymentAddressType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct PersistedConfig {
    pub profile: Option<String>,
    pub data_dir: Option<String>,
    pub password_env: Option<String>,
    pub network: Option<String>,
    pub scheme: Option<String>,
    pub payment_address_type: Option<String>,
    pub esplora_url: Option<String>,
    pub ord_url: Option<String>,
    pub pulse_url: Option<String>,
    pub pulse_api_token: Option<String>,
    pub pulse_session: Option<PulseSession>,
    pub ascii: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PulseSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_unix: u64,
    pub metadata: Option<serde_json::Value>,
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
    write_bytes_atomic(&path, &bytes, "config")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Profile,
    DataDir,
    PasswordEnv,
    Network,
    Scheme,
    PaymentAddressType,
    EsploraUrl,
    OrdUrl,
    PulseUrl,
    PulseApiToken,
    Ascii,
}

pub const CONFIG_KEYS: &[&str] = &[
    "profile",
    "data-dir",
    "password-env",
    "network",
    "scheme",
    "payment-address-type",
    "esplora-url",
    "ord-url",
    "pulse-url",
    "pulse-api-token",
    "ascii",
];

impl ConfigField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::DataDir => "data-dir",
            Self::PasswordEnv => "password-env",
            Self::Network => "network",
            Self::Scheme => "scheme",
            Self::PaymentAddressType => "payment-address-type",
            Self::EsploraUrl => "esplora-url",
            Self::OrdUrl => "ord-url",
            Self::PulseUrl => "pulse-url",
            Self::PulseApiToken => "pulse-api-token",
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
            "payment-address-type" | "payment_address_type" => Ok(Self::PaymentAddressType),
            "esplora-url" | "esplora_url" => Ok(Self::EsploraUrl),
            "ord-url" | "ord_url" => Ok(Self::OrdUrl),
            "pulse-url" | "pulse_url" => Ok(Self::PulseUrl),
            "pulse-api-token" | "pulse_api_token" => Ok(Self::PulseApiToken),
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
        ConfigField::PaymentAddressType => {
            let parsed = parse_payment_address_type(value)?;
            let canonical = parsed.to_string();
            config.payment_address_type = Some(canonical.clone());
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
        ConfigField::PulseUrl => {
            config.pulse_url = Some(value.to_string());
            Ok(Value::String(value.to_string()))
        }
        ConfigField::PulseApiToken => {
            config.pulse_api_token = Some(value.to_string());
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
        ConfigField::PaymentAddressType => config.payment_address_type.take().is_some(),
        ConfigField::EsploraUrl => config.esplora_url.take().is_some(),
        ConfigField::OrdUrl => config.ord_url.take().is_some(),
        ConfigField::PulseUrl => config.pulse_url.take().is_some(),
        ConfigField::PulseApiToken => config.pulse_api_token.take().is_some(),
        ConfigField::Ascii => config.ascii.take().is_some(),
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct ServiceConfig<'a> {
    pub data_dir: Option<&'a Path>,
    pub profile: &'a str,
    pub password_env: &'a str,
    pub password_stdin: bool,
    pub password_override: Option<&'a str>,
    pub agent: bool,
    pub network_override: Option<&'a str>,
    pub explicit_network: bool,
    pub scheme_override: Option<&'a str>,
    pub payment_address_type_override: Option<&'a str>,
    pub esplora_url_override: Option<&'a str>,
    pub ord_url_override: Option<&'a str>,
    pub pulse_url_override: Option<&'a str>,
    pub pulse_api_token_override: Option<&'a str>,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchemeArg {
    Unified,
    Dual,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PaymentAddressTypeArg {
    #[default]
    Native,
    Nested,
    Legacy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProfileModeArg {
    #[default]
    Seed,
    Watch,
    WatchAddress,
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

impl std::fmt::Display for ProfileModeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProfileModeArg::Seed => "seed",
            ProfileModeArg::Watch => "watch",
            ProfileModeArg::WatchAddress => "watch-address",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for PaymentAddressTypeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PaymentAddressTypeArg::Native => "native",
            PaymentAddressTypeArg::Nested => "nested",
            PaymentAddressTypeArg::Legacy => "legacy",
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
            Network::Testnet4 => NetworkArg::Testnet,
        }
    }
}

impl From<ProfileModeArg> for zinc_core::ProfileMode {
    fn from(value: ProfileModeArg) -> Self {
        match value {
            ProfileModeArg::Seed => zinc_core::ProfileMode::Seed,
            ProfileModeArg::Watch | ProfileModeArg::WatchAddress => zinc_core::ProfileMode::Watch,
        }
    }
}

impl From<zinc_core::ProfileMode> for ProfileModeArg {
    fn from(value: zinc_core::ProfileMode) -> Self {
        match value {
            zinc_core::ProfileMode::Seed => ProfileModeArg::Seed,
            zinc_core::ProfileMode::Watch => ProfileModeArg::Watch,
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

impl From<PaymentAddressTypeArg> for PaymentAddressType {
    fn from(value: PaymentAddressTypeArg) -> Self {
        match value {
            PaymentAddressTypeArg::Native => PaymentAddressType::NativeSegwit,
            PaymentAddressTypeArg::Nested => PaymentAddressType::NestedSegwit,
            PaymentAddressTypeArg::Legacy => PaymentAddressType::Legacy,
        }
    }
}

impl From<PaymentAddressType> for PaymentAddressTypeArg {
    fn from(value: PaymentAddressType) -> Self {
        match value {
            PaymentAddressType::NativeSegwit => PaymentAddressTypeArg::Native,
            PaymentAddressType::NestedSegwit => PaymentAddressTypeArg::Nested,
            PaymentAddressType::Legacy => PaymentAddressTypeArg::Legacy,
        }
    }
}

pub const SCAN_POLICY_VERSION_MAIN_ONLY: u32 = 1;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountState {
    pub persistence_json: Option<String>,
    pub inscriptions_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    pub version: u32,
    #[serde(default)]
    pub scan_policy_version: u32,
    pub network: NetworkArg,
    pub scheme: SchemeArg,
    #[serde(default)]
    pub payment_address_type: PaymentAddressTypeArg,
    pub account_index: u32,
    pub esplora_url: String,
    pub ord_url: String,
    #[serde(default)]
    pub pulse_url: String,
    #[serde(default = "default_bitcoin_cli")]
    pub bitcoin_cli: String,
    #[serde(default = "default_bitcoin_cli_args")]
    pub bitcoin_cli_args: Vec<String>,
    pub encrypted_mnemonic: Option<String>,
    #[serde(default)]
    pub mode: ProfileModeArg,
    #[serde(default, alias = "xpub")]
    pub taproot_xpub: Option<String>,
    #[serde(default)]
    pub payment_xpub: Option<String>,
    #[serde(default)]
    pub watch_address: Option<String>,
    #[serde(default = "default_gap_limit")]
    pub account_gap_limit: u32,
    #[serde(default = "default_scan_depth")]
    pub address_scan_depth: u32,
    pub accounts: BTreeMap<u32, AccountState>,
    pub updated_at_unix: u64,
    #[serde(default)]
    pub pulse_session: Option<PulseSession>,
}

pub fn default_gap_limit() -> u32 {
    20
}

pub fn default_scan_depth() -> u32 {
    1
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
        NetworkArg::Regtest => "https://esplora-rt.exittheloop.com",
    }
}

#[must_use]
pub fn default_ord_url(network: NetworkArg) -> &'static str {
    match network {
        NetworkArg::Bitcoin => "https://o.exittheloop.com",
        NetworkArg::Signet => "https://signet.ordinals.com",
        NetworkArg::Testnet => "https://testnet.ordinals.com",
        NetworkArg::Regtest => "https://ord-rt.exittheloop.com",
    }
}

#[must_use]
pub fn default_pulse_url(network: NetworkArg) -> &'static str {
    match network {
        NetworkArg::Regtest => "http://localhost:8080",
        _ => "",
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

    #[test]
    fn set_config_payment_address_type_validates_and_canonicalizes() {
        let mut cfg = PersistedConfig::default();
        let value = set_config_field(&mut cfg, ConfigField::PaymentAddressType, "NESTED")
            .expect("nested should parse");
        assert_eq!(value.as_str(), Some("nested"));
        assert_eq!(cfg.payment_address_type.as_deref(), Some("nested"));
    }

    use super::{
        default_esplora_url, default_ord_url, default_pulse_url, read_profile, unset_config_field,
        write_profile, AccountState, NetworkArg, PaymentAddressTypeArg, Profile, ProfileModeArg,
        PulseSession, SchemeArg,
    };
    use zinc_core::{AddressScheme, Network, PaymentAddressType, ProfileMode};

    fn sample_profile() -> Profile {
        Profile {
            version: 1,
            scan_policy_version: 1,
            network: NetworkArg::Signet,
            scheme: SchemeArg::Dual,
            payment_address_type: PaymentAddressTypeArg::Native,
            account_index: 0,
            esplora_url: "https://esplora".to_string(),
            ord_url: "https://ord".to_string(),
            pulse_url: "https://pulse".to_string(),
            bitcoin_cli: "bitcoin-cli".to_string(),
            bitcoin_cli_args: vec!["-signet".to_string()],
            encrypted_mnemonic: Some("ciphertext".to_string()),
            mode: ProfileModeArg::Seed,
            taproot_xpub: None,
            payment_xpub: None,
            watch_address: None,
            account_gap_limit: super::default_gap_limit(),
            address_scan_depth: super::default_scan_depth(),
            accounts: std::collections::BTreeMap::new(),
            updated_at_unix: 1,
            pulse_session: None,
        }
    }

    #[test]
    fn config_field_parse_roundtrips_canonical_keys() {
        for key in super::CONFIG_KEYS {
            let field = ConfigField::parse(key).expect("known key parses");
            assert_eq!(&field.as_str(), key);
        }
    }

    #[test]
    fn config_field_parse_accepts_underscore_aliases() {
        assert_eq!(ConfigField::parse("data_dir").unwrap(), ConfigField::DataDir);
        assert_eq!(
            ConfigField::parse("password_env").unwrap(),
            ConfigField::PasswordEnv
        );
        assert_eq!(
            ConfigField::parse("payment_address_type").unwrap(),
            ConfigField::PaymentAddressType
        );
        assert_eq!(
            ConfigField::parse("esplora_url").unwrap(),
            ConfigField::EsploraUrl
        );
        assert_eq!(ConfigField::parse("ord_url").unwrap(), ConfigField::OrdUrl);
        assert_eq!(
            ConfigField::parse("pulse_url").unwrap(),
            ConfigField::PulseUrl
        );
    }

    #[test]
    fn config_field_parse_unknown_key_suggests_close_match() {
        let err = ConfigField::parse("netwrok").expect_err("unknown key rejected");
        match err {
            AppError::Invalid(msg) => {
                assert!(msg.contains("unknown config key"));
                assert!(msg.contains("network"));
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn set_config_field_stores_string_fields_trimmed() {
        let mut cfg = PersistedConfig::default();
        set_config_field(&mut cfg, ConfigField::Profile, "  alpha ").unwrap();
        assert_eq!(cfg.profile.as_deref(), Some("alpha"));
        set_config_field(&mut cfg, ConfigField::DataDir, "/data").unwrap();
        assert_eq!(cfg.data_dir.as_deref(), Some("/data"));
        set_config_field(&mut cfg, ConfigField::PasswordEnv, "ENV").unwrap();
        assert_eq!(cfg.password_env.as_deref(), Some("ENV"));
        set_config_field(&mut cfg, ConfigField::EsploraUrl, "https://e").unwrap();
        assert_eq!(cfg.esplora_url.as_deref(), Some("https://e"));
        set_config_field(&mut cfg, ConfigField::OrdUrl, "https://o").unwrap();
        assert_eq!(cfg.ord_url.as_deref(), Some("https://o"));
        set_config_field(&mut cfg, ConfigField::PulseUrl, "https://p").unwrap();
        assert_eq!(cfg.pulse_url.as_deref(), Some("https://p"));
        set_config_field(&mut cfg, ConfigField::PulseApiToken, "tok").unwrap();
        assert_eq!(cfg.pulse_api_token.as_deref(), Some("tok"));
    }

    #[test]
    fn set_config_field_rejects_empty_value() {
        let mut cfg = PersistedConfig::default();
        let err = set_config_field(&mut cfg, ConfigField::Profile, "   ")
            .expect_err("empty value rejected");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn set_config_field_ascii_parses_bool() {
        let mut cfg = PersistedConfig::default();
        let value = set_config_field(&mut cfg, ConfigField::Ascii, "yes").unwrap();
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(cfg.ascii, Some(true));
        let err = set_config_field(&mut cfg, ConfigField::Ascii, "maybe")
            .expect_err("invalid bool rejected");
        assert!(matches!(err, AppError::Invalid(_)));
    }

    #[test]
    fn unset_config_field_reports_presence() {
        let mut cfg = PersistedConfig::default();
        assert!(!unset_config_field(&mut cfg, ConfigField::Network));
        cfg.network = Some("signet".to_string());
        assert!(unset_config_field(&mut cfg, ConfigField::Network));
        assert!(cfg.network.is_none());
    }

    #[test]
    fn display_impls_are_canonical() {
        assert_eq!(NetworkArg::Bitcoin.to_string(), "bitcoin");
        assert_eq!(NetworkArg::Regtest.to_string(), "regtest");
        assert_eq!(SchemeArg::Unified.to_string(), "unified");
        assert_eq!(SchemeArg::Dual.to_string(), "dual");
        assert_eq!(ProfileModeArg::Seed.to_string(), "seed");
        assert_eq!(ProfileModeArg::WatchAddress.to_string(), "watch-address");
        assert_eq!(PaymentAddressTypeArg::Legacy.to_string(), "legacy");
    }

    #[test]
    fn network_arg_converts_to_and_from_core() {
        for arg in [
            NetworkArg::Bitcoin,
            NetworkArg::Signet,
            NetworkArg::Testnet,
            NetworkArg::Regtest,
        ] {
            let core: Network = arg.into();
            let back: NetworkArg = core.into();
            assert_eq!(arg.to_string(), back.to_string());
        }
        // Core-only Testnet4 folds into Testnet.
        let folded: NetworkArg = Network::Testnet4.into();
        assert!(matches!(folded, NetworkArg::Testnet));
    }

    #[test]
    fn scheme_and_payment_and_mode_conversions() {
        assert_eq!(AddressScheme::from(SchemeArg::Unified), AddressScheme::Unified);
        assert_eq!(SchemeArg::from(AddressScheme::Dual), SchemeArg::Dual);

        assert_eq!(
            PaymentAddressType::from(PaymentAddressTypeArg::Nested),
            PaymentAddressType::NestedSegwit
        );
        assert_eq!(
            PaymentAddressTypeArg::from(PaymentAddressType::Legacy),
            PaymentAddressTypeArg::Legacy
        );

        assert_eq!(ProfileMode::from(ProfileModeArg::Seed), ProfileMode::Seed);
        // Both Watch variants map to the core Watch mode.
        assert_eq!(ProfileMode::from(ProfileModeArg::Watch), ProfileMode::Watch);
        assert_eq!(
            ProfileMode::from(ProfileModeArg::WatchAddress),
            ProfileMode::Watch
        );
        assert_eq!(ProfileModeArg::from(ProfileMode::Watch), ProfileModeArg::Watch);
    }

    #[test]
    fn default_url_helpers_match_network() {
        assert_eq!(default_esplora_url(NetworkArg::Regtest), "https://esplora-rt.exittheloop.com");
        assert_eq!(default_ord_url(NetworkArg::Signet), "https://signet.ordinals.com");
        assert_eq!(default_pulse_url(NetworkArg::Regtest), "http://localhost:8080");
        assert_eq!(default_pulse_url(NetworkArg::Bitcoin), "");
    }

    #[test]
    fn persisted_config_serde_roundtrip() {
        let mut cfg = PersistedConfig::default();
        cfg.network = Some("signet".to_string());
        cfg.ascii = Some(true);
        cfg.pulse_session = Some(PulseSession {
            access_token: "at".to_string(),
            refresh_token: Some("rt".to_string()),
            expires_at_unix: 42,
            metadata: Some(serde_json::json!({"k": "v"})),
        });
        let json = serde_json::to_string(&cfg).unwrap();
        let back: PersistedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.network.as_deref(), Some("signet"));
        assert_eq!(back.ascii, Some(true));
        assert_eq!(back.pulse_session, cfg.pulse_session);
    }

    #[test]
    fn profile_account_state_default_and_set() {
        let mut profile = sample_profile();
        // No stored state yet -> default (all None).
        let state = profile.account_state();
        assert!(state.persistence_json.is_none());
        assert!(state.inscriptions_json.is_none());

        profile.set_account_state(AccountState {
            persistence_json: Some("{\"p\":1}".to_string()),
            inscriptions_json: None,
        });
        let state = profile.account_state();
        assert_eq!(state.persistence_json.as_deref(), Some("{\"p\":1}"));
    }

    #[test]
    fn write_then_read_profile_roundtrips() {
        let dir = std::env::temp_dir().join(format!("zinc-cli-cfg-{}", std::process::id()));
        let path = dir.join("profile.json");
        let profile = sample_profile();
        write_profile(&path, &profile).expect("write profile");
        let loaded = read_profile(&path).expect("read profile");
        assert_eq!(loaded.network.to_string(), "signet");
        assert_eq!(loaded.esplora_url, "https://esplora");
        assert_eq!(loaded.encrypted_mnemonic.as_deref(), Some("ciphertext"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_profile_missing_path_is_not_found() {
        let path = std::path::Path::new("/nonexistent/zinc-cli/profile-does-not-exist.json");
        let err = read_profile(path).expect_err("missing profile errors");
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
