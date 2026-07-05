//! Renders the tray icon: the remaining percentage as large colored digits
//! (green -> yellow -> red as the tank drains) over a thin fuel bar.
//! "F" at a full tank. Drawn straight into an RGBA buffer — no image deps.

const W: usize = 32;
const H: usize = 32;

const GREEN: [u8; 4] = [63, 185, 80, 255];
const YELLOW: [u8; 4] = [222, 163, 24, 255];
const RED: [u8; 4] = [235, 68, 50, 255];
/// Translucent mid-gray reads on both light and dark taskbars.
const TRACK: [u8; 4] = [128, 128, 128, 120];

/// 3x5 pixel font, one byte per row, low 3 bits used (MSB = left pixel).
fn glyph(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'F' => [0b111, 0b100, 0b111, 0b100, 0b100],
        _ => [0; 5],
    }
}

const SCALE: usize = 3; // glyphs render 9x15
const GLYPH_W: usize = 3 * SCALE;
const GLYPH_H: usize = 5 * SCALE;
const GAP: usize = 2;

pub fn render(remaining: f64) -> (Vec<u8>, u32, u32) {
    let mut px = vec![0u8; W * H * 4]; // transparent
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        if x < W && y < H {
            let i = (y * W + x) * 4;
            px[i..i + 4].copy_from_slice(&c);
        }
    };

    let remaining = remaining.clamp(0.0, 1.0);
    let color = if remaining < 0.25 {
        RED
    } else if remaining < 0.5 {
        YELLOW
    } else {
        GREEN
    };

    // Text: "F" at full, otherwise the whole percent number (0-99).
    let pct = (remaining * 100.0).round() as u32;
    let text: Vec<char> = if pct >= 100 {
        vec!['F']
    } else {
        pct.to_string().chars().collect()
    };

    let text_w = text.len() * GLYPH_W + text.len().saturating_sub(1) * GAP;
    let x0 = W.saturating_sub(text_w) / 2;
    let y0 = 3usize;

    for (gi, ch) in text.iter().enumerate() {
        let rows = glyph(*ch);
        let gx = x0 + gi * (GLYPH_W + GAP);
        for (ry, row) in rows.iter().enumerate() {
            for rx in 0..3 {
                if row & (0b100 >> rx) != 0 {
                    for sy in 0..SCALE {
                        for sx in 0..SCALE {
                            put(gx + rx * SCALE + sx, y0 + ry * SCALE + sy, color);
                        }
                    }
                }
            }
        }
    }

    // Fuel bar under the number: gray track, colored fill from the left
    // (E on the left, F on the right, like the dial).
    let (bx0, bx1, by0, by1) = (2usize, 30usize, y0 + GLYPH_H + 3, y0 + GLYPH_H + 8);
    for y in by0..by1 {
        for x in bx0..bx1 {
            put(x, y, TRACK);
        }
    }
    let fill_w = ((bx1 - bx0) as f64 * remaining).round() as usize;
    for y in by0..by1 {
        for x in bx0..bx0 + fill_w {
            put(x, y, color);
        }
    }

    (px, W as u32, H as u32)
}
