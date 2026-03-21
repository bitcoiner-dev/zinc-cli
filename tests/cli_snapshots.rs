use assert_cmd::Command;
use insta::assert_json_snapshot;

#[test]
fn test_inscription_list_no_wallet_json() {
    let mut cmd = Command::cargo_bin("zinc-cli").unwrap();

    // Run the command that requires a wallet but in an empty environment
    cmd.env("ZINC_CLI_DATA_DIR", "/tmp/nonexistent_zinc_dir_test")
        .arg("inscription")
        .arg("list")
        .arg("--json");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Parse the output as JSON so insta can format it nicely
    let json_output: serde_json::Value =
        serde_json::from_str(&stdout).expect("CLI did not output valid JSON");

    // We expect it to fail and return an error JSON schema
    assert_json_snapshot!(json_output, {
        ".correlation_id" => "[REDACTED_CORRELATION_ID]",
        ".meta.started_at_unix_ms" => 0,
        ".meta.duration_ms" => 0,
        ".error.message" => "[REDACTED_ERROR_MESSAGE]" // Redact the exact message if it contains paths
    });
}
