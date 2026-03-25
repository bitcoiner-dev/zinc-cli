use crate::cli::{Cli, InscriptionArgs, ThumbMode};
use crate::error::AppError;
use crate::presenter::thumbnail::{render_non_image_badge, render_thumbnail_from_bytes};
use crate::load_wallet_session;
use serde_json::{json, Value};
use zinc_core::{ordinals::Inscription, OrdClient};

pub async fn run(cli: &Cli, _args: &InscriptionArgs) -> Result<Value, AppError> {
    let session = load_wallet_session(cli)?;
    let inscriptions = session.wallet.inscriptions();

    if cli.json {
        Ok(json!({
            "inscriptions": inscriptions
        }))
    } else {
        if cli.thumb == ThumbMode::None {
            let table = crate::presenter::inscription::format_inscriptions(&inscriptions);
            println!("{table}");
        } else {
            render_inscriptions_with_thumbnails(
                &session.profile.ord_url,
                inscriptions,
                cli.thumb,
            )
            .await;
        }
        Ok(Value::Null)
    }
}

async fn render_inscriptions_with_thumbnails(
    ord_url: &str,
    inscriptions: &[Inscription],
    mode: ThumbMode,
) {
    const MAX_VISIBLE: usize = 8;
    let client = OrdClient::new(ord_url.to_string());

    for (idx, ins) in inscriptions.iter().take(MAX_VISIBLE).enumerate() {
        let content_type = ins.content_type.as_deref().unwrap_or("unknown");
        let value = ins
            .value
            .map(|amount| amount.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "[{}] #{} {} sats {}",
            idx + 1,
            ins.number,
            value,
            content_type
        );

        if content_type.starts_with("image/") {
            match client.get_inscription_content(&ins.id).await {
                Ok(content) => match render_thumbnail_from_bytes(&content.bytes, mode, 24) {
                    Ok(lines) => {
                        for line in lines {
                            println!("{line}");
                        }
                    }
                    Err(_) => {
                        println!(
                            "(thumbnail unavailable: {})",
                            abbreviate(&ins.id, 12, 8)
                        );
                    }
                },
                Err(_) => {
                    println!("(thumbnail unavailable: {})", abbreviate(&ins.id, 12, 8));
                }
            }
        } else {
            for line in render_non_image_badge(Some(content_type)) {
                println!("{line}");
            }
        }

        println!();
    }

    if inscriptions.len() > MAX_VISIBLE {
        println!("... and {} more inscriptions", inscriptions.len() - MAX_VISIBLE);
    }
}

fn abbreviate(value: &str, prefix: usize, suffix: usize) -> String {
    if value.chars().count() <= prefix + suffix + 3 {
        return value.to_string();
    }
    let start: String = value.chars().take(prefix).collect();
    let end: String = value
        .chars()
        .rev()
        .take(suffix)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{start}...{end}")
}
