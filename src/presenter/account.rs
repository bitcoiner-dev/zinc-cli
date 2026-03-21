use comfy_table::{Cell, Color, Table};
use zinc_core::Account;

pub fn format_accounts(accounts: &[Account]) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Index").fg(Color::Cyan),
        Cell::new("Label").fg(Color::Green),
        Cell::new("Taproot Address").fg(Color::Yellow),
    ]);

    for account in accounts {
        table.add_row(vec![
            Cell::new(account.index.to_string()),
            Cell::new(&account.label),
            Cell::new(&account.taproot_address),
        ]);
    }

    table
}
