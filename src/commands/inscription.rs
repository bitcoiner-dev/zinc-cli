use crate::cli::{Cli, InscriptionArgs, ThumbMode};
use crate::error::AppError;
use crate::presenter::thumbnail::{render_non_image_badge, render_thumbnail_from_bytes};
use crate::load_wallet_session;
use crate::output::{CommandOutput, InscriptionItemDisplay};
use zinc_core::{ordinals::Inscription, OrdClient};

pub async fn run(cli: &Cli, _args: &InscriptionArgs) -> Result<CommandOutput, AppError> {
    let session = load_wallet_session(cli)?;
    let inscriptions = session.wallet.inscriptions();

    let display_items = if cli.thumb == ThumbMode::None {
        None
    } else {
        Some(get_inscription_display_items(&session.profile.ord_url, &inscriptions, cli.thumb).await)
    };

    Ok(CommandOutput::InscriptionList {
        inscriptions: inscriptions.to_vec(),
        display_items,
        thumb_mode_enabled: cli.thumb != ThumbMode::None,
    })
}

async fn get_inscription_display_items(
    ord_url: &str,
    inscriptions: &[Inscription],
    mode: ThumbMode,
) -> Vec<InscriptionItemDisplay> {
    const MAX_VISIBLE: usize = 8;
    let client = OrdClient::new(ord_url.to_string());
    let mut items = Vec::new();

    for ins in inscriptions.iter().take(MAX_VISIBLE) {
        let content_type = ins.content_type.as_deref().unwrap_or("unknown");
        let value = ins
            .value
            .map(|amount| amount.to_string())
            .unwrap_or_else(|| "-".to_string());

        let mut badge_lines = Vec::new();
        if content_type.starts_with("image/") {
            match client.get_inscription_content(&ins.id).await {
                Ok(content) => match render_thumbnail_from_bytes(&content.bytes, mode, 48) {
                    Ok(lines) => {
                        badge_lines.extend(lines);
                    }
                    Err(_) => {
                        badge_lines.push(format!(
                            "(thumbnail unavailable: {})",
                            crate::commands::offer::abbreviate(&ins.id, 12, 8)
                        ));
                    }
                },
                Err(_) => {
                    badge_lines.push(format!("(thumbnail unavailable: {})", crate::commands::offer::abbreviate(&ins.id, 12, 8)));
                }
            }
        } else {
            badge_lines.extend(render_non_image_badge(Some(content_type)));
        }

        items.push(InscriptionItemDisplay {
            number: ins.number,
            id: ins.id.clone(),
            value_sats: value,
            content_type: content_type.to_string(),
            badge_lines,
        });
    }
    
    items
}
