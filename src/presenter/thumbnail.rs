/// Print a thumbnail directly to stdout using the best available terminal
/// graphics protocol (Kitty, iTerm2, Sixel, or halfblock fallback).
/// Returns the (width, height) of the rendered image in terminal cells,
/// or `None` if rendering failed.
#[allow(dead_code)]
pub fn print_thumbnail(bytes: &[u8], width: u32) -> Option<(u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let conf = viuer::Config {
        width: Some(width),
        height: None,
        transparent: true,
        absolute_offset: false,
        ..Default::default()
    };
    viuer::print(&img, &conf).ok()
}

/// Print a thumbnail at a specific column offset (for grid layouts).
/// Saves and restores cursor position so the caller can print more
/// images on the same row. Returns the (width, height) of the rendered
/// image in terminal cells, or `None` if rendering failed.
pub fn print_thumbnail_at(bytes: &[u8], width: u32, x_offset: u16) -> Option<(u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let conf = viuer::Config {
        width: Some(width),
        height: Some(width / 2 + 1),
        transparent: true,
        absolute_offset: false,
        x: x_offset,
        restore_cursor: true,
        ..Default::default()
    };
    viuer::print(&img, &conf).ok()
}

pub fn render_non_image_badge(content_type: Option<&str>) -> Vec<String> {
    let label = content_type.unwrap_or("unknown");
    vec![
        "[non-image inscription]".to_string(),
        format!("content-type: {label}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::render_non_image_badge;

    #[test]
    fn non_image_badge_contains_content_type() {
        let badge = render_non_image_badge(Some("text/plain"));
        assert!(badge.join("\n").contains("text/plain"));
    }
}
