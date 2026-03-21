use crate::cli::Cli;
use crate::error::AppError;
use crate::{profile_path, read_profile};
use serde_json::{json, Value};
use zinc_core::{OrdClient, ZincWallet};

pub async fn run(cli: &Cli) -> Result<Value, AppError> {
    let profile = read_profile(&profile_path(cli)?)?;
    let esplora_ok = ZincWallet::check_connection(&profile.esplora_url).await;
    let ord_height_result = OrdClient::new(profile.ord_url.clone())
        .get_indexing_height()
        .await;

    let (ord_ok, ord_height, ord_error) = match ord_height_result {
        Ok(height) => (true, Some(height), None),
        Err(err) => (false, None, Some(err.to_string())),
    };

    Ok(json!({
        "healthy": esplora_ok && ord_ok,
        "esplora": {
            "url": profile.esplora_url,
            "reachable": esplora_ok
        },
        "ord": {
            "url": profile.ord_url,
            "reachable": ord_ok,
            "indexing_height": ord_height,
            "error": ord_error
        }
    }))
}
