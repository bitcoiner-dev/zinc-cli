use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceAuthorizationResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthError {
    pub error: String,
    pub error_description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WhoamiResponse {
    pub sub: String,
    pub client_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub scopes: Vec<String>,
}

pub struct PulseAuthClient {
    client: reqwest::Client,
    base_url: String,
}

impl PulseAuthClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn start_device_authorization(
        &self,
        client_id: &str,
    ) -> Result<DeviceAuthorizationResponse, AppError> {
        let url = format!("{}/oauth/device/code", self.base_url);
        let resp = self
            .client
            .post(url)
            .form(&[("client_id", client_id)])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("failed to start device auth: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Internal(format!(
                "device auth server returned error: {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| AppError::Internal(format!("failed to parse device auth response: {e}")))
    }

    pub async fn poll_for_token(
        &self,
        client_id: &str,
        device_code: &str,
        interval: u64,
        expires_in: u64,
    ) -> Result<TokenResponse, AppError> {
        let url = format!("{}/oauth/token", self.base_url);
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(expires_in);

        loop {
            if start.elapsed() > timeout {
                return Err(AppError::Internal("device authorization expired".into()));
            }

            let resp = self
                .client
                .post(&url)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", client_id),
                    ("device_code", device_code),
                ])
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("failed to poll for token: {e}")))?;

            if resp.status().is_success() {
                return resp.json().await.map_err(|e| {
                    AppError::Internal(format!("failed to parse token response: {e}"))
                });
            }

            let err: OAuthError = resp
                .json()
                .await
                .map_err(|e| AppError::Internal(format!("failed to parse oauth error: {e}")))?;
            if err.error != "authorization_pending" {
                return Err(AppError::Internal(format!(
                    "oauth error: {} - {:?}",
                    err.error, err.error_description
                )));
            }

            sleep(Duration::from_secs(interval)).await;
        }
    }

    pub async fn refresh_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<TokenResponse, AppError> {
        let url = format!("{}/oauth/token", self.base_url);
        let resp = self
            .client
            .post(url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", client_id),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("failed to refresh token: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Internal(format!(
                "token refresh failed: {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| AppError::Internal(format!("failed to parse refresh response: {e}")))
    }

    pub async fn revoke_token(&self, token: &str) -> Result<(), AppError> {
        let url = format!("{}/oauth/revoke", self.base_url);
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("failed to revoke token: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Internal(format!(
                "token revocation failed: {}",
                resp.status()
            )));
        }

        Ok(())
    }

    pub async fn whoami(&self, token: &str) -> Result<WhoamiResponse, AppError> {
        let url = format!("{}/v1/auth/whoami", self.base_url);
        let resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("failed to call whoami: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Internal(format!(
                "whoami call failed: {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| AppError::Internal(format!("failed to parse whoami response: {e}")))
    }
}
