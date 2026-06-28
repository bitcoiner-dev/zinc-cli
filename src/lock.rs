use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockMetadata {
    pub pid: u32,
    pub created_at_unix: u64,
}

pub struct ProfileLock {
    path: PathBuf,
    _file: fs::File,
}

impl ProfileLock {
    pub fn acquire(profile_path: &Path) -> Result<Self, crate::error::AppError> {
        let lock_path = profile_path.with_extension("lock");

        if let Some(parent) = lock_path.parent() {
            crate::paths::create_secure_dir_all(parent).map_err(|e| {
                crate::error::AppError::Config(format!("failed to create lock dir: {e}"))
            })?;
        }

        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let file = options
            .open(&lock_path)
            .map_err(|_| AppError::Config("profile is locked by another instance".to_string()))?;

        let metadata = LockMetadata {
            pid: process::id(),
            created_at_unix: now_unix(),
        };
        let bytes = serde_json::to_vec(&metadata)
            .map_err(|e| AppError::Internal(format!("failed to serialize lock metadata: {e}")))?;
        (&file)
            .write_all(&bytes)
            .map_err(|e| AppError::Config(format!("failed to write lock metadata: {e}")))?;
        (&file)
            .flush()
            .map_err(|e| AppError::Config(format!("failed to flush lock metadata: {e}")))?;

        Ok(Self {
            path: lock_path,
            _file: file,
        })
    }
}

impl Drop for ProfileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[must_use]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{now_unix, LockMetadata, ProfileLock};
    use crate::error::AppError;
    use std::path::PathBuf;

    fn unique_profile_path(tag: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("zinc-cli-lock-{}-{}", std::process::id(), tag))
            .join("profiles")
            .join("default.json")
    }

    #[test]
    fn now_unix_is_after_2020() {
        // 2020-01-01 in unix seconds; guards against a broken clock conversion.
        assert!(now_unix() > 1_577_836_800);
    }

    #[test]
    fn lock_metadata_serde_roundtrip() {
        let meta = LockMetadata {
            pid: 4321,
            created_at_unix: 1_700_000_000,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: LockMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pid, 4321);
        assert_eq!(back.created_at_unix, 1_700_000_000);
    }

    #[test]
    fn acquire_creates_lock_then_releases_on_drop() {
        let profile_path = unique_profile_path("drop");
        let lock_file = profile_path.with_extension("lock");

        {
            let _lock = ProfileLock::acquire(&profile_path).expect("acquire lock");
            assert!(lock_file.exists(), "lock file should exist while held");
        }
        // Dropping the guard removes the lock file.
        assert!(!lock_file.exists(), "lock file should be gone after drop");

        let _ = std::fs::remove_dir_all(profile_path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn second_acquire_is_rejected_while_held() {
        let profile_path = unique_profile_path("contend");
        // `ProfileLock` is not `Debug`, so match on the Result rather than unwrap.
        let held = ProfileLock::acquire(&profile_path);
        assert!(held.is_ok(), "first acquire should succeed");
        match ProfileLock::acquire(&profile_path) {
            Ok(_) => panic!("second acquire should fail while lock is held"),
            Err(AppError::Config(msg)) => {
                assert!(msg.contains("locked by another instance"));
            }
            Err(other) => panic!("expected Config error, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(profile_path.parent().unwrap().parent().unwrap());
    }
}
