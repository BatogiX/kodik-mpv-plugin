# kodik-mpv-plugin

MPV plugin that resolves indirect video links into direct playback URLs. Works and sync (with cookies) with [Shikimori](https://shikimori.one), playing episodes through Kodik.

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

   (MPV auto-loads `.so` / `.dll` files in that directory.) 

2. (Optional) Create a config file at `script-opts/kodik.conf`:

   ```ini
    # Specify video quality [default: 720] [possible values: 360, 480, 720]
    quality=720
    
    # Netscape formatted file to read cookies from (need for media database integration)
    # cookies=~/cookies.txt
    
    # Specify translation title (regex)
    # translation_title=Subtitles
    
    # Specify translation type [possible values: voice, subtitles] (fallback if translation_title not found)
    # translation_type=voice
    
    # Expand a media database URL into all related URLs [default: none] [possible values: all, essential, none]
    related_mode=none
   ```

### Usage

mpv [https://kodikplayer.com/video/91875/013ac13bfd06b08fabaefddce91a7107/720p](https://kodikplayer.com/video/91875/013ac13bfd06b08fabaefddce91a7107/720p)

mpv [https://shikimori.one/animes/z12345-anime-title](https://shikimori.one/animes/z12345-anime-title)

### Key Bindings

Add to your MPV `input.conf`:

```
Ctrl+ENTER script-binding "kodik/watched"
```

Or use `input.conf` shipped with the plugin (see `input.conf` in the repo).

### Android Installation Guide

To use this plugin on mpv-android, you will need to manually inject the library into the APK package.

1. Download Assets: Download the latest official mpv-android APK and an APK editor/decompiler tool of your choice.
2. Decompile the APK: Use the editor tool to decompile the mpv-android APK.
3. Inject the Plugin: Copy the Android-compiled version of this plugin (kodik.so) into the following path inside the decompiled structure: ```lib/arm64-v8a/kodik.so```
4. Recompile & Sign: Rebuild, sign, and install the modified APK.
5. Enable the Script: Open mpv, navigate to Settings ➔ Advanced ➔ Edit mpv.conf, and append the following line: ```script=kodik.so```
7. (Optional) Cookie Setup: To use cookies, place your cookies.txt file in the application's media folder and link it via your mpv.conf: ```script-opts=kodik-cookies=/sdcard/Android/media/is.xyz.mpv/cookies.txt```
