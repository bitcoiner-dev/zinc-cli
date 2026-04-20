use crate::config::{NetworkArg, Profile, SchemeArg};
use crate::error::AppError;
use std::env;
use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from(".")
    }
}

pub fn env_non_empty(name: &str) -> Option<String> {
    let value = env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn env_bool(name: &str) -> Option<bool> {
    let value = env_non_empty(name)?;
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn parse_network(s: &str) -> Result<NetworkArg, AppError> {
    match s.to_lowercase().as_str() {
        "bitcoin" | "mainnet" => Ok(NetworkArg::Bitcoin),
        "signet" => Ok(NetworkArg::Signet),
        "testnet" => Ok(NetworkArg::Testnet),
        "regtest" => Ok(NetworkArg::Regtest),
        _ => Err(AppError::Invalid(format!("unknown network: {s}"))),
    }
}

pub fn parse_scheme(s: &str) -> Result<SchemeArg, AppError> {
    match s.to_lowercase().as_str() {
        "unified" => Ok(SchemeArg::Unified),
        "dual" => Ok(SchemeArg::Dual),
        _ => Err(AppError::Invalid(format!("unknown scheme: {s}"))),
    }
}

pub(crate) fn parse_bool_value(value: &str, context: &str) -> Result<bool, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "{context} must be one of: true,false,yes,no,on,off,1,0"
        )),
    }
}

pub(crate) fn unknown_with_hint(kind: &str, unknown: &str, candidates: &[&str]) -> String {
    if let Some(suggestion) = best_match(unknown, candidates) {
        return format!("unknown {kind}: {unknown} (did you mean {suggestion}?)");
    }
    format!("unknown {kind}: {unknown}")
}

pub(crate) fn best_match<'a>(needle: &str, candidates: &'a [&'a str]) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for &candidate in candidates {
        let score = levenshtein(needle, candidate);
        match best {
            Some((_, best_score)) if score >= best_score => {}
            _ => best = Some((candidate, score)),
        }
    }

    let (candidate, score) = best?;
    let threshold = match needle.len() {
        0..=4 => 1,
        5..=9 => 2,
        _ => 3,
    };

    if score <= threshold {
        Some(candidate)
    } else {
        None
    }
}

pub fn maybe_write_text(path: Option<&str>, text: &str) -> Result<(), crate::error::AppError> {
    if let Some(path) = path {
        crate::paths::write_secure_file(path, text.as_bytes())
            .map_err(|e| crate::error::AppError::Io(format!("failed to write to {path}: {e}")))
    } else {
        Ok(())
    }
}

pub fn run_bitcoin_cli(
    profile: &Profile,
    args: &[String],
) -> Result<String, crate::error::AppError> {
    let mut cmd = std::process::Command::new(&profile.bitcoin_cli);
    for arg in &profile.bitcoin_cli_args {
        cmd.arg(arg);
    }
    cmd.args(args);
    let output = cmd
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("bitcoin-cli failed: {e}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::AppError::Internal(format!(
            "bitcoin-cli error: {err}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }

    let b_len = b.chars().count();
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}
pub fn resolve_psbt_source(
    psbt: Option<&str>,
    psbt_file: Option<&Path>,
    psbt_stdin: bool,
) -> Result<String, AppError> {
    let count = (psbt.is_some() as u8) + (psbt_file.is_some() as u8) + (psbt_stdin as u8);
    if count > 1 {
        return Err(AppError::Invalid(
            "accepts only one of --psbt, --psbt-file, --psbt-stdin".to_string(),
        ));
    }
    if let Some(psbt) = psbt {
        return Ok(psbt.to_string());
    }
    if let Some(path) = psbt_file {
        return std::fs::read_to_string(path).map_err(|e| {
            AppError::Io(format!("failed to read psbt file {}: {e}", path.display()))
        });
    }
    if psbt_stdin {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| AppError::Io(format!("failed to read psbt from stdin: {e}")))?;
        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            return Err(AppError::Invalid(
                "stdin did not contain a PSBT string".to_string(),
            ));
        }
        return Ok(trimmed.to_string());
    }
    Err(AppError::Invalid(
        "requires one of --psbt, --psbt-file, --psbt-stdin".to_string(),
    ))
}

pub fn validate_file_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Invalid("file name cannot be empty".to_string()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::Invalid(
            "file name must contain only alphanumeric characters, underscores, and dashes"
                .to_string(),
        ));
    }
    Ok(())
}

pub fn parse_indices(s: Option<&str>) -> Result<Vec<usize>, AppError> {
    let s = match s {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };
    let mut indices = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').collect();
            if bounds.len() != 2 {
                return Err(AppError::Invalid(format!("invalid index range: {part}")));
            }
            let start: usize = bounds[0]
                .parse()
                .map_err(|_| AppError::Invalid(format!("invalid start index: {}", bounds[0])))?;
            let end: usize = bounds[1]
                .parse()
                .map_err(|_| AppError::Invalid(format!("invalid end index: {}", bounds[1])))?;
            for i in start..=end {
                indices.push(i);
            }
        } else {
            let index: usize = part
                .parse()
                .map_err(|_| AppError::Invalid(format!("invalid index: {part}")))?;
            indices.push(index);
        }
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_name() {
        assert!(validate_file_name("valid_name-123").is_ok());
        assert!(validate_file_name("test").is_ok());
        assert!(validate_file_name("A-B_C").is_ok());

        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("../invalid").is_err());
        assert!(validate_file_name("/etc/passwd").is_err());
        assert!(validate_file_name("invalid/name").is_err());
        assert!(validate_file_name("invalid.name").is_err());
        assert!(validate_file_name("invalid name").is_err());
    }
}
