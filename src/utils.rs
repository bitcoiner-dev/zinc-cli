use crate::config::{NetworkArg, PaymentAddressTypeArg, Profile, SchemeArg};
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

pub fn parse_payment_address_type(s: &str) -> Result<PaymentAddressTypeArg, AppError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "native" => Ok(PaymentAddressTypeArg::Native),
        "nested" => Ok(PaymentAddressTypeArg::Nested),
        "legacy" => Ok(PaymentAddressTypeArg::Legacy),
        _ => Err(AppError::Invalid(format!(
            "unknown payment address type: {s}"
        ))),
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
    let count = u8::from(psbt.is_some()) + u8::from(psbt_file.is_some()) + u8::from(psbt_stdin);
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

    // Unique env var name per test keeps the process-global environment from
    // leaking across the test binary's parallel threads.
    fn unique_env_key(suffix: &str) -> String {
        format!("ZINC_CLI_TEST_{}_{}", std::process::id(), suffix)
    }

    #[test]
    fn env_non_empty_trims_and_filters_blank() {
        let key = unique_env_key("NONEMPTY");
        std::env::set_var(&key, "  hello  ");
        assert_eq!(env_non_empty(&key).as_deref(), Some("hello"));
        std::env::set_var(&key, "   ");
        assert_eq!(env_non_empty(&key), None);
        std::env::remove_var(&key);
        assert_eq!(env_non_empty(&key), None);
    }

    #[test]
    fn env_bool_recognizes_truthy_and_falsy_and_rejects_garbage() {
        let key = unique_env_key("BOOL");
        for truthy in ["1", "true", "YES", "On"] {
            std::env::set_var(&key, truthy);
            assert_eq!(env_bool(&key), Some(true), "{truthy} should be true");
        }
        for falsy in ["0", "false", "NO", "Off"] {
            std::env::set_var(&key, falsy);
            assert_eq!(env_bool(&key), Some(false), "{falsy} should be false");
        }
        std::env::set_var(&key, "maybe");
        assert_eq!(env_bool(&key), None);
        std::env::remove_var(&key);
        assert_eq!(env_bool(&key), None);
    }

    #[test]
    fn parse_network_accepts_known_aliases_and_rejects_unknown() {
        assert!(matches!(parse_network("mainnet"), Ok(NetworkArg::Bitcoin)));
        assert!(matches!(parse_network("BITCOIN"), Ok(NetworkArg::Bitcoin)));
        assert!(matches!(parse_network("signet"), Ok(NetworkArg::Signet)));
        assert!(matches!(parse_network("Testnet"), Ok(NetworkArg::Testnet)));
        assert!(matches!(parse_network("regtest"), Ok(NetworkArg::Regtest)));
        let err = parse_network("moonnet").expect_err("unknown network rejected");
        assert!(matches!(err, AppError::Invalid(_)));
        assert!(err.to_string().contains("moonnet"));
    }

    #[test]
    fn parse_scheme_accepts_known_and_rejects_unknown() {
        assert_eq!(parse_scheme("unified").unwrap(), SchemeArg::Unified);
        assert_eq!(parse_scheme("DUAL").unwrap(), SchemeArg::Dual);
        assert!(matches!(parse_scheme("triple"), Err(AppError::Invalid(_))));
    }

    #[test]
    fn parse_payment_address_type_accepts_known_and_rejects_unknown() {
        assert_eq!(
            parse_payment_address_type(" native ").unwrap(),
            PaymentAddressTypeArg::Native
        );
        assert_eq!(
            parse_payment_address_type("NESTED").unwrap(),
            PaymentAddressTypeArg::Nested
        );
        assert_eq!(
            parse_payment_address_type("legacy").unwrap(),
            PaymentAddressTypeArg::Legacy
        );
        assert!(matches!(
            parse_payment_address_type("p2pk"),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn parse_bool_value_reports_context_on_error() {
        assert!(parse_bool_value("YES", "flag").unwrap());
        assert!(!parse_bool_value(" off ", "flag").unwrap());
        let err = parse_bool_value("nope", "my-flag").expect_err("garbage rejected");
        assert!(err.contains("my-flag"));
        assert!(err.contains("true,false"));
    }

    #[test]
    fn levenshtein_basic_distances() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("kitten", "kitten"), 0);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("flaw", "lawn"), 2);
    }

    #[test]
    fn best_match_returns_close_candidate_within_threshold() {
        let candidates = ["network", "scheme", "payment"];
        assert_eq!(best_match("netwrk", &candidates), Some("network"));
        // Too far from any candidate -> None.
        assert_eq!(best_match("zzzzzzzz", &candidates), None);
        // Exact match has distance 0.
        assert_eq!(best_match("scheme", &candidates), Some("scheme"));
    }

    #[test]
    fn best_match_empty_candidates_is_none() {
        assert_eq!(best_match("anything", &[]), None);
    }

    #[test]
    fn unknown_with_hint_includes_suggestion_when_close() {
        let candidates = ["mainnet", "signet", "testnet", "regtest"];
        let hinted = unknown_with_hint("network", "mainet", &candidates);
        assert!(hinted.contains("unknown network: mainet"));
        assert!(hinted.contains("did you mean mainnet?"));

        let no_hint = unknown_with_hint("network", "zzzzzzzzz", &candidates);
        assert!(no_hint.contains("unknown network: zzzzzzzzz"));
        assert!(!no_hint.contains("did you mean"));
    }

    #[test]
    fn parse_indices_handles_none_csv_and_ranges() {
        assert_eq!(parse_indices(None).unwrap(), Vec::<usize>::new());
        assert_eq!(parse_indices(Some("")).unwrap(), Vec::<usize>::new());
        assert_eq!(parse_indices(Some("3")).unwrap(), vec![3]);
        assert_eq!(parse_indices(Some("1, 2 ,3")).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_indices(Some("0-3")).unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(parse_indices(Some("5,7-8")).unwrap(), vec![5, 7, 8]);
        // Empty segments between commas are skipped.
        assert_eq!(parse_indices(Some("1,,2")).unwrap(), vec![1, 2]);
    }

    #[test]
    fn parse_indices_rejects_malformed_input() {
        assert!(matches!(
            parse_indices(Some("1-2-3")),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            parse_indices(Some("a")),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            parse_indices(Some("x-2")),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            parse_indices(Some("1-y")),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn resolve_psbt_source_rejects_multiple_sources() {
        let err = resolve_psbt_source(Some("psbtdata"), None, true)
            .expect_err("multiple sources rejected");
        assert!(matches!(err, AppError::Invalid(_)));
        assert!(err.to_string().contains("only one"));
    }

    #[test]
    fn resolve_psbt_source_returns_inline_value() {
        assert_eq!(
            resolve_psbt_source(Some("cHNidP8="), None, false).unwrap(),
            "cHNidP8="
        );
    }

    #[test]
    fn resolve_psbt_source_requires_a_source() {
        let err = resolve_psbt_source(None, None, false).expect_err("a source is required");
        assert!(matches!(err, AppError::Invalid(_)));
        assert!(err.to_string().contains("requires one of"));
    }

    #[test]
    fn resolve_psbt_source_reads_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("zinc_psbt_{}_{}.txt", std::process::id(), "src"));
        std::fs::write(&path, "psbt-from-file").unwrap();
        let got = resolve_psbt_source(None, Some(path.as_path()), false).unwrap();
        assert_eq!(got, "psbt-from-file");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolve_psbt_source_missing_file_maps_to_io_error() {
        let missing = Path::new("/nonexistent/zinc-cli-test/does-not-exist.psbt");
        let err = resolve_psbt_source(None, Some(missing), false)
            .expect_err("missing file should error");
        assert!(matches!(err, AppError::Io(_)));
    }

    #[test]
    fn maybe_write_text_is_noop_for_none() {
        assert!(maybe_write_text(None, "ignored").is_ok());
    }

    #[test]
    fn maybe_write_text_writes_file_contents() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("zinc_write_{}_{}.txt", std::process::id(), "ok"));
        let path_str = path.to_string_lossy().to_string();
        maybe_write_text(Some(&path_str), "hello world").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn run_bitcoin_cli_missing_binary_is_internal_error() {
        let profile = Profile {
            version: 1,
            scan_policy_version: 0,
            network: NetworkArg::Regtest,
            scheme: SchemeArg::Dual,
            payment_address_type: PaymentAddressTypeArg::Native,
            account_index: 0,
            esplora_url: String::new(),
            ord_url: String::new(),
            pulse_url: String::new(),
            bitcoin_cli: "/nonexistent/zinc-bitcoin-cli-binary".to_string(),
            bitcoin_cli_args: vec![],
            encrypted_mnemonic: None,
            mode: crate::config::ProfileModeArg::Seed,
            taproot_xpub: None,
            payment_xpub: None,
            watch_address: None,
            account_gap_limit: crate::config::default_gap_limit(),
            address_scan_depth: crate::config::default_scan_depth(),
            accounts: std::collections::BTreeMap::new(),
            updated_at_unix: 0,
            pulse_session: None,
        };
        let err =
            run_bitcoin_cli(&profile, &["getblockchaininfo".to_string()]).expect_err("no binary");
        assert!(matches!(err, AppError::Internal(_)));
        assert!(err.to_string().contains("bitcoin-cli"));
    }
}
