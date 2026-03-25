use crate::cli::{AddressArgs, AddressKind, Cli};
use crate::error::AppError;
use crate::wallet_service::map_wallet_error;
use crate::{load_wallet_session, persist_wallet_session};
use crate::output::CommandOutput;

pub async fn run(cli: &Cli, args: &AddressArgs) -> Result<CommandOutput, AppError> {
    let mut session = load_wallet_session(cli)?;
    let (kind, address) = match &args.kind {
        AddressKind::Taproot { index, new } => {
            let address = if *new {
                session
                    .wallet
                    .next_taproot_address()
                    .map_err(map_wallet_error)?
                    .to_string()
            } else {
                session
                    .wallet
                    .peek_taproot_address(index.unwrap_or(0))
                    .to_string()
            };
            ("taproot", address)
        }
        AddressKind::Payment { index, new } => {
            let address = if *new {
                session
                    .wallet
                    .get_payment_address()
                    .map_err(map_wallet_error)?
                    .to_string()
            } else {
                session
                    .wallet
                    .peek_payment_address(index.unwrap_or(0))
                    .ok_or_else(|| {
                        AppError::Invalid(
                            "payment wallet not enabled for unified scheme".to_string(),
                        )
                    })?
                    .to_string()
            };
            ("payment", address)
        }
    };
    persist_wallet_session(&mut session)?;
    Ok(CommandOutput::Address {
        kind: kind.to_string(),
        address,
    })
}
