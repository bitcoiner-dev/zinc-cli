use crate::cli::{Cli, TxAction, TxArgs};
use crate::error::AppError;
use crate::load_wallet_session;
use serde_json::{json, Value};

pub async fn run(cli: &Cli, args: &TxArgs) -> Result<Value, AppError> {
    match &args.action {
        TxAction::List { limit } => {
            let session = load_wallet_session(cli)?;
            let txs = session.wallet.get_transactions(limit.unwrap_or(20));
            if cli.json {
                Ok(json!({"transactions": txs}))
            } else {
                let table = crate::presenter::tx::format_transactions(&txs);
                println!("{table}");
                Ok(Value::Null)
            }
        }
    }
}
