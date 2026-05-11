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
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
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
    crate::utils::validate_file_name(config.profile)?;
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
    crate::utils::validate_file_name(config.profile)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;

    #[test]
    fn test_profile_path_validation() {
        let mut config = ServiceConfig {
            profile: "valid_name",
            data_dir: None,
            password: None,
            password_env: "ZINC_PASSWORD",
            password_stdin: false,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            esplora_url_override: None,
            ord_url_override: None,
            ascii_mode: false,
        };
        assert!(profile_path(&config).is_ok());

        config.profile = "../invalid";
        assert!(matches!(profile_path(&config), Err(AppError::Invalid(_))));

        config.profile = "/etc/passwd";
        assert!(matches!(profile_path(&config), Err(AppError::Invalid(_))));

        config.profile = "a/b";
        assert!(matches!(profile_path(&config), Err(AppError::Invalid(_))));
    }

    #[test]
    fn test_snapshot_dir_validation() {
        let mut config = ServiceConfig {
            profile: "valid-name-2",
            data_dir: None,
            password: None,
            password_env: "ZINC_PASSWORD",
            password_stdin: false,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            esplora_url_override: None,
            ord_url_override: None,
            ascii_mode: false,
        };
        assert!(snapshot_dir(&config).is_ok());

        config.profile = "../invalid";
        assert!(matches!(snapshot_dir(&config), Err(AppError::Invalid(_))));

        config.profile = "/etc/passwd";
        assert!(matches!(snapshot_dir(&config), Err(AppError::Invalid(_))));

        config.profile = "a/b";
        assert!(matches!(snapshot_dir(&config), Err(AppError::Invalid(_))));
    }
}
