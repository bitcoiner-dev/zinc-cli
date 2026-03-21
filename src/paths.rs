use crate::error::AppError;
use std::fs;
use std::path::{Path, PathBuf};

use crate::lock::now_unix;
use std::process;

pub fn data_dir(config: &crate::config::ServiceConfig<'_>) -> std::path::PathBuf {
    if let Some(path) = config.data_dir {
        path.to_path_buf()
    } else {
        crate::paths::home_dir().join(".zinc-cli")
    }
}
#[must_use]
pub fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from(".")
    }
}

pub fn profile_path(config: &crate::config::ServiceConfig<'_>) -> Result<PathBuf, AppError> {
    let root = data_dir(config);
    let profiles = root.join("profiles");
    if !profiles.exists() {
        fs::create_dir_all(&profiles)
            .map_err(|e| AppError::Config(format!("failed to create profiles dir: {e}")))?;
    }
    Ok(profiles.join(format!("{}.json", config.profile)))
}

pub fn profile_lock_path(config: &crate::config::ServiceConfig<'_>) -> Result<PathBuf, AppError> {
    Ok(profile_path(config)?.with_extension("lock"))
}

pub fn snapshot_dir(config: &crate::config::ServiceConfig<'_>) -> Result<PathBuf, AppError> {
    let root = data_dir(config);
    let directory = root.join("snapshots").join(config.profile);
    fs::create_dir_all(&directory)
        .map_err(|e| AppError::Config(format!("failed to create snapshot dir: {e}")))?;
    Ok(directory)
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8], label: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Config(format!("failed to create dir for {label}: {e}")))?;
    }
    let tmp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp"),
        process::id(),
        now_unix()
    );
    let tmp_path = path.with_file_name(tmp_name);

    fs::write(&tmp_path, bytes)
        .map_err(|e| AppError::Config(format!("failed to write temp {label}: {e}")))?;
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(AppError::Config(format!(
            "failed to commit {label} write: {e}"
        )));
    }
    Ok(())
}
