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

fn render_ascii(img: &DynamicImage, width: u32, height: u32) -> Vec<String> {
    let resized = img.resize_exact(width, height, FilterType::Triangle).to_rgb8();
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

fn render_ansi(img: &DynamicImage, width: u32, height: u32) -> Vec<String> {
    let resized = img.resize_exact(width, height, FilterType::Triangle).to_rgb8();
    let mut lines = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            let pixel = resized.get_pixel(x, y);
            line.push_str(&format!(
                "\x1b[48;2;{};{};{}m  ",
                pixel[0], pixel[1], pixel[2]
            ));
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
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;

    fn sample_png() -> Vec<u8> {
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 255, 0]));
        img.put_pixel(0, 1, Rgb([0, 0, 255]));
        img.put_pixel(1, 1, Rgb([255, 255, 255]));
        let dyn_img = DynamicImage::ImageRgb8(img);
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
        assert!(lines.len() >= 4);
        assert!(lines.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn ansi_thumbnail_contains_color_escape_sequences() {
        let bytes = sample_png();
        let lines = render_thumbnail_from_bytes(&bytes, ThumbMode::Ansi, 16).expect("ansi");
        assert!(lines.iter().any(|line| line.contains("\x1b[48;2;")));
        assert!(lines.iter().all(|line| line.ends_with("\x1b[0m")));
    }

    #[test]
    fn non_image_badge_contains_content_type() {
        let badge = render_non_image_badge(Some("text/plain"));
        assert!(badge.join("\n").contains("text/plain"));
    }
}
