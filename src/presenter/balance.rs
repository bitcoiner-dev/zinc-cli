use comfy_table::{Cell, Color, Table};
use zinc_core::ZincBalance;

pub fn format_balance(balance: &ZincBalance) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Type").fg(Color::Cyan),
        Cell::new("Confirmed (sats)").fg(Color::Green),
        Cell::new("Trusted Pending (sats)").fg(Color::Yellow),
        Cell::new("Untrusted Pending (sats)").fg(Color::Magenta),
    ]);

    table.add_row(vec![
        Cell::new("Total (Combined)"),
        Cell::new(balance.total.confirmed.to_sat().to_string()),
        Cell::new(balance.total.trusted_pending.to_sat().to_string()),
        Cell::new(balance.total.untrusted_pending.to_sat().to_string()),
    ]);

    table.add_row(vec![
        Cell::new("Spendable (Safe)"),
        Cell::new(balance.spendable.confirmed.to_sat().to_string()),
        Cell::new(balance.spendable.trusted_pending.to_sat().to_string()),
        Cell::new(balance.spendable.untrusted_pending.to_sat().to_string()),
    ]);

    table.add_row(vec![
        Cell::new("Inscribed/Protected"),
        Cell::new("-"),
        Cell::new("-"),
        Cell::new(balance.inscribed.to_string()),
    ]);

    table
}
