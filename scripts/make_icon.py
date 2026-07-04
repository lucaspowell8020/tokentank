"""Generate the Claude Gauge source icon (1024x1024 PNG) with stdlib only.
Vermillion rounded square, paper fuel-tank glyph, three-quarter fill.
Run once, then: npx tauri icon src-tauri/icon-src.png
"""
import struct
import zlib

S = 1024
VERMILLION = (200, 66, 31, 255)
PAPER = (247, 243, 236, 255)
INK = (26, 24, 21, 255)
TRANSPARENT = (0, 0, 0, 0)

px = [[TRANSPARENT] * S for _ in range(S)]

# Rounded-square background
R = 180
for y in range(S):
    for x in range(S):
        dx = max(R - x, x - (S - 1 - R), 0)
        dy = max(R - y, y - (S - 1 - R), 0)
        if dx * dx + dy * dy <= R * R:
            px[y][x] = VERMILLION

# Fuel tank: body rect with cap, centered
bx0, bx1 = 330, 694          # body x range (364 wide)
by0, by1 = 300, 820          # body y range (520 tall)
border = 34

# Cap
for y in range(210, by0):
    for x in range(440, 584):
        px[y][x] = PAPER

# Body border
for y in range(by0, by1):
    for x in range(bx0, bx1):
        on_border = (
            x < bx0 + border or x >= bx1 - border or
            y < by0 + border or y >= by1 - border
        )
        if on_border:
            px[y][x] = PAPER

# Fill: 70% from the bottom of the inner area
ix0, ix1 = bx0 + border + 24, bx1 - border - 24
iy0, iy1 = by0 + border + 24, by1 - border - 24
fill_top = iy1 - int((iy1 - iy0) * 0.70)
for y in range(fill_top, iy1):
    for x in range(ix0, ix1):
        px[y][x] = PAPER

# Encode PNG
raw = b"".join(
    b"\x00" + b"".join(struct.pack("4B", *px[y][x]) for x in range(S))
    for y in range(S)
)


def chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data)) + tag + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(raw, 9))
    + chunk(b"IEND", b"")
)

with open("src-tauri/icon-src.png", "wb") as f:
    f.write(png)
print("wrote src-tauri/icon-src.png")
