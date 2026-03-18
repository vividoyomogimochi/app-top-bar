# LED AppBar

Windows desktop top bar that displays a web page (LED news ticker) at the top edge of the screen, reserving work area so maximized windows stay below it — just like the taskbar.

## Features

- Displays any URL in a frameless, always-on-top WebView2 window
- Reserves screen space via Windows AppBar API (`SHAppBarNotify`)
- System tray with monitor selection, bar height presets, and auto-start toggle
- Persistent configuration saved to `%APPDATA%/app-top-bar/config.json`
- Auto-start on Windows login (registry-based)
- Multi-monitor support with saved preference

## Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with "C++ desktop development" workload
- WebView2 Runtime (pre-installed on Windows 10/11)

### Build & Run

```powershell
git clone https://github.com/vividoyomogimochi/app-top-bar.git
cd app-top-bar
npm install
npm run tauri dev      # development
npm run tauri build    # release (outputs MSI + NSIS installer)
```

## Configuration

Settings are stored in `%APPDATA%/app-top-bar/config.json`:

```json
{
  "bar_height": 80,
  "monitor": 0,
  "auto_start": true,
  "url": "https://ticker.samoyed.moe/ticker/"
}
```

All settings except `url` can be changed from the system tray menu.

## System Tray Menu

- **Monitor** — select which display to attach the bar to
- **Bar Height** — 40 / 60 / 80 / 100 / 120 px
- **Auto Start** — toggle Windows startup registration
- **Quit** — unregister AppBar and exit

## Known Issues

- On monitors with a taskbar, the bar may appear with a slight vertical offset on first launch. Changing the bar height from the tray menu corrects it.

## License

MIT - see [LICENSE](LICENSE)
