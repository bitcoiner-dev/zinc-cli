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
    use super::{
        create_secure_dir_all, data_dir, home_dir, profile_lock_path, profile_path, snapshot_dir,
        write_bytes_atomic, write_secure_file,
    };
    use crate::config::ServiceConfig;
    use std::path::{Path, PathBuf};

    fn unique_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("zinc-cli-paths-{}-{}", std::process::id(), tag))
    }

    fn service<'a>(data_dir: &'a Path, profile: &'a str) -> ServiceConfig<'a> {
        ServiceConfig {
            data_dir: Some(data_dir),
            profile,
            password_env: "ZINC_WALLET_PASSWORD",
            password_stdin: false,
            password_override: None,
            agent: false,
            network_override: None,
            explicit_network: false,
            scheme_override: None,
            payment_address_type_override: None,
            esplora_url_override: None,
            ord_url_override: None,
            pulse_url_override: None,
            pulse_api_token_override: None,
            ascii_mode: false,
        }
    }

    #[test]
    fn home_dir_is_non_empty() {
        // Mirrors HOME when set, otherwise falls back to ".".
        let home = home_dir();
        assert!(!home.as_os_str().is_empty());
    }

    #[test]
    fn data_dir_uses_override_then_default() {
        let dir = unique_dir("data");
        let svc = service(&dir, "default");
        assert_eq!(data_dir(&svc), dir);

        let mut no_override = service(&dir, "default");
        no_override.data_dir = None;
        assert!(data_dir(&no_override).ends_with(".zinc-cli"));
    }

    #[test]
    fn profile_path_and_lock_path_are_derived_and_created() {
        let dir = unique_dir("profile");
        let svc = service(&dir, "acct");
        let path = profile_path(&svc).expect("profile path");
        assert!(path.ends_with("profiles/acct.json"));
        // The profiles directory is created as a side effect.
        assert!(dir.join("profiles").is_dir());

        let lock = profile_lock_path(&svc).expect("lock path");
        assert!(lock.ends_with("profiles/acct.lock"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snapshot_dir_is_created_per_profile() {
        let dir = unique_dir("snap");
        let svc = service(&dir, "myprofile");
        let snap = snapshot_dir(&svc).expect("snapshot dir");
        assert!(snap.ends_with("snapshots/myprofile"));
        assert!(snap.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_bytes_atomic_creates_parents_and_writes() {
        let dir = unique_dir("atomic");
        let path = dir.join("nested").join("file.bin");
        write_bytes_atomic(&path, b"payload", "test").expect("atomic write");
        assert_eq!(std::fs::read(&path).unwrap(), b"payload");

        // Overwrite is atomic and replaces contents.
        write_bytes_atomic(&path, b"updated", "test").expect("atomic overwrite");
        assert_eq!(std::fs::read(&path).unwrap(), b"updated");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_dir_and_write_secure_file() {
        let dir = unique_dir("secure");
        create_secure_dir_all(&dir).expect("mkdir");
        assert!(dir.is_dir());
        let path = dir.join("secret.txt");
        write_secure_file(&path, b"top secret").expect("write");
        assert_eq!(std::fs::read(&path).unwrap(), b"top secret");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_path_rejects_path_traversal() {
        let dir = unique_dir("traversal");
        let svc = service(&dir, "../../../etc/passwd");
        let err = profile_path(&svc).expect_err("should reject traversal");
        assert!(matches!(err, crate::error::AppError::Invalid(_)));
    }

    #[test]
    fn snapshot_dir_rejects_path_traversal() {
        let dir = unique_dir("traversal");
        let svc = service(&dir, "../../../etc/passwd");
        let err = snapshot_dir(&svc).expect_err("should reject traversal");
        assert!(matches!(err, crate::error::AppError::Invalid(_)));
    }
}
