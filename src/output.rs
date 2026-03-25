use serde::Serialize;
use serde_json::Value;
use zinc_core::{Account, TxItem};

#[derive(Serialize)]
pub struct BtcBalance {
    pub immature: u64,
    pub trusted_pending: u64,
    pub untrusted_pending: u64,
    pub confirmed: u64,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum CommandOutput {
    WalletInit {
        profile: Option<String>,
        version: u32,
        network: String,
        scheme: String,
        account_index: u32,
        esplora_url: String,
        ord_url: String,
        bitcoin_cli: String,
        bitcoin_cli_args: String,
        phrase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        words: Option<usize>,
    },
    WalletImport {
        profile: Option<String>,
        network: String,
        scheme: String,
        account_index: u32,
        imported: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        phrase: Option<String>,
    },
    WalletInfo {
        profile: Option<String>,
        version: u32,
        network: String,
        scheme: String,
        account_index: u32,
        esplora_url: String,
        ord_url: String,
        bitcoin_cli: String,
        bitcoin_cli_args: String,
        has_persistence: bool,
        has_inscriptions: bool,
        updated_at_unix: u64,
    },
    RevealMnemonic {
        phrase: String,
        words: usize,
    },
    Address {
        #[serde(rename = "type")]
        kind: String,
        address: String,
    },
    Balance {
        total: BtcBalance,
        spendable: BtcBalance,
        inscribed_sats: u64,
    },
    AccountList {
        accounts: Vec<Account>,
    },
    AccountUse {
        previous_account_index: u32,
        account_index: u32,
        taproot_address: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        payment_address: Option<String>,
    },
    TxList {
        transactions: Vec<TxItem>,
    },
    PsbtCreate {
        psbt: String,
    },
    PsbtAnalyze {
        analysis: Value,
        safe_to_send: bool,
        inscription_risk: String,
        policy_reasons: Vec<String>,
        policy: Value,
    },
    PsbtSign {
        psbt: String,
        safe_to_send: bool,
        inscription_risk: String,
        policy_reasons: Vec<String>,
        analysis: Value,
    },
    PsbtBroadcast {
        txid: String,
        safe_to_send: bool,
        inscription_risk: String,
        policy_reasons: Vec<String>,
        analysis: Value,
    },
    SyncChain {
        events: Vec<String>,
    },
    SyncOrdinals {
        inscriptions: usize,
    },
    WaitTxConfirmed {
        txid: String,
        confirmation_time: Option<u64>,
        confirmed: bool,
        waited_secs: u64,
    },
    WaitBalance {
        confirmed: u64,
        confirmed_balance: u64,
        target: u64,
        waited_secs: u64,
    },
    SnapshotSave {
        snapshot: String,
    },
    SnapshotRestore {
        restored: String,
    },
    SnapshotList {
        snapshots: Vec<String>,
    },
    ConfigShow {
        config: Value,
    },
    ConfigSet {
        key: String,
        value: String,
        saved: bool,
    },
    ConfigUnset {
        key: String,
        was_set: bool,
        saved: bool,
    },
    LockInfo {
        profile: Option<String>,
        lock_path: String,
        locked: bool,
        owner_pid: Option<u32>,
        created_at_unix: Option<u64>,
        age_secs: Option<u64>,
    },
    LockClear {
        profile: Option<String>,
        lock_path: String,
        cleared: bool,
    },
    Doctor {
        healthy: bool,
        esplora_url: String,
        esplora_reachable: bool,
        ord_url: String,
        ord_reachable: bool,
        ord_indexing_height: Option<u64>,
        ord_error: Option<String>,
    },
    InscriptionList {
        inscriptions: Vec<zinc_core::ordinals::Inscription>,
        display_items: Option<Vec<InscriptionItemDisplay>>, // Present if thumb mode is enabled, otherwise use table
        thumb_mode_enabled: bool,
    },
    OfferCreate {
        inscription: String,
        ask_sats: u64,
        fee_rate_sat_vb: u64,
        seller_address: String,
        seller_outpoint: String,
        seller_pubkey_hex: String,
        expires_at_unix: i64,
        thumbnail_lines: Option<Vec<String>>,
        hide_inscription_ids: bool,
        raw_response: serde_json::Value,
    },
    OfferPublish {
        event_id: String,
        accepted_relays: u64,
        total_relays: u64,
        publish_results: Vec<serde_json::Value>,
        raw_response: serde_json::Value,
    },
    OfferDiscover {
        event_count: u64,
        offer_count: u64,
        offers: Vec<serde_json::Value>,
        thumbnail_lines: Option<Vec<String>>,
        hide_inscription_ids: bool,
        raw_response: serde_json::Value,
    },
    OfferSubmitOrd {
        ord_url: String,
        submitted: bool,
        raw_response: serde_json::Value,
    },
    OfferListOrd {
        ord_url: String,
        count: u64,
        offers: Vec<serde_json::Value>,
        raw_response: serde_json::Value,
    },
    OfferAccept {
        inscription: String,
        ask_sats: u64,
        txid: String,
        dry_run: bool,
        inscription_risk: String,
        thumbnail_lines: Option<Vec<String>>,
        hide_inscription_ids: bool,
        raw_response: serde_json::Value,
    },
    Setup {
        config_saved: bool,
        wizard_used: bool,
        profile: Option<String>,
        data_dir: String,
        password_env: String,
        default_network: String,
        default_scheme: String,
        default_esplora_url: String,
        default_ord_url: String,
        quiet_default: bool,
        wallet_requested: bool,
        wallet_initialized: bool,
        wallet_mode: Option<String>,
        wallet_phrase: Option<String>,
        wallet_word_count: Option<usize>,
    },
    ScenarioMine {
        blocks: u64,
        address: String,
        raw_output: String,
    },
    ScenarioFund {
        address: String,
        amount_btc: String,
        txid: String,
        mine_blocks: u64,
        mine_address: String,
        generated_blocks: String,
    },
    ScenarioReset {
        removed: Vec<String>,
    },
    Generic(Value),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InscriptionItemDisplay {
    pub number: i64,
    pub id: String,
    pub value_sats: String,
    pub content_type: String,
    pub badge_lines: Vec<String>,
}

pub trait Presenter {
    fn render(&self, output: &CommandOutput) -> String;
}

pub struct AgentPresenter;

impl AgentPresenter {
    pub fn new() -> Self {
        Self
    }
}

impl Presenter for AgentPresenter {
    fn render(&self, output: &CommandOutput) -> String {
        match output {
            CommandOutput::ConfigShow { config } => {
                serde_json::to_string_pretty(config).unwrap_or_default()
            }
            CommandOutput::Setup {
                config_saved,
                wizard_used,
                profile,
                data_dir,
                password_env,
                default_network,
                default_scheme,
                default_esplora_url,
                default_ord_url,
                quiet_default,
                wallet_requested,
                wallet_initialized,
                wallet_mode,
                wallet_phrase,
                wallet_word_count,
            } => {
                let wallet_info = if *wallet_requested || *wallet_initialized {
                    serde_json::json!({
                        "wallet_initialized": wallet_initialized,
                        "mode": wallet_mode,
                        "phrase": wallet_phrase,
                        "word_count": wallet_word_count,
                    })
                } else {
                    serde_json::json!({
                        "requested": false,
                        "initialized": false,
                    })
                };

                let base = serde_json::json!({
                    "config_saved": config_saved,
                    "wizard_used": wizard_used,
                    "profile": profile,
                    "data_dir": data_dir,
                    "password_env": password_env,
                    "quiet_default": quiet_default,
                    "defaults": {
                        "network": default_network,
                        "scheme": default_scheme,
                        "esplora_url": default_esplora_url,
                        "ord_url": default_ord_url,
                    },
                    "wallet": wallet_info
                });
                serde_json::to_string_pretty(&base).unwrap_or_default()
            }
            _ => serde_json::to_string_pretty(output).unwrap_or_default(),
        }
    }
}

pub struct HumanPresenter {
    #[allow(dead_code)]
    pub use_color: bool,
    #[allow(dead_code)]
    pub thumb_mode: crate::cli::ThumbMode,
}

impl HumanPresenter {
    pub fn new(use_color: bool, thumb_mode: crate::cli::ThumbMode) -> Self {
        Self {
            use_color,
            thumb_mode,
        }
    }

    fn print_doctor(&self, output: &CommandOutput) -> String {
        if let CommandOutput::Doctor { healthy, esplora_url, esplora_reachable, ord_url, ord_reachable, ord_indexing_height, ord_error } = output {
            use console::style;
            let mut out = String::new();
            let status = if *healthy { style("Healthy").green() } else { style("Unhealthy").red() };
            out.push_str(&format!("{} {}\n\n", style("Status:").bold(), status));
            
            out.push_str(&format!("{} {}\n", style("Esplora RPC:").bold(), esplora_url));
            out.push_str(&format!("  Reachable: {}\n", if *esplora_reachable { style("Yes").green() } else { style("No").red() }));
            
            out.push_str(&format!("{} {}\n", style("Ord RPC:").bold(), ord_url));
            out.push_str(&format!("  Reachable: {}\n", if *ord_reachable { style("Yes").green() } else { style("No").red() }));
            if let Some(h) = ord_indexing_height {
                out.push_str(&format!("  Height:    {}\n", h));
            }
            if let Some(e) = ord_error {
                out.push_str(&format!("  Error:     {}\n", style(e).red()));
            }
            out
        } else {
            String::new() // Should not happen
        }
    }

    fn print_inscription_list(&self, inscriptions: &[zinc_core::ordinals::Inscription], display_items: &Option<Vec<InscriptionItemDisplay>>, thumb_mode_enabled: bool) -> String {
        let mut out = String::new();
        if let Some(items) = display_items {
            for (idx, item) in items.iter().enumerate() {
                out.push_str(&format!(
                    "[{}] #{} {} sats {}\n",
                    idx + 1,
                    item.number,
                    item.value_sats,
                    item.content_type
                ));
                for line in &item.badge_lines {
                    out.push_str(&format!("{line}\n"));
                }
                out.push_str("\n");
            }
            if inscriptions.len() > items.len() {
                out.push_str(&format!("... and {} more inscriptions\n", inscriptions.len() - items.len()));
            }
        } else if !thumb_mode_enabled {
            let table = crate::presenter::inscription::format_inscriptions(inscriptions);
            out.push_str(&format!("{table}\n"));
        }
        out
    }

    fn print_offer_create(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferCreate { inscription, ask_sats, fee_rate_sat_vb, seller_address, seller_outpoint, seller_pubkey_hex, expires_at_unix, thumbnail_lines, hide_inscription_ids, .. } = output {
            let mut out = String::new();
            if let Some(lines) = thumbnail_lines {
                for line in lines {
                    out.push_str(&format!("{line}\n"));
                }
            }
            let mut lines = vec!["OFFER CREATE".to_string()];
            if *hide_inscription_ids {
                lines.push("inscription: [thumbnail shown above]".to_string());
            } else {
                lines.push(format!("inscription: {}", crate::commands::offer::abbreviate(inscription, 12, 8)));
            }
            lines.push(format!("ask: {} sats @ {} sat/vB", ask_sats, fee_rate_sat_vb));
            lines.push(format!("seller input: {}", crate::commands::offer::abbreviate(seller_address, 12, 8)));
            lines.push(format!("outpoint: {}", crate::commands::offer::abbreviate(seller_outpoint, 16, 6)));
            
            lines.push(format!(
                "seller pubkey: {}",
                crate::commands::offer::abbreviate(seller_pubkey_hex, 10, 6)
            ));
            lines.push(format!("expires_at: {expires_at_unix}"));
            out.push_str(&format!("{}\n", lines.join("\n")));
            out
        } else {
            String::new()
        }
    }

    fn print_offer_publish(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferPublish { event_id, accepted_relays, total_relays, publish_results, .. } = output {
            let mut out = String::new();
            let mut lines = vec![
                "OFFER PUBLISH".to_string(),
                format!("event: {}", crate::commands::offer::abbreviate(event_id, 12, 8)),
                format!("accepted relays: {accepted_relays}/{total_relays}"),
            ];
            for result in publish_results.iter().take(3) {
                let relay = result.get("relay_url").and_then(serde_json::Value::as_str).unwrap_or("-");
                let accepted = result.get("accepted").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let status = if accepted { "ok" } else { "reject" };
                lines.push(format!("{status}: {relay}"));
            }
            if publish_results.len() > 3 {
                lines.push(format!("... and {} more relays", publish_results.len() - 3));
            }
            out.push_str(&format!("{}\n", lines.join("\n")));
            out
        } else {
            String::new()
        }
    }

    fn print_offer_discover(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferDiscover { event_count, offer_count, offers, thumbnail_lines, hide_inscription_ids, .. } = output {
            let mut out = String::new();
            if let Some(lines) = thumbnail_lines {
                for line in lines {
                    out.push_str(&format!("{line}\n"));
                }
            }
            let mut lines = vec![
                "OFFER DISCOVER".to_string(),
                format!("decoded offers: {offer_count} (events: {event_count})"),
            ];
            for (idx, entry) in offers.iter().take(8).enumerate() {
                let event_id = entry.get("event_id").and_then(serde_json::Value::as_str).unwrap_or("-");
                let offer = entry.get("offer").and_then(serde_json::Value::as_object);
                let inscription = offer.and_then(|o| o.get("inscription_id")).and_then(serde_json::Value::as_str).unwrap_or("-");
                let ask_sats = offer.and_then(|o| o.get("ask_sats")).and_then(serde_json::Value::as_u64).unwrap_or(0);
                let seller = offer.and_then(|o| o.get("seller_pubkey_hex")).and_then(serde_json::Value::as_str).unwrap_or("-");

                if *hide_inscription_ids {
                    lines.push(format!(
                        "{:>2}. ask {} sats | seller {} | evt {}",
                        idx + 1,
                        ask_sats,
                        crate::commands::offer::abbreviate(seller, 10, 6),
                        crate::commands::offer::abbreviate(event_id, 10, 6)
                    ));
                } else {
                    lines.push(format!(
                        "{:>2}. {} | {} sats | seller {} | evt {}",
                        idx + 1,
                        crate::commands::offer::abbreviate(inscription, 12, 8),
                        ask_sats,
                        crate::commands::offer::abbreviate(seller, 10, 6),
                        crate::commands::offer::abbreviate(event_id, 10, 6)
                    ));
                }
            }
            if offers.len() > 8 {
                lines.push(format!("... and {} more offers", offers.len() - 8));
            }
            out.push_str(&format!("{}\n", lines.join("\n")));
            out
        } else {
            String::new()
        }
    }

    fn print_offer_submit_ord(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferSubmitOrd { ord_url, .. } = output {
            format!("OFFER SUBMIT-ORD\nsubmitted: true\nord endpoint: {ord_url}\n")
        } else {
            String::new()
        }
    }

    fn print_offer_list_ord(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferListOrd { ord_url, count, offers, .. } = output {
            let mut out = String::new();
            let mut lines = vec![
                "OFFER LIST-ORD".to_string(),
                format!("count: {count}"),
                format!("ord endpoint: {ord_url}"),
            ];
            for (idx, psbt) in offers.iter().take(3).enumerate() {
                if let Some(psbt_str) = psbt.as_str() {
                    lines.push(format!("{:>2}. {}", idx + 1, crate::commands::offer::abbreviate(psbt_str, 14, 8)));
                }
            }
            out.push_str(&format!("{}\n", lines.join("\n")));
            out
        } else {
            String::new()
        }
    }

    fn print_offer_accept(&self, output: &CommandOutput) -> String {
        if let CommandOutput::OfferAccept { inscription, ask_sats, txid, dry_run, inscription_risk, thumbnail_lines, hide_inscription_ids, .. } = output {
            let mut out = String::new();
            if let Some(lines) = thumbnail_lines {
                for line in lines {
                    out.push_str(&format!("{line}\n"));
                }
            }
            let mut lines = vec!["OFFER ACCEPT".to_string()];
            if *hide_inscription_ids {
                lines.push("inscription: [thumbnail shown above]".to_string());
            } else {
                lines.push(format!("inscription: {}", crate::commands::offer::abbreviate(inscription, 12, 8)));
            }
            lines.push(format!("ask: {ask_sats} sats"));
            lines.push(format!("mode: {}", if *dry_run { "dry-run" } else { "broadcast" }));
            lines.push(format!("inscription risk: {inscription_risk}"));
            if txid != "-" {
                lines.push(format!("txid: {}", crate::commands::offer::abbreviate(txid, 12, 8)));
            }
            out.push_str(&format!("{}\n", lines.join("\n")));
            out
        } else {
            String::new()
        }
    }
}

impl Presenter for HumanPresenter {
    fn render(&self, output: &CommandOutput) -> String {
        use console::style;
        match output {
            CommandOutput::WalletInfo { profile, network, scheme, account_index, esplora_url, ord_url, has_persistence, has_inscriptions, updated_at_unix, .. } => {
                let mut out = String::new();
                out.push_str(&format!("  {:<12} {}\n", style("Profile").dim(), profile.as_deref().unwrap_or("default")));
                out.push_str(&format!("  {:<12} {}\n", style("Network").dim(), network));
                out.push_str(&format!("  {:<12} {}\n", style("Scheme").dim(), scheme));
                out.push_str(&format!("  {:<12} {}\n", style("Account").dim(), account_index));
                out.push_str(&format!("  {:<12} {}\n", style("Esplora").dim(), esplora_url));
                out.push_str(&format!("  {:<12} {}\n", style("Ord").dim(), ord_url));
                
                let check = style("✓").green();
                let dash = style("-").dim();
                out.push_str(&format!("  {:<12} {}\n", style("Storage").dim(), if *has_persistence { &check } else { &dash }));
                out.push_str(&format!("  {:<12} {}\n", style("Inscriptions").dim(), if *has_inscriptions { &check } else { &dash }));
                let time_str = {
                    let d = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*updated_at_unix);
                    let datetime: chrono::DateTime<chrono::Utc> = d.into();
                    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                };
                out.push_str(&format!("  {:<12} {}\n", style("Updated").dim(), time_str));
                out
            }
            CommandOutput::WalletInit { profile, network, phrase, words, .. } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "Wallet initialized");
                out.push_str(&format!("  {:<12} {}\n", style("Profile").dim(), profile.as_deref().unwrap_or("default")));
                out.push_str(&format!("  {:<12} {}\n", style("Network").dim(), network));
                out.push_str(&format!("  {:<12} {}\n", style("Phrase").dim(), if phrase.contains("<hidden") { style(phrase).dim() } else { style(phrase) }));
                if let Some(w) = words {
                    out.push_str(&format!("  {:<12} {}\n", style("Words").dim(), w));
                }
                out
            }
            CommandOutput::WalletImport { profile, network, phrase, .. } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "Wallet imported");
                out.push_str(&format!("  {:<12} {}\n", style("Profile").dim(), profile.as_deref().unwrap_or("default")));
                out.push_str(&format!("  {:<12} {}\n", style("Network").dim(), network));
                if let Some(p) = phrase {
                    out.push_str(&format!("  {:<12} {}\n", style("Phrase").dim(), if p.contains("<hidden") { style(p).dim() } else { style(p) }));
                }
                out
            }
            CommandOutput::RevealMnemonic { phrase, words } => {
                let mut out = String::new();
                out.push_str(&format!("  {:<12} {}\n", style("Phrase").dim(), phrase));
                out.push_str(&format!("  {:<12} {}\n", style("Words").dim(), words));
                out
            }
            CommandOutput::Address { kind, address } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("Type").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Address").fg(comfy_table::Color::Green),
                ]);
                table.add_row(vec![comfy_table::Cell::new(kind), comfy_table::Cell::new(address)]);
                format!("{table}")
            }
            CommandOutput::Balance { total, spendable, inscribed_sats } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("Type").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Confirmed (sats)").fg(comfy_table::Color::Green),
                    comfy_table::Cell::new("Trusted Pending (sats)").fg(comfy_table::Color::Yellow),
                    comfy_table::Cell::new("Untrusted Pending (sats)").fg(comfy_table::Color::Magenta),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Total (Combined)"),
                    comfy_table::Cell::new(total.confirmed.to_string()),
                    comfy_table::Cell::new(total.trusted_pending.to_string()),
                    comfy_table::Cell::new(total.untrusted_pending.to_string()),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Spendable (Safe)"),
                    comfy_table::Cell::new(spendable.confirmed.to_string()),
                    comfy_table::Cell::new(spendable.trusted_pending.to_string()),
                    comfy_table::Cell::new(spendable.untrusted_pending.to_string()),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Inscribed/Protected"),
                    comfy_table::Cell::new("-"),
                    comfy_table::Cell::new("-"),
                    comfy_table::Cell::new(inscribed_sats.to_string()),
                ]);
                format!("{table}")
            }
            CommandOutput::AccountList { accounts } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("Index").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Label").fg(comfy_table::Color::Green),
                    comfy_table::Cell::new("Taproot Address").fg(comfy_table::Color::Yellow),
                ]);
                for account in accounts {
                    table.add_row(vec![
                        comfy_table::Cell::new(account.index.to_string()),
                        comfy_table::Cell::new(&account.label),
                        comfy_table::Cell::new(&account.taproot_address),
                    ]);
                }
                format!("{table}")
            }
            CommandOutput::AccountUse { account_index, taproot_address, payment_address, .. } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "Switched account");
                out.push_str(&format!("  {:<12} {}\n", style("Index").dim(), account_index));
                out.push_str(&format!("  {:<12} {}\n", style("Taproot").dim(), taproot_address));
                if let Some(payment) = payment_address {
                    out.push_str(&format!("  {:<12} {}\n", style("Payment").dim(), payment));
                }
                out
            }
            CommandOutput::TxList { transactions } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("TxID").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Type").fg(comfy_table::Color::Green),
                    comfy_table::Cell::new("Amount (sats)").fg(comfy_table::Color::Yellow),
                    comfy_table::Cell::new("Fee (sats)").fg(comfy_table::Color::Magenta),
                    comfy_table::Cell::new("Confirm Time (UTC)").fg(comfy_table::Color::Blue),
                    comfy_table::Cell::new("Inscription #s").fg(comfy_table::Color::Red),
                ]);

                for tx in transactions {
                    let time_str = tx
                        .confirmation_time
                        .map(|t| {
                            let d = std::time::UNIX_EPOCH + std::time::Duration::from_secs(t);
                            let datetime: chrono::DateTime<chrono::Utc> = d.into();
                            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                        })
                        .unwrap_or_else(|| "Unconfirmed".to_string());

                    let ins_str = if tx.inscriptions.is_empty() {
                        "-".to_string()
                    } else {
                        tx.inscriptions
                            .iter()
                            .map(|i| format!("#{}", i.number))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    table.add_row(vec![
                        comfy_table::Cell::new(crate::commands::offer::abbreviate(&tx.txid, 6, 6)),
                        comfy_table::Cell::new(&tx.tx_type),
                        comfy_table::Cell::new(tx.amount_sats.to_string()),
                        comfy_table::Cell::new(tx.fee_sats.to_string()),
                        comfy_table::Cell::new(time_str),
                        comfy_table::Cell::new(ins_str),
                    ]);
                }

                format!("{table}")
            }
            CommandOutput::PsbtCreate { psbt } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "PSBT Created");
                out.push_str(&format!("  {:<12} {}\n", style("PSBT").dim(), psbt));
                out
            }
            CommandOutput::PsbtAnalyze { safe_to_send, inscription_risk, policy_reasons, .. } => {
                let mut out = format!("{}  {}\n", style("ℹ").blue().bold(), "PSBT Analysis");
                let risk_style = if *safe_to_send { style(inscription_risk.as_str()).green() } else { style(inscription_risk.as_str()).red() };
                out.push_str(&format!("  {:<12} {}\n", style("Safe").dim(), if *safe_to_send { "yes" } else { "no" }));
                out.push_str(&format!("  {:<12} {}\n", style("Risk").dim(), risk_style));
                if !policy_reasons.is_empty() {
                    out.push_str(&format!("  {:<12}\n", style("Reasons").dim()));
                    for r in policy_reasons {
                        out.push_str(&format!("    - {}\n", r));
                    }
                }
                out
            }
            CommandOutput::PsbtSign { psbt, safe_to_send, inscription_risk, .. } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "PSBT Signed");
                let risk_style = if *safe_to_send { style(inscription_risk.as_str()).green() } else { style(inscription_risk.as_str()).red() };
                out.push_str(&format!("  {:<12} {}\n", style("Safe").dim(), if *safe_to_send { "yes" } else { "no" }));
                out.push_str(&format!("  {:<12} {}\n", style("Risk").dim(), risk_style));
                out.push_str(&format!("  {:<12} {}\n", style("PSBT").dim(), psbt));
                out
            }
            CommandOutput::PsbtBroadcast { txid, safe_to_send, inscription_risk, .. } => {
                let mut out = format!("{}  {}\n", style("✓").green().bold(), "PSBT Broadcasted");
                let risk_style = if *safe_to_send { style(inscription_risk.as_str()).green() } else { style(inscription_risk.as_str()).red() };
                out.push_str(&format!("  {:<12} {}\n", style("Safe").dim(), if *safe_to_send { "yes" } else { "no" }));
                out.push_str(&format!("  {:<12} {}\n", style("Risk").dim(), risk_style));
                out.push_str(&format!("  {:<12} {}\n", style("TxID").dim(), txid));
                out
            }
            CommandOutput::SyncChain { events } => {
                format!("{}  Synced {} events\n", style("✓").green().bold(), events.len())
            }
            CommandOutput::SyncOrdinals { inscriptions } => {
                format!("{}  Synced {} inscriptions\n", style("✓").green().bold(), inscriptions)
            }
            CommandOutput::WaitTxConfirmed { txid, waited_secs, .. } => {
                format!("{}  Tx {} confirmed after {}s\n", style("✓").green().bold(), txid, waited_secs)
            }
            CommandOutput::WaitBalance { confirmed_balance, target, waited_secs, .. } => {
                format!("{}  Target balance {} reached (current: {}) after {}s\n", style("✓").green().bold(), target, confirmed_balance, waited_secs)
            }
            CommandOutput::SnapshotSave { snapshot } => {
                format!("{}  Saved snapshot to {}\n", style("✓").green().bold(), snapshot)
            }
            CommandOutput::SnapshotRestore { restored } => {
                format!("{}  Restored snapshot from {}\n", style("✓").green().bold(), restored)
            }
            CommandOutput::SnapshotList { snapshots } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("Name").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Path").fg(comfy_table::Color::Green),
                ]);

                for path_str in snapshots {
                    let path = std::path::Path::new(path_str);
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path_str);
                    table.add_row(vec![comfy_table::Cell::new(name), comfy_table::Cell::new(path_str)]);
                }
                format!("{table}")
            }
            CommandOutput::ConfigShow { config } => {
                let mut table = comfy_table::Table::new();
                table.set_header(vec![
                    comfy_table::Cell::new("Setting").fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new("Value").fg(comfy_table::Color::Green),
                ]);

                if let Some(obj) = config.as_object() {
                    for (k, v) in obj {
                        let val_str = match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            Value::Null => "-".to_string(),
                            _ => v.to_string(),
                        };
                        table.add_row(vec![comfy_table::Cell::new(k), comfy_table::Cell::new(val_str)]);
                    }
                }
                format!("{table}")
            }
            CommandOutput::ConfigSet { key, value, .. } => {
                format!("{}  Set {} to {}\n", style("✓").green().bold(), key, value)
            }
            CommandOutput::ConfigUnset { key, was_set, .. } => {
                if *was_set {
                    format!("{}  Unset {}\n", style("✓").green().bold(), key)
                } else {
                    format!("{}  {} was not set\n", style("ℹ").blue().bold(), key)
                }
            }
            CommandOutput::LockInfo { lock_path, locked, owner_pid, age_secs, .. } => {
                let mut out = format!("{}  {}\n", style("ℹ").blue().bold(), "Lock Status");
                let status = if *locked { style("Locked").red() } else { style("Unlocked").green() };
                out.push_str(&format!("  {:<12} {}\n", style("Status").dim(), status));
                out.push_str(&format!("  {:<12} {}\n", style("Path").dim(), lock_path));
                if let Some(pid) = owner_pid {
                    out.push_str(&format!("  {:<12} {}\n", style("Owner PID").dim(), pid));
                }
                if let Some(age) = age_secs {
                    out.push_str(&format!("  {:<12} {}s\n", style("Age").dim(), age));
                }
                out
            }
            CommandOutput::LockClear { lock_path, cleared, .. } => {
                if *cleared {
                    format!("{}  Cleared lock at {}\n", style("✓").green().bold(), lock_path)
                } else {
                    format!("{}  No lock to clear at {}\n", style("ℹ").blue().bold(), lock_path)
                }
            }
            CommandOutput::Doctor { .. } => self.print_doctor(output),
            CommandOutput::Setup { config_saved, wizard_used, profile, default_network, default_scheme, wallet_initialized, wallet_mode, wallet_phrase, .. } => {
                let mut out = String::new();
                out.push_str(&format!("{}  Setup complete\n", style("✓").green().bold()));
                out.push_str(&format!("  {:<15} {}\n", style("Config Saved:").dim(), config_saved));
                out.push_str(&format!("  {:<15} {}\n", style("Wizard Used:").dim(), wizard_used));
                out.push_str(&format!("  {:<15} {}\n", style("Profile:").dim(), profile.as_deref().unwrap_or("default")));
                out.push_str(&format!("  {:<15} {}\n", style("Network:").dim(), default_network));
                out.push_str(&format!("  {:<15} {}\n", style("Scheme:").dim(), default_scheme));
                if *wallet_initialized {
                    out.push_str(&format!("  {:<15} {}\n", style("Wallet:").dim(), "Initialized"));
                    if let Some(mode) = wallet_mode {
                        out.push_str(&format!("  {:<15} {}\n", style("Mode:").dim(), mode));
                    }
                    if let Some(phrase) = wallet_phrase {
                        if phrase != "<hidden; use --reveal to show>" {
                            out.push_str(&format!("\n{}\n{}\n", style("Mnemonic Phrase (keep this safe!):").red().bold(), phrase));
                        } else {
                            out.push_str(&format!("  {:<15} {}\n", style("Phrase:").dim(), phrase));
                        }
                    }
                }
                out
            }
            CommandOutput::ScenarioMine { blocks, address, raw_output } => {
                format!("{}  Mined {} blocks to {}\nOutput:\n{}\n", style("✓").green().bold(), blocks, address, raw_output)
            }
            CommandOutput::ScenarioFund { address, amount_btc, txid, mine_blocks, mine_address, generated_blocks } => {
                format!("{}  Funded {} with {} BTC\nTxID: {}\n{}  Mined {} blocks to {}\nOutput:\n{}\n", style("✓").green().bold(), address, amount_btc, txid, style("✓").green().bold(), mine_blocks, mine_address, generated_blocks)
            }
            CommandOutput::ScenarioReset { removed } => {
                let mut out = format!("{}  Scenario Reset\n", style("✓").green().bold());
                for path in removed {
                    out.push_str(&format!("  Removed: {}\n", path));
                }
                out
            }
            CommandOutput::InscriptionList { inscriptions, display_items, thumb_mode_enabled } => self.print_inscription_list(inscriptions, display_items, *thumb_mode_enabled),
            CommandOutput::OfferCreate { .. } => self.print_offer_create(output),
            CommandOutput::OfferPublish { .. } => self.print_offer_publish(output),
            CommandOutput::OfferDiscover { .. } => self.print_offer_discover(output),
            CommandOutput::OfferSubmitOrd { .. } => self.print_offer_submit_ord(output),
            CommandOutput::OfferListOrd { .. } => self.print_offer_list_ord(output),
            CommandOutput::OfferAccept { .. } => self.print_offer_accept(output),
            CommandOutput::Generic(val) => {
                // Fallback for human mode when a command hasn't been fully refactored yet
                serde_json::to_string_pretty(val).unwrap_or_default()
            }
        }
    }
}
