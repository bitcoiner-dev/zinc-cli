use crate::cli::{Cli, SnapshotAction, SnapshotArgs};
use crate::error::AppError;
use crate::output::CommandOutput;
use crate::paths::create_secure_dir_all;
use crate::{confirm, profile_path, read_profile, snapshot_dir, write_bytes_atomic};
use std::fs;

pub async fn run(cli: &Cli, args: &SnapshotArgs) -> Result<CommandOutput, AppError> {
    let profile_path = profile_path(cli)?;
    let snap_dir = snapshot_dir(cli)?;
    create_secure_dir_all(&snap_dir)
        .map_err(|e| AppError::Config(format!("failed to create snapshot dir: {e}")))?;

    match &args.action {
        SnapshotAction::Save { name, overwrite } => {
            let source = read_profile(&profile_path)?;
            let destination = snap_dir.join(format!("{name}.json"));
            if destination.exists() && !(*overwrite || cli.yes) {
                return Err(AppError::Config(
                    "snapshot already exists (use --overwrite or --yes)".to_string(),
                ));
            }
            let bytes = serde_json::to_vec_pretty(&source)
                .map_err(|e| AppError::Internal(format!("snapshot serialize failed: {e}")))?;
            write_bytes_atomic(&destination, &bytes, "snapshot")?;
            Ok(CommandOutput::SnapshotSave {
                snapshot: destination.display().to_string(),
            })
        }
        SnapshotAction::Restore { name } => {
            if !confirm(&format!("Are you sure you want to restore snapshot '{name}'? This will overwrite your current profile."), cli) {
                return Err(AppError::Internal("aborted by user".to_string()));
            }
            let source = snap_dir.join(format!("{name}.json"));
            if !source.exists() {
                return Err(AppError::NotFound(format!(
                    "snapshot does not exist: {}",
                    source.display()
                )));
            }
            let data = fs::read(&source)
                .map_err(|e| AppError::Config(format!("failed to read snapshot: {e}")))?;
            write_bytes_atomic(&profile_path, &data, "profile restore")?;
            Ok(CommandOutput::SnapshotRestore {
                restored: source.display().to_string(),
            })
        }
        SnapshotAction::List => {
            let mut names = Vec::new();
            let entries = fs::read_dir(&snap_dir)
                .map_err(|e| AppError::Config(format!("failed to list snapshots: {e}")))?;
            for entry in entries {
                let entry =
                    entry.map_err(|e| AppError::Config(format!("failed to read entry: {e}")))?;
                if entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "json")
                {
                    names.push(entry.path().display().to_string());
                }
            }
            names.sort();
            Ok(CommandOutput::SnapshotList { snapshots: names })
        }
    }
}
