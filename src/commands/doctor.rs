use crate::cli::Cli;
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::{profile_path, read_profile};
use zinc_core::{OrdClient, ZincWallet};

pub async fn run(cli: &Cli) -> Result<CommandOutput, AppError> {
    let profile = read_profile(&profile_path(cli)?)?;
    let esplora_ok = ZincWallet::check_connection(&profile.esplora_url).await;
    let ord_height_result = OrdClient::new(profile.ord_url.clone())
        .get_indexing_height()
        .await;

    let (ord_ok, ord_height, ord_error) = match ord_height_result {
        Ok(height) => (true, Some(height), None),
        Err(err) => (false, None, Some(err.to_string())),
    };

    Ok(CommandOutput::Doctor {
        healthy: esplora_ok && ord_ok,
        esplora_url: profile.esplora_url.clone(),
        esplora_reachable: esplora_ok,
        ord_url: profile.ord_url.clone(),
        ord_reachable: ord_ok,
        ord_indexing_height: ord_height.map(|h| h as u64),
        ord_error,
    })
}
