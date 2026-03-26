use crate::cli::{Cli, InscriptionArgs};
use crate::error::AppError;
use crate::load_wallet_session;
use crate::output::{CommandOutput, InscriptionItemDisplay};
use crate::presenter::thumbnail::render_non_image_badge;
use zinc_core::{ordinals::Inscription, OrdClient};

pub async fn run(cli: &Cli, _args: &InscriptionArgs) -> Result<CommandOutput, AppError> {
    let session = load_wallet_session(cli)?;
    let sorted_inscriptions = sort_inscriptions_latest_first(session.wallet.inscriptions());

    let display_items = if !cli.thumb_enabled() {
        None
    } else {
        Some(get_inscription_display_items(
            &session.profile.ord_url,
            &sorted_inscriptions,
        )
        .await)
    };

    Ok(CommandOutput::InscriptionList {
        inscriptions: sorted_inscriptions,
        display_items,
        thumb_mode_enabled: cli.thumb_enabled(),
    })
}

fn sort_inscriptions_latest_first(inscriptions: &[Inscription]) -> Vec<Inscription> {
    let mut sorted = inscriptions.to_vec();
    sorted.sort_by(|a, b| {
        b.timestamp
            .cmp(&a.timestamp)
            .then_with(|| b.number.cmp(&a.number))
            .then_with(|| b.id.cmp(&a.id))
    });
    sorted
}

async fn get_inscription_display_items(
    ord_url: &str,
    inscriptions: &[Inscription],
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
        let mut image_bytes = None;

        if content_type.starts_with("image/") {
            match client.get_inscription_content(&ins.id).await {
                Ok(content) => {
                    // Store raw bytes — viuer will render them during output.
                    image_bytes = Some(content.bytes);
                }
                Err(_) => {
                    badge_lines.push(format!(
                        "(thumbnail unavailable: {})",
                        crate::commands::offer::abbreviate(&ins.id, 12, 8)
                    ));
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
            image_bytes,
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use super::sort_inscriptions_latest_first;
    use zinc_core::ordinals::{Inscription, Satpoint};

    fn sample(id: &str, number: i64, timestamp: Option<u64>) -> Inscription {
        Inscription {
            id: id.to_string(),
            number,
            satpoint: Satpoint::default(),
            content_type: None,
            value: None,
            content_length: None,
            timestamp,
        }
    }

    #[test]
    fn sorts_newest_timestamp_first_then_number() {
        let inscriptions = vec![
            sample("old", 10, Some(100)),
            sample("new", 1, Some(300)),
            sample("mid-high-number", 99, Some(200)),
            sample("no-ts", 500, None),
            sample("same-ts-lower-number", 5, Some(200)),
        ];

        let sorted = sort_inscriptions_latest_first(&inscriptions);
        let ids: Vec<&str> = sorted.iter().map(|i| i.id.as_str()).collect();

        assert_eq!(
            ids,
            vec![
                "new",
                "mid-high-number",
                "same-ts-lower-number",
                "old",
                "no-ts"
            ]
        );
    }
}
