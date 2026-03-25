use crate::cli::Cli;
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::{BtcBalance, CommandOutput};

pub async fn run(cli: &Cli) -> Result<CommandOutput, AppError> {
    let session = load_wallet_session(cli)?;
    let balance = session.wallet.get_balance();

    Ok(CommandOutput::Balance {
        total: BtcBalance {
            immature: balance.total.immature.to_sat(),
            trusted_pending: balance.total.trusted_pending.to_sat(),
            untrusted_pending: balance.total.untrusted_pending.to_sat(),
            confirmed: balance.total.confirmed.to_sat(),
        },
        spendable: BtcBalance {
            immature: balance.spendable.immature.to_sat(),
            trusted_pending: balance.spendable.trusted_pending.to_sat(),
            untrusted_pending: balance.spendable.untrusted_pending.to_sat(),
            confirmed: balance.spendable.confirmed.to_sat(),
        },
        inscribed_sats: balance.inscribed,
    })
}
