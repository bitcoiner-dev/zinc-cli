use serde_json::{json, Value};
use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TEST_MNEMONIC: &str = "rotate text off rich waste jump grab doctor today renew fault exotic";
const TEST_PASSWORD: &str = "test_password_123";
const REGTEST_ESPLORA_URL: &str = "https://regtest.exittheloop.com/api";
const REGTEST_ORD_URL: &str = "https://ord-regtest.exittheloop.com";
const DEFAULT_NOSTR_RELAY_URL: &str = "wss://nostr-regtest.exittheloop.com";
const TEST_SECRET_KEY_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000001";
const TEST_SELLER_PUBKEY_HEX: &str =
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn unique_data_dir(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("/tmp/{}_{}_{}", prefix, std::process::id(), nanos)
}

fn cargo_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    let isolated_home = unique_data_dir("zinc_cli_offer_live_home");
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
    ] {
        cmd.env_remove(key);
    }
    cmd
}

fn run_zinc(args: &[&str], data_dir: &str, password: &str) -> Value {
    let mut cmd = cargo_cmd();
    cmd.args(["run", "--quiet", "--"])
        .arg("--agent")
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--password")
        .arg(password);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().expect("failed to execute process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        panic!(
            "Command failed: zinc {}\nstdout: {}\nstderr: {}",
            args.join(" "),
            stdout,
            stderr
        );
    }

    let json = parse_json_from_output(&stdout);
    if !get_bool(&json, "ok") {
        panic!(
            "Command returned ok=false: zinc {}\njson: {}",
            args.join(" "),
            json
        );
    }
    json
}

fn run_zinc_allow_error(args: &[&str], data_dir: &str, password: &str) -> Value {
    let mut cmd = cargo_cmd();
    cmd.args(["run", "--quiet", "--"])
        .arg("--agent")
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--password")
        .arg(password);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().expect("failed to execute process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let json = parse_json_from_output(&stdout);
    if !output.status.success() && stderr.trim().is_empty() && !get_bool(&json, "ok") {
        return json;
    }
    if !output.status.success() && !stderr.trim().is_empty() {
        panic!(
            "Command failed with stderr output: zinc {}\nstdout: {}\nstderr: {}",
            args.join(" "),
            stdout,
            stderr
        );
    }
    json
}

fn is_ord_indexer_lag_policy_error(json: &Value) -> bool {
    if get_bool(json, "ok") {
        return false;
    }

    let err_type = json
        .get("error")
        .and_then(|e| e.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let err_message = json
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    err_type == "policy" && err_message.contains("indexer is lagging")
}

fn is_ord_offer_submission_disabled_error(json: &Value) -> bool {
    if get_bool(json, "ok") {
        return false;
    }

    let err_message = json
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    err_message.contains("does not accept offers")
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

fn get_bool(json: &Value, key: &str) -> bool {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {}", key))
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool for {}", key))
}

fn get_str(json: &Value, key: &str) -> String {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {} in JSON: {}", key, json))
        .as_str()
        .unwrap_or_else(|| panic!("Expected string for {}", key))
        .to_string()
}

fn get_u64(json: &Value, key: &str) -> u64 {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {} in JSON: {}", key, json))
        .as_u64()
        .unwrap_or_else(|| panic!("Expected u64 for {}", key))
}

fn tx_list_contains(tx_list_json: &Value, txid: &str) -> bool {
    tx_list_json
        .get("transactions")
        .and_then(Value::as_array)
        .map(|txs| {
            txs.iter()
                .any(|tx| tx.get("txid").and_then(Value::as_str) == Some(txid))
        })
        .unwrap_or(false)
}

fn live_tests_enabled() -> bool {
    std::env::var("ZINC_CLI_LIVE_TESTS")
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn ensure_live_test_enabled(test_name: &str) -> bool {
    if live_tests_enabled() {
        return true;
    }

    eprintln!(
        "Skipping {test_name}: set ZINC_CLI_LIVE_TESTS=1 to run live network integration checks."
    );
    false
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn relay_url() -> String {
    std::env::var("ZINC_CLI_TEST_NOSTR_RELAY_URL")
        .unwrap_or_else(|_| DEFAULT_NOSTR_RELAY_URL.to_string())
}

fn offer_json_payload() -> String {
    let created = now_unix();
    let expires = created + 3600;
    let nonce = now_nanos();
    let offer = json!({
        "version": 1u8,
        "seller_pubkey_hex": TEST_SELLER_PUBKEY_HEX,
        "network": "regtest",
        "inscription_id": "6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799i0",
        "seller_outpoint": "6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799:0",
        "ask_sats": 100_000u64,
        "fee_rate_sat_vb": 1u64,
        "psbt_base64": "cHNidP8BAAoCAAAAAQAAAAA=",
        "created_at_unix": created as i64,
        "expires_at_unix": expires as i64,
        "nonce": nonce
    });
    offer.to_string()
}

#[test]
fn test_offer_ord_submit_and_list_live() {
    if !ensure_live_test_enabled("test_offer_ord_submit_and_list_live") {
        return;
    }

    let data_dir = unique_data_dir("zinc_offer_ord_live");
    let _ = fs::remove_dir_all(&data_dir);

    let import_result = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "dual",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_eq!(get_str(&import_result, "network"), "regtest");

    run_zinc(
        &[
            "--esplora-url",
            REGTEST_ESPLORA_URL,
            "--ord-url",
            REGTEST_ORD_URL,
            "sync",
            "chain",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    let sync_ordinals = run_zinc_allow_error(
        &[
            "--esplora-url",
            REGTEST_ESPLORA_URL,
            "--ord-url",
            REGTEST_ORD_URL,
            "sync",
            "ordinals",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    if !get_bool(&sync_ordinals, "ok") {
        if is_ord_indexer_lag_policy_error(&sync_ordinals) {
            eprintln!(
                "Continuing test despite ord lag policy response: {}",
                sync_ordinals
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown ord lag policy error")
            );
        } else {
            panic!("sync ordinals failed unexpectedly: {sync_ordinals}");
        }
    }

    let recipient = run_zinc(&["address", "payment", "--new"], &data_dir, TEST_PASSWORD);
    let recipient_address = get_str(&recipient, "address");

    let create = run_zinc(
        &[
            "psbt",
            "create",
            "--to",
            &recipient_address,
            "--amount-sats",
            "1000",
            "--fee-rate",
            "1",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    let unsigned_psbt = get_str(&create, "psbt");
    assert!(!unsigned_psbt.is_empty());

    let submit = run_zinc_allow_error(
        &[
            "--ord-url",
            REGTEST_ORD_URL,
            "offer",
            "submit-ord",
            "--psbt",
            &unsigned_psbt,
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    if !get_bool(&submit, "ok") {
        if is_ord_offer_submission_disabled_error(&submit) {
            eprintln!(
                "Skipping test_offer_ord_submit_and_list_live: ord server does not accept offers (enable --accept-offers)."
            );
            return;
        }
        panic!("offer submit-ord failed unexpectedly: {submit}");
    }
    assert!(get_bool(&submit, "submitted"));

    let mut found = false;
    let mut last_count = 0usize;
    for _ in 0..10 {
        let listed = run_zinc(
            &["--ord-url", REGTEST_ORD_URL, "offer", "list-ord"],
            &data_dir,
            TEST_PASSWORD,
        );
        let offers = listed
            .get("offers")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("Missing offers array in list output: {listed}"));
        last_count = offers.len();
        if offers
            .iter()
            .filter_map(Value::as_str)
            .any(|offer| offer == unsigned_psbt)
        {
            found = true;
            break;
        }
        thread::sleep(Duration::from_secs(2));
    }

    assert!(
        found,
        "Submitted PSBT not found in ord offers after retries (last offer count={last_count})"
    );
}

#[test]
fn test_offer_nostr_publish_discover_live() {
    if !ensure_live_test_enabled("test_offer_nostr_publish_discover_live") {
        return;
    }

    let data_dir = unique_data_dir("zinc_offer_nostr_live");
    let _ = fs::remove_dir_all(&data_dir);
    let relay = relay_url();
    let offer_json = offer_json_payload();

    let publish = run_zinc(
        &[
            "offer",
            "publish",
            "--offer-json",
            &offer_json,
            "--secret-key-hex",
            TEST_SECRET_KEY_HEX,
            "--relay",
            &relay,
            "--timeout-ms",
            "10000",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    let accepted_relays = get_u64(&publish, "accepted_relays");
    if accepted_relays == 0 {
        let relay_message = publish
            .get("publish_results")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("no relay acceptance details returned");
        if relay_message
            .to_ascii_lowercase()
            .contains("tls support not compiled in")
        {
            panic!(
                "Nostr publish failed because wss TLS support is missing in this build: {relay_message}"
            );
        }
        eprintln!(
            "Skipping test_offer_nostr_publish_discover_live: no relays accepted publish ({}).",
            relay_message
        );
        return;
    }

    let event_id = publish
        .get("event")
        .and_then(Value::as_object)
        .and_then(|event| event.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Missing event.id in publish output: {publish}"))
        .to_string();

    let mut found = false;
    let mut last_event_count = 0usize;
    for _ in 0..12 {
        let discovered = run_zinc(
            &[
                "offer",
                "discover",
                "--relay",
                &relay,
                "--limit",
                "500",
                "--timeout-ms",
                "10000",
            ],
            &data_dir,
            TEST_PASSWORD,
        );

        let events = discovered
            .get("events")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("Missing events array in discover output: {discovered}"));
        last_event_count = events.len();

        if events.iter().any(|event| {
            event
                .get("id")
                .and_then(Value::as_str)
                .map(|id| id == event_id)
                .unwrap_or(false)
        }) {
            found = true;
            break;
        }

        thread::sleep(Duration::from_secs(2));
    }

    assert!(
        found,
        "Published event {} not found via discover after retries (last event count={})",
        event_id, last_event_count
    );
}

#[test]
fn test_offer_create_and_accept_live() {
    if !ensure_live_test_enabled("test_offer_create_and_accept_live") {
        return;
    }

    let data_dir = unique_data_dir("zinc_offer_accept_live");
    let _ = fs::remove_dir_all(&data_dir);

    let import_result = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "dual",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_eq!(get_str(&import_result, "network"), "regtest");

    run_zinc(
        &["--esplora-url", REGTEST_ESPLORA_URL, "sync", "chain"],
        &data_dir,
        TEST_PASSWORD,
    );

    run_zinc(
        &["account", "use", "--index", "1"],
        &data_dir,
        TEST_PASSWORD,
    );
    let recipient = run_zinc(&["address", "payment", "--new"], &data_dir, TEST_PASSWORD);
    let recipient_address = get_str(&recipient, "address");
    run_zinc(
        &["account", "use", "--index", "0"],
        &data_dir,
        TEST_PASSWORD,
    );

    let create = run_zinc(
        &[
            "psbt",
            "create",
            "--to",
            &recipient_address,
            "--amount-sats",
            "1000",
            "--fee-rate",
            "1",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    let unsigned_psbt = get_str(&create, "psbt");
    assert!(!unsigned_psbt.is_empty());

    let analyze = run_zinc(
        &["psbt", "analyze", "--psbt", &unsigned_psbt],
        &data_dir,
        TEST_PASSWORD,
    );
    let analysis = analyze
        .get("analysis")
        .unwrap_or_else(|| panic!("Missing analysis in psbt analyze output: {analyze}"));
    let inputs = analysis
        .get("inputs")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("Missing analysis.inputs in psbt analyze output: {analyze}"));
    assert!(
        !inputs.is_empty(),
        "Expected at least one PSBT input in analysis: {analysis}"
    );

    let seller_txid = inputs[0]
        .get("txid")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Missing inputs[0].txid in analysis: {analysis}"));
    let seller_vout = inputs[0]
        .get("vout")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("Missing inputs[0].vout in analysis: {analysis}"));
    let seller_outpoint = format!("{seller_txid}:{seller_vout}");

    let offer_psbt = if inputs.len() == 1 {
        unsigned_psbt.clone()
    } else {
        let buyer_indices = (1..inputs.len())
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let signed_buyers = run_zinc(
            &[
                "psbt",
                "sign",
                "--psbt",
                &unsigned_psbt,
                "--sign-inputs",
                &buyer_indices,
                "--finalize",
            ],
            &data_dir,
            TEST_PASSWORD,
        );
        get_str(&signed_buyers, "psbt")
    };

    let created_at = now_unix();
    let expires_at = created_at + 3600;
    let ask_sats = 1000u64;
    let inscription_id = "integration-offer-inscription-1";
    let offer_json = json!({
        "version": 1u8,
        "seller_pubkey_hex": TEST_SELLER_PUBKEY_HEX,
        "network": "regtest",
        "inscription_id": inscription_id,
        "seller_outpoint": seller_outpoint,
        "ask_sats": ask_sats,
        "fee_rate_sat_vb": 1u64,
        "psbt_base64": offer_psbt,
        "created_at_unix": created_at as i64,
        "expires_at_unix": expires_at as i64,
        "nonce": now_nanos()
    })
    .to_string();

    let accepted = run_zinc(
        &[
            "--esplora-url",
            REGTEST_ESPLORA_URL,
            "offer",
            "accept",
            "--offer-json",
            &offer_json,
            "--expect-inscription",
            inscription_id,
            "--expect-ask-sats",
            &ask_sats.to_string(),
        ],
        &data_dir,
        TEST_PASSWORD,
    );

    assert!(get_bool(&accepted, "accepted"));
    assert!(!get_bool(&accepted, "dry_run"));
    assert_eq!(get_u64(&accepted, "input_count"), inputs.len() as u64);
    assert_eq!(get_u64(&accepted, "seller_input_index"), 0);

    let txid = get_str(&accepted, "txid");
    assert!(
        !txid.is_empty(),
        "Expected txid in offer accept output: {accepted}"
    );

    let mut found_tx = false;
    for _ in 0..10 {
        run_zinc(
            &["--esplora-url", REGTEST_ESPLORA_URL, "sync", "chain"],
            &data_dir,
            TEST_PASSWORD,
        );
        let tx_list = run_zinc(&["tx", "list", "--limit", "100"], &data_dir, TEST_PASSWORD);
        if tx_list_contains(&tx_list, &txid) {
            found_tx = true;
            break;
        }
        thread::sleep(Duration::from_secs(2));
    }
    assert!(
        found_tx,
        "Accepted offer txid {} was not found in tx list after retries",
        txid
    );
}
