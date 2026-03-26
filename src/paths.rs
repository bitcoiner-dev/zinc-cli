use crate::error::AppError;
use std::fs;
use std::path::{Path, PathBuf};

use crate::lock::now_unix;
use std::process;

pub fn create_secure_dir_all<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let path = path.as_ref();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        builder.mode(0o700);
        builder.create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)
    }
}

pub fn write_secure_file<P: AsRef<Path>>(path: P, contents: &[u8]) -> std::io::Result<()> {
    let path = path.as_ref();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        let mut file = options.open(path)?;
        file.write_all(contents)?;
        file.sync_all()
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents)
    }
}

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
        create_secure_dir_all(&profiles)
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
    create_secure_dir_all(&directory)
        .map_err(|e| AppError::Config(format!("failed to create snapshot dir: {e}")))?;
    Ok(directory)
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8], label: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        create_secure_dir_all(parent)
            .map_err(|e| AppError::Config(format!("failed to create dir for {label}: {e}")))?;
    }
    let tmp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp"),
        process::id(),
        now_unix()
    );
    let tmp_path = path.with_file_name(tmp_name);

    write_secure_file(&tmp_path, bytes)
        .map_err(|e| AppError::Config(format!("failed to write temp {label}: {e}")))?;
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(AppError::Config(format!(
            "failed to commit {label} write: {e}"
        )));
    }
    Ok(())
}
