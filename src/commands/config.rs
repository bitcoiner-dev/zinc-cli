use crate::cli::{Cli, ConfigAction, ConfigArgs};
use crate::config::{
    load_persisted_config, persisted_config_path, save_persisted_config, set_config_field,
    unset_config_field, ConfigField,
};
use crate::error::AppError;
use serde_json::{json, Value};

/// `zinc config {show|set|unset}` — manage persisted CLI defaults.
pub async fn run(cli: &Cli, args: &ConfigArgs) -> Result<Value, AppError> {
    match &args.action {
        ConfigAction::Show => {
            let config = load_persisted_config()?;
            let res = json!({
                "profile": cli.profile.as_ref().or(config.profile.as_ref()).cloned().unwrap_or_else(|| "default".to_string()),
                "data_dir": cli.data_dir.as_ref().map(|p| p.display().to_string()).or(config.data_dir.clone()).unwrap_or_else(|| "~/.zinc-cli".to_string()),
                "password_env": cli.password_env.as_ref().or(config.password_env.as_ref()).cloned().unwrap_or_else(|| "ZINC_WALLET_PASSWORD".to_string()),
                "json": cli.json || config.json.unwrap_or(false),
                "quiet": cli.quiet || config.quiet.unwrap_or(false),
                "defaults": {
                    "network": config.network.clone(),
                    "scheme": config.scheme.clone(),
                    "esplora_url": config.esplora_url.clone(),
                    "ord_url": config.ord_url.clone(),
                },
                "path": persisted_config_path().display().to_string(),
            });
            if cli.json {
                Ok(res)
            } else {
                let table = crate::presenter::config::format_config(&res);
                println!("{table}");
                Ok(res)
            }
        }
        ConfigAction::Set { key, value } => {
            let mut config = load_persisted_config()?;
            let field = ConfigField::parse(key)?;
            let applied = set_config_field(&mut config, field, value)?;
            save_persisted_config(&config)?;
            Ok(json!({
                "key": field.as_str(),
                "value": applied,
                "saved": true,
            }))
        }
        ConfigAction::Unset { key } => {
            let mut config = load_persisted_config()?;
            let field = ConfigField::parse(key)?;
            let was_set = unset_config_field(&mut config, field);
            save_persisted_config(&config)?;
            Ok(json!({
                "key": field.as_str(),
                "was_set": was_set,
                "saved": true,
            }))
        }
    }
}
