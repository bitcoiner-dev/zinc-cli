use crate::config::{save_persisted_config, PersistedConfig, Profile, PulseSession, ServiceConfig};
use crate::error::AppError;
use crate::pulse_auth_client::PulseAuthClient;
use crate::wallet_service::{now_unix, write_profile};
use std::path::Path;

pub struct PulseAuthResolver<'a> {
    persisted: &'a PersistedConfig,
    service: &'a ServiceConfig<'a>,
    pulse_url: String,
}

impl<'a> PulseAuthResolver<'a> {
    pub fn new(persisted: &'a PersistedConfig, service: &'a ServiceConfig<'a>) -> Self {
        let pulse_url = service
            .pulse_url_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                persisted
                    .pulse_url
                    .clone()
                    .unwrap_or_else(|| "https://pulse.ordinals.com".to_string())
            });

        Self {
            persisted,
            service,
            pulse_url,
        }
    }

    pub async fn resolve_token(
        &self,
        mut profile: Option<&mut Profile>,
        profile_path: Option<&Path>,
    ) -> Result<Option<String>, AppError> {
        // 1. CLI flag
        if let Some(token) = self.service.pulse_api_token_override {
            return Ok(Some(token.to_string()));
        }

        // 2. Env var
        if let Ok(token) = std::env::var("PULSE_API_TOKEN") {
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }

        // 3. Profile session
        if let (Some(profile_mut), Some(path)) = (profile.as_mut(), profile_path) {
            if let Some(session) = profile_mut.pulse_session.clone() {
                let token = self
                    .get_valid_token(&session, true, Some(profile_mut), Some(path))
                    .await?;
                if let Some(token) = token {
                    return Ok(Some(token));
                }
            }
        }

        // 4. Global session
        if let Some(session) = self.persisted.pulse_session.clone() {
            let token = self.get_valid_token(&session, false, None, None).await?;
            if let Some(token) = token {
                return Ok(Some(token));
            }
        }

        // 5. Legacy global token
        if let Some(token) = &self.persisted.pulse_api_token {
            return Ok(Some(token.clone()));
        }

        Ok(None)
    }

    async fn get_valid_token(
        &self,
        session: &PulseSession,
        is_profile: bool,
        profile: Option<&mut Profile>,
        profile_path: Option<&Path>,
    ) -> Result<Option<String>, AppError> {
        let now = now_unix();

        // If expired or expiring soon (within 60s), try refresh
        if session.expires_at_unix < now + 60 {
            if let Some(refresh_token) = &session.refresh_token {
                let client = PulseAuthClient::new(self.pulse_url.clone());
                let client_id = "zinc-cli";

                match client.refresh_token(client_id, refresh_token).await {
                    Ok(resp) => {
                        let new_session = PulseSession {
                            access_token: resp.access_token.clone(),
                            refresh_token: resp
                                .refresh_token
                                .clone()
                                .or(Some(refresh_token.clone())),
                            expires_at_unix: now + resp.expires_in,
                            metadata: None,
                        };

                        if is_profile {
                            if let (Some(profile), Some(path)) = (profile, profile_path) {
                                profile.pulse_session = Some(new_session);
                                write_profile(path, profile)?;
                            }
                        } else {
                            let mut new_config = self.persisted.clone();
                            new_config.pulse_session = Some(new_session);
                            save_persisted_config(&new_config)?;
                        }

                        return Ok(Some(resp.access_token));
                    }
                    Err(_) => {
                        // Refresh failed, fall back
                        return Ok(None);
                    }
                }
            }
            return Ok(None);
        }

        Ok(Some(session.access_token.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::PulseAuthResolver;
    use crate::config::{PersistedConfig, PulseSession, ServiceConfig};
    use std::path::Path;

    fn service<'a>(
        pulse_url_override: Option<&'a str>,
        pulse_api_token_override: Option<&'a str>,
    ) -> ServiceConfig<'a> {
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
            payment_address_type_override: None,
            esplora_url_override: None,
            ord_url_override: None,
            pulse_url_override,
            pulse_api_token_override,
            ascii_mode: false,
        }
    }

    // expires far in the future so get_valid_token never attempts a network refresh.
    fn fresh_session(token: &str) -> PulseSession {
        PulseSession {
            access_token: token.to_string(),
            refresh_token: None,
            expires_at_unix: 9_999_999_999,
            metadata: None,
        }
    }

    #[test]
    fn new_prefers_pulse_url_override() {
        let persisted = PersistedConfig::default();
        let svc = service(Some("https://override.example"), None);
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        assert_eq!(resolver.pulse_url, "https://override.example");
    }

    #[test]
    fn new_falls_back_to_persisted_pulse_url() {
        let mut persisted = PersistedConfig::default();
        persisted.pulse_url = Some("https://persisted.example".to_string());
        let svc = service(None, None);
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        assert_eq!(resolver.pulse_url, "https://persisted.example");
    }

    #[test]
    fn new_falls_back_to_default_pulse_url() {
        let persisted = PersistedConfig::default();
        let svc = service(None, None);
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        assert_eq!(resolver.pulse_url, "https://pulse.ordinals.com");
    }

    #[tokio::test]
    async fn resolve_token_prefers_cli_override() {
        // CLI override short-circuits before any env/session lookup.
        let persisted = PersistedConfig::default();
        let svc = service(None, Some("cli-token"));
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        let token = resolver.resolve_token(None, None).await.unwrap();
        assert_eq!(token.as_deref(), Some("cli-token"));
    }

    // The env var PULSE_API_TOKEN is process-global; `pulse_auth_resolver` is the
    // only reader in this crate, so all env-sensitive resolution branches are
    // exercised serially in a single test to avoid cross-test interference.
    #[tokio::test]
    async fn resolve_token_resolution_order_for_env_session_and_legacy() {
        // Env var takes precedence over persisted state.
        std::env::set_var("PULSE_API_TOKEN", "env-token");
        let mut persisted = PersistedConfig::default();
        persisted.pulse_api_token = Some("legacy-token".to_string());
        persisted.pulse_session = Some(fresh_session("session-token"));
        let svc = service(None, None);
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        assert_eq!(
            resolver.resolve_token(None, None).await.unwrap().as_deref(),
            Some("env-token")
        );

        // With env cleared, a valid global session wins over the legacy token.
        std::env::remove_var("PULSE_API_TOKEN");
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        assert_eq!(
            resolver.resolve_token(None, None).await.unwrap().as_deref(),
            Some("session-token")
        );

        // Without a session, the legacy global token is used.
        let mut legacy_only = PersistedConfig::default();
        legacy_only.pulse_api_token = Some("legacy-token".to_string());
        let resolver = PulseAuthResolver::new(&legacy_only, &svc);
        assert_eq!(
            resolver.resolve_token(None, None).await.unwrap().as_deref(),
            Some("legacy-token")
        );

        // Nothing configured -> None.
        let empty = PersistedConfig::default();
        let resolver = PulseAuthResolver::new(&empty, &svc);
        assert_eq!(resolver.resolve_token(None, None).await.unwrap(), None);

        // A valid profile session is preferred over a global session.
        let resolver = PulseAuthResolver::new(&persisted, &svc);
        let mut profile = sample_profile(Some(fresh_session("profile-token")));
        let path = Path::new("/tmp/zinc-cli-nonexistent-profile.json");
        assert_eq!(
            resolver
                .resolve_token(Some(&mut profile), Some(path))
                .await
                .unwrap()
                .as_deref(),
            Some("profile-token")
        );
    }

    fn sample_profile(pulse_session: Option<PulseSession>) -> crate::config::Profile {
        crate::config::Profile {
            version: 1,
            scan_policy_version: 0,
            network: crate::config::NetworkArg::Regtest,
            scheme: crate::config::SchemeArg::Dual,
            payment_address_type: crate::config::PaymentAddressTypeArg::Native,
            account_index: 0,
            esplora_url: String::new(),
            ord_url: String::new(),
            pulse_url: String::new(),
            bitcoin_cli: "bitcoin-cli".to_string(),
            bitcoin_cli_args: vec![],
            encrypted_mnemonic: None,
            mode: crate::config::ProfileModeArg::Seed,
            taproot_xpub: None,
            payment_xpub: None,
            watch_address: None,
            account_gap_limit: crate::config::default_gap_limit(),
            address_scan_depth: crate::config::default_scan_depth(),
            accounts: std::collections::BTreeMap::new(),
            updated_at_unix: 0,
            pulse_session,
        }
    }
}
