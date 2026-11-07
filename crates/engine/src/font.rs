use crate::sprites::{put_pixel, fill_rect};
#[cfg(test)]
use crate::sprites::{W, H};

// 3x5 bitmap font. Each glyph is 5 rows, with bits 0..2 representing pixels from left to right.
// Bit 0 = left, Bit 1 = center, Bit 2 = right.
pub fn get_glyph_3x5(c: char) -> Option<[u8; 5]> {
    let c = c.to_ascii_uppercase();
    let glyph = match c {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b110, 0b100, 0b110, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'V' => [0b101, 0b101, 0b101, 0b010, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],

        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b011, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b110],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],

        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '?' => [0b110, 0b001, 0b010, 0b000, 0b010],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '(' => [0b010, 0b100, 0b100, 0b100, 0b010],
        ')' => [0b010, 0b001, 0b001, 0b001, 0b010],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        _ => return None,
    };
    Some(glyph)
}

pub fn text_width(text: &str) -> i32 {
    let mut w = 0;
    for c in text.chars() {
        if c == ' ' {
            w += 2;
        } else {
            w += 4; // 3 px glyph + 1 px spacing
        }
    }
    w.max(1) - 1
}

pub fn draw_text(
    buf: &mut [u8],
    mut x: i32,
    y: i32,
    text: &str,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    for c in text.chars() {
        if let Some(glyph) = get_glyph_3x5(c) {
            for row in 0..5 {
                let bits = glyph[row];
                for col in 0..3 {
                    if (bits & (1 << col)) != 0 {
                        put_pixel(buf, x + col as i32, y + row as i32, r, g, b, a);
                    }
                }
            }
            x += 4;
        } else {
            x += 2;
        }
    }
}

pub fn draw_text_centered(
    buf: &mut [u8],
    cx: i32,
    y: i32,
    text: &str,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let w = text_width(text);
    draw_text(buf, cx - w / 2, y, text, r, g, b, a);
}

pub fn draw_text_badge(
    buf: &mut [u8],
    cx: i32,
    y: i32,
    text: &str,
    text_color: (u8, u8, u8),
    bg_color: (u8, u8, u8, u8),
) {
    let w = text_width(text);
    let h = 7;
    let bx = cx - w / 2 - 2;
    let by = y - 1;
    fill_rect(buf, bx, by, w + 4, h, bg_color.0, bg_color.1, bg_color.2, bg_color.3);
    draw_text(buf, cx - w / 2, y, text, text_color.0, text_color.1, text_color.2, 255);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_rendering() {
        let mut buf = vec![0u8; W * H * 4];
        let text = "HOLD CALM 2.4S +10 GOATS: 100%";
        draw_text(&mut buf, 10, 10, text, 255, 255, 255, 255);
        let any_pixel = buf.iter().any(|&p| p != 0);
        assert!(any_pixel, "Text should draw pixels into the buffer");

        draw_text_centered(&mut buf, 160, 90, "STAGE 1", 255, 220, 100, 255);
        draw_text_badge(&mut buf, 160, 50, "IPO +25%", (255, 255, 255), (20, 20, 40, 200));
    }
}
