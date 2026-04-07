use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[value(rename_all = "lower")]
pub enum PolicyMode {
    #[default]
    Warn,
    Strict,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "zinc-cli",
    version,
    about = "CLI wallet for Zinc Bitcoin + Ordinals"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(
        long,
        global = true,
        help = "Agent mode (machine-readable JSON output)"
    )]
    pub agent: bool,

    #[arg(long, global = true, help = "Automatically say yes to prompts")]
    pub yes: bool,
 
    #[arg(
        long,
        global = true,
        help = "Environment variable containing password (default: ZINC_WALLET_PASSWORD)"
    )]
    pub password_env: Option<String>,

    #[arg(long, global = true, help = "Read password from stdin")]
    pub password_stdin: bool,

    #[arg(long, global = true, help = "Reveal secrets in output")]
    pub reveal: bool,

    #[arg(long, global = true, help = "Custom data directory")]
    pub data_dir: Option<PathBuf>,

    #[arg(long, global = true, help = "Profile name to use")]
    pub profile: Option<String>,

    #[arg(long, global = true, help = "Override network")]
    pub network: Option<String>,

    #[arg(long, global = true, help = "Override address scheme")]
    pub scheme: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Override payment address type (native, nested, legacy)"
    )]
    pub payment_address_type: Option<String>,

    #[arg(long, global = true, help = "Override Esplora API URL")]
    pub esplora_url: Option<String>,

    #[arg(long, global = true, help = "Override Ordinals indexer URL")]
    pub ord_url: Option<String>,
    #[arg(long, global = true, help = "Override Pulse Oracle URL")]
    pub pulse_url: Option<String>,
    #[arg(long, global = true, help = "Override Pulse Oracle API Token")]
    pub pulse_api_token: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Disable graphical images and fallback to ASCII/Halfblocks"
    )]
    pub no_images: bool,
    #[arg(long, global = true, help = "Force ASCII fallback mode")]
    pub ascii: bool,

    #[arg(
        long,
        global = true,
        help = "Show inscription thumbnails (auto-detects best rendering)"
    )]
    pub thumb: bool,

    #[arg(
        long,
        global = true,
        help = "Correlation ID for agent workflows (auto-generated if omitted)"
    )]
    pub correlation_id: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Emit structured JSON logs to stderr with command lifecycle events"
    )]
    pub log_json: bool,

    #[arg(
        long,
        global = true,
        help = "Idempotency key for mutating commands (replays cached success for same command+key)"
    )]
    pub idempotency_key: Option<String>,

    #[arg(
        long,
        global = true,
        default_value_t = 30,
        help = "Network timeout in seconds for remote calls"
    )]
    pub network_timeout_secs: u64,

    #[arg(
        long,
        global = true,
        default_value_t = 0,
        help = "Retry count for transient network failures/timeouts"
    )]
    pub network_retries: u32,

    #[arg(long, global = true, help = "Disable inscription thumbnails")]
    pub no_thumb: bool,

    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = PolicyMode::Warn,
        help = "Policy behavior for potentially risky PSBT operations: warn|strict"
    )]
    pub policy_mode: PolicyMode,

    #[arg(skip)]
    pub explicit_network: bool,

    #[arg(skip)]
    pub started_at_unix_ms: u128,
}

impl Cli {
    pub fn thumb_enabled(&self) -> bool {
        if self.no_thumb || self.no_images {
            false
        } else if self.thumb {
            true
        } else {
            !self.agent
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    Setup(SetupArgs),
    Config(ConfigArgs),
    Wallet(WalletArgs),
    Sync(SyncArgs),
    Address(AddressArgs),
    Balance,
    Tx(TxArgs),
    Psbt(PsbtArgs),
    #[command(hide = true)]
    Intent(IntentArgs),
    #[command(hide = true)]
    Pair(IntentPairArgs),
    #[command(hide = true)]
    Offer(OfferArgs),
    Account(AccountArgs),
    Wait(WaitArgs),
    Snapshot(SnapshotArgs),
    Lock(LockArgs),
    Scenario(ScenarioArgs),
    Inscription(InscriptionArgs),
    Version,
    #[cfg(feature = "ui")]
    Dashboard,
    Doctor,
    Insight(InsightArgs),
    Pulse(PulseArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct SetupArgs {
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long)]
    pub password_env: Option<String>,
    #[arg(long)]
    pub default_network: Option<String>,
    #[arg(long)]
    pub default_scheme: Option<String>,
    #[arg(long)]
    pub default_payment_address_type: Option<String>,
    #[arg(long)]
    pub default_esplora_url: Option<String>,
    #[arg(long)]
    pub default_ord_url: Option<String>,
    #[arg(long)]
    pub default_pulse_url: Option<String>,
    #[arg(long)]
    pub restore_mnemonic: Option<String>,
    #[arg(long)]
    pub words: Option<u8>,
}

#[derive(Parser, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    Show,
    Set { key: String, value: String },
    Unset { key: String },
}

#[derive(Parser, Debug, Clone)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub action: WalletAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum WalletAction {
    Init {
        #[arg(long)]
        words: Option<u8>,
        #[arg(long)]
        network: Option<String>,
        #[arg(long)]
        scheme: Option<String>,
        #[arg(long)]
        payment_address_type: Option<String>,
        #[arg(long)]
        overwrite: bool,
    },
    Import {
        #[arg(long)]
        mnemonic: String,
        #[arg(long)]
        network: Option<String>,
        #[arg(long)]
        scheme: Option<String>,
        #[arg(long)]
        payment_address_type: Option<String>,
        #[arg(long)]
        overwrite: bool,
    },
    Info,
    RevealMnemonic,
}

#[derive(Parser, Debug, Clone)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub target: SyncTarget,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SyncTarget {
    Chain,
    Ordinals,
}

#[derive(Parser, Debug, Clone)]
pub struct AddressArgs {
    #[command(subcommand)]
    pub kind: AddressKind,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AddressKind {
    Taproot {
        #[arg(long)]
        index: Option<u32>,
        #[arg(long)]
        new: bool,
    },
    Payment {
        #[arg(long)]
        index: Option<u32>,
        #[arg(long)]
        new: bool,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct TxArgs {
    #[command(subcommand)]
    pub action: TxAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TxAction {
    List {
        #[arg(long)]
        limit: Option<usize>,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct PsbtArgs {
    #[command(subcommand)]
    pub action: PsbtAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PsbtAction {
    Create {
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount_sats: u64,
        #[arg(long)]
        fee_rate: u64,
        #[arg(long)]
        out_file: Option<PathBuf>,
    },
    Analyze {
        #[arg(long)]
        psbt: Option<String>,
        #[arg(long)]
        psbt_file: Option<PathBuf>,
        #[arg(long)]
        psbt_stdin: bool,
    },
    Sign {
        #[arg(long)]
        psbt: Option<String>,
        #[arg(long)]
        psbt_file: Option<PathBuf>,
        #[arg(long)]
        psbt_stdin: bool,
        #[arg(long)]
        sign_inputs: Option<String>,
        #[arg(long)]
        sighash: Option<u8>,
        #[arg(long)]
        finalize: bool,
        #[arg(long)]
        out_file: Option<PathBuf>,
    },
    Broadcast {
        #[arg(long)]
        psbt: Option<String>,
        #[arg(long)]
        psbt_file: Option<PathBuf>,
        #[arg(long)]
        psbt_stdin: bool,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct OfferArgs {
    #[command(subcommand)]
    pub action: OfferAction,
}

#[derive(Parser, Debug, Clone)]
pub struct IntentArgs {
    #[command(subcommand)]
    pub action: IntentAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum IntentAction {
    Pair(IntentPairArgs),
    Send {
        #[arg(long)]
        pairing_id: String,
        #[arg(long)]
        payload_json: String,
        #[arg(long)]
        now_unix: Option<u64>,
        #[arg(long)]
        expires_in_secs: Option<u64>,
        #[arg(long)]
        nonce: Option<u64>,
        #[arg(long)]
        agent_secret_key_hex: Option<String>,
        #[arg(long = "relay")]
        relay: Vec<String>,
        #[arg(long, default_value = "regtest")]
        network: String,
    },
    WaitReceipt {
        #[arg(long)]
        pairing_id: String,
        #[arg(long)]
        intent_id: String,
        #[arg(long, default_value_t = 30_000)]
        timeout_ms: u64,
        #[arg(long)]
        agent_secret_key_hex: Option<String>,
        #[arg(long = "relay")]
        relay: Vec<String>,
        #[arg(
            long,
            default_value_t = false,
            help = "Allow wait-receipt to proceed when intent action cannot be resolved locally (cross-device recovery)"
        )]
        allow_unknown_intent_action: bool,
    },
    FixtureGenerate {
        #[arg(long)]
        now_unix: Option<u64>,
        #[arg(long)]
        agent_secret_key_hex: Option<String>,
        #[arg(long)]
        wallet_secret_key_hex: Option<String>,
    },
    FixtureVerify {
        #[arg(long)]
        fixture_json: Option<String>,
        #[arg(long)]
        fixture_file: Option<PathBuf>,
        #[arg(long)]
        fixture_stdin: bool,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct IntentPairArgs {
    #[command(subcommand)]
    pub action: IntentPairAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum IntentPairAction {
    Start {
        #[arg(long)]
        now_unix: Option<u64>,
        #[arg(long)]
        expires_in_secs: Option<u64>,
        #[arg(long)]
        agent_secret_key_hex: Option<String>,
        #[arg(long = "relay")]
        relay: Vec<String>,
        #[arg(long, default_value = "regtest")]
        network: String,
        #[arg(long)]
        show_json: bool,
        #[arg(long)]
        no_wait: bool,
    },
    Finish {
        #[arg(long)]
        now_unix: Option<u64>,
        #[arg(long)]
        request_json: Option<String>,
        #[arg(long)]
        request_file: Option<PathBuf>,
        #[arg(long)]
        agent_secret_key_hex: Option<String>,
        #[arg(long)]
        ack_json: Option<String>,
        #[arg(long)]
        ack_file: Option<PathBuf>,
        #[arg(long)]
        ack_code: Option<String>,
    },
    List {},
    Show {
        #[arg(long)]
        pairing_id: String,
    },
    Pause {
        #[arg(long)]
        pairing_id: String,
        #[arg(long)]
        now_unix: Option<u64>,
    },
    Resume {
        #[arg(long)]
        pairing_id: String,
        #[arg(long)]
        now_unix: Option<u64>,
    },
    Revoke {
        #[arg(long)]
        pairing_id: String,
        #[arg(long)]
        now_unix: Option<u64>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum OfferAction {
    Create {
        #[arg(long)]
        inscription: String,
        #[arg(long, alias = "ask-sats")]
        amount: u64,
        #[arg(long)]
        fee_rate: u64,
        #[arg(long, default_value_t = 3600)]
        expires_in_secs: u64,
        #[arg(long)]
        created_at_unix: Option<u64>,
        #[arg(long)]
        nonce: Option<u64>,
        #[arg(long)]
        publisher_pubkey_hex: Option<String>,
        #[arg(long)]
        seller_payout_address: Option<String>,
        #[arg(long)]
        submit_ord: bool,
        #[arg(long)]
        offer_out_file: Option<PathBuf>,
        #[arg(long)]
        psbt_out_file: Option<PathBuf>,
    },
    Publish {
        #[arg(long)]
        offer_json: Option<String>,
        #[arg(long)]
        offer_file: Option<PathBuf>,
        #[arg(long)]
        offer_stdin: bool,
        #[arg(long)]
        secret_key_hex: String,
        #[arg(long)]
        relay: Vec<String>,
        #[arg(long)]
        created_at_unix: Option<u64>,
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
    Discover {
        #[arg(long)]
        relay: Vec<String>,
        #[arg(long, default_value_t = 256)]
        limit: usize,
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
    SubmitOrd {
        #[arg(long)]
        psbt: Option<String>,
        #[arg(long)]
        psbt_file: Option<PathBuf>,
        #[arg(long)]
        psbt_stdin: bool,
    },
    ListOrd,
    Accept {
        #[arg(long)]
        offer_json: Option<String>,
        #[arg(long)]
        offer_file: Option<PathBuf>,
        #[arg(long)]
        offer_stdin: bool,
        #[arg(long)]
        expect_inscription: Option<String>,
        #[arg(long)]
        expect_ask_sats: Option<u64>,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct AccountArgs {
    #[command(subcommand)]
    pub action: AccountAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AccountAction {
    List {
        #[arg(long)]
        count: Option<u32>,
    },
    Use {
        #[arg(long)]
        index: u32,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct WaitArgs {
    #[command(subcommand)]
    pub action: WaitAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum WaitAction {
    TxConfirmed {
        #[arg(long)]
        txid: String,
        #[arg(long, default_value_t = 300)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 5)]
        poll_secs: u64,
    },
    Balance {
        #[arg(long)]
        confirmed_at_least: u64,
        #[arg(long, default_value_t = 300)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 5)]
        poll_secs: u64,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct SnapshotArgs {
    #[command(subcommand)]
    pub action: SnapshotAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SnapshotAction {
    Save {
        #[arg(long)]
        name: String,
        #[arg(long)]
        overwrite: bool,
    },
    Restore {
        #[arg(long)]
        name: String,
    },
    List,
}

#[derive(Parser, Debug, Clone)]
pub struct LockArgs {
    #[command(subcommand)]
    pub action: LockAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum LockAction {
    Info,
    Clear,
}

#[derive(Parser, Debug, Clone)]
pub struct InsightArgs {
    #[command(subcommand)]
    pub action: InsightAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum InsightAction {
    #[command(about = "Appraise all assets in the current wallet")]
    Appraise {
        #[arg(long, help = "Only show assets with a known collection")]
        known_only: bool,
    },
    #[command(about = "Search for a collection by name")]
    Search {
        #[arg(index = 1, help = "Search query")]
        query: String,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct ScenarioArgs {
    #[command(subcommand)]
    pub action: ScenarioAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ScenarioAction {
    Mine {
        #[arg(long, default_value_t = 1)]
        blocks: u32,
        #[arg(long)]
        address: Option<String>,
    },
    Fund {
        #[arg(long, default_value = "1.0")]
        amount_btc: String,
        #[arg(long)]
        address: Option<String>,
        #[arg(long, default_value_t = 1)]
        mine_blocks: u32,
    },
    Reset {
        #[arg(long)]
        remove_profile: bool,
        #[arg(long)]
        remove_snapshots: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, IntentAction, IntentPairAction, OfferAction};
    use clap::Parser;

    #[test]
    fn parses_global_thumb_switch() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "--thumb",
            "offer",
            "discover",
            "--relay",
            "wss://relay.example",
        ])
        .expect("cli parse");

        assert!(cli.thumb_enabled());
    }

    #[test]
    fn defaults_global_thumb_switch() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "offer",
            "discover",
            "--relay",
            "wss://relay.example",
        ])
        .expect("cli parse");

        // Should default to true because output mode is human by default
        assert!(cli.thumb_enabled());
        assert!(!cli.no_thumb);
    }

    #[test]
    fn parses_offer_publish_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "offer",
            "publish",
            "--offer-json",
            "{\"version\":1}",
            "--secret-key-hex",
            "abc123",
            "--relay",
            "wss://relay.example",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Offer(args) => match args.action {
                OfferAction::Publish {
                    relay,
                    secret_key_hex,
                    ..
                } => {
                    assert_eq!(relay, vec!["wss://relay.example".to_string()]);
                    assert_eq!(secret_key_hex, "abc123");
                }
                _ => panic!("expected offer publish action"),
            },
            _ => panic!("expected offer command"),
        }
    }

    #[test]
    fn parses_intent_fixture_generate_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "fixture-generate",
            "--now-unix",
            "1710000000",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::FixtureGenerate { now_unix, .. } => {
                    assert_eq!(now_unix, Some(1_710_000_000));
                }
                _ => panic!("expected intent fixture-generate action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_intent_pair_start_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "pair",
            "start",
            "--network",
            "signet",
            "--expires-in-secs",
            "900",
            "--relay",
            "wss://relay.one",
            "--relay",
            "wss://relay.two",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::Pair(pair_args) => match pair_args.action {
                    IntentPairAction::Start {
                        network,
                        expires_in_secs,
                        relay,
                        show_json,
                        no_wait,
                        ..
                    } => {
                        assert_eq!(network, "signet");
                        assert_eq!(expires_in_secs, Some(900));
                        assert_eq!(relay, vec!["wss://relay.one", "wss://relay.two"]);
                        assert!(!show_json);
                        assert!(!no_wait);
                    }
                    _ => panic!("expected intent pair start action"),
                },
                _ => panic!("expected intent pair action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_intent_pair_finish_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "pair",
            "finish",
            "--request-file",
            "/tmp/request.json",
            "--ack-file",
            "/tmp/ack.json",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::Pair(pair_args) => match pair_args.action {
                    IntentPairAction::Finish {
                        request_file,
                        ack_file,
                        ..
                    } => {
                        assert_eq!(
                            request_file.as_deref().map(|p| p.display().to_string()),
                            Some("/tmp/request.json".to_string())
                        );
                        assert_eq!(
                            ack_file.as_deref().map(|p| p.display().to_string()),
                            Some("/tmp/ack.json".to_string())
                        );
                    }
                    _ => panic!("expected intent pair finish action"),
                },
                _ => panic!("expected intent pair action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_intent_send_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "send",
            "--pairing-id",
            "abcd1234",
            "--payload-json",
            "{\"action\":\"buildBuyerOffer\",\"params\":{\"inscriptionId\":\"insc1\",\"sellerOutpoint\":\"tx:0\",\"askSats\":12345,\"feeRateSatVb\":2}}",
            "--network",
            "regtest",
            "--expires-in-secs",
            "180",
            "--nonce",
            "7",
            "--relay",
            "wss://nostr.example",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::Send {
                    pairing_id,
                    network,
                    expires_in_secs,
                    nonce,
                    relay,
                    ..
                } => {
                    assert_eq!(pairing_id, "abcd1234");
                    assert_eq!(network, "regtest");
                    assert_eq!(expires_in_secs, Some(180));
                    assert_eq!(nonce, Some(7));
                    assert_eq!(relay, vec!["wss://nostr.example".to_string()]);
                }
                _ => panic!("expected intent send action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_intent_wait_receipt_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "wait-receipt",
            "--pairing-id",
            "abcd1234",
            "--intent-id",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "--timeout-ms",
            "45000",
            "--relay",
            "wss://nostr.example",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::WaitReceipt {
                    pairing_id,
                    intent_id,
                    timeout_ms,
                    relay,
                    allow_unknown_intent_action,
                    ..
                } => {
                    assert_eq!(pairing_id, "abcd1234");
                    assert_eq!(
                        intent_id,
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    );
                    assert_eq!(timeout_ms, 45_000);
                    assert_eq!(relay, vec!["wss://nostr.example".to_string()]);
                    assert!(!allow_unknown_intent_action);
                }
                _ => panic!("expected intent wait-receipt action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_intent_wait_receipt_allow_unknown_action_flag() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "intent",
            "wait-receipt",
            "--pairing-id",
            "abcd1234",
            "--intent-id",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "--allow-unknown-intent-action",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Intent(args) => match args.action {
                IntentAction::WaitReceipt {
                    allow_unknown_intent_action,
                    ..
                } => {
                    assert!(allow_unknown_intent_action);
                }
                _ => panic!("expected intent wait-receipt action"),
            },
            _ => panic!("expected intent command"),
        }
    }

    #[test]
    fn parses_pair_start_alias_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "pair",
            "start",
            "--network",
            "signet",
            "--show-json",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Start {
                    network, show_json, ..
                } => {
                    assert_eq!(network, "signet");
                    assert!(show_json);
                }
                _ => panic!("expected pair start action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_start_no_wait_flag() {
        let cli =
            Cli::try_parse_from(["zinc-cli", "pair", "start", "--no-wait"]).expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Start { no_wait, .. } => {
                    assert!(no_wait);
                }
                _ => panic!("expected pair start action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_finish_alias_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "pair",
            "finish",
            "--request-file",
            "/tmp/request.json",
            "--ack-file",
            "/tmp/ack.json",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Finish {
                    request_file,
                    ack_file,
                    ..
                } => {
                    assert_eq!(
                        request_file.as_deref().map(|p| p.display().to_string()),
                        Some("/tmp/request.json".to_string())
                    );
                    assert_eq!(
                        ack_file.as_deref().map(|p| p.display().to_string()),
                        Some("/tmp/ack.json".to_string())
                    );
                }
                _ => panic!("expected pair finish action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_finish_ack_code_without_request_flags() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "pair",
            "finish",
            "--ack-code",
            "zincack1_deadbeef",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Finish {
                    request_json,
                    request_file,
                    ack_json,
                    ack_file,
                    ack_code,
                    ..
                } => {
                    assert!(request_json.is_none());
                    assert!(request_file.is_none());
                    assert!(ack_json.is_none());
                    assert!(ack_file.is_none());
                    assert_eq!(ack_code, Some("zincack1_deadbeef".to_string()));
                }
                _ => panic!("expected pair finish action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_list_alias_subcommand() {
        let cli = Cli::try_parse_from(["zinc-cli", "pair", "list"]).expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::List {} => {}
                _ => panic!("expected pair list action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_show_alias_subcommand() {
        let cli = Cli::try_parse_from(["zinc-cli", "pair", "show", "--pairing-id", "deadbeef"])
            .expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Show { pairing_id } => {
                    assert_eq!(pairing_id, "deadbeef".to_string());
                }
                _ => panic!("expected pair show action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_pair_revoke_alias_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "pair",
            "revoke",
            "--pairing-id",
            "deadbeef",
            "--now-unix",
            "1710000000",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Pair(args) => match args.action {
                IntentPairAction::Revoke {
                    pairing_id,
                    now_unix,
                } => {
                    assert_eq!(pairing_id, "deadbeef".to_string());
                    assert_eq!(now_unix, Some(1_710_000_000));
                }
                _ => panic!("expected pair revoke action"),
            },
            _ => panic!("expected pair command"),
        }
    }

    #[test]
    fn parses_offer_discover_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "offer",
            "discover",
            "--relay",
            "wss://relay.example",
            "--limit",
            "50",
            "--timeout-ms",
            "2000",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Offer(args) => match args.action {
                OfferAction::Discover {
                    relay,
                    limit,
                    timeout_ms,
                } => {
                    assert_eq!(relay, vec!["wss://relay.example".to_string()]);
                    assert_eq!(limit, 50);
                    assert_eq!(timeout_ms, 2000);
                }
                _ => panic!("expected offer discover action"),
            },
            _ => panic!("expected offer command"),
        }
    }

    #[test]
    fn parses_offer_create_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "offer",
            "create",
            "--inscription",
            "inscription123",
            "--amount",
            "1234",
            "--fee-rate",
            "2",
            "--expires-in-secs",
            "7200",
            "--publisher-pubkey-hex",
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            "--seller-payout-address",
            "bcrt1qexample0000000000000000000000000000000",
            "--submit-ord",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Offer(args) => match args.action {
                OfferAction::Create {
                    inscription,
                    amount,
                    fee_rate,
                    expires_in_secs,
                    publisher_pubkey_hex,
                    seller_payout_address,
                    submit_ord,
                    ..
                } => {
                    assert_eq!(inscription, "inscription123");
                    assert_eq!(amount, 1234);
                    assert_eq!(fee_rate, 2);
                    assert_eq!(expires_in_secs, 7200);
                    assert_eq!(
                        publisher_pubkey_hex.as_deref(),
                        Some("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                    );
                    assert_eq!(
                        seller_payout_address.as_deref(),
                        Some("bcrt1qexample0000000000000000000000000000000")
                    );
                    assert!(submit_ord);
                }
                _ => panic!("expected offer create action"),
            },
            _ => panic!("expected offer command"),
        }
    }

    #[test]
    fn parses_offer_accept_subcommand() {
        let cli = Cli::try_parse_from([
            "zinc-cli",
            "offer",
            "accept",
            "--offer-json",
            "{\"version\":1}",
            "--expect-inscription",
            "inscription123",
            "--expect-ask-sats",
            "1234",
            "--dry-run",
        ])
        .expect("cli parse");

        match cli.command {
            Command::Offer(args) => match args.action {
                OfferAction::Accept {
                    offer_json,
                    expect_inscription,
                    expect_ask_sats,
                    dry_run,
                    ..
                } => {
                    assert_eq!(offer_json.as_deref(), Some("{\"version\":1}"));
                    assert_eq!(expect_inscription.as_deref(), Some("inscription123"));
                    assert_eq!(expect_ask_sats, Some(1234));
                    assert!(dry_run);
                }
                _ => panic!("expected offer accept action"),
            },
            _ => panic!("expected offer command"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct InscriptionArgs {
    #[command(subcommand)]
    pub action: InscriptionAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum InscriptionAction {
    List,
}

#[derive(Parser, Debug, Clone)]
pub struct PulseArgs {
    #[command(subcommand)]
    pub action: PulseAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PulseAction {
    #[command(about = "Login to Pulse Metadata Oracle")]
    Login {
        #[arg(index = 1, help = "Pulse API Token")]
        token: String,
    },
}
