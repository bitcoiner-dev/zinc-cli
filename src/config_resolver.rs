use crate::config::{PersistedConfig, Profile, ServiceConfig};
use std::fmt;
use zinc_core::{AddressScheme, Network, PaymentAddressType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    Default,
    GlobalConfig,
    Profile,
    ExplicitCli,
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::GlobalConfig => write!(f, "global config"),
            ConfigSource::Profile => write!(f, "profile"),
            ConfigSource::ExplicitCli => write!(f, "cli override"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedValue<T> {
    pub value: T,
    #[allow(dead_code)]
    pub source: ConfigSource,
}

#[derive(Clone)]
pub struct ConfigResolver<'a> {
    persisted: &'a PersistedConfig,
    service: &'a ServiceConfig<'a>,
}

impl<'a> ConfigResolver<'a> {
    pub fn new(persisted: &'a PersistedConfig, service: &'a ServiceConfig<'a>) -> Self {
        Self { persisted, service }
    }

    pub fn resolve_network(&self, profile: Option<&Profile>) -> ResolvedValue<Network> {
        // Priority 1: Explicit CLI
        if self.service.explicit_network {
            if let Some(net_str) = self.service.network_override {
                if let Ok(net) = crate::utils::parse_network(net_str) {
                    return ResolvedValue {
                        value: net.into(),
                        source: ConfigSource::ExplicitCli,
                    };
                }
            }
        }

        // Priority 2: Profile
        if let Some(profile) = profile {
            return ResolvedValue {
                value: profile.network.into(),
                source: ConfigSource::Profile,
            };
        }

        // Priority 3: Global Config
        if let Some(net_str) = self.persisted.network.as_deref() {
            if let Ok(net) = crate::utils::parse_network(net_str) {
                return ResolvedValue {
                    value: net.into(),
                    source: ConfigSource::GlobalConfig,
                };
            }
        }

        // Priority 4: Default fallback
        ResolvedValue {
            value: Network::Regtest,
            source: ConfigSource::Default,
        }
    }

    pub fn resolve_scheme(&self, profile: Option<&Profile>) -> ResolvedValue<AddressScheme> {
        // Priority 1: Explicit CLI
        if let Some(scheme_str) = self.service.scheme_override {
            if let Ok(scheme) = crate::utils::parse_scheme(scheme_str) {
                return ResolvedValue {
                    value: scheme.into(),
                    source: ConfigSource::ExplicitCli,
                };
            }
        }

        // Priority 2: Profile
        if let Some(profile) = profile {
            return ResolvedValue {
                value: profile.scheme.into(),
                source: ConfigSource::Profile,
            };
        }

        // Priority 3: Global Config
        if let Some(scheme_str) = self.persisted.scheme.as_deref() {
            if let Ok(scheme) = crate::utils::parse_scheme(scheme_str) {
                return ResolvedValue {
                    value: scheme.into(),
                    source: ConfigSource::GlobalConfig,
                };
            }
        }

        // Priority 4: Default fallback
        ResolvedValue {
            value: AddressScheme::Dual,
            source: ConfigSource::Default,
        }
    }

    pub fn resolve_payment_address_type(
        &self,
        profile: Option<&Profile>,
    ) -> ResolvedValue<PaymentAddressType> {
        // Priority 1: Explicit CLI
        if let Some(payment_type_str) = self.service.payment_address_type_override {
            if let Ok(payment_type) = crate::utils::parse_payment_address_type(payment_type_str) {
                return ResolvedValue {
                    value: payment_type.into(),
                    source: ConfigSource::ExplicitCli,
                };
            }
        }

        // Priority 2: Profile
        if let Some(profile) = profile {
            return ResolvedValue {
                value: profile.payment_address_type.into(),
                source: ConfigSource::Profile,
            };
        }

        // Priority 3: Global Config
        if let Some(payment_type_str) = self.persisted.payment_address_type.as_deref() {
            if let Ok(payment_type) = crate::utils::parse_payment_address_type(payment_type_str) {
                return ResolvedValue {
                    value: payment_type.into(),
                    source: ConfigSource::GlobalConfig,
                };
            }
        }

        // Priority 4: Default fallback
        ResolvedValue {
            value: PaymentAddressType::NativeSegwit,
            source: ConfigSource::Default,
        }
    }

    pub fn resolve_pulse_url(&self, profile: Option<&Profile>) -> ResolvedValue<String> {
        // Priority 1: Explicit CLI
        if let Some(url) = self
            .service
            .pulse_url_override
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            return ResolvedValue {
                value: url.to_string(),
                source: ConfigSource::ExplicitCli,
            };
        }

        // Priority 2: Profile
        if let Some(profile) = profile {
            let profile_url = profile.pulse_url.trim();
            if !profile_url.is_empty() {
                return ResolvedValue {
                    value: profile_url.to_string(),
                    source: ConfigSource::Profile,
                };
            }
        }

        // Priority 3: Global Config
        if let Some(url) = self
            .persisted
            .pulse_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            return ResolvedValue {
                value: url.to_string(),
                source: ConfigSource::GlobalConfig,
            };
        }

        // Priority 4: Default fallback (regtest only, otherwise empty)
        let network = self.resolve_network(profile).value;
        let default_url = crate::config::default_pulse_url(network.into());

        ResolvedValue {
            value: default_url.to_string(),
            source: ConfigSource::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigResolver, ConfigSource};
    use crate::config::{
        default_gap_limit, default_scan_depth, NetworkArg, PaymentAddressTypeArg, PersistedConfig,
        Profile, ProfileModeArg, SchemeArg, ServiceConfig,
    };
    use std::collections::BTreeMap;
    use std::path::Path;
    use zinc_core::PaymentAddressType;

    fn service_config<'a>(override_payment_type: Option<&'a str>) -> ServiceConfig<'a> {
        ServiceConfig {
            data_dir: Some(Path::new("/tmp")),
            profile: "default",
            password_env: "ZINC_WALLET_PASSWORD",
            password_stdin: false,
            password_override: None,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            payment_address_type_override: override_payment_type,
            esplora_url_override: None,
            ord_url_override: None,
            pulse_url_override: None,
            pulse_api_token_override: None,
            ascii_mode: false,
        }
    }

    fn profile(payment_address_type: PaymentAddressTypeArg) -> Profile {
        Profile {
            version: 1,
            scan_policy_version: 1,
            network: NetworkArg::Regtest,
            scheme: SchemeArg::Dual,
            payment_address_type,
            account_index: 0,
            esplora_url: "https://regtest.exittheloop.com/api".to_string(),
            ord_url: "https://ord-regtest.exittheloop.com".to_string(),
            pulse_url: "http://localhost:8080".to_string(),
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

    #[test]
    fn resolve_payment_address_type_prefers_cli_override() {
        let mut persisted = PersistedConfig::default();
        persisted.payment_address_type = Some("legacy".to_string());
        let cfg = service_config(Some("nested"));
        let resolver = ConfigResolver::new(&persisted, &cfg);
        let result =
            resolver.resolve_payment_address_type(Some(&profile(PaymentAddressTypeArg::Native)));
        assert_eq!(result.value, PaymentAddressType::NestedSegwit);
        assert_eq!(result.source, ConfigSource::ExplicitCli);
    }

    #[test]
    fn resolve_payment_address_type_uses_profile_then_persisted_then_default() {
        let mut persisted = PersistedConfig::default();
        persisted.payment_address_type = Some("legacy".to_string());
        let cfg = service_config(None);
        let resolver = ConfigResolver::new(&persisted, &cfg);

        let profile_result =
            resolver.resolve_payment_address_type(Some(&profile(PaymentAddressTypeArg::Native)));
        assert_eq!(profile_result.value, PaymentAddressType::NativeSegwit);
        assert_eq!(profile_result.source, ConfigSource::Profile);

        let persisted_result = resolver.resolve_payment_address_type(None);
        assert_eq!(persisted_result.value, PaymentAddressType::Legacy);
        assert_eq!(persisted_result.source, ConfigSource::GlobalConfig);

        let persisted_empty = PersistedConfig::default();
        let resolver_default = ConfigResolver::new(&persisted_empty, &cfg);
        let default_result = resolver_default.resolve_payment_address_type(None);
        assert_eq!(default_result.value, PaymentAddressType::NativeSegwit);
        assert_eq!(default_result.source, ConfigSource::Default);
    }

    #[test]
    fn resolve_pulse_url_returns_empty_on_mainnet_by_default() {
        let mut persisted = PersistedConfig::default();
        persisted.network = Some("bitcoin".to_string());
        let cfg = service_config(None);
        let resolver = ConfigResolver::new(&persisted, &cfg);

        let result = resolver.resolve_pulse_url(None);
        assert_eq!(result.value, "");
        assert_eq!(result.source, ConfigSource::Default);
    }

    #[test]
    fn resolve_pulse_url_returns_localhost_on_regtest_by_default() {
        let persisted = PersistedConfig::default();
        let cfg = service_config(None);
        let resolver = ConfigResolver::new(&persisted, &cfg);

        let result = resolver.resolve_pulse_url(None);
        assert_eq!(result.value, "http://localhost:8080");
        assert_eq!(result.source, ConfigSource::Default);
    }

    #[test]
    fn resolve_pulse_url_skips_empty_profile_and_uses_global() {
        let mut persisted = PersistedConfig::default();
        persisted.pulse_url = Some("https://pulse.example".to_string());
        let cfg = service_config(None);
        let resolver = ConfigResolver::new(&persisted, &cfg);

        let mut prof = profile(PaymentAddressTypeArg::Native);
        prof.pulse_url = "   ".to_string();

        let result = resolver.resolve_pulse_url(Some(&prof));
        assert_eq!(result.value, "https://pulse.example");
        assert_eq!(result.source, ConfigSource::GlobalConfig);
    }

    #[test]
    fn resolve_pulse_url_skips_empty_global_and_uses_default() {
        let mut persisted = PersistedConfig::default();
        persisted.pulse_url = Some("   ".to_string());
        let cfg = service_config(None);
        let resolver = ConfigResolver::new(&persisted, &cfg);

        let result = resolver.resolve_pulse_url(None);
        assert_eq!(result.value, "http://localhost:8080");
        assert_eq!(result.source, ConfigSource::Default);
    }

    #[test]
    fn resolve_pulse_url_cli_override_wins_and_is_trimmed() {
        let mut persisted = PersistedConfig::default();
        persisted.pulse_url = Some("https://global.example".to_string());
        let mut prof = profile(PaymentAddressTypeArg::Native);
        prof.pulse_url = "https://profile.example".to_string();

        let cfg = ServiceConfig {
            data_dir: Some(Path::new("/tmp")),
            profile: "default",
            password_env: "ZINC_WALLET_PASSWORD",
            password_stdin: false,
            password_override: None,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            payment_address_type_override: None,
            esplora_url_override: None,
            ord_url_override: None,
            pulse_url_override: Some("  https://cli.example  "),
            pulse_api_token_override: None,
            ascii_mode: false,
        };

        let resolver = ConfigResolver::new(&persisted, &cfg);
        let result = resolver.resolve_pulse_url(Some(&prof));
        assert_eq!(result.value, "https://cli.example");
        assert_eq!(result.source, ConfigSource::ExplicitCli);
    }
}
