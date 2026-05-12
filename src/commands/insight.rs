use crate::cli::{Cli, InsightAction, InsightArgs, MarketAction};
use crate::commands::psbt::{analyze_psbt_with_policy, enforce_policy_mode};
use crate::config::load_persisted_config;
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::{load_wallet_session, persist_wallet_session};
use comfy_table::{Cell, Color, Table};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use zinc_core::SignOptions;

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

    async fn ordnet_json(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Vec<(String, String)>,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/v1/ordnet{}", self.base_url.trim_end_matches('/'), path);
        let mut req = self.authenticated_builder(method, &url).query(&query);
        if let Some(body) = body {
            req = req.json(&body);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AppError::Network(format!("ord.net gateway request failed: {e}")))?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                && body.contains("trading_provider_unsupported")
            {
                return Err(AppError::Capability(extract_error_message(&body).unwrap_or_else(
                    || {
                        "Hosted trading is currently supported only through ord.net. Satflow is metadata/statistics only."
                            .to_string()
                    },
                )));
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AppError::Auth(
                    "ord.net wallet binding required. Run 'zinc pulse ordnet bind'.".to_string(),
                ));
            }
            if status == reqwest::StatusCode::FORBIDDEN
                || status == reqwest::StatusCode::PAYMENT_REQUIRED
            {
                return Err(AppError::Capability(format!(
                    "ord.net access rejected; wallet binding may be missing the 0.01 BTC confirmed payment-address requirement ({body})"
                )));
            }
            return Err(AppError::Network(format!(
                "ord.net gateway returned {status}: {body}"
            )));
        }
        if body.trim().is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::from_str(&body).map_err(|e| {
                AppError::Internal(format!("failed to parse ord.net gateway response: {e}"))
            })
        }
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
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
        InsightAction::Market { action } => {
            handle_market(cli, &pulse_client, &mut session, action).await
        }
    }
}

async fn handle_market(
    cli: &Cli,
    pulse: &PulseClient,
    session: &mut crate::wallet_service::WalletSession,
    action: &MarketAction,
) -> Result<CommandOutput, AppError> {
    let value = match action {
        MarketAction::Listings {
            collection_slug,
            inscription_id,
            seller_address,
            sort,
            limit,
            cursor,
        } => {
            let query = optional_query(&[
                ("collectionSlug", collection_slug.clone()),
                ("inscriptionId", inscription_id.clone()),
                ("sellerAddress", seller_address.clone()),
                ("sort", sort.clone()),
                ("limit", limit.map(|v| v.to_string())),
                ("cursor", cursor.clone()),
            ]);
            pulse
                .ordnet_json(reqwest::Method::GET, "/listings", query, None)
                .await?
        }
        MarketAction::Sales {
            collection_slug,
            limit,
            cursor,
        } => {
            let query = optional_query(&[
                ("collectionSlug", collection_slug.clone()),
                ("limit", limit.map(|v| v.to_string())),
                ("cursor", cursor.clone()),
            ]);
            pulse
                .ordnet_json(reqwest::Method::GET, "/sales", query, None)
                .await?
        }
        MarketAction::CollectionInscriptions {
            slug,
            sort,
            limit,
            cursor,
        } => {
            let query = optional_query(&[
                ("sort", sort.clone()),
                ("limit", limit.map(|v| v.to_string())),
                ("cursor", cursor.clone()),
            ]);
            pulse
                .ordnet_json(
                    reqwest::Method::GET,
                    &format!("/collection/{slug}/inscriptions"),
                    query,
                    None,
                )
                .await?
        }
        MarketAction::BuyPreflight {
            collection_slug,
            listing_id,
            inscription_id,
            expect_price_sats,
            raw_out_file,
        } => {
            let payment_public_key = session
                .wallet
                .get_payment_public_key(0)
                .map_err(AppError::Internal)?;
            let body = serde_json::json!({
                "paymentPublicKey": payment_public_key,
                "listings": [{
                    "listingId": listing_id,
                    "inscriptionId": inscription_id,
                }],
                "zincExpectations": {
                    "collectionSlug": collection_slug,
                    "inscriptionId": inscription_id,
                    "listingId": listing_id,
                    "priceSats": expect_price_sats,
                }
            });
            let response = pulse
                .ordnet_json(
                    reqwest::Method::POST,
                    &format!("/collection/{collection_slug}/purchases/preflight"),
                    vec![],
                    Some(body),
                )
                .await?;
            let response = with_zinc_expectations(
                response,
                HostedMarketExpectations {
                    collection_slug: Some(collection_slug.clone()),
                    inscription_id: Some(inscription_id.clone()),
                    listing_id: Some(listing_id.clone()),
                    offer_id: None,
                    price_sats: *expect_price_sats,
                    seller_address: None,
                    buyer_address: None,
                },
            );
            if let Some(path) = raw_out_file {
                crate::utils::maybe_write_text(
                    Some(&path.display().to_string()),
                    &serde_json::to_string_pretty(&response).map_err(|e| {
                        AppError::Internal(format!("failed to serialize response: {e}"))
                    })?,
                )?;
            }
            response
        }
        MarketAction::BuySubmit {
            collection_slug,
            expect_inscription,
            expect_listing_id,
            expect_price_sats,
            expect_seller_address,
            expect_buyer_address,
            json,
            file,
            stdin,
        } => {
            sign_and_submit_json_source(
                cli,
                pulse,
                session,
                &format!("/collection/{collection_slug}/purchases/submit"),
                HostedMarketExpectations {
                    collection_slug: Some(collection_slug.clone()),
                    inscription_id: Some(expect_inscription.clone()),
                    listing_id: Some(expect_listing_id.clone()),
                    offer_id: None,
                    price_sats: Some(*expect_price_sats),
                    seller_address: expect_seller_address.clone(),
                    buyer_address: expect_buyer_address.clone(),
                },
                json.as_deref(),
                file.as_deref(),
                *stdin,
            )
            .await?
        }
        MarketAction::ListPreflight {
            collection_slug,
            json,
            file,
            stdin,
        } => {
            post_json_source(
                pulse,
                &format!("/collection/{collection_slug}/listings/preflight"),
                json.as_deref(),
                file.as_deref(),
                *stdin,
            )
            .await?
        }
        MarketAction::ListSubmit {
            collection_slug,
            expect_inscription,
            expect_price_sats,
            expect_seller_address,
            json,
            file,
            stdin,
        } => {
            sign_and_submit_json_source(
                cli,
                pulse,
                session,
                &format!("/collection/{collection_slug}/listings/submit"),
                HostedMarketExpectations {
                    collection_slug: Some(collection_slug.clone()),
                    inscription_id: Some(expect_inscription.clone()),
                    listing_id: None,
                    offer_id: None,
                    price_sats: Some(*expect_price_sats),
                    seller_address: expect_seller_address.clone(),
                    buyer_address: None,
                },
                json.as_deref(),
                file.as_deref(),
                *stdin,
            )
            .await?
        }
        MarketAction::Delist {
            collection_slug,
            expect_inscription,
            expect_listing_id,
            expect_seller_address,
            json,
            file,
            stdin,
        } => {
            sign_and_submit_json_source(
                cli,
                pulse,
                session,
                &format!("/collection/{collection_slug}/listings/delist"),
                HostedMarketExpectations {
                    collection_slug: Some(collection_slug.clone()),
                    inscription_id: Some(expect_inscription.clone()),
                    listing_id: Some(expect_listing_id.clone()),
                    offer_id: None,
                    price_sats: None,
                    seller_address: expect_seller_address.clone(),
                    buyer_address: None,
                },
                json.as_deref(),
                file.as_deref(),
                *stdin,
            )
            .await?
        }
        MarketAction::Offers {
            inscription_id,
            history,
            page,
        } => {
            let path = if *history {
                format!("/inscriptions/{inscription_id}/offers/history")
            } else {
                format!("/inscriptions/{inscription_id}/offers")
            };
            pulse
                .ordnet_json(
                    reqwest::Method::GET,
                    &path,
                    optional_query(&[("page", page.map(|v| v.to_string()))]),
                    None,
                )
                .await?
        }
        MarketAction::OfferCreate {
            collection_slug,
            submit,
            expect_inscription,
            expect_price_sats,
            expect_buyer_address,
            json,
            file,
            stdin,
        } => {
            let path = if *submit {
                format!("/collection/{collection_slug}/offers/submit")
            } else {
                format!("/collection/{collection_slug}/offers/preflight")
            };
            if *submit {
                require_expectation(
                    "offer-create --submit",
                    "--expect-price-sats",
                    expect_price_sats.is_some(),
                )?;
                sign_and_submit_json_source(
                    cli,
                    pulse,
                    session,
                    &path,
                    HostedMarketExpectations {
                        collection_slug: Some(collection_slug.clone()),
                        inscription_id: expect_inscription.clone(),
                        listing_id: None,
                        offer_id: None,
                        price_sats: *expect_price_sats,
                        seller_address: None,
                        buyer_address: expect_buyer_address.clone(),
                    },
                    json.as_deref(),
                    file.as_deref(),
                    *stdin,
                )
                .await?
            } else {
                post_json_source(pulse, &path, json.as_deref(), file.as_deref(), *stdin).await?
            }
        }
        MarketAction::OfferCancel {
            inscription_id,
            offer_id,
        } => {
            pulse
                .ordnet_json(
                    reqwest::Method::POST,
                    &format!("/inscriptions/{inscription_id}/offers/{offer_id}/cancel"),
                    vec![],
                    None,
                )
                .await?
        }
        MarketAction::OfferReject {
            inscription_id,
            offer_id,
        } => {
            pulse
                .ordnet_json(
                    reqwest::Method::POST,
                    &format!("/inscriptions/{inscription_id}/offers/{offer_id}/reject"),
                    vec![],
                    None,
                )
                .await?
        }
        MarketAction::OfferAccept {
            inscription_id,
            offer_id,
            submit,
            expect_price_sats,
            expect_seller_address,
            expect_buyer_address,
            json,
            file,
            stdin,
        } => {
            let suffix = if *submit { "submit" } else { "preflight" };
            let path = format!("/inscriptions/{inscription_id}/offers/{offer_id}/accept/{suffix}");
            if *submit {
                require_expectation(
                    "offer-accept --submit",
                    "--expect-price-sats",
                    expect_price_sats.is_some(),
                )?;
                sign_and_submit_json_source(
                    cli,
                    pulse,
                    session,
                    &path,
                    HostedMarketExpectations {
                        collection_slug: None,
                        inscription_id: Some(inscription_id.clone()),
                        listing_id: None,
                        offer_id: Some(offer_id.clone()),
                        price_sats: *expect_price_sats,
                        seller_address: expect_seller_address.clone(),
                        buyer_address: expect_buyer_address.clone(),
                    },
                    json.as_deref(),
                    file.as_deref(),
                    *stdin,
                )
                .await?
            } else {
                post_json_source(pulse, &path, json.as_deref(), file.as_deref(), *stdin).await?
            }
        }
        MarketAction::OfferCounter {
            inscription_id,
            offer_id,
            submit,
            reject,
            accept,
            expect_price_sats,
            expect_seller_address,
            expect_buyer_address,
            json,
            file,
            stdin,
        } => {
            if *reject {
                post_json_source(
                    pulse,
                    &format!("/inscriptions/{inscription_id}/offers/{offer_id}/counter/reject"),
                    json.as_deref(),
                    file.as_deref(),
                    *stdin,
                )
                .await?
            } else {
                let suffix = match (*accept, *submit) {
                    (true, true) => "counter/accept/submit",
                    (true, false) => "counter/accept/preflight",
                    (false, true) => "counter/submit",
                    (false, false) => "counter/preflight",
                };
                let path = format!("/inscriptions/{inscription_id}/offers/{offer_id}/{suffix}");
                if *submit {
                    require_expectation(
                        "offer-counter --submit",
                        "--expect-price-sats",
                        expect_price_sats.is_some(),
                    )?;
                    sign_and_submit_json_source(
                        cli,
                        pulse,
                        session,
                        &path,
                        HostedMarketExpectations {
                            collection_slug: None,
                            inscription_id: Some(inscription_id.clone()),
                            listing_id: None,
                            offer_id: Some(offer_id.clone()),
                            price_sats: *expect_price_sats,
                            seller_address: expect_seller_address.clone(),
                            buyer_address: expect_buyer_address.clone(),
                        },
                        json.as_deref(),
                        file.as_deref(),
                        *stdin,
                    )
                    .await?
                } else {
                    post_json_source(pulse, &path, json.as_deref(), file.as_deref(), *stdin).await?
                }
            }
        }
        MarketAction::MyOffers {
            view,
            limit,
            cursor,
        } => {
            pulse
                .ordnet_json(
                    reqwest::Method::GET,
                    "/me/offers",
                    optional_query(&[
                        ("view", view.clone()),
                        ("limit", limit.map(|v| v.to_string())),
                        ("cursor", cursor.clone()),
                    ]),
                    None,
                )
                .await?
        }
    };
    Ok(CommandOutput::RawJson(value))
}

fn optional_query(items: &[(&str, Option<String>)]) -> Vec<(String, String)> {
    items
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_ref()
                .map(|value| ((*key).to_string(), value.clone()))
        })
        .collect()
}

async fn post_json_source(
    pulse: &PulseClient,
    path: &str,
    json: Option<&str>,
    file: Option<&Path>,
    stdin: bool,
) -> Result<serde_json::Value, AppError> {
    let source = resolve_json_source(json, file, stdin)?;
    let body = serde_json::from_str(&source)
        .map_err(|e| AppError::Invalid(format!("invalid market json: {e}")))?;
    pulse
        .ordnet_json(reqwest::Method::POST, path, vec![], Some(body))
        .await
}

fn resolve_json_source(
    json: Option<&str>,
    file: Option<&Path>,
    stdin: bool,
) -> Result<String, AppError> {
    let count = u8::from(json.is_some()) + u8::from(file.is_some()) + u8::from(stdin);
    if count != 1 {
        return Err(AppError::Invalid(
            "requires exactly one of --json, --file, --stdin".to_string(),
        ));
    }
    if let Some(json) = json {
        return Ok(json.to_string());
    }
    if let Some(path) = file {
        return std::fs::read_to_string(path).map_err(|e| {
            AppError::Io(format!("failed to read json file {}: {e}", path.display()))
        });
    }
    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .map_err(|e| AppError::Io(format!("failed to read json from stdin: {e}")))?;
    if source.trim().is_empty() {
        return Err(AppError::Invalid("stdin was empty".to_string()));
    }
    Ok(source)
}

#[derive(Debug, Clone, Default)]
struct HostedMarketExpectations {
    collection_slug: Option<String>,
    inscription_id: Option<String>,
    listing_id: Option<String>,
    offer_id: Option<String>,
    price_sats: Option<u64>,
    seller_address: Option<String>,
    buyer_address: Option<String>,
}

impl HostedMarketExpectations {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "collectionSlug": self.collection_slug,
            "inscriptionId": self.inscription_id,
            "listingId": self.listing_id,
            "offerId": self.offer_id,
            "priceSats": self.price_sats,
            "sellerAddress": self.seller_address,
            "buyerAddress": self.buyer_address,
        })
    }
}

fn with_zinc_expectations(
    mut value: serde_json::Value,
    expectations: HostedMarketExpectations,
) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("zincExpectations".to_string(), expectations.to_json());
    }
    value
}

fn require_expectation(command: &str, flag: &str, present: bool) -> Result<(), AppError> {
    if present {
        Ok(())
    } else {
        Err(AppError::Invalid(format!(
            "{command} requires {flag} before hosted PSBT signing"
        )))
    }
}

async fn sign_and_submit_json_source(
    cli: &Cli,
    pulse: &PulseClient,
    session: &mut crate::wallet_service::WalletSession,
    path: &str,
    expectations: HostedMarketExpectations,
    json: Option<&str>,
    file: Option<&Path>,
    stdin: bool,
) -> Result<serde_json::Value, AppError> {
    let source = resolve_json_source(json, file, stdin)?;
    let mut body: serde_json::Value = serde_json::from_str(&source)
        .map_err(|e| AppError::Invalid(format!("invalid market json: {e}")))?;
    session.require_seed_mode()?;
    enforce_hosted_expectations(&body, &expectations)?;
    let signed_steps = sign_hosted_psbts(cli, session, &mut body)?;
    if signed_steps == 0 {
        return Err(AppError::Invalid(
            "hosted market submit payload did not contain any unsigned PSBT steps".to_string(),
        ));
    }
    if let Some(object) = body.as_object_mut() {
        object.insert("zincExpectations".to_string(), expectations.to_json());
        object.insert(
            "zincSignedPsbtSteps".to_string(),
            serde_json::json!(signed_steps),
        );
    }
    let response = pulse
        .ordnet_json(reqwest::Method::POST, path, vec![], Some(body))
        .await?;
    persist_wallet_session(session)?;
    Ok(response)
}

fn enforce_hosted_expectations(
    payload: &serde_json::Value,
    expectations: &HostedMarketExpectations,
) -> Result<(), AppError> {
    let checks: Vec<(&str, &[&str], Option<&str>)> = vec![
        (
            "collection slug",
            &["collectionSlug", "collection_slug", "slug"],
            expectations.collection_slug.as_deref(),
        ),
        (
            "inscription id",
            &["inscriptionId", "inscription_id"],
            expectations.inscription_id.as_deref(),
        ),
        (
            "listing id",
            &["listingId", "listing_id"],
            expectations.listing_id.as_deref(),
        ),
        (
            "offer id",
            &["offerId", "offer_id"],
            expectations.offer_id.as_deref(),
        ),
        (
            "seller address",
            &["sellerAddress", "seller_address", "sellerPaymentAddress"],
            expectations.seller_address.as_deref(),
        ),
        (
            "buyer address",
            &["buyerAddress", "buyer_address", "buyerPaymentAddress"],
            expectations.buyer_address.as_deref(),
        ),
    ];

    for (label, keys, expected) in checks {
        if let Some(expected) = expected {
            if !json_contains_expected_string(payload, keys, expected) {
                return Err(AppError::Invalid(format!(
                    "hosted market expectation mismatch: expected {label} {expected}"
                )));
            }
        }
    }

    if let Some(expected) = expectations.price_sats {
        let price_keys = [
            "priceSats",
            "price_sats",
            "askSats",
            "ask_sats",
            "offerSats",
            "offer_sats",
            "amountSats",
            "amount_sats",
        ];
        if !json_contains_expected_u64(payload, &price_keys, expected) {
            return Err(AppError::Invalid(format!(
                "hosted market expectation mismatch: expected price {expected} sats"
            )));
        }
    }

    Ok(())
}

fn json_contains_expected_string(
    value: &serde_json::Value,
    accepted_keys: &[&str],
    expected: &str,
) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            (key_matches(key, accepted_keys) && value.as_str() == Some(expected))
                || json_contains_expected_string(value, accepted_keys, expected)
        }),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| json_contains_expected_string(item, accepted_keys, expected)),
        _ => false,
    }
}

fn json_contains_expected_u64(
    value: &serde_json::Value,
    accepted_keys: &[&str],
    expected: u64,
) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            if key_matches(key, accepted_keys) && json_value_as_u64(value) == Some(expected) {
                return true;
            }
            json_contains_expected_u64(value, accepted_keys, expected)
        }),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| json_contains_expected_u64(item, accepted_keys, expected)),
        _ => false,
    }
}

fn key_matches(key: &str, accepted: &[&str]) -> bool {
    let normalized = normalize_json_key(key);
    accepted
        .iter()
        .any(|candidate| normalized == normalize_json_key(candidate))
}

fn normalize_json_key(key: &str) -> String {
    key.chars()
        .filter(|ch| *ch != '_' && *ch != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

fn json_value_as_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
}

fn sign_hosted_psbts(
    cli: &Cli,
    session: &mut crate::wallet_service::WalletSession,
    value: &mut serde_json::Value,
) -> Result<usize, AppError> {
    match value {
        serde_json::Value::Object(object) => {
            let candidate_keys: Vec<String> = object
                .iter()
                .filter_map(|(key, value)| {
                    let key_lower = key.to_ascii_lowercase();
                    let is_unsigned_psbt = key_lower.contains("psbt")
                        && !key_lower.contains("signed")
                        && value.as_str().is_some_and(looks_like_psbt_base64);
                    is_unsigned_psbt.then(|| key.clone())
                })
                .collect();

            let mut signed_count = 0;
            for key in candidate_keys {
                let psbt = object
                    .get(&key)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| AppError::Invalid(format!("missing PSBT field {key}")))?
                    .to_string();
                let sign_inputs = hosted_input_indices(object);
                let (analysis, policy) = analyze_psbt_with_policy(&session.wallet, &psbt)?;
                enforce_policy_mode(cli, &policy)?;
                let signed = session
                    .wallet
                    .sign_psbt(
                        &psbt,
                        Some(SignOptions {
                            sign_inputs,
                            sighash: None,
                            finalize: false,
                        }),
                    )
                    .map_err(AppError::Invalid)?;
                let signed_key = if key.to_ascii_lowercase().contains("base64") {
                    "signedPsbtBase64"
                } else {
                    "signedPsbt"
                };
                object.insert(signed_key.to_string(), serde_json::Value::String(signed));
                object.insert(
                    "zincPsbtAnalysis".to_string(),
                    serde_json::json!({
                        "safe_to_send": policy.safe_to_send,
                        "inscription_risk": policy.inscription_risk,
                        "policy_reasons": policy.policy_reasons,
                        "analysis": analysis,
                    }),
                );
                signed_count += 1;
            }

            let nested_count = object
                .values_mut()
                .map(|child| sign_hosted_psbts(cli, session, child))
                .try_fold(0usize, |acc, item| item.map(|count| acc + count))?;
            Ok(signed_count + nested_count)
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|child| sign_hosted_psbts(cli, session, child))
            .try_fold(0usize, |acc, item| item.map(|count| acc + count)),
        _ => Ok(0),
    }
}

fn looks_like_psbt_base64(value: &str) -> bool {
    value.starts_with("cHNidP")
}

fn hosted_input_indices(object: &serde_json::Map<String, serde_json::Value>) -> Option<Vec<usize>> {
    let keys = [
        "signInputIndices",
        "sign_input_indices",
        "inputIndices",
        "input_indices",
        "inputsToSign",
        "inputs_to_sign",
        "buyerInputIndices",
        "sellerInputIndices",
    ];
    object.iter().find_map(|(key, value)| {
        key_matches(key, &keys)
            .then(|| parse_index_array(value))
            .flatten()
    })
}

fn parse_index_array(value: &serde_json::Value) -> Option<Vec<usize>> {
    let values = value.as_array()?;
    let mut indices = Vec::new();
    for value in values {
        let index = value.as_u64()? as usize;
        indices.push(index);
    }
    Some(indices)
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

    #[tokio::test]
    async fn ordnet_gateway_unsupported_provider_maps_to_capability_error() {
        let server = MockServer::start();
        let client = PulseClient::new(server.base_url(), Some("token".to_string()));

        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/ordnet/listings");
            then.status(422).json_body(serde_json::json!({
                "error": {
                    "code": "trading_provider_unsupported",
                    "message": "Hosted trading is currently supported only through ord.net. Satflow is metadata/statistics only."
                }
            }));
        });

        let err = client
            .ordnet_json(reqwest::Method::GET, "/listings", vec![], None)
            .await
            .expect_err("expected capability error");

        match err {
            AppError::Capability(message) => {
                assert!(message.contains("ord.net"));
                assert!(message.contains("Satflow is metadata/statistics only"));
            }
            other => panic!("expected capability error, got {other:?}"),
        }

        mock.assert();
    }

    #[test]
    fn hosted_market_expectations_accept_matching_payload() {
        let payload = serde_json::json!({
            "zincExpectations": {
                "collectionSlug": "nodemonkes",
                "inscriptionId": "inscription123",
                "listingId": "listing123",
                "priceSats": 100000,
                "sellerAddress": "bc1pseller",
                "buyerAddress": "bc1pbuyer"
            }
        });
        let expectations = HostedMarketExpectations {
            collection_slug: Some("nodemonkes".to_string()),
            inscription_id: Some("inscription123".to_string()),
            listing_id: Some("listing123".to_string()),
            offer_id: None,
            price_sats: Some(100000),
            seller_address: Some("bc1pseller".to_string()),
            buyer_address: Some("bc1pbuyer".to_string()),
        };

        enforce_hosted_expectations(&payload, &expectations).expect("matching payload");
    }

    #[test]
    fn hosted_market_expectations_reject_price_mismatch() {
        let payload = serde_json::json!({
            "zincExpectations": {
                "collectionSlug": "nodemonkes",
                "inscriptionId": "inscription123",
                "listingId": "listing123",
                "priceSats": 99999
            }
        });
        let expectations = HostedMarketExpectations {
            collection_slug: Some("nodemonkes".to_string()),
            inscription_id: Some("inscription123".to_string()),
            listing_id: Some("listing123".to_string()),
            offer_id: None,
            price_sats: Some(100000),
            seller_address: None,
            buyer_address: None,
        };

        let err =
            enforce_hosted_expectations(&payload, &expectations).expect_err("mismatch should fail");
        assert!(format!("{err:?}").contains("expected price 100000 sats"));
    }

    #[test]
    fn hosted_market_psbt_detection_ignores_signed_fields() {
        assert!(looks_like_psbt_base64("cHNidP8BAHECA"));
        assert!(!looks_like_psbt_base64("not-a-psbt"));
    }

    #[test]
    fn hosted_market_input_indices_parse_common_shapes() {
        let payload = serde_json::json!({
            "buyerInputIndices": [1, 3]
        });
        let object = payload.as_object().expect("object");
        assert_eq!(hosted_input_indices(object), Some(vec![1, 3]));
    }
}
