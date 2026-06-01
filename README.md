# kodik-mpv-plugin

MPV plugin that resolves indirect video links into direct playback URLs. Works with [Shikimori](https://shikimori.one) — open a Shikimori anime page in MPV and it expands into a full playlist, playing episodes through Kodik.

Part of the **kodik** Rust workspace: `kodik-rs` (CLI), `kodik-parser`, `kodik-utils`, `kodik-shiki`, and `kodik-mpv-plugin`.

## Installation

### Prerequisites

- Rust 2024 edition
- MPV media player

### Build

```bash
cargo build --release -p kodik-mpv-plugin
```

Output shared library lands in `target/release/` (`kodik.dll` on Windows, `kodik.so` on Linux, `kodik.dylib` on macOS).

### Setup

1. Copy the built shared library to MPV's `scripts/` directory:
   - Linux/macOS: `~/.config/mpv/scripts/`
   - Windows: `%APPDATA%\mpv\scripts\`

2. MPV auto-loads `.so` / `.dll` files in that directory.

3. Create a config file at `~~/script-opts/kodik.conf` (MPV `scripts-opts` directory):

   ```ini
   # Video quality: 360, 480, or 720 (default: 720)
   quality=720

   # Netscape-format cookie file for Shikimori auth
   # Export from Firefox: Tools > Cookies > export as Netscape
   cookies=~/cookies.txt

   # Filter translations by title (regex)
   translation_title=

   # Filter translations by type: voice or subtitles
   translation_type=

   # Expand Shikimori URLs into playlist: all, essential, or none
   related_mode=none

   # Log level: off, error, warn, info, debug, trace
   log_level=error
   ```

### Key Bindings

Add to your MPV `input.conf`:

```
Ctrl+ENTER script-binding "kodik/watched"
```

Or use `input.conf` shipped with the plugin (see `input.conf` in the repo).
