# voxtype-tools

Companion utilities for [voxtype](https://github.com/peteonrails/voxtype) (push-to-talk voice-to-text for Linux).

**⚠️ This entire thing is vibe coded.** Built in one session with an AI assistant. It works on my machine. No tests, no CI, no guarantees. Use at your own risk.

## Crates

### voxtype-tray

KDE system tray icon for voxtype. Uses the StatusNotifierItem (SNI) protocol via [ksni](https://github.com/iovxw/ksni).

- Tray icon changes based on state (idle/recording/transcribing)
- Left-click to toggle recording
- Right-click menu: status, toggle recording, restart daemon, quit
- Auto-reconnects when voxtype daemon restarts
- Uses freedesktop icon names (no embedded assets)

Consumes `voxtype status --follow --format json` for state updates.

### voxtype-hotkey

Minimal evdev hotkey helper that provides push-to-talk without adding your user to the `input` group. Designed to run as a system service with `SupplementaryGroups=input` while voxtype itself runs as a user service with full session access.

- Watches `/dev/input` for key press/release events
- Calls `voxtype record start` / `voxtype record stop`
- Configurable key (`--key CAPSLOCK`) and post-release tail (`--tail-ms 300`)
- Auto-detects keyboards, handles device hotplug

## Install

```bash
git clone https://github.com/caspark/voxtype-tools
cd voxtype-tools
cargo install --path crates/voxtype-tray
cargo install --path crates/voxtype-hotkey
```

## Setup

### voxtype-tray (user service)

```ini
# ~/.config/systemd/user/voxtype-tray.service
[Unit]
Description=Voxtype system tray icon
PartOf=graphical-session.target
After=graphical-session.target voxtype.service

[Service]
Type=simple
Environment="PATH=%h/.cargo/bin:%h/.local/bin:/usr/local/bin:/usr/bin:/bin"
ExecStart=%h/.cargo/bin/voxtype-tray
Restart=on-failure
RestartSec=5

[Install]
WantedBy=graphical-session.target
```

```bash
systemctl --user enable --now voxtype-tray
```

### voxtype-hotkey (system service)

Runs as a system service so it can have `input` group access without your user being in the group.

Requires voxtype to be configured with `[hotkey] enabled = false` (voxtype-hotkey replaces the built-in evdev listener).

```ini
# /etc/systemd/system/voxtype-hotkey.service
[Unit]
Description=Voxtype hotkey helper (evdev push-to-talk)
After=graphical.target

[Service]
Type=simple
User=YOUR_USER
SupplementaryGroups=input
Environment="PATH=/home/YOUR_USER/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
Environment="XDG_RUNTIME_DIR=/run/user/YOUR_UID"
ExecStart=/home/YOUR_USER/.cargo/bin/voxtype-hotkey --key CAPSLOCK --tail-ms 300
Restart=on-failure
RestartSec=5

[Install]
WantedBy=graphical.target
```

```bash
sudo systemctl enable --now voxtype-hotkey
```

Supported keys: `CAPSLOCK`, `SCROLLLOCK`, `PAUSE`, `INSERT`, `NUMLOCK`, `F13`-`F24`, `RIGHTALT`, `RIGHTCTRL`, `RIGHTSHIFT`, `RIGHTMETA`.

The `--tail-ms` option keeps recording for a short time after key release to avoid cutting off the last word. Default: 300ms.

## License

MIT
