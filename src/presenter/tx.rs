use comfy_table::{Cell, Color, Table};
use zinc_core::TxItem;

pub fn format_transactions(txs: &[TxItem]) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("TxID").fg(Color::Cyan),
        Cell::new("Type").fg(Color::Green),
        Cell::new("Amount (sats)").fg(Color::Yellow),
        Cell::new("Fee (sats)").fg(Color::Magenta),
        Cell::new("Confirm Time (UTC)").fg(Color::Blue),
        Cell::new("Inscriptions").fg(Color::Red),
    ]);

    for tx in txs {
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
                .map(|i| i.number.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        table.add_row(vec![
            Cell::new(format!("{}...", &tx.txid[..8])),
            Cell::new(&tx.tx_type),
            Cell::new(tx.amount_sats.to_string()),
            Cell::new(tx.fee_sats.to_string()),
            Cell::new(time_str),
            Cell::new(ins_str),
        ]);
    }

    table
}
