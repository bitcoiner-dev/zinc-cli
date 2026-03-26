use crate::cli::Cli;
use crate::error::AppError;
use crate::output::CommandOutput;
use serde_json::json;
use std::io::Write;

const ANSI_LOGO_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/zinc-cli-logo.ans");
const VERSION_TAGLINE: &str = "Agent-first Bitcoin + Ordinals CLI wallet";
const ACCENT_ORANGE: &str = "\x1b[38;2;249;115;22m";
const ANSI_RESET: &str = "\x1b[0m";
const LOGO_COLUMNS: u16 = 40;
const LOGO_ROWS: u16 = 20;
const TEXT_GAP_COLUMNS: u16 = 4;
const TEXT_TOP_OFFSET_ROWS: u16 = 8;

pub async fn run(cli: &Cli) -> Result<CommandOutput, AppError> {
    if cli.agent {
        // Do nothing for stdout in agent mode
    } else if cli.ascii || !is_terminal::is_terminal(&std::io::stdout()) {
        println!("zinc-cli v{}", env!("CARGO_PKG_VERSION"));
    } else {
        render_logo()?;
    }

    Ok(CommandOutput::Generic(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "zinc-cli",
    })))
}

fn render_logo() -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    let logo = std::fs::read(ANSI_LOGO_PATH)?;
    let version_line = format!("zinc-cli v{}", env!("CARGO_PKG_VERSION"));

    stdout.write_all(&logo)?;

    // Place text to the right side of the rendered ANSI logo.
    let text_col = LOGO_COLUMNS + TEXT_GAP_COLUMNS + 1; // CSI G is 1-based.
    let rise_rows = LOGO_ROWS.saturating_sub(TEXT_TOP_OFFSET_ROWS);
    stdout.write_all(b"\x1b[s")?;
    stdout.write_all(format!("\x1b[{rise_rows}A").as_bytes())?;
    stdout.write_all(
        format!("\x1b[{text_col}G{ACCENT_ORANGE}{version_line}{ANSI_RESET}").as_bytes(),
    )?;
    stdout.write_all(b"\n\n")?;
    stdout.write_all(
        format!("\x1b[{text_col}G{ACCENT_ORANGE}{VERSION_TAGLINE}{ANSI_RESET}").as_bytes(),
    )?;
    stdout.write_all(b"\x1b[u")?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}
