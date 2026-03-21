use crate::cli::{Cli, InscriptionArgs};
use crate::error::AppError;
use crate::load_wallet_session;
use serde_json::{json, Value};

pub async fn run(cli: &Cli, _args: &InscriptionArgs) -> Result<Value, AppError> {
    let session = load_wallet_session(cli)?;
    let inscriptions = session.wallet.inscriptions();

    if cli.json {
        Ok(json!({
            "inscriptions": inscriptions
        }))
    } else {
        let table = crate::presenter::inscription::format_inscriptions(&inscriptions);
        println!("{table}");
        Ok(Value::Null)
    }
}
