use comfy_table::{Cell, Color, Table};
use std::path::Path;

pub fn format_snapshots(paths: &[String]) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Path").fg(Color::Green),
    ]);

    for path_str in paths {
        let path = Path::new(path_str);
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path_str);

        table.add_row(vec![Cell::new(name), Cell::new(path_str)]);
    }

    table
}
