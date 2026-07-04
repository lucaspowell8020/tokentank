# TokenTank

**The fuel gauge for your AI tools, in your system tray.** Live fuel levels for your usage windows, a countdown to the next reset, your burn rate in API-equivalent dollars per hour, and your plan multiple — measured from local transcripts, updated every 15 seconds.

Claude Code is the first supported tool; OpenAI Codex, Gemini CLI, and others are on the roadmap (they all write local session logs — same pattern, different parsers).

Everything runs locally. It reads `~/.claude/projects` and nothing else. No network calls, no telemetry, no account.

## What the gauge shows

- **Tray icon** — a fuel tank that drains as you consume your 5-hour session or weekly quota (whichever is tighter). White = plenty, amber = under half, vermillion = under a quarter. Hover for percentages and reset countdowns.
- **Click the icon** for the popover: an E-to-F fuel dial for the 5-hour session with a **live countdown to the reset**, a horizontal tank for the week with its refill time, burn rate ($/hour), today's total, last 30 days, and your plan multiple.

## How the windows work

- **5-hour session**: mirrors Claude's session blocks — a block opens with your first message after the previous one expires and lasts five hours. The countdown is to the end of the current block. No active block = full tank.
- **Weekly quota**: Claude resets weekly at a fixed time (shown in the Claude app under Settings → Usage, e.g. "Resets Wed 5:59 AM"). Put that in config as `weekly_reset` and the gauge tracks the real window; without it, it falls back to a rolling 7-day estimate.

## How it knows the ceiling

Anthropic doesn't publish subscription quotas as numbers, so the gauge starts with clearly-labelled estimates per plan. Two ways it gets accurate:

1. **Self-calibration** — Claude Code records a local notice every time you actually hit a limit; the gauge learns your real ceiling from the consumption that preceded each one.
2. **Panel calibration** — open the Claude app's Settings → Usage panel, note the "% used" it shows, and pin your ceilings: `ceiling = current API-equivalent spend ÷ fraction used`. Set the result in `gauge_ceilings`.

## Config

Reads the same `~/.claude/claude_usage.config.json` as the [usage dashboard](https://agentshortlist.com/claude-usage):

```json
{
  "plan": "max_5x",
  "weekly_reset": "wed 05:59",
  "gauge_ceilings": { "five_h": 265.0, "weekly": 2200.0 }
}
```

`plan` is one of `pro`, `max_5x`, `max_20x`, `api`. `weekly_reset` is the local day/time from the Claude app's Usage panel. `gauge_ceilings` (optional) pins the estimated ceilings in API-equivalent dollars.

## Development

Prereqs: Node 18+, Rust (MSVC toolchain on Windows).

```bash
npm install
npx tauri dev      # run with hot reload
npx tauri build    # produce the installer
```

## Status

v0.1 — Windows first. macOS build is the same codebase; packaging and notarization to follow.

## License

MIT.

---

Built by [Agent Shortlist](https://agentshortlist.com/claude-usage). Not affiliated with Anthropic. "Claude" and "Claude Code" are trademarks of Anthropic.
