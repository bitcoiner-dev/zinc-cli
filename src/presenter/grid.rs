/// Grid layout renderer for arranging "cards" (header + thumbnail) side-by-side
/// in pure terminal output. Handles ANSI escape sequences when calculating
/// visible column widths.

/// Strip ANSI escape sequences to measure visible character width.
fn visible_len(s: &str) -> usize {
    let mut len = 0usize;
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        len += 1;
    }
    len
}

/// Pad a string (which may contain ANSI escapes) to a target *visible* width
/// by appending spaces.
fn pad_to_visible(s: &str, target: usize) -> String {
    let vis = visible_len(s);
    if vis >= target {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target - vis))
    }
}

/// A single card to be placed in the grid.
pub struct GridCard {
    /// Lines of text (may contain ANSI escape sequences).
    pub lines: Vec<String>,
}

impl GridCard {
    /// Maximum visible width across all lines.
    pub fn visible_width(&self) -> usize {
        self.lines.iter().map(|l| visible_len(l)).max().unwrap_or(0)
    }
}

/// Arrange cards into a grid and return the composed output string.
///
/// * `cards`      – the cards to lay out
/// * `max_cols`   – maximum number of columns in the terminal (e.g. 120)
/// * `gutter`     – number of blank columns between cards
pub fn render_grid(cards: &[GridCard], max_cols: usize, gutter: usize) -> String {
    if cards.is_empty() {
        return String::new();
    }

    // Determine how many cards fit in one row.
    // We use the widest card as the cell width so columns align.
    let cell_width = cards.iter().map(|c| c.visible_width()).max().unwrap_or(0);
    if cell_width == 0 {
        return String::new();
    }

    let cols_per_row = ((max_cols + gutter) / (cell_width + gutter)).max(1);

    let mut out = String::new();

    for row_cards in cards.chunks(cols_per_row) {
        // Find the tallest card in this row.
        let max_height = row_cards.iter().map(|c| c.lines.len()).max().unwrap_or(0);

        for line_idx in 0..max_height {
            for (card_idx, card) in row_cards.iter().enumerate() {
                let line = card
                    .lines
                    .get(line_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");

                // Pad every card to cell_width so columns stay aligned.
                out.push_str(&pad_to_visible(line, cell_width));

                // Add gutter between cards (but not after the last one in a row).
                if card_idx + 1 < row_cards.len() {
                    out.push_str(&" ".repeat(gutter));
                }
            }
            out.push('\n');
        }

        // Blank line between rows of cards.
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_len_strips_ansi() {
        assert_eq!(visible_len("\x1b[38;2;255;0;0mHello\x1b[0m"), 5);
        assert_eq!(visible_len("Plain text"), 10);
        assert_eq!(visible_len(""), 0);
    }

    #[test]
    fn pad_to_visible_pads_ansi_string() {
        let s = "\x1b[31mHi\x1b[0m"; // visible len = 2
        let padded = pad_to_visible(s, 5);
        assert_eq!(visible_len(&padded), 5);
        assert!(padded.ends_with("   ")); // 3 spaces
    }

    #[test]
    fn grid_arranges_cards_side_by_side() {
        let cards = vec![
            GridCard { lines: vec!["AAAA".into(), "AAAA".into()] },
            GridCard { lines: vec!["BBBB".into(), "BBBB".into()] },
            GridCard { lines: vec!["CCCC".into()] }, // shorter card
        ];
        let output = render_grid(&cards, 20, 2);
        // With cell_width=4, gutter=2, cols_per_row = (20+2)/(4+2) = 3
        // All three should fit on one row.
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines[0].contains("AAAA"));
        assert!(lines[0].contains("BBBB"));
        assert!(lines[0].contains("CCCC"));
        // Second line: card C has no second line, so it should be padded with spaces.
        assert!(lines[1].contains("AAAA"));
        assert!(lines[1].contains("BBBB"));
    }

    #[test]
    fn grid_wraps_to_multiple_rows() {
        let cards = vec![
            GridCard { lines: vec!["AAAA".into()] },
            GridCard { lines: vec!["BBBB".into()] },
            GridCard { lines: vec!["CCCC".into()] },
        ];
        // max_cols=10, gutter=2 → cell_width=4, cols_per_row = (10+2)/(4+2) = 2
        let output = render_grid(&cards, 10, 2);
        let lines: Vec<&str> = output.lines().collect();
        // First row: A and B
        assert!(lines[0].contains("AAAA"));
        assert!(lines[0].contains("BBBB"));
        // Second row (after blank): C alone
        // lines[1] is blank separator
        assert!(lines[2].contains("CCCC"));
    }

    #[test]
    fn empty_cards_returns_empty() {
        assert_eq!(render_grid(&[], 80, 2), "");
    }
}
