use crate::config::{PersistedConfig, Profile, ServiceConfig};
use std::fmt;
use zinc_core::{AddressScheme, Network};

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

        // Priority 2: Global Config
        if let Some(net_str) = self.persisted.network.as_deref() {
            if let Ok(net) = crate::utils::parse_network(net_str) {
                return ResolvedValue {
                    value: net.into(),
                    source: ConfigSource::GlobalConfig,
                };
            }
        }

        // Priority 3: Profile
        if let Some(profile) = profile {
            return ResolvedValue {
                value: profile.network.into(),
                source: ConfigSource::Profile,
            };
        }

        // Priority 4: Default fallback
        ResolvedValue {
            value: Network::Regtest,
            source: ConfigSource::Default,
        }
    }

    pub fn resolve_scheme(&self, profile: Option<&Profile>) -> ResolvedValue<AddressScheme> {
        // Implementation for scheme...
        if let Some(scheme_str) = self.service.scheme_override {
            if let Ok(scheme) = crate::utils::parse_scheme(scheme_str) {
                return ResolvedValue {
                    value: scheme.into(),
                    source: ConfigSource::ExplicitCli,
                };
            }
        }

        if let Some(scheme_str) = self.persisted.scheme.as_deref() {
            if let Ok(scheme) = crate::utils::parse_scheme(scheme_str) {
                return ResolvedValue {
                    value: scheme.into(),
                    source: ConfigSource::GlobalConfig,
                };
            }
        }

        if let Some(profile) = profile {
            return ResolvedValue {
                value: profile.scheme.into(),
                source: ConfigSource::Profile,
            };
        }

        ResolvedValue {
            value: AddressScheme::Dual,
            source: ConfigSource::Default,
        }
    }
}
