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
    about = "CLI wallet for Zinc Bitcoin + Ordinals workflows"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true, help = "Output exactly as JSON")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        help = "Agent mode (implies --json --quiet --ascii)"
    )]
    pub agent: bool,

    #[arg(long, global = true, help = "Suppress all non-error output")]
    pub quiet: bool,

    #[arg(long, global = true, help = "Automatically say yes to prompts")]
    pub yes: bool,

    #[arg(long, global = true, help = "Password for wallet operations")]
    pub password: Option<String>,

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

    #[arg(long, global = true, help = "Override Esplora API URL")]
    pub esplora_url: Option<String>,

    #[arg(long, global = true, help = "Override Ordinals indexer URL")]
    pub ord_url: Option<String>,

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
    Offer(OfferArgs),
    Account(AccountArgs),
    Wait(WaitArgs),
    Snapshot(SnapshotArgs),
    Lock(LockArgs),
    Scenario(ScenarioArgs),
    Inscription(InscriptionArgs),
    #[cfg(feature = "ui")]
    Dashboard,
    Doctor,
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
    pub default_esplora_url: Option<String>,
    #[arg(long)]
    pub default_ord_url: Option<String>,
    #[arg(long)]
    pub json_default: Option<bool>,
    #[arg(long)]
    pub quiet_default: Option<bool>,
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

#[derive(Subcommand, Debug, Clone)]
pub enum OfferAction {
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
    use super::{Cli, Command, OfferAction};
    use clap::Parser;

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
