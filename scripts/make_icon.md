# App icon

The source icon is `src-tauri/icon-src.png` (1024×1024) — a cream fuel-gauge
dial on a vermillion tile. It's rendered from `scripts/icon.html` (a small
canvas sketch) rather than hand-drawn pixels, so it's anti-aliased.

To regenerate the full icon set (`.ico`, PNGs, platform sizes) after editing
the design:

```bash
# 1. Open scripts/icon.html in a browser, right-click the canvas, "Save image
#    as" -> src-tauri/icon-src.png  (or re-render at 1024 and export)
# 2. Regenerate every size from the source:
npx tauri icon src-tauri/icon-src.png
```

Design: brand vermillion (#bd3b1c → #9a3116 tile), cream gauge (#f7f3ec),
needle pointing toward "full", three notches at E / mid / F. Keep it bold —
it has to read at 16px in the Windows tray.
