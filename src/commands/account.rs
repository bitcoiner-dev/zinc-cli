use crate::cli::{AccountAction, AccountArgs, Cli};
use crate::error::AppError;
use crate::wallet_service::{now_unix, AccountState};
use crate::{load_wallet_session, profile_path, read_profile, write_profile};
use crate::output::CommandOutput;
use zinc_core::Account;

pub async fn run(cli: &Cli, args: &AccountArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        AccountAction::List { count } => {
            let session = load_wallet_session(cli)?;
            let accounts: Vec<Account> = session.wallet.get_accounts(count.unwrap_or(20));
            Ok(CommandOutput::AccountList { accounts })
        }
        AccountAction::Use { index } => {
            let path = profile_path(cli)?;
            let mut profile = read_profile(&path)?;
            let old_index = profile.account_index;
            profile.account_index = *index;
            profile.updated_at_unix = now_unix();
            profile.accounts.entry(*index).or_insert(AccountState {
                persistence_json: None,
                inscriptions_json: None,
            });
            write_profile(&path, &profile)?;

            let session = load_wallet_session(cli)?;
            // Display the first receive address for the active account.
            let taproot_addr = session.wallet.peek_taproot_address(0).to_string();
            let payment_addr = session
                .wallet
                .peek_payment_address(0)
                .map(|s| s.to_string());

            Ok(CommandOutput::AccountUse {
                previous_account_index: old_index,
                account_index: *index,
                taproot_address: taproot_addr,
                payment_address: payment_addr,
            })
        }
    }
}
