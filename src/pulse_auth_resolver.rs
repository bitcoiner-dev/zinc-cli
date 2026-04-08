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
