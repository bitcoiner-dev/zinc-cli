use serde_json::Value;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn unique_data_dir(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("/tmp/{}_{}_{}", prefix, std::process::id(), nanos)
}

fn cargo_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    let isolated_home = unique_data_dir("zinc_cli_test_home");
    let _ = fs::create_dir_all(&isolated_home);
    cmd.env("HOME", isolated_home);
    for key in [
        "ZINC_CLI_PROFILE",
        "ZINC_CLI_DATA_DIR",
        "ZINC_CLI_PASSWORD_ENV",
        "ZINC_CLI_JSON",
        "ZINC_CLI_QUIET",
        "ZINC_CLI_NETWORK",
        "ZINC_CLI_SCHEME",
        "ZINC_CLI_ESPLORA_URL",
        "ZINC_CLI_ORD_URL",
        "ZINC_CLI_CORRELATION_ID",
        "ZINC_CLI_LOG_JSON",
        "ZINC_CLI_IDEMPOTENCY_KEY",
        "ZINC_CLI_NETWORK_TIMEOUT_SECS",
        "ZINC_CLI_NETWORK_RETRIES",
        "ZINC_CLI_POLICY_MODE",
    ] {
        cmd.env_remove(key);
    }
    cmd
}

fn init_wallet(data_dir: &str, password: &str) {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        data_dir,
        "--password",
        password,
        "wallet",
        "init",
        "--overwrite",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(
        output.status.success(),
        "wallet init failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn parse_json_from_output(output: &str) -> Value {
    for line in output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            if let Ok(json) = serde_json::from_str(trimmed) {
                return json;
            }
        }
    }
    panic!("No valid JSON found in output: {}", output);
}

fn parse_json_lines(output: &str) -> Vec<Value> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('{') {
                serde_json::from_str::<Value>(trimmed).ok()
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn test_json_envelope_help() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "help"]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], true);
    assert_eq!(json["schema_version"], "1.0");
    assert_eq!(json["command"], "help");
    assert!(json.get("command_list").is_some());
    assert!(json.get("correlation_id").is_some());
    assert!(json.get("meta").is_some());
}

#[test]
fn test_correlation_id_can_be_overridden_by_flag() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--correlation-id",
        "agent-run-42",
        "help",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["correlation_id"], "agent-run-42");
}

#[test]
fn test_log_json_emits_structured_stderr_events() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--log-json",
        "config",
        "show",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope = parse_json_from_output(&stdout);
    let correlation_id = envelope
        .get("correlation_id")
        .and_then(Value::as_str)
        .expect("missing correlation_id in envelope")
        .to_string();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let logs = parse_json_lines(&stderr);
    assert!(
        logs.len() >= 2,
        "expected at least start/finish log lines, got {}",
        logs.len()
    );

    let has_start = logs.iter().any(|line| {
        line.get("event") == Some(&Value::String("command_start".to_string()))
            && line.get("correlation_id") == Some(&Value::String(correlation_id.clone()))
    });
    let has_finish = logs.iter().any(|line| {
        line.get("event") == Some(&Value::String("command_finish".to_string()))
            && line.get("correlation_id") == Some(&Value::String(correlation_id.clone()))
    });

    assert!(has_start, "missing command_start structured log");
    assert!(has_finish, "missing command_finish structured log");
}

#[test]
fn test_json_envelope_error() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "invalid-command"]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], false);
    assert_eq!(json["schema_version"], "1.0");
    assert!(json.get("error").is_some());
}

#[test]
fn test_mnemonic_hiding() {
    let data_dir = "/tmp/zinc_test_hiding";
    let _ = fs::remove_dir_all(data_dir);

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        data_dir,
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);

    let output = cmd.output().expect("failed to execute process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], true);
    assert_eq!(json["phrase"], "<hidden; use --reveal to show>");
    assert!(json.get("words").is_none());
}

#[test]
fn test_wallet_init_human_mode_shows_mnemonic() {
    let data_dir = unique_data_dir("zinc_test_human_init_shows_phrase");
    let _ = fs::remove_dir_all(&data_dir);

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Wallet initialized"));
    assert!(stdout.contains("Phrase"));
    assert!(!stdout.contains("<hidden"));
}

#[test]
fn test_mnemonic_reveal() {
    let data_dir = "/tmp/zinc_test_reveal";
    let _ = fs::remove_dir_all(data_dir);

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--reveal",
        "--data-dir",
        data_dir,
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);

    let output = cmd.output().expect("failed to execute process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], true);
    assert_ne!(json["phrase"], "<hidden; use --reveal to show>");
    assert!(json.get("words").is_some());
}

#[test]
fn test_password_stdin() {
    let data_dir = "/tmp/zinc_test_stdin";
    let _ = fs::remove_dir_all(data_dir);

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        data_dir,
        "--password-stdin",
        "wallet",
        "init",
        "--overwrite",
    ]);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn child");
    let mut stdin = child.stdin.take().expect("failed to open stdin");
    stdin
        .write_all(b"testpass\n")
        .expect("failed to write to stdin");
    drop(stdin);

    let output = child.wait_with_output().expect("failed to wait for child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], true);
}

#[test]
fn test_lock_failure() {
    let data_dir = "/tmp/zinc_test_lock";
    let _ = fs::remove_dir_all(data_dir);
    fs::create_dir_all(format!("{}/profiles", data_dir)).unwrap();

    // Create a lock file
    let lock_path = format!("{}/profiles/default.lock", data_dir);
    fs::write(&lock_path, "test").unwrap();

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        data_dir,
        "wallet",
        "init",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    assert_eq!(json["ok"], false);
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("locked"));
}

#[test]
fn test_lock_info_and_clear() {
    let data_dir = "/tmp/zinc_test_lock_ops";
    let _ = fs::remove_dir_all(data_dir);
    fs::create_dir_all(format!("{}/profiles", data_dir)).unwrap();

    let lock_path = format!("{}/profiles/default.lock", data_dir);
    fs::write(&lock_path, r#"{"pid":1234,"created_at_unix":1}"#).unwrap();

    let mut info_cmd = cargo_cmd();
    info_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        data_dir,
        "lock",
        "info",
    ]);
    let info_output = info_cmd.output().expect("failed to execute process");
    assert!(info_output.status.success());
    let info_stdout = String::from_utf8_lossy(&info_output.stdout);
    let info_json = parse_json_from_output(&info_stdout);
    assert_eq!(info_json["ok"], true);
    assert_eq!(info_json["locked"], true);
    assert_eq!(info_json["owner_pid"], 1234);

    let mut clear_cmd = cargo_cmd();
    clear_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--yes",
        "--data-dir",
        data_dir,
        "lock",
        "clear",
    ]);
    let clear_output = clear_cmd.output().expect("failed to execute process");
    assert!(clear_output.status.success());
    let clear_stdout = String::from_utf8_lossy(&clear_output.stdout);
    let clear_json = parse_json_from_output(&clear_stdout);
    assert_eq!(clear_json["ok"], true);
    assert_eq!(clear_json["cleared"], true);
    assert!(!std::path::Path::new(&lock_path).exists());
}

#[test]
fn test_password_stdin_conflicts_with_psbt_stdin() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password-stdin",
        "psbt",
        "analyze",
        "--psbt-stdin",
    ]);
    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--password-stdin cannot be combined with --psbt-stdin"));
}

#[test]
fn test_psbt_analyze_requires_input_source() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "psbt", "analyze"]);
    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("requires one of --psbt, --psbt-file, --psbt-stdin"));
}

#[test]
fn test_psbt_analyze_rejects_multiple_input_sources() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "psbt",
        "analyze",
        "--psbt",
        "cHNidP8BAAoCAAAAAQAAAAA=",
        "--psbt-file",
        "/tmp/not-used.psbt",
    ]);
    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("accepts only one of --psbt, --psbt-file, --psbt-stdin"));
}

#[test]
fn test_psbt_analyze_stdin_empty_is_invalid() {
    let data_dir = unique_data_dir("zinc_test_psbt_stdin_empty");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "psbt",
        "analyze",
        "--psbt-stdin",
    ]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn child");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("failed to wait for child");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("stdin did not contain a PSBT string"));
}

#[test]
fn test_atomic_write_ignores_corrupt_temp_file() {
    let data_dir = unique_data_dir("zinc_test_atomic_temp");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let profiles_dir = format!("{}/profiles", data_dir);
    let profile_path = format!("{}/default.json", profiles_dir);
    let corrupt_temp = format!("{}/.default.json.tmp-999-1", profiles_dir);
    fs::write(&corrupt_temp, "{ not-valid-json").expect("failed to write temp");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "account",
        "use",
        "--index",
        "1",
    ]);
    let output = cmd.output().expect("failed to execute process");
    assert!(
        output.status.success(),
        "account use failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let profile_bytes = fs::read_to_string(&profile_path).expect("failed to read profile");
    let parsed: Value = serde_json::from_str(&profile_bytes).expect("profile should be valid json");
    assert_eq!(parsed["account_index"], 1);
}

#[test]
fn test_corrupt_profile_json_maps_to_config_error() {
    let data_dir = unique_data_dir("zinc_test_corrupt_profile");
    let _ = fs::remove_dir_all(&data_dir);
    let profiles_dir = format!("{}/profiles", data_dir);
    fs::create_dir_all(&profiles_dir).expect("failed to create profiles dir");
    fs::write(format!("{}/default.json", profiles_dir), "{invalid-json")
        .expect("failed to write corrupt profile");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "wallet",
        "info",
    ]);
    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "config");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("failed to parse profile"));
}

#[test]
fn test_agent_flag_emits_json_envelope() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "help"]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "help");
}

#[test]
fn test_unknown_command_has_suggestion() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "walet", "info"]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("did you mean wallet"));
}

#[test]
fn test_unknown_global_flag_has_suggestion() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "--jons", "wallet", "info"]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unexpected argument"));
}

#[test]
fn test_unknown_option_is_rejected_with_hint() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password",
        "testpass",
        "wallet",
        "init",
        "--word",
        "24",
        "--overwrite",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("did you mean --words"));
}

#[test]
fn test_global_flags_work_after_command() {
    let data_dir = unique_data_dir("zinc_test_global_flags_anywhere");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "wallet",
        "info",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "--agent",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "wallet info");
}

#[test]
fn test_config_show_reflects_env_defaults() {
    let data_dir = unique_data_dir("zinc_test_config_show");
    let _ = fs::remove_dir_all(&data_dir);

    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "config", "show"]);
    cmd.env("ZINC_CLI_PROFILE", "env-profile");
    cmd.env("ZINC_CLI_DATA_DIR", &data_dir);
    cmd.env("ZINC_CLI_PASSWORD_ENV", "BOT_PASS_ENV");
    cmd.env("ZINC_CLI_OUTPUT", "agent");
    cmd.env("ZINC_CLI_QUIET", "true");

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["profile"], "env-profile");
    assert_eq!(json["data_dir"], data_dir);
    assert_eq!(json["password_env"], "BOT_PASS_ENV");
    assert_eq!(json["agent"], true);
    assert_eq!(json["quiet"], true);
}

#[test]
fn test_wallet_init_uses_env_network_defaults() {
    let data_dir = unique_data_dir("zinc_test_env_network");
    let _ = fs::remove_dir_all(&data_dir);

    let mut init_cmd = cargo_cmd();
    init_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);
    init_cmd.env("ZINC_CLI_NETWORK", "signet");
    init_cmd.env("ZINC_CLI_SCHEME", "unified");

    let init_output = init_cmd.output().expect("failed to execute process");
    assert!(
        init_output.status.success(),
        "wallet init failed: {}",
        String::from_utf8_lossy(&init_output.stdout)
    );

    let mut info_cmd = cargo_cmd();
    info_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "info",
    ]);
    let info_output = info_cmd.output().expect("failed to execute process");
    assert!(info_output.status.success());
    let info_json = parse_json_from_output(&String::from_utf8_lossy(&info_output.stdout));
    assert_eq!(info_json["network"], "signet");
    assert_eq!(info_json["scheme"], "unified");
}

#[test]
fn test_json_mode_can_be_enabled_by_env() {
    let data_dir = unique_data_dir("zinc_test_env_json");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "info",
    ]);
    cmd.env("ZINC_CLI_OUTPUT", "agent");

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "wallet info");
}

#[test]
fn test_config_set_and_unset_roundtrip() {
    let home = unique_data_dir("zinc_test_config_roundtrip_home");
    fs::create_dir_all(&home).expect("failed to create test home");

    let mut set_cmd = cargo_cmd();
    set_cmd.args(&[
        "run", "--quiet", "--", "--agent", "config", "set", "network", "signet",
    ]);
    set_cmd.env("HOME", &home);
    let set_output = set_cmd.output().expect("failed to execute process");
    assert!(set_output.status.success());

    let mut show_cmd = cargo_cmd();
    show_cmd.args(&["run", "--quiet", "--", "--agent", "config", "show"]);
    show_cmd.env("HOME", &home);
    let show_output = show_cmd.output().expect("failed to execute process");
    assert!(show_output.status.success());
    let show_json = parse_json_from_output(&String::from_utf8_lossy(&show_output.stdout));
    assert_eq!(show_json["defaults"]["network"], "signet");

    let mut unset_cmd = cargo_cmd();
    unset_cmd.args(&[
        "run", "--quiet", "--", "--agent", "config", "unset", "network",
    ]);
    unset_cmd.env("HOME", &home);
    let unset_output = unset_cmd.output().expect("failed to execute process");
    assert!(unset_output.status.success());

    let mut show_after_cmd = cargo_cmd();
    show_after_cmd.args(&["run", "--quiet", "--", "--agent", "config", "show"]);
    show_after_cmd.env("HOME", &home);
    let show_after_output = show_after_cmd.output().expect("failed to execute process");
    assert!(show_after_output.status.success());
    let show_after_json =
        parse_json_from_output(&String::from_utf8_lossy(&show_after_output.stdout));
    assert!(show_after_json["defaults"]["network"].is_null());
}

#[test]
fn test_setup_persists_defaults_and_wallet_uses_them() {
    let home = unique_data_dir("zinc_test_setup_home");
    fs::create_dir_all(&home).expect("failed to create test home");
    let data_dir = unique_data_dir("zinc_test_setup_data");
    let _ = fs::remove_dir_all(&data_dir);

    let mut setup_cmd = cargo_cmd();
    setup_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "setup",
        "--profile",
        "bot-a",
        "--data-dir",
        &data_dir,
        "--password-env",
        "BOT_PASS_ENV",
        "--default-network",
        "signet",
        "--default-scheme",
        "unified",
        "--quiet-default",
        "true",
    ]);
    setup_cmd.env("HOME", &home);
    let setup_output = setup_cmd.output().expect("failed to execute process");
    assert!(setup_output.status.success());

    let mut init_cmd = cargo_cmd();
    init_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);
    init_cmd.env("HOME", &home);
    let init_output = init_cmd.output().expect("failed to execute process");
    assert!(init_output.status.success());
    let init_json = parse_json_from_output(&String::from_utf8_lossy(&init_output.stdout));
    assert_eq!(init_json["network"], "signet");
    assert_eq!(init_json["scheme"], "unified");

    let mut info_cmd = cargo_cmd();
    info_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password",
        "testpass",
        "wallet",
        "info",
    ]);
    info_cmd.env("HOME", &home);
    let info_output = info_cmd.output().expect("failed to execute process");
    assert!(info_output.status.success());
    let info_json = parse_json_from_output(&String::from_utf8_lossy(&info_output.stdout));
    assert_eq!(info_json["profile"], "bot-a");
    assert_eq!(info_json["network"], "signet");
    assert_eq!(info_json["scheme"], "unified");
}

#[test]
fn test_setup_without_flags_uses_noninteractive_path_in_tests() {
    let home = unique_data_dir("zinc_test_setup_no_flags_home");
    fs::create_dir_all(&home).expect("failed to create test home");

    let mut setup_cmd = cargo_cmd();
    setup_cmd.args(&["run", "--quiet", "--", "--agent", "setup"]);
    setup_cmd.env("HOME", &home);

    let output = setup_cmd.output().expect("failed to execute process");
    assert!(output.status.success());

    let json = parse_json_from_output(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["ok"], true);
    assert_eq!(json["wizard_used"], false);
    assert_eq!(json["profile"], "default");
    assert_eq!(json["data_dir"], format!("{}/.zinc-cli", home));
    assert_eq!(json["defaults"]["network"], "regtest");
    assert_eq!(
        json["defaults"]["esplora_url"],
        "https://regtest.exittheloop.com/api"
    );
    assert_eq!(
        json["defaults"]["ord_url"],
        "https://ord-regtest.exittheloop.com"
    );
    assert_eq!(json["wallet"]["requested"], false);
    assert_eq!(json["wallet"]["initialized"], false);

    let config_path = format!("{}/.zinc-cli/config.json", home);
    assert!(std::path::Path::new(&config_path).exists());
}

#[test]
fn test_wallet_reveal_mnemonic_matches_created_wallet_phrase() {
    let data_dir = unique_data_dir("zinc_test_reveal_existing_wallet");
    let _ = fs::remove_dir_all(&data_dir);

    let mut init_cmd = cargo_cmd();
    init_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--reveal",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "init",
        "--overwrite",
    ]);
    let init_output = init_cmd.output().expect("failed to execute process");
    assert!(init_output.status.success());
    let init_json = parse_json_from_output(&String::from_utf8_lossy(&init_output.stdout));
    let init_phrase = init_json["phrase"]
        .as_str()
        .expect("missing init phrase")
        .to_string();

    let mut reveal_cmd = cargo_cmd();
    reveal_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "reveal-mnemonic",
    ]);
    let reveal_output = reveal_cmd.output().expect("failed to execute process");
    assert!(reveal_output.status.success());
    let reveal_json = parse_json_from_output(&String::from_utf8_lossy(&reveal_output.stdout));
    assert_eq!(reveal_json["phrase"], init_phrase);
    assert_eq!(
        reveal_json["words"],
        init_phrase.split_whitespace().count() as u64
    );
}

#[test]
fn test_wallet_reveal_mnemonic_requires_correct_password() {
    let data_dir = unique_data_dir("zinc_test_reveal_wrong_password");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "wrong-pass",
        "wallet",
        "reveal-mnemonic",
    ]);
    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let json = parse_json_from_output(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "auth");
}

#[test]
fn test_parse_error_agent_mode_enabled_by_env() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "invalid-command"]);
    cmd.env("ZINC_CLI_OUTPUT", "agent");
    cmd.env("ZINC_CLI_CORRELATION_ID", "preparse-env-cid");

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "invalid");
    assert_eq!(json["correlation_id"], "preparse-env-cid");
}

#[test]
fn test_setup_propagates_password_env_and_restore_mnemonic() {
    let home = unique_data_dir("zinc_test_setup_restore_home");
    fs::create_dir_all(&home).expect("failed to create test home");

    let data_dir = unique_data_dir("zinc_test_setup_restore_data");
    let _ = fs::remove_dir_all(&data_dir);
    let mnemonic = "rotate text off rich waste jump grab doctor today renew fault exotic";

    let mut setup_cmd = cargo_cmd();
    setup_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password",
        "testpass",
        "setup",
        "--data-dir",
        &data_dir,
        "--password-env",
        "BOT_PASS",
        "--restore-mnemonic",
        mnemonic,
    ]);
    setup_cmd.env("HOME", &home);

    let setup_output = setup_cmd.output().expect("failed to execute process");
    assert!(setup_output.status.success());
    let setup_json = parse_json_from_output(&String::from_utf8_lossy(&setup_output.stdout));

    assert_eq!(setup_json["ok"], true);
    assert_eq!(setup_json["password_env"], "BOT_PASS");
    assert_eq!(setup_json["wallet"]["wallet_initialized"], true);
    assert_eq!(setup_json["wallet"]["mode"], "restore");
    assert_eq!(setup_json["wallet"]["word_count"], 12);

    let config_path = format!("{}/.zinc-cli/config.json", home);
    let config_bytes = fs::read_to_string(&config_path).expect("failed to read config");
    let config_json: Value =
        serde_json::from_str(&config_bytes).expect("config should contain valid json");
    assert_eq!(config_json["password_env"], "BOT_PASS");

    let mut reveal_cmd = cargo_cmd();
    reveal_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wallet",
        "reveal-mnemonic",
    ]);
    reveal_cmd.env("HOME", &home);

    let reveal_output = reveal_cmd.output().expect("failed to execute process");
    assert!(reveal_output.status.success());
    let reveal_json = parse_json_from_output(&String::from_utf8_lossy(&reveal_output.stdout));
    assert_eq!(reveal_json["phrase"], mnemonic);
}

#[test]
fn test_setup_words_24_generates_24_word_wallet() {
    let home = unique_data_dir("zinc_test_setup_words_home");
    fs::create_dir_all(&home).expect("failed to create test home");
    let data_dir = unique_data_dir("zinc_test_setup_words_data");
    let _ = fs::remove_dir_all(&data_dir);

    let mut setup_cmd = cargo_cmd();
    setup_cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--password",
        "testpass",
        "setup",
        "--data-dir",
        &data_dir,
        "--words",
        "24",
    ]);
    setup_cmd.env("HOME", &home);

    let setup_output = setup_cmd.output().expect("failed to execute process");
    assert!(setup_output.status.success());
    let setup_json = parse_json_from_output(&String::from_utf8_lossy(&setup_output.stdout));
    assert_eq!(setup_json["ok"], true);
    assert_eq!(setup_json["wallet"]["wallet_initialized"], true);
    assert_eq!(setup_json["wallet"]["mode"], "generate");
    assert_eq!(setup_json["wallet"]["word_count"], 24);
}

#[test]
fn test_wait_balance_zero_target_includes_contract_fields() {
    let data_dir = unique_data_dir("zinc_test_wait_balance_zero");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "wait",
        "balance",
        "--confirmed-at-least",
        "0",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(output.status.success());
    let json = parse_json_from_output(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["ok"], true);
    assert!(json.get("confirmed").is_some());
    assert!(json.get("confirmed_balance").is_some());
    assert_eq!(json["target"], 0);
}

#[test]
fn test_sync_chain_network_error_maps_to_network_type() {
    let data_dir = unique_data_dir("zinc_test_sync_network_error");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "--esplora-url",
        "http://127.0.0.1:1",
        "sync",
        "chain",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let json = parse_json_from_output(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "network");
}

#[test]
fn test_psbt_analyze_missing_file_maps_to_config_type() {
    let mut cmd = cargo_cmd();
    cmd.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "psbt",
        "analyze",
        "--psbt-file",
        "/tmp/zinc_missing_psbt_input_file.psbt",
    ]);

    let output = cmd.output().expect("failed to execute process");
    assert!(!output.status.success());
    let json = parse_json_from_output(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["type"], "config");
}

#[test]
fn test_idempotency_replays_mutating_command_response() {
    let data_dir = unique_data_dir("zinc_test_idempotency_data");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut first = cargo_cmd();
    first.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--idempotency-key",
        "idem-001",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "account",
        "use",
        "--index",
        "1",
    ]);
    let first_output = first.output().expect("failed to execute process");
    assert!(first_output.status.success());
    let first_json = parse_json_from_output(&String::from_utf8_lossy(&first_output.stdout));
    assert_eq!(first_json["ok"], true);
    assert_eq!(first_json["account_index"], 1);
    assert_eq!(first_json["idempotency"]["replayed"], false);

    let mut second = cargo_cmd();
    second.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--idempotency-key",
        "idem-001",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "account",
        "use",
        "--index",
        "1",
    ]);
    let second_output = second.output().expect("failed to execute process");
    assert!(second_output.status.success());
    let second_json = parse_json_from_output(&String::from_utf8_lossy(&second_output.stdout));
    assert_eq!(second_json["ok"], true);
    assert_eq!(second_json["account_index"], 1);
    assert_eq!(second_json["idempotency"]["replayed"], true);
}

#[test]
fn test_idempotency_key_reuse_with_different_payload_is_rejected() {
    let data_dir = unique_data_dir("zinc_test_idempotency_conflict_data");
    let _ = fs::remove_dir_all(&data_dir);
    init_wallet(&data_dir, "testpass");

    let mut first = cargo_cmd();
    first.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--idempotency-key",
        "idem-conflict",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "account",
        "use",
        "--index",
        "1",
    ]);
    let first_output = first.output().expect("failed to execute process");
    assert!(first_output.status.success());

    let mut second = cargo_cmd();
    second.args(&[
        "run",
        "--quiet",
        "--",
        "--agent",
        "--idempotency-key",
        "idem-conflict",
        "--data-dir",
        &data_dir,
        "--password",
        "testpass",
        "account",
        "use",
        "--index",
        "2",
    ]);
    let second_output = second.output().expect("failed to execute process");
    assert!(!second_output.status.success());
    let second_json = parse_json_from_output(&String::from_utf8_lossy(&second_output.stdout));
    assert_eq!(second_json["ok"], false);
    assert_eq!(second_json["error"]["type"], "invalid");
    assert!(second_json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("idempotency key"));
}
