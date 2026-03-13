# AGENTS.md — voxtype-tools

## Project Overview

Companion utilities for [voxtype](https://github.com/peteonrails/voxtype), a push-to-talk voice-to-text tool for Linux. This is a Rust workspace with two crates. The project is self-described as "vibe coded" — no tests, no CI.

## Workspace Layout

```
Cargo.toml              # Workspace root (resolver = "2")
crates/
  voxtype-hotkey/       # Minimal evdev push-to-talk hotkey daemon
    Cargo.toml          # deps: evdev 0.12, libc 0.2
    src/main.rs          # Single-file binary (~150 lines)
  voxtype-tray/         # KDE system tray icon for voxtype
    Cargo.toml          # deps: ksni 0.3.3 (tokio), tokio 1, serde_json 1, dirs 6
    src/main.rs          # Single-file binary (~250 lines)
```

## Crate Details

### voxtype-hotkey

A system service that reads evdev input events to provide push-to-talk without adding the user to the `input` group. Runs as a **system** systemd service (`/etc/systemd/system/voxtype-hotkey.service`) with `SupplementaryGroups=input`.

**How it works:**
- Scans `/dev/input/event*` for devices that look like keyboards (have KEY_A, KEY_Z, KEY_ENTER)
- Sets file descriptors to non-blocking via `fcntl`
- Polls devices in a tight loop (5ms sleep) for target key press/release
- On key down: runs `voxtype record start`
- On key up: sleeps `--tail-ms` then runs `voxtype record stop`
- Handles device hotplug (removes devices that return ENODEV)
- No async runtime — pure synchronous polling with `std::thread::sleep`

**CLI args:** `--key <KEY_NAME>` (default: CAPSLOCK), `--tail-ms <ms>` (default: 300)

**Supported keys:** CAPSLOCK, SCROLLLOCK, PAUSE, INSERT, NUMLOCK, F13–F24, RIGHTALT, RIGHTCTRL, RIGHTSHIFT, RIGHTMETA

**Deployed config:** `/etc/systemd/system/voxtype-hotkey.service` — runs as `ckrieger`, key=CAPSLOCK, tail-ms=500, binary at `/usr/local/bin/voxtype-hotkey`.

### voxtype-tray

A KDE system tray icon using the StatusNotifierItem (SNI) protocol via `ksni`. Runs as a **user** systemd service.

**How it works:**
- Spawns `voxtype status --follow --format json` and parses JSON lines for state changes
- Maps state (`idle`/`recording`/`transcribing`) to freedesktop icon names and tray status
- Left-click: toggle recording (`voxtype record toggle`)
- Right-click menu: status display, toggle recording, edit config (xdg-open), config reference (opens GitHub docs URL), restart daemon (`systemctl --user restart voxtype`), quit
- Auto-reconnects if voxtype daemon exits (2s retry)
- Uses `tokio` single-threaded runtime

**States → Icons:**
| State | Icon | Tray Status |
|-------|------|-------------|
| Idle | `audio-input-microphone` | Passive |
| Recording | `media-record` | NeedsAttention |
| Transcribing | `preferences-desktop-locale` | Active |
| Unknown | `dialog-question` | Passive |

## Build & Install

```bash
cargo build --release
# Hotkey (needs root-owned location for system service):
sudo install -Dm755 target/release/voxtype-hotkey /usr/local/bin/voxtype-hotkey
# Tray:
cargo install --path crates/voxtype-tray
```

After rebuilding voxtype-hotkey, restart the system service:
```bash
sudo systemctl restart voxtype-hotkey
```

## Key Conventions

- No tests, no CI — manual verification only
- Both crates are single-file binaries (`src/main.rs` only)
- No shared code between crates
- External state comes entirely from the `voxtype` CLI tool
- Freedesktop icon names used (no embedded image assets)
