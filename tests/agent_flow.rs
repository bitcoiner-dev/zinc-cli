use serde_json::Value;
use std::fs;
use std::process::Command;

fn unique_data_dir(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("/tmp/{}_{}_{}", prefix, std::process::id(), nanos)
}

fn cargo_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    let isolated_home = unique_data_dir("zinc_cli_agent_home");
    let _ = fs::create_dir_all(&isolated_home);
    cmd.env("HOME", isolated_home);
    for key in [
        "ZINC_CLI_PROFILE",
        "ZINC_CLI_DATA_DIR",
        "ZINC_CLI_PASSWORD_ENV",
        "ZINC_CLI_JSON",
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
    cmd.args(&["run", "--quiet", "--"])
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

    parse_json_from_output(&stdout)
}

fn run_zinc_ignore_error(args: &[&str], data_dir: &str, password: &str) -> Value {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--"])
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

    parse_json_from_output(&stdout)
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

fn get_str(json: &Value, key: &str) -> String {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {} in JSON: {}", key, json))
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn get_u64(json: &Value, key: &str) -> u64 {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {}", key))
        .as_u64()
        .unwrap_or_else(|| panic!("Expected u64 for {}", key))
}

fn get_bool(json: &Value, key: &str) -> bool {
    json.get(key)
        .unwrap_or_else(|| panic!("Missing key: {}", key))
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool for {}", key))
}

fn assert_ok(json: &Value) {
    if !get_bool(json, "ok") {
        panic!("Expected ok=true but got: {:?}", json);
    }
}

fn shorten_address(addr: &str) -> String {
    if addr.len() <= 10 {
        addr.to_string()
    } else {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    }
}

fn total_balance_sats(balance_json: &Value) -> u64 {
    let total = balance_json
        .get("total")
        .unwrap_or_else(|| panic!("Missing total in balance JSON: {balance_json}"));
    [
        "immature",
        "trusted_pending",
        "untrusted_pending",
        "confirmed",
    ]
    .iter()
    .map(|k| {
        total
            .get(*k)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("Missing/invalid total.{k} in balance JSON: {balance_json}"))
    })
    .sum()
}

fn spendable_balance_sats(balance_json: &Value) -> u64 {
    let spendable = balance_json
        .get("spendable")
        .unwrap_or_else(|| panic!("Missing spendable in balance JSON: {balance_json}"));
    [
        "immature",
        "trusted_pending",
        "untrusted_pending",
        "confirmed",
    ]
    .iter()
    .map(|k| {
        spendable
            .get(*k)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("Missing/invalid spendable.{k} in balance JSON: {balance_json}")
            })
    })
    .sum()
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

const TEST_MNEMONIC: &str = "rotate text off rich waste jump grab doctor today renew fault exotic";
const TEST_PASSWORD: &str = "test_password_123";

#[test]
fn test_agent_wallet_workflow() {
    if !ensure_live_test_enabled("test_agent_wallet_workflow") {
        return;
    }

    let data_dir = unique_data_dir("zinc_agent_wallet");
    let _ = fs::remove_dir_all(&data_dir);
    const TRANSFER_SATS: u64 = 1_000;
    const FEE_BUFFER_SATS: u64 = 500;

    println!("[1] Import wallet with seed phrase (dual scheme)");
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
    assert_ok(&import_result);
    assert_eq!(get_str(&import_result, "network"), "regtest");
    assert_eq!(get_str(&import_result, "scheme"), "dual");
    println!(
        "  ✓ Network: {}, Scheme: {}",
        get_str(&import_result, "network"),
        get_str(&import_result, "scheme")
    );

    println!("[2] Get wallet info");
    let info = run_zinc(&["wallet", "info"], &data_dir, TEST_PASSWORD);
    assert_ok(&info);
    assert_eq!(get_str(&info, "network"), "regtest");
    assert_eq!(get_str(&info, "scheme"), "dual");
    println!(
        "  ✓ Profile: {}, Network: {}, Scheme: {}",
        get_str(&info, "profile"),
        get_str(&info, "network"),
        get_str(&info, "scheme")
    );

    println!("[3] Sync chain");
    let sync_chain = run_zinc(&["sync", "chain"], &data_dir, TEST_PASSWORD);
    assert_ok(&sync_chain);
    println!("  ✓ Chain synced");

    println!("[4] Sync ordinals");
    let sync_ordinals = run_zinc(&["sync", "ordinals"], &data_dir, TEST_PASSWORD);
    assert_ok(&sync_ordinals);
    let inscription_count = get_u64(&sync_ordinals, "inscriptions");
    println!("  ✓ Ordinals synced: {inscription_count} inscriptions");
    assert!(
        inscription_count >= 3,
        "Expected at least 3 inscriptions in seeded test wallet, got {inscription_count}"
    );

    println!("[5] List inscriptions");
    let inscriptions = run_zinc(&["inscription", "list"], &data_dir, TEST_PASSWORD);
    assert_ok(&inscriptions);
    let listed_inscriptions = inscriptions
        .get("inscriptions")
        .and_then(Value::as_array)
        .map_or(0, |a| a.len());
    assert!(
        listed_inscriptions >= 3,
        "Expected at least 3 inscriptions in list, got {listed_inscriptions}"
    );
    println!("  ✓ Inscriptions listed: {listed_inscriptions}");

    println!("[6] Get account 0 taproot/payment addresses");
    let taproot = run_zinc(&["address", "taproot"], &data_dir, TEST_PASSWORD);
    assert_ok(&taproot);
    assert_eq!(get_str(&taproot, "type"), "taproot");
    let taproot_addr = get_str(&taproot, "address");
    assert!(taproot_addr.starts_with("bcrt1p"));

    let payment = run_zinc(&["address", "payment"], &data_dir, TEST_PASSWORD);
    assert_ok(&payment);
    assert_eq!(get_str(&payment, "type"), "payment");
    let payment_addr = get_str(&payment, "address");
    assert!(payment_addr.starts_with("bcrt1q"));
    assert_ne!(taproot_addr, payment_addr);
    println!(
        "  ✓ A0 taproot={} payment={}",
        shorten_address(&taproot_addr),
        shorten_address(&payment_addr)
    );

    println!("[7] Get taproot address at index 3");
    let taproot3 = run_zinc(
        &["address", "taproot", "--index", "3"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&taproot3);
    let taproot_addr3 = get_str(&taproot3, "address");
    println!("  ✓ Taproot[3]: {}", shorten_address(&taproot_addr3));

    println!("[8] Get account 0 balance");
    let balance_0_before = run_zinc(&["balance"], &data_dir, TEST_PASSWORD);
    assert_ok(&balance_0_before);
    let account0_total_before = total_balance_sats(&balance_0_before);
    let account0_spendable_before = spendable_balance_sats(&balance_0_before);
    println!(
        "  ✓ Account 0 total sats before transfer: {account0_total_before} (spendable: {account0_spendable_before})"
    );

    println!("[9] Account list");
    let accounts = run_zinc(&["account", "list"], &data_dir, TEST_PASSWORD);
    assert_ok(&accounts);
    let account_entries = accounts
        .get("accounts")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("Missing accounts array: {accounts}"));
    assert!(
        account_entries.len() >= 2,
        "Expected at least 2 accounts, got {}",
        account_entries.len()
    );
    let acct0 = &account_entries[0];
    let acct0_taproot = acct0
        .get("taprootAddress")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Missing taprootAddress on account 0: {acct0}"));
    let acct0_payment = acct0
        .get("paymentAddress")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Missing paymentAddress on account 0 (dual expected): {acct0}"));
    println!(
        "  ✓ Account 0 taproot={} payment={}",
        shorten_address(acct0_taproot),
        shorten_address(acct0_payment)
    );

    println!("[10] Switch to account 1");
    let use_account = run_zinc(
        &["account", "use", "--index", "1"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&use_account);
    assert_eq!(get_u64(&use_account, "account_index"), 1);
    let account1_taproot = get_str(&use_account, "taproot_address");
    let account1_payment = get_str(&use_account, "payment_address");
    assert!(account1_taproot.starts_with("bcrt1p"));
    assert!(account1_payment.starts_with("bcrt1q"));
    assert_ne!(account1_taproot, account1_payment);
    println!(
        "  ✓ Switched to account 1 taproot={} payment={}",
        shorten_address(&account1_taproot),
        shorten_address(&account1_payment)
    );

    println!("[11] Verify new taproot after account switch");
    let new_taproot = run_zinc(&["address", "taproot"], &data_dir, TEST_PASSWORD);
    let new_addr = get_str(&new_taproot, "address");
    assert_ne!(
        taproot_addr, new_addr,
        "Address should change after account switch"
    );
    println!(
        "  ✓ Address changed: {} -> {}",
        shorten_address(&taproot_addr),
        shorten_address(&new_addr)
    );

    let balance_1_before = run_zinc(&["balance"], &data_dir, TEST_PASSWORD);
    assert_ok(&balance_1_before);
    let account1_total_before = total_balance_sats(&balance_1_before);
    println!("  ✓ Account 1 total sats before transfer: {account1_total_before}");

    println!("[12] Switch back to account 0");
    let use0 = run_zinc(
        &["account", "use", "--index", "0"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&use0);
    println!("  ✓ Switched back to account 0");

    let min_required = TRANSFER_SATS + FEE_BUFFER_SATS;
    if account0_spendable_before < min_required {
        println!(
            "[13] Skipping BTC transfer: account 0 spendable {} < required minimum {} ({} transfer + {} fee buffer)",
            account0_spendable_before, min_required, TRANSFER_SATS, FEE_BUFFER_SATS
        );
        println!("\n✅ Wallet workflow test passed (transfer skipped due low balance)!");
        return;
    }

    println!(
        "[13] Create PSBT to transfer {} sats from account 0 -> account 1 payment",
        TRANSFER_SATS
    );
    let create = run_zinc(
        &[
            "psbt",
            "create",
            "--to",
            &account1_payment,
            "--amount-sats",
            "1000",
            "--fee-rate",
            "1",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&create);
    let unsigned_psbt = get_str(&create, "psbt");
    assert!(!unsigned_psbt.is_empty());
    println!("  ✓ PSBT created");

    println!("[14] Analyze PSBT");
    let analyze = run_zinc(
        &["psbt", "analyze", "--psbt", &unsigned_psbt],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&analyze);
    let analysis = analyze
        .get("analysis")
        .unwrap_or_else(|| panic!("Missing analysis field: {analyze}"));
    assert!(
        analysis.get("fee_sats").is_some(),
        "Expected fee_sats in analysis: {analysis}"
    );
    if let Some(inscriptions_burned) = analysis.get("inscriptions_burned").and_then(Value::as_bool)
    {
        assert!(
            !inscriptions_burned,
            "Expected no inscription burns for pure BTC transfer in dual mode"
        );
    }
    println!("  ✓ PSBT analyzed");

    println!("[15] Sign + finalize PSBT");
    let sign = run_zinc(
        &["psbt", "sign", "--psbt", &unsigned_psbt, "--finalize"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&sign);
    let signed_psbt = get_str(&sign, "psbt");
    assert!(!signed_psbt.is_empty());
    println!("  ✓ PSBT signed");

    println!("[16] Broadcast transaction");
    let broadcast = run_zinc(
        &["psbt", "broadcast", "--psbt", &signed_psbt],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&broadcast);
    let transfer_txid = get_str(&broadcast, "txid");
    assert!(!transfer_txid.is_empty());
    println!("  ✓ Broadcast txid={}", shorten_address(&transfer_txid));

    println!("[17] Verify account 0 sees transfer tx");
    let sync0_after = run_zinc(&["sync", "chain"], &data_dir, TEST_PASSWORD);
    assert_ok(&sync0_after);
    let tx_list_0 = run_zinc(&["tx", "list", "--limit", "100"], &data_dir, TEST_PASSWORD);
    assert_ok(&tx_list_0);
    assert!(
        tx_list_contains(&tx_list_0, &transfer_txid),
        "Account 0 tx list did not include transfer txid {}",
        transfer_txid
    );
    let balance_0_after = run_zinc(&["balance"], &data_dir, TEST_PASSWORD);
    assert_ok(&balance_0_after);
    let account0_total_after = total_balance_sats(&balance_0_after);
    println!(
        "  ✓ Account 0 total sats after transfer: {} (before {})",
        account0_total_after, account0_total_before
    );

    println!("[18] Verify account 1 sees transfer tx + balance increase");
    let use1_again = run_zinc(
        &["account", "use", "--index", "1"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&use1_again);
    let sync1_after = run_zinc(&["sync", "chain"], &data_dir, TEST_PASSWORD);
    assert_ok(&sync1_after);
    let tx_list_1 = run_zinc(&["tx", "list", "--limit", "100"], &data_dir, TEST_PASSWORD);
    assert_ok(&tx_list_1);
    assert!(
        tx_list_contains(&tx_list_1, &transfer_txid),
        "Account 1 tx list did not include transfer txid {}",
        transfer_txid
    );
    let balance_1_after = run_zinc(&["balance"], &data_dir, TEST_PASSWORD);
    assert_ok(&balance_1_after);
    let account1_total_after = total_balance_sats(&balance_1_after);
    assert!(
        account1_total_after >= account1_total_before + TRANSFER_SATS,
        "Expected account 1 total sats to increase by at least {TRANSFER_SATS} (before {account1_total_before}, after {account1_total_after})"
    );
    println!(
        "  ✓ Account 1 total sats after transfer: {} (before {})",
        account1_total_after, account1_total_before
    );

    println!("\n✅ Wallet workflow test passed!");
}

#[test]
fn test_agent_config_workflow() {
    let data_dir = unique_data_dir("zinc_agent_config");
    let _ = fs::remove_dir_all(&data_dir);

    // Import wallet first
    let _ = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "unified",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );

    println!("[1] Config show");
    let config = run_zinc(&["config", "show"], &data_dir, TEST_PASSWORD);
    assert_ok(&config);
    println!("  ✓ Config shown");

    println!("[2] Config set esplora-url");
    let _ = run_zinc(
        &[
            "config",
            "set",
            "esplora-url",
            "https://blockstream.info/api",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    let config2 = run_zinc(&["config", "show"], &data_dir, TEST_PASSWORD);
    assert_ok(&config2);
    println!("  ✓ Esplora URL set");

    println!("[3] Config unset esplora-url");
    let _ = run_zinc(
        &["config", "unset", "esplora-url"],
        &data_dir,
        TEST_PASSWORD,
    );
    println!("  ✓ Esplora URL unset");

    println!("\n✅ Config workflow test passed!");
}

#[test]
fn test_agent_snapshot_workflow() {
    let data_dir = unique_data_dir("zinc_agent_snapshot");
    let _ = fs::remove_dir_all(&data_dir);

    // Import wallet first
    let _ = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "unified",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );

    println!("[1] Snapshot save");
    let save = run_zinc(
        &["snapshot", "save", "--name", "test-snap"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&save);
    let snap_path = get_str(&save, "snapshot");
    println!("  ✓ Saved: {}", snap_path);

    println!("[2] Snapshot list");
    let list = run_zinc(&["snapshot", "list"], &data_dir, TEST_PASSWORD);
    assert_ok(&list);
    let has_snap = list
        .get("snapshots")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .any(|v| v.as_str().unwrap_or("").contains("test-snap"))
        })
        .unwrap_or(false);
    assert!(has_snap);
    println!("  ✓ test-snap found in list");

    println!("[3] Snapshot restore");
    let restore = run_zinc(
        &["snapshot", "restore", "--name", "test-snap"],
        &data_dir,
        TEST_PASSWORD,
    );
    assert_ok(&restore);
    println!("  ✓ Restored");

    println!("\n✅ Snapshot workflow test passed!");
}

#[test]
fn test_agent_misc_commands() {
    if !ensure_live_test_enabled("test_agent_misc_commands") {
        return;
    }

    let data_dir = unique_data_dir("zinc_agent_misc");
    let _ = fs::remove_dir_all(&data_dir);

    // Import wallet first
    let _ = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "unified",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );

    println!("[1] Lock info");
    let lock = run_zinc(&["lock", "info"], &data_dir, TEST_PASSWORD);
    assert_ok(&lock);
    println!("  ✓ Lock info works");

    println!("[2] Doctor");
    let doctor = run_zinc(&["doctor"], &data_dir, TEST_PASSWORD);
    assert_ok(&doctor);
    println!("  ✓ Doctor works");

    println!("\n✅ Misc commands test passed!");
}

#[test]
fn test_psbt_error_handling() {
    let data_dir = unique_data_dir("zinc_psbt_error");
    let _ = fs::remove_dir_all(&data_dir);

    // Import wallet first
    let _ = run_zinc(
        &[
            "wallet",
            "import",
            "--mnemonic",
            TEST_MNEMONIC,
            "--network",
            "regtest",
            "--scheme",
            "unified",
            "--overwrite",
        ],
        &data_dir,
        TEST_PASSWORD,
    );

    // Get a taproot address
    let taproot = run_zinc(&["address", "taproot"], &data_dir, TEST_PASSWORD);
    let dest = get_str(&taproot, "address");

    // Create PSBT without funds - should error gracefully
    let create = run_zinc_ignore_error(
        &[
            "psbt",
            "create",
            "--to",
            &dest,
            "--amount-sats",
            "1000",
            "--fee-rate",
            "1",
        ],
        &data_dir,
        TEST_PASSWORD,
    );
    // Should have ok=false due to insufficient funds
    assert!(
        !get_bool(&create, "ok"),
        "PSBT create without funds should fail"
    );
    assert!(create.get("error").is_some(), "Should have error field");
    println!(
        "  ✓ PSBT create fails gracefully without funds: {:?}",
        create.get("error").map(|e| e.get("type"))
    );

    println!("\n✅ PSBT error handling test passed!");
}

#[test]
fn test_command_list_complete() {
    let mut cmd = cargo_cmd();
    cmd.args(&["run", "--quiet", "--", "--agent", "help"]);
    let output = cmd.output().expect("failed to execute process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = parse_json_from_output(&stdout);

    let commands = json.get("command_list").unwrap().as_array().unwrap();
    let command_strings: Vec<&str> = commands.iter().map(|v| v.as_str().unwrap()).collect();

    let required = vec![
        "wallet init",
        "wallet import",
        "wallet info",
        "address taproot",
        "address payment",
        "balance",
        "sync chain",
        "sync ordinals",
        "account list",
        "account use",
        "psbt create",
        "psbt analyze",
        "psbt sign",
        "psbt broadcast",
        "snapshot save",
        "snapshot list",
        "snapshot restore",
        "lock info",
        "lock clear",
        "doctor",
    ];

    let mut missing = Vec::new();
    for r in &required {
        if !command_strings.contains(r) {
            missing.push(*r);
        }
    }

    assert!(missing.is_empty(), "Missing commands: {:?}", missing);
    println!("  ✓ All {} required commands present", required.len());
    println!("\n✅ Command list test passed!");
}
