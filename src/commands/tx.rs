use crate::cli::{Cli, TxAction, TxArgs};
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::CommandOutput;

pub async fn run(cli: &Cli, args: &TxArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        TxAction::List { limit } => {
            let session = load_wallet_session(cli)?;
            let txs = session.wallet.get_transactions(limit.unwrap_or(20));
            Ok(CommandOutput::TxList { transactions: txs })
        }
    }
}
