use comfy_table::{Cell, Color, Table};
use serde_json::Value;

pub fn format_config(config: &Value) -> Table {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Setting").fg(Color::Cyan),
        Cell::new("Value").fg(Color::Green),
    ]);

    if let Some(obj) = config.as_object() {
        for (k, v) in obj {
            let val_str = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "-".to_string(),
                _ => v.to_string(),
            };
            table.add_row(vec![Cell::new(k), Cell::new(val_str)]);
        }
    }

    table
}
