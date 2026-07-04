//! Renders the tray icon: a vertical fuel-tank bar whose fill level is the
//! remaining fraction of the tighter gauge. Drawn straight into an RGBA
//! buffer — no image dependencies.

const W: usize = 32;
const H: usize = 32;

const BORDER: [u8; 4] = [240, 238, 233, 255]; // near-paper, visible on dark trays
const FILL_OK: [u8; 4] = [240, 238, 233, 255];
const FILL_WARN: [u8; 4] = [224, 161, 65, 255]; // amber
const FILL_LOW: [u8; 4] = [200, 66, 31, 255]; // vermillion

pub fn render(remaining: f64) -> (Vec<u8>, u32, u32) {
    let mut px = vec![0u8; W * H * 4]; // transparent
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        if x < W && y < H {
            let i = (y * W + x) * 4;
            px[i..i + 4].copy_from_slice(&c);
        }
    };

    // Tank body: x 9..23, y 6..30. Cap notch on top: x 13..19, y 3..6.
    let (x0, x1, y0, y1) = (9usize, 23usize, 6usize, 30usize);

    // Cap
    for y in 3..y0 {
        for x in 13..19 {
            put(x, y, BORDER);
        }
    }
    // Border (2px feel via single crisp outline at 32px)
    for x in x0..x1 {
        put(x, y0, BORDER);
        put(x, y1 - 1, BORDER);
    }
    for y in y0..y1 {
        put(x0, y, BORDER);
        put(x1 - 1, y, BORDER);
    }

    // Fill from the bottom
    let inner_h = (y1 - 1) - (y0 + 1); // 22 rows
    let filled = ((remaining.clamp(0.0, 1.0)) * inner_h as f64).round() as usize;
    let color = if remaining < 0.25 {
        FILL_LOW
    } else if remaining < 0.5 {
        FILL_WARN
    } else {
        FILL_OK
    };
    for row in 0..filled {
        let y = (y1 - 2) - row;
        for x in (x0 + 2)..(x1 - 2) {
            put(x, y, color);
        }
    }

    (px, W as u32, H as u32)
}
