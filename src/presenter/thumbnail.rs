use crate::cli::ThumbMode;
use image::{imageops::FilterType, DynamicImage};

const ASCII_RAMP: &[u8] = b"@%#*+=-:. ";

pub fn render_thumbnail_from_bytes(
    bytes: &[u8],
    mode: ThumbMode,
    width: u32,
) -> Result<Vec<String>, String> {
    if mode == ThumbMode::None {
        return Ok(Vec::new());
    }

    let img = image::load_from_memory(bytes).map_err(|e| format!("decode image: {e}"))?;
    let safe_width = width.clamp(8, 80);
    let safe_height = (safe_width / 2).clamp(4, 40);

    match mode {
        ThumbMode::None => Ok(Vec::new()),
        ThumbMode::Ascii => Ok(render_ascii(&img, safe_width, safe_height)),
        ThumbMode::Ansi => Ok(render_ansi(&img, safe_width / 2, safe_height)),
    }
}

pub fn render_non_image_badge(content_type: Option<&str>) -> Vec<String> {
    let label = content_type.unwrap_or("unknown");
    vec![
        format!("[non-image inscription]"),
        format!("content-type: {label}"),
    ]
}

fn render_ascii(img: &DynamicImage, max_cols: u32, max_rows: u32) -> Vec<String> {
    let resized = img.resize(max_cols, max_rows, FilterType::Triangle).to_rgb8();
    let width = resized.width();
    let height = resized.height();
    let mut lines = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut line = String::with_capacity(width as usize);
        for x in 0..width {
            let pixel = resized.get_pixel(x, y);
            let luma =
                (0.299 * f32::from(pixel[0]) + 0.587 * f32::from(pixel[1]) + 0.114 * f32::from(pixel[2]))
                    as u8;
            let idx = (usize::from(luma) * (ASCII_RAMP.len() - 1)) / 255;
            line.push(ASCII_RAMP[idx] as char);
        }
        lines.push(line);
    }

    lines
}

fn render_ansi(img: &DynamicImage, max_cols: u32, max_rows: u32) -> Vec<String> {
    let target_height_pixels = max_rows * 2;
    let resized = img.resize(max_cols, target_height_pixels, FilterType::Triangle).to_rgba8();
    
    let width = resized.width();
    let height = resized.height();
    let rows = (height + 1) / 2;
    let mut lines = Vec::with_capacity(rows as usize);

    for row in 0..rows {
        let y_top = row * 2;
        let y_bottom = y_top + 1;
        
        let mut line = String::new();
        for x in 0..width {
            let top_rgba = resized.get_pixel(x, y_top);
            let bottom_rgba = if y_bottom < height {
                resized.get_pixel(x, y_bottom)
            } else {
                &image::Rgba([0, 0, 0, 0])
            };

            let top_trans = top_rgba[3] < 128;
            let bot_trans = bottom_rgba[3] < 128;

            match (top_trans, bot_trans) {
                (true, true) => {
                    line.push_str("\x1b[0m ");
                }
                (false, true) => {
                    line.push_str(&format!(
                        "\x1b[0m\x1b[38;2;{};{};{}m▀",
                        top_rgba[0], top_rgba[1], top_rgba[2]
                    ));
                }
                (true, false) => {
                    line.push_str(&format!(
                        "\x1b[0m\x1b[38;2;{};{};{}m▄",
                        bottom_rgba[0], bottom_rgba[1], bottom_rgba[2]
                    ));
                }
                (false, false) => {
                    line.push_str(&format!(
                        "\x1b[0m\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                        top_rgba[0], top_rgba[1], top_rgba[2],
                        bottom_rgba[0], bottom_rgba[1], bottom_rgba[2]
                    ));
                }
            }
        }
        line.push_str("\x1b[0m");
        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::{render_non_image_badge, render_thumbnail_from_bytes};
    use crate::cli::ThumbMode;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn sample_png() -> Vec<u8> {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 0])); // Transparent
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let mut bytes = Vec::new();
        dyn_img
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("png encode");
        bytes
    }

    #[test]
    fn ascii_thumbnail_renders_multiple_lines() {
        let bytes = sample_png();
        let lines = render_thumbnail_from_bytes(&bytes, ThumbMode::Ascii, 16).expect("ascii");
        assert!(lines.len() >= 1);
        assert!(lines.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn ansi_thumbnail_contains_color_escape_sequences() {
        let bytes = sample_png();
        let lines = render_thumbnail_from_bytes(&bytes, ThumbMode::Ansi, 16).expect("ansi");
        assert!(lines.iter().any(|line| line.contains("\x1b[38;2;")));
        assert!(lines.iter().all(|line| line.ends_with("\x1b[0m")));
    }

    #[test]
    fn non_image_badge_contains_content_type() {
        let badge = render_non_image_badge(Some("text/plain"));
        assert!(badge.join("\n").contains("text/plain"));
    }
}
