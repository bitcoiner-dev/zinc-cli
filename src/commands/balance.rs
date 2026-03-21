use crate::cli::Cli;
use crate::error::AppError;
use crate::load_wallet_session;
use serde_json::{json, Value};

pub async fn run(cli: &Cli) -> Result<Value, AppError> {
    let session = load_wallet_session(cli)?;
    let balance = session.wallet.get_balance();

    if cli.json {
        Ok(json!({
            "total": {
                "immature": balance.total.immature.to_sat(),
                "trusted_pending": balance.total.trusted_pending.to_sat(),
                "untrusted_pending": balance.total.untrusted_pending.to_sat(),
                "confirmed": balance.total.confirmed.to_sat()
            },
            "spendable": {
                "immature": balance.spendable.immature.to_sat(),
                "trusted_pending": balance.spendable.trusted_pending.to_sat(),
                "untrusted_pending": balance.spendable.untrusted_pending.to_sat(),
                "confirmed": balance.spendable.confirmed.to_sat()
            },
            "inscribed_sats": balance.inscribed
        }))
    } else {
        let table = crate::presenter::balance::format_balance(&balance);
        println!("{table}");
        Ok(Value::Null)
    }
}
