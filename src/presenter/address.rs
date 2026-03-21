use comfy_table::{Cell, Color, Table};

pub fn format_address(address_type: &str, address: &str) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Type").fg(Color::Cyan),
        Cell::new("Address").fg(Color::Green),
    ]);

    table.add_row(vec![Cell::new(address_type), Cell::new(address)]);

    table
}
