use crate::cli::{Cli, PulseArgs, PulseAction};
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::config::{load_persisted_config, save_persisted_config, ConfigField, set_config_field};

pub async fn run(_cli: &Cli, args: &PulseArgs) -> Result<CommandOutput, AppError> {
    match &args.action {
        PulseAction::Login { token } => handle_login(token).await,
    }
}

async fn handle_login(token: &str) -> Result<CommandOutput, AppError> {
    let mut config = load_persisted_config()?;
    set_config_field(&mut config, ConfigField::PulseApiToken, token)?;
    save_persisted_config(&config)?;
    
    Ok(CommandOutput::Message("Pulse API token saved successfully.".to_string()))
}
