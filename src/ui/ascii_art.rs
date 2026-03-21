#![allow(dead_code)]

use image::DynamicImage;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// A high-detail, feature-preserving Half-Block TrueColor renderer.
pub struct ShapeArt;

impl ShapeArt {
    /// Converts a DynamicImage to high-fidelity "Pixel Art" with feature preservation (eyes, lines, etc.)
    pub fn to_colored_ascii(img: &DynamicImage, width: u32, height: u32) -> Vec<Line<'static>> {
        // 1. Preprocessing: Sharpen the image to harden edges of small features
        // We do this at a reasonably high resolution before the final downsample
        let sharpened = img.unsharpen(1.0, 15);

        // 2. We need two versions:
        // - A high-res version to "scout" for details (e.g. 2x horizontal, 4x vertical of target)
        // - A target-res version for the base colors
        let scout_w = width * 2;
        let scout_h = height * 4;
        let scout_img =
            sharpened.resize_exact(scout_w, scout_h, image::imageops::FilterType::Triangle);
        let scout_rgb = scout_img.to_rgb8();
        let scout_luma = scout_img.to_luma8();

        let mut lines = Vec::with_capacity(height as usize);

        for y_cell in 0..height {
            let mut spans = Vec::with_capacity(width as usize);
            for x_cell in 0..width {
                // Each character cell (1x1) corresponds to:
                // - 2 vertical half-blocks (Top, Bottom)
                // - 2x4 pixels in our scout image (x_cell*2..+2, y_cell*4..+4)

                // --- Process Top Half-Block (Scout rows 0,1) ---
                let top_color = Self::sample_with_feature_bias(
                    &scout_rgb,
                    &scout_luma,
                    x_cell * 2,
                    y_cell * 4,
                    2,
                    2,
                );

                // --- Process Bottom Half-Block (Scout rows 2,3) ---
                let bottom_color = Self::sample_with_feature_bias(
                    &scout_rgb,
                    &scout_luma,
                    x_cell * 2,
                    y_cell * 4 + 2,
                    2,
                    2,
                );

                spans.push(Span::styled(
                    "▄",
                    Style::default().fg(bottom_color).bg(top_color),
                ));
            }
            lines.push(Line::from(spans));
        }

        lines
    }

    /// Samples a patch of pixels and biases the result towards high-contrast features (like eyes).
    fn sample_with_feature_bias(
        rgb: &image::RgbImage,
        luma: &image::GrayImage,
        start_x: u32,
        start_y: u32,
        width: u32,
        height: u32,
    ) -> Color {
        let mut sum_r = 0u32;
        let mut sum_g = 0u32;
        let mut sum_b = 0u32;
        let mut min_luma = 255u8;
        let mut max_luma = 0u8;
        let mut min_idx = (0, 0);
        let mut max_idx = (0, 0);

        let count = width * height;

        for py in 0..height {
            for px in 0..width {
                let x = start_x + px;
                let y = start_y + py;
                let l = luma.get_pixel(x, y)[0];
                let c = rgb.get_pixel(x, y);

                sum_r += c[0] as u32;
                sum_g += c[1] as u32;
                sum_b += c[2] as u32;

                if l < min_luma {
                    min_luma = l;
                    min_idx = (x, y);
                }
                if l > max_luma {
                    max_luma = l;
                    max_idx = (x, y);
                }
            }
        }

        let avg_luma = ((sum_r + sum_g + sum_b) / (3 * count)) as u32;
        let contrast = max_luma.saturating_sub(min_luma);

        // FEATURE PRESERVATION LOGIC:
        // If there is high contrast in this tiny patch (e.g. > 40 units),
        // it means there's a significant detail (like an eye or a sharp line).
        // Instead of averaging it away, we "anchor" to the feature.
        if contrast > 40 {
            // If the average is relatively bright, and we found a very dark spot,
            // it's likely an eye or a line. Prioritize the dark spot.
            if avg_luma > 100 && (avg_luma as i32 - min_luma as i32) > 30 {
                let c = rgb.get_pixel(min_idx.0, min_idx.1);
                return Color::Rgb(c[0], c[1], c[2]);
            }
            // If the average is dark and we found a bright spot, prioritize the bright spot.
            if avg_luma < 100 && (max_luma as i32 - avg_luma as i32) > 30 {
                let c = rgb.get_pixel(max_idx.0, max_idx.1);
                return Color::Rgb(c[0], c[1], c[2]);
            }
        }

        // Default: Return average color
        Color::Rgb(
            (sum_r / count) as u8,
            (sum_g / count) as u8,
            (sum_b / count) as u8,
        )
    }
}
