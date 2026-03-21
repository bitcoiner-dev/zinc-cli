#[cfg(feature = "ui")]
pub mod state;
#[cfg(feature = "ui")]
pub mod tui;
use crate::cli::{Cli, SetupArgs};
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct SetupValues {
    pub profile: String,
    pub data_dir: Option<String>,
    pub password_env: String,
    pub default_network: Option<String>,
    pub default_scheme: Option<String>,
    pub default_esplora_url: Option<String>,
    pub default_ord_url: Option<String>,
    pub json_default: bool,
    pub quiet_default: bool,
    pub initialize_wallet: bool,
    pub restore_mnemonic: Option<String>,
    pub words: Option<u8>,
    pub password: Option<String>,
}

pub(crate) fn resolve_setup_values(cli: &Cli, args: &SetupArgs) -> Result<SetupValues, AppError> {
    let profile = args
        .profile
        .clone()
        .unwrap_or_else(|| cli.profile.clone().unwrap_or_else(|| "default".to_string()));
    let data_dir = args
        .data_dir
        .as_ref()
        .map(|path| path.display().to_string());
    let password_env = args.password_env.clone().unwrap_or_else(|| {
        cli.password_env
            .clone()
            .unwrap_or_else(|| "ZINC_WALLET_PASSWORD".to_string())
    });
    let default_network = args.default_network.clone().or(cli.network.clone());
    let default_scheme = args.default_scheme.clone().or(cli.scheme.clone());
    let default_esplora_url = args.default_esplora_url.clone().or(cli.esplora_url.clone());
    let default_ord_url = args.default_ord_url.clone().or(cli.ord_url.clone());

    let json_default = match &args.json_default {
        Some(value) => *value,
        None => cli.json,
    };
    let quiet_default = match &args.quiet_default {
        Some(value) => *value,
        None => cli.quiet,
    };

    Ok(SetupValues {
        profile,
        data_dir,
        password_env,
        default_network,
        default_scheme,
        default_esplora_url,
        default_ord_url,
        json_default,
        quiet_default,
        initialize_wallet: args.restore_mnemonic.is_some() || args.words.is_some(),
        restore_mnemonic: args.restore_mnemonic.clone(),
        words: args.words,
        password: None,
    })
}

pub(crate) fn should_run_setup_wizard(
    args: &SetupArgs,
    cli: &Cli,
    stdin_is_tty: bool,
    stdout_is_tty: bool,
) -> bool {
    #[cfg(not(feature = "ui"))]
    {
        let _ = (args, cli, stdin_is_tty, stdout_is_tty);
        return false;
    }

    #[cfg(feature = "ui")]
    {
        // Check if SetupArgs is empty
        let is_empty = args.profile.is_none()
            && args.data_dir.is_none()
            && args.password_env.is_none()
            && args.default_network.is_none()
            && args.default_scheme.is_none()
            && args.default_esplora_url.is_none()
            && args.default_ord_url.is_none()
            && args.json_default.is_none()
            && args.quiet_default.is_none()
            && args.restore_mnemonic.is_none()
            && args.words.is_none();

        is_empty && !cli.json && stdin_is_tty && stdout_is_tty
    }
}

#[cfg(feature = "ui")]
pub(crate) async fn run_tui_setup_wizard(seed: SetupValues) -> Result<SetupValues, AppError> {
    let state = state::SetupState::new(seed);
    let wizard = tui::TuiWizard::new(state)?;
    let final_state = wizard.run().await?;
    Ok(final_state.values)
}

#[cfg(not(feature = "ui"))]
pub(crate) async fn run_tui_setup_wizard(_seed: SetupValues) -> Result<SetupValues, AppError> {
    Err(AppError::Invalid(
        "setup wizard requires a zinc-cli build with the `ui` feature enabled".to_string(),
    ))
}
