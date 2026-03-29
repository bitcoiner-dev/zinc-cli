use crate::cli::{Cli, LockAction, LockArgs};
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::wallet_service::now_unix;
use crate::{confirm, profile_lock_path, read_lock_metadata};
use std::fs;

pub async fn run(cli: &Cli, args: &LockArgs) -> Result<CommandOutput, AppError> {
    let lock_path = profile_lock_path(cli)?;
    match &args.action {
        LockAction::Info => {
            let exists = lock_path.exists();
            let metadata = if exists {
                read_lock_metadata(&lock_path)
            } else {
                None
            };
            let age_secs = metadata
                .as_ref()
                .map(|m| now_unix().saturating_sub(m.created_at_unix));
            Ok(CommandOutput::LockInfo {
                profile: cli.profile.clone(),
                lock_path: lock_path.display().to_string(),
                locked: exists,
                owner_pid: metadata.as_ref().map(|m| m.pid),
                created_at_unix: metadata.as_ref().map(|m| m.created_at_unix),
                age_secs,
            })
        }
        LockAction::Clear => {
            if !lock_path.exists() {
                return Ok(CommandOutput::LockClear {
                    profile: cli.profile.clone(),
                    lock_path: lock_path.display().to_string(),
                    cleared: false,
                });
            }

            if !confirm(
                "Are you sure you want to clear the profile lock? Only do this if no other zinc-cli process is running.",
                cli,
            ) {
                return Err(AppError::Internal("aborted by user".to_string()));
            }

            fs::remove_file(&lock_path)
                .map_err(|e| AppError::Config(format!("failed to clear lock: {e}")))?;
            Ok(CommandOutput::LockClear {
                profile: cli.profile.clone(),
                lock_path: lock_path.display().to_string(),
                cleared: true,
            })
        }
    }
}
