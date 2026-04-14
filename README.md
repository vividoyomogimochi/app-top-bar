# LED AppBar

**A scrolling LED dot-matrix ticker that lives at the top edge of your Windows desktop.**

LED AppBar pins a dot-matrix LED display across the very top of your screen. It is not an always-on-top floating window — it registers itself as a real Windows **AppBar** via `SHAppBarMessage`, the same mechanism the taskbar uses, so maximized windows respect the bar's space instead of overlapping it.

The content shown in the bar is a normal web page rendered inside WebView2. By default it displays an RSS/WebSocket-driven news ticker ([led-news-ticker](https://github.com/vividoyomogimochi/led-news-ticker)), but the source URL is configurable — you can point it at any web page.

**Download:** [bunnyyears.itch.io/led-appbar](https://bunnyyears.itch.io/led-appbar) (pay-what-you-want)
**Japanese README:** [README.ja.md](README.ja.md)

## Features

- Registered as a real AppBar — maximized apps will not overlap the bar
- Multi-monitor support (choose the host monitor from the tray menu)
- Adjustable height (40–120 px, 5 presets)
- Auto-hides when another app enters fullscreen (`ABN_FULLSCREENAPP`)
- Auto-start with Windows via the registry Run key
- Optional co-launched server process (e.g. an SSE/WebSocket feed server) that starts and stops with the app, auto-restarted on crash
- All configuration is available from the system tray menu

## Requirements

- Windows 10 or 11
- WebView2 Runtime (preinstalled on virtually all modern Windows machines)

## How it works

The bar itself is a frameless Tauri (Rust) window. At startup it sends `ABM_NEW` followed by `ABM_SETPOS` to register itself with the shell as an AppBar attached to the top edge of the selected monitor. The shell responds by reserving that screen area, so maximized windows automatically exclude it from their client region — exactly how the taskbar behaves.

The window content is a local HTML page loaded into WebView2. Inside that page, the LED dot-matrix animation is drawn on a `<canvas>` using a precompiled bitmap font atlas of roughly 22,000 glyphs (covering ASCII, JIS-X 0208, and common symbols). Using a pre-rendered atlas instead of runtime font rendering keeps the dot pattern identical across machines and avoids font-renderer smoothing artifacts.

Fullscreen detection uses the standard `ABN_FULLSCREENAPP` AppBar notification — when another app goes fullscreen on the same monitor, the bar hides itself and comes back when fullscreen exits.

The optional companion server process is supervised by the main app: it is launched with a configured command, its stdout/stderr are captured, and it is automatically restarted if it exits unexpectedly. This is useful for running a local feed server (RSS aggregator, WebSocket relay, etc.) alongside the bar.

## Building from source

Requires Node.js, Rust, and Visual Studio Build Tools (C++).

```powershell
npm install
npm run tauri dev    # development mode
npm run tauri build  # release build (produces MSI / NSIS installers)
```

## Configuration

Settings are stored in `%APPDATA%/app-top-bar/config.json`:

```json
{
  "bar_height": 80,
  "monitor": 0,
  "auto_start": true,
  "url": "https://ticker.samoyed.moe/ticker/",
  "auto_hide_fullscreen": true,
  "server_command": null
}
```

All fields are editable from the tray menu UI — you don't normally need to touch the file directly.

## Default widget: LED News Ticker

The default content source is [led-news-ticker](https://github.com/vividoyomogimochi/led-news-ticker), a browser-based LED dot-matrix news ticker that ingests RSS feeds and WebSocket streams. Themes and color schemes are customizable from its built-in config page (`/config`).

To use a different widget, change the `url` field in `config.json` or edit it from the tray menu. Any web page will work.

## License

MIT. See [LICENSE](LICENSE).
