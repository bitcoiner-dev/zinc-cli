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
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::AppError::Config(format!("failed to create lock dir: {e}"))
            })?;
        }

        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
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
