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
}

#[cfg(test)]
mod tests {
    use super::{ConfigResolver, ConfigSource};
    use crate::config::{
        NetworkArg, PaymentAddressTypeArg, PersistedConfig, Profile, SchemeArg, ServiceConfig,
    };
    use std::collections::BTreeMap;
    use std::path::Path;
    use zinc_core::PaymentAddressType;

    fn service_config<'a>(override_payment_type: Option<&'a str>) -> ServiceConfig<'a> {
        ServiceConfig {
            data_dir: Some(Path::new("/tmp")),
            profile: "default",
            password: None,
            password_env: "ZINC_WALLET_PASSWORD",
            password_stdin: false,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            payment_address_type_override: override_payment_type,
            esplora_url_override: None,
            ord_url_override: None,
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
            bitcoin_cli: "bitcoin-cli".to_string(),
            bitcoin_cli_args: vec!["-regtest".to_string()],
            encrypted_mnemonic: "encrypted".to_string(),
            accounts: BTreeMap::new(),
            updated_at_unix: 1,
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
}
