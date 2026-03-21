use comfy_table::{Cell, Color, Table};
use zinc_core::ordinals::Inscription;

pub fn format_inscriptions(inscriptions: &[Inscription]) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Number").fg(Color::Green),
        Cell::new("Content Type").fg(Color::Yellow),
        Cell::new("Value (sats)").fg(Color::Magenta),
    ]);

    for ins in inscriptions {
        let content_type = ins.content_type.as_deref().unwrap_or("unknown");
        let value = ins
            .value
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![
            Cell::new(&ins.id),
            Cell::new(ins.number.to_string()),
            Cell::new(content_type),
            Cell::new(value),
        ]);
    }

    table
}
