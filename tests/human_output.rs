use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_data_dir(prefix: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("zinc-cli-{prefix}-{}-{now}", std::process::id()))
}

#[test]
fn pulse_whoami_human_mode_renders_message_output() {
    let data_dir = temp_data_dir("human-output");
    fs::create_dir_all(&data_dir).expect("create test data dir");

    let output = Command::cargo_bin("zinc-cli")
        .expect("binary should build")
        .env("ZINC_CLI_DATA_DIR", data_dir.as_os_str())
        .arg("pulse")
        .arg("whoami")
        .output()
        .expect("command should execute");

    let _ = fs::remove_dir_all(&data_dir);

    assert!(
        output.status.success(),
        "expected success status; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "expected non-empty stdout for message output"
    );
    assert!(
        stdout.contains("Not logged in."),
        "expected login status message in stdout; got: {}",
        stdout
    );
}
