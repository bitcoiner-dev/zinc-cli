use crate::cli::{Cli, InsightAction, InsightArgs};
use crate::config::load_persisted_config;
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::CommandOutput;
use comfy_table::Table;
use console::style;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionStats {
    pub slug: String,
    pub floor_sats: u64,
    pub owners: u32,
    pub listings: u32,
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
    }
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
    table.set_header(vec!["Inscription", "Collection", "Floor (Sats)", "Status"]);

    for ins in inscriptions {
        let res = all_resolved.get(&ins.id);

        let (col_name, floor, status) = match res {
            Some(ResolutionResult::Success(p)) => {
                if let Some(collection) = collection_map.get(&p.collection_slug) {
                    (
                        p.collection_slug.clone(),
                        collection.stats.floor_sats.to_string(),
                        style("Resolved").green().to_string(),
                    )
                } else {
                    (
                        p.collection_slug.clone(),
                        "N/A".to_string(),
                        style("Stats Unavailable").yellow().to_string(),
                    )
                }
            }
            Some(ResolutionResult::NotFound) => {
                if known_only {
                    continue;
                }
                (
                    style("Unknown").dim().to_string(),
                    "-".to_string(),
                    style("Not Found").yellow().to_string(),
                )
            }
            _ => {
                if known_only {
                    continue;
                }
                (
                    style("Error").red().to_string(),
                    "-".to_string(),
                    style("Failed").red().to_string(),
                )
            }
        };

        let label = format!("#{}", ins.number);
        table.add_row(vec![label, col_name, floor, status]);
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
    table.set_header(vec!["Collection", "Slug", "Floor (Sats)", "Listings"]);

    for res in results {
        let name = res
            .metadata
            .and_then(|m| m.name)
            .unwrap_or_else(|| "Unknown".to_string());
        table.add_row(vec![
            style(name).bold().to_string(),
            res.stats.slug,
            res.stats.floor_sats.to_string(),
            res.stats.listings.to_string(),
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
}
