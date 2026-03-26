<p align="center">
  <img src="assets/logo.png" alt="Polterdesk" width="128" height="128" />
</p>

<h1 align="center">Polterdesk</h1>

<p align="center">
  Toggle desktop icon and taskbar visibility on Windows with a double-click or hotkey.<br />
  A tiny Windows utility that stays out of your way.
</p>

<p align="center">
  <a href="https://github.com/EternityX/polterdesk/releases">Download</a> &middot;
  <a href="https://github.com/EternityX/polterdesk/issues">Report a Bug</a>
</p>


<div align="center">

[![CI](https://github.com/EternityX/polterdesk/actions/workflows/ci.yml/badge.svg?branch=master&event=push)](https://github.com/EternityX/polterdesk/actions/workflows/ci.yml)

</div>

---

<p align="center">
  <img src="https://github.com/EternityX/polterdesk/blob/master/assets/image.png" alt="Polterdesk" width="512" height="512" />
</p>

Inspired by [Stardock Fences'](https://www.stardock.com/products/fences/) double-click to hide/show desktop icons feature. **Polterdesk** is a lightweight, open-source, and most importantly, free alternative to that feature.

## Features

- **Double-click** an empty area of your desktop to hide/show all icons
- **Global hotkey** (default `Ctrl+Alt+H`) works from any application
- **Taskbar auto-hide** toggles alongside icons (configurable)
- **Click blocking** prevents accidentally opening files when icons are hidden
- **Tray icon** with right-click menu for quick access

## Requirements

- Windows 10 (1903+) or Windows 11
- [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe) (if not already installed)

## Usage

1. Download `polterdesk.exe` from the [releases page](https://github.com/EternityX/polterdesk/releases)
2. Run it - a tray icon appears in the notification area
3. **Double-click** the desktop to hide icons, double-click again to show them
4. Or press **Ctrl+Alt+H** from anywhere

### Settings

Right-click the tray icon and select **Open** (or single-click) to access settings:

- **Hotkey** - click the field and press a new key combination to change it
- **Start with Windows** - launch automatically on login
- **Also hide taskbar** - toggle the taskbar alongside icons

### Planned
- macOS support

## Building from Source

### Prerequisites

- Rust stable (>= 1.85) with `x86_64-pc-windows-msvc` target
- Visual Studio Build Tools 2022 (MSVC v143)
- Windows SDK 10.0.22621.0+

### Build

```powershell
cargo build --release
```

The binary is at `target\release\polterdesk.exe`.

### Run Tests

```powershell
cargo test --lib          # unit tests
cargo test                # all tests (requires Windows desktop)
```

## License

MIT
