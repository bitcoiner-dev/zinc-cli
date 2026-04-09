use crate::cli::{Cli, InsightAction, InsightArgs};
use crate::config::load_persisted_config;
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::CommandOutput;
use comfy_table::{Cell, Color, Table};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataQuality {
    pub is_stale: bool,
    pub is_fallback: bool,
    pub source_reliable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionStats {
    pub slug: String,
    pub floor_sats: u64,
    pub owners: u32,
    pub listings: u32,
    pub as_of: Option<chrono::DateTime<chrono::Utc>>,
    pub change_6h_pct: Option<f64>,
    pub change_24h_pct: Option<f64>,
    pub change_7d_pct: Option<f64>,
    pub change_30d_pct: Option<f64>,
    pub owners_known: bool,
    pub data_quality: DataQuality,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionProfile {
    pub metadata: Option<CollectionMetadata>,
    pub stats: CollectionStats,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InscriptionProfile {
    pub id: String,
    pub collection_slug: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "status", content = "data", rename_all = "lowercase")]
pub enum ResolutionResult<T> {
    Success(T),
    Error(String),
    NotFound,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatchResolutionResponse<T> {
    pub results: HashMap<String, ResolutionResult<T>>,
}

pub struct PulseClient {
    base_url: String,
    api_token: Option<String>,
    http: reqwest::Client,
}

impl PulseClient {
    pub fn new(base_url: String, api_token: Option<String>) -> Self {
        Self {
            base_url,
            api_token,
            http: reqwest::Client::new(),
        }
    }

    fn authenticated_builder(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut builder = self.http.request(method, url);
        if let Some(token) = &self.api_token {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    pub async fn resolve_inscriptions_batch(
        &self,
        ids: &[String],
    ) -> Result<BatchResolutionResponse<InscriptionProfile>, AppError> {
        let url = format!("{}/v1/inscriptions/batch", self.base_url);
        let res = self
            .authenticated_builder(reqwest::Method::POST, &url)
            .json(&serde_json::json!({ "ids": ids }))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Pulse resolution failed: {e}")))?;

        if !res.status().is_success() {
            return Err(AppError::Network(format!(
                "Pulse returned error: {}",
                res.status()
            )));
        }

        res.json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Pulse response: {e}")))
    }

    pub async fn get_collection_profile(&self, slug: &str) -> Result<CollectionProfile, AppError> {
        let url = format!("{}/v1/collections/{}", self.base_url, slug);
        let res = self
            .authenticated_builder(reqwest::Method::GET, &url)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Pulse collection fetch failed: {e}")))?;

        if !res.status().is_success() {
            return Err(AppError::Network(format!(
                "Pulse returned error for collection {}: {}",
                slug,
                res.status()
            )));
        }

        res.json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse collection profile: {e}")))
    }

    pub async fn search_collections(
        &self,
        query: &str,
    ) -> Result<Vec<CollectionProfile>, AppError> {
        let url = format!("{}/v1/search/collections", self.base_url);
        let res = self
            .authenticated_builder(reqwest::Method::GET, &url)
            .query(&[("q", query)])
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Pulse search failed: {e}")))?;

        if !res.status().is_success() {
            return Err(AppError::Network(format!(
                "Pulse search returned error: {}",
                res.status()
            )));
        }

        res.json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse search results: {e}")))
    }

    pub async fn get_agent_snapshot(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, Option<CollectionProfile>>, AppError> {
        let url = format!("{}/v1/agent/snapshot", self.base_url);
        let res = self
            .authenticated_builder(reqwest::Method::POST, &url)
            .json(&serde_json::json!({ "inscription_ids": ids }))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Agent snapshot failed: {e}")))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            let body_trimmed = body.trim();

            if status == reqwest::StatusCode::FORBIDDEN {
                let msg = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| {
                        v.get("error")
                            .and_then(|value| value.as_str())
                            .map(ToString::to_string)
                    })
                    .or_else(|| {
                        if body_trimmed.is_empty() {
                            None
                        } else {
                            Some(body_trimmed.to_string())
                        }
                    })
                    .unwrap_or_else(|| {
                        "Upgrade required: Access denied to agent intelligence features."
                            .to_string()
                    });
                return Err(AppError::Capability(msg));
            }

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::Auth(
                    "Pulse authentication required. Run 'zinc pulse login'.".to_string(),
                ));
            }

            if status == reqwest::StatusCode::INTERNAL_SERVER_ERROR
                && body.contains("Missing request extension")
                && body.contains("UserContext")
            {
                return Err(AppError::Auth(
                    "Pulse authentication failed (token missing or expired). Run 'zinc pulse login'."
                        .to_string(),
                ));
            }

            if body_trimmed.is_empty() {
                return Err(AppError::Network(format!(
                    "Pulse snapshot returned error: {}",
                    status
                )));
            }

            return Err(AppError::Network(format!(
                "Pulse snapshot returned error: {} ({})",
                status, body_trimmed
            )));
        }

        #[derive(Deserialize)]
        struct SnapshotResponse {
            inscriptions: HashMap<String, Option<CollectionProfile>>,
        }

        let resp: SnapshotResponse = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Pulse response: {e}")))?;

        Ok(resp.inscriptions)
    }
}

pub async fn run(cli: &Cli, args: &InsightArgs) -> Result<CommandOutput, AppError> {
    let mut session = load_wallet_session(cli)?;
    let persisted = load_persisted_config().unwrap_or_default();
    let service = crate::service_config(cli);
    let resolver = crate::config_resolver::ConfigResolver::new(&persisted, &service);

    let auth_resolver = crate::pulse_auth_resolver::PulseAuthResolver::new(&persisted, &service);
    let path = crate::profile_path(cli)?;
    let token = auth_resolver
        .resolve_token(Some(&mut session.profile), Some(&path))
        .await?;
    let pulse_url = resolver.resolve_pulse_url(Some(&session.profile)).value;

    if pulse_url.is_empty() {
        return Err(AppError::Invalid(
            "Pulse Oracle URL is not configured. Please set ZINC_CLI_PULSE_URL or run 'zinc pulse login'.".to_string(),
        ));
    }

    let pulse_client = PulseClient::new(pulse_url, token);

    match &args.action {
        InsightAction::Appraise { known_only } => {
            handle_appraise(cli, &pulse_client, &session, *known_only).await
        }
        InsightAction::Search { query } => handle_search(cli, &pulse_client, query).await,
        InsightAction::Snapshot { agent } => {
            handle_snapshot(cli, &pulse_client, &session, *agent).await
        }
        InsightAction::RecommendSell {
            agent,
            strategy,
            max,
            min_confidence,
        } => {
            handle_recommend_sell(
                cli,
                &pulse_client,
                &session,
                *agent,
                strategy,
                *max,
                *min_confidence,
            )
            .await
        }
    }
}

async fn handle_snapshot(
    cli: &Cli,
    pulse: &PulseClient,
    session: &crate::wallet_service::WalletSession,
    agent: bool,
) -> Result<CommandOutput, AppError> {
    let inscriptions = session.wallet.inscriptions();
    if inscriptions.is_empty() {
        return Ok(CommandOutput::Message(
            "No inscriptions found in wallet.".to_string(),
        ));
    }

    let ids: Vec<String> = inscriptions.iter().map(|i| i.id.clone()).collect();
    let snapshot = pulse.get_agent_snapshot(&ids).await?;

    if agent || cli.agent {
        return Ok(CommandOutput::RawJson(
            serde_json::to_value(snapshot).unwrap(),
        ));
    }

    Ok(CommandOutput::Message(
        "Snapshot generated (use --agent for JSON output)".to_string(),
    ))
}

async fn handle_recommend_sell(
    cli: &Cli,
    pulse: &PulseClient,
    session: &crate::wallet_service::WalletSession,
    agent: bool,
    strategy: &str,
    max: usize,
    min_confidence: f64,
) -> Result<CommandOutput, AppError> {
    let inscriptions = session.wallet.inscriptions();
    if inscriptions.is_empty() {
        return Ok(CommandOutput::Message(
            "No inscriptions found in wallet.".to_string(),
        ));
    }

    let ids: Vec<String> = inscriptions.iter().map(|i| i.id.clone()).collect();
    let snapshot = pulse.get_agent_snapshot(&ids).await?;

    let mut recommendations = Vec::new();

    for ins in inscriptions {
        if let Some(Some(profile)) = snapshot.get(&ins.id) {
            let stats = &profile.stats;

            let mut score = 0.5; // Baseline
            let mut reasons = Vec::new();

            // Strategy logic (Balanced)
            if let Some(change) = stats.change_24h_pct {
                if change < -5.0 {
                    score += 0.2;
                    reasons.push(format!("24h trend negative ({:.1}%)", change));
                }
            }

            if let Some(change) = stats.change_7d_pct {
                if change > 50.0 {
                    score += 0.1;
                    reasons.push("7d surge suggests blow-off top risk".into());
                }
            }

            if stats.listings < 50 {
                score += 0.1;
                reasons.push("Low listing density increases slippage risk".into());
            }

            if score >= min_confidence {
                recommendations.push(serde_json::json!({
                    "inscription_id": ins.id,
                    "action": "sell",
                    "confidence": score,
                    "strategy": strategy,
                    "reasons": reasons,
                    "ask_band_sats": {
                        "min": stats.floor_sats,
                        "target": (stats.floor_sats as f64 * 1.05) as u64,
                        "max": (stats.floor_sats as f64 * 1.15) as u64,
                    },
                    "as_of": stats.as_of,
                    "data_quality": stats.data_quality,
                }));
            }
        }
    }

    // Sort by confidence
    recommendations.sort_by(|a, b| {
        let sc_a = a["confidence"].as_f64().unwrap_or(0.0);
        let sc_b = b["confidence"].as_f64().unwrap_or(0.0);
        sc_b.partial_cmp(&sc_a).unwrap()
    });

    let result = recommendations.into_iter().take(max).collect::<Vec<_>>();

    if agent || cli.agent {
        return Ok(CommandOutput::RawJson(serde_json::Value::Array(result)));
    }

    Ok(CommandOutput::Message(format!(
        "Found {} sell recommendations.",
        result.len()
    )))
}

async fn handle_appraise(
    cli: &Cli,
    pulse: &PulseClient,
    session: &crate::wallet_service::WalletSession,
    known_only: bool,
) -> Result<CommandOutput, AppError> {
    let inscriptions = session.wallet.inscriptions();
    if inscriptions.is_empty() {
        return Ok(CommandOutput::Message(
            "No inscriptions found in wallet.".to_string(),
        ));
    }

    let ids: Vec<String> = inscriptions.iter().map(|i| i.id.clone()).collect();

    // Pulse supports batches of 100
    let mut all_resolved = HashMap::new();
    for chunk in ids.chunks(100) {
        let batch = pulse.resolve_inscriptions_batch(chunk).await?;
        all_resolved.extend(batch.results);
    }

    // Identify unique collections to fetch floor prices
    let mut collection_slugs = std::collections::HashSet::new();
    for res in all_resolved.values() {
        if let ResolutionResult::Success(profile) = res {
            collection_slugs.insert(profile.collection_slug.clone());
        }
    }

    let mut collection_map = HashMap::new();
    for slug in collection_slugs {
        if let Ok(profile) = pulse.get_collection_profile(&slug).await {
            collection_map.insert(slug, profile);
        }
    }

    if cli.agent {
        // Prepare detailed JSON for agent
        let mut appraisal_results = Vec::new();
        for ins in inscriptions {
            let res = all_resolved.get(&ins.id);
            let collection = res.and_then(|r| {
                if let ResolutionResult::Success(p) = r {
                    collection_map.get(&p.collection_slug)
                } else {
                    None
                }
            });

            appraisal_results.push(serde_json::json!({
                "inscription_id": ins.id,
                "number": ins.number,
                "collection": collection.map(|c| &c.stats.slug),
                "floor_sats": collection.map(|c| c.stats.floor_sats),
            }));
        }

        return Ok(CommandOutput::RawJson(serde_json::Value::Array(
            appraisal_results,
        )));
    }

    // Human output
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Inscription").fg(Color::Cyan),
        Cell::new("Collection").fg(Color::Green),
        Cell::new("Floor (Sats)").fg(Color::Yellow),
        Cell::new("Status").fg(Color::Magenta),
    ]);

    for ins in inscriptions {
        let res = all_resolved.get(&ins.id);

        let (col_name, floor, status, status_color) = match res {
            Some(ResolutionResult::Success(p)) => {
                if let Some(collection) = collection_map.get(&p.collection_slug) {
                    (
                        p.collection_slug.clone(),
                        collection.stats.floor_sats.to_string(),
                        "Resolved".to_string(),
                        Some(Color::Green),
                    )
                } else {
                    (
                        p.collection_slug.clone(),
                        "N/A".to_string(),
                        "Stats Unavailable".to_string(),
                        Some(Color::Yellow),
                    )
                }
            }
            Some(ResolutionResult::NotFound) => {
                if known_only {
                    continue;
                }
                (
                    "Unknown".to_string(),
                    "-".to_string(),
                    "Not Found".to_string(),
                    Some(Color::Yellow),
                )
            }
            _ => {
                if known_only {
                    continue;
                }
                (
                    "Error".to_string(),
                    "-".to_string(),
                    "Failed".to_string(),
                    Some(Color::Red),
                )
            }
        };

        let label = format!("#{}", ins.number);
        let mut status_cell = Cell::new(status);
        if let Some(color) = status_color {
            status_cell = status_cell.fg(color);
        }
        table.add_row(vec![
            Cell::new(label),
            Cell::new(col_name),
            Cell::new(floor),
            status_cell,
        ]);
    }

    Ok(CommandOutput::Message(format!(
        "Wallet Appraisal:\n{table}"
    )))
}

async fn handle_search(
    cli: &Cli,
    pulse: &PulseClient,
    query: &str,
) -> Result<CommandOutput, AppError> {
    let results = pulse.search_collections(query).await?;

    if cli.agent {
        return Ok(CommandOutput::RawJson(
            serde_json::to_value(&results).unwrap(),
        ));
    }

    if results.is_empty() {
        return Ok(CommandOutput::Message(format!(
            "No collections found matching '{}'",
            query
        )));
    }

    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Collection").fg(Color::Cyan),
        Cell::new("Slug").fg(Color::Green),
        Cell::new("Floor (Sats)").fg(Color::Yellow),
        Cell::new("Listings").fg(Color::Magenta),
    ]);

    for res in results {
        let name = res
            .metadata
            .and_then(|m| m.name)
            .unwrap_or_else(|| "Unknown".to_string());
        table.add_row(vec![
            Cell::new(name),
            Cell::new(res.stats.slug),
            Cell::new(res.stats.floor_sats.to_string()),
            Cell::new(res.stats.listings.to_string()),
        ]);
    }

    Ok(CommandOutput::Message(format!(
        "Search Results for '{}':\n{table}",
        query
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_pulse_client_authentication_header() {
        let server = MockServer::start();
        let token = "test_pulse_token_123";

        let client = PulseClient::new(server.base_url(), Some(token.to_string()));

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/inscriptions/batch")
                .header("Authorization", &format!("Bearer {}", token));
            then.status(200)
                .json_body(serde_json::json!({ "results": {} }));
        });

        let _ = client.resolve_inscriptions_batch(&[]).await.unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn test_pulse_client_no_auth_when_token_missing() {
        let server = MockServer::start();
        let client = PulseClient::new(server.base_url(), None);

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/inscriptions/batch")
                .matches(|req| {
                    !req.headers
                        .as_ref()
                        .map(|h| h.iter().any(|(n, _)| n == "authorization"))
                        .unwrap_or(false)
                });
            then.status(200)
                .json_body(serde_json::json!({ "results": {} }));
        });

        let _ = client.resolve_inscriptions_batch(&[]).await.unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn test_agent_snapshot_unauthorized_maps_to_auth_error() {
        let server = MockServer::start();
        let client = PulseClient::new(server.base_url(), Some("token".to_string()));

        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/agent/snapshot");
            then.status(401).body("unauthorized");
        });

        let err = client
            .get_agent_snapshot(&["abc".to_string()])
            .await
            .expect_err("expected auth error");

        match err {
            AppError::Auth(message) => {
                assert!(message.contains("zinc pulse login"));
            }
            other => panic!("expected auth error, got: {other:?}"),
        }

        mock.assert();
    }

    #[tokio::test]
    async fn test_agent_snapshot_missing_user_context_500_maps_to_auth_error() {
        let server = MockServer::start();
        let client = PulseClient::new(server.base_url(), Some("token".to_string()));

        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/agent/snapshot");
            then.status(500).body(
                "Missing request extension: Extension of type `ord_pulse::middleware::auth::UserContext` was not found. Perhaps you forgot to add it? See `axum::Extension`.",
            );
        });

        let err = client
            .get_agent_snapshot(&["abc".to_string()])
            .await
            .expect_err("expected auth error");

        match err {
            AppError::Auth(message) => {
                assert!(message.contains("zinc pulse login"));
            }
            other => panic!("expected auth error, got: {other:?}"),
        }

        mock.assert();
    }
}
