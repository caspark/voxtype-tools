use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

fn find_keyboards() -> HashMap<PathBuf, Device> {
    let mut devices = HashMap::new();

    let input_dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Cannot read /dev/input: {e}");
            return devices;
        }
    };

    for entry in input_dir.flatten() {
        let path = entry.path();
        let is_event = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("event"))
            .unwrap_or(false);

        if !is_event {
            continue;
        }

        if let Ok(device) = Device::open(&path) {
            let has_keys = device
                .supported_keys()
                .map(|keys| {
                    keys.contains(Key::KEY_A)
                        && keys.contains(Key::KEY_Z)
                        && keys.contains(Key::KEY_ENTER)
                })
                .unwrap_or(false);

            if has_keys {
                // Set non-blocking
                let fd = device.as_raw_fd();
                unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL);
                    if flags != -1 {
                        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                    }
                }
                eprintln!("  {:?} ({})", path, device.name().unwrap_or("unknown"));
                devices.insert(path, device);
            }
        }
    }

    devices
}

fn resolve_key(name: &str) -> Key {
    match name.to_uppercase().as_str() {
        "CAPSLOCK" => Key::KEY_CAPSLOCK,
        "SCROLLLOCK" => Key::KEY_SCROLLLOCK,
        "PAUSE" => Key::KEY_PAUSE,
        "INSERT" => Key::KEY_INSERT,
        "NUMLOCK" => Key::KEY_NUMLOCK,
        "F13" => Key::KEY_F13,
        "F14" => Key::KEY_F14,
        "F15" => Key::KEY_F15,
        "F16" => Key::KEY_F16,
        "F17" => Key::KEY_F17,
        "F18" => Key::KEY_F18,
        "F19" => Key::KEY_F19,
        "F20" => Key::KEY_F20,
        "F21" => Key::KEY_F21,
        "F22" => Key::KEY_F22,
        "F23" => Key::KEY_F23,
        "F24" => Key::KEY_F24,
        "RIGHTALT" => Key::KEY_RIGHTALT,
        "RIGHTCTRL" => Key::KEY_RIGHTCTRL,
        "RIGHTSHIFT" => Key::KEY_RIGHTSHIFT,
        "RIGHTMETA" => Key::KEY_RIGHTMETA,
        other => {
            eprintln!("Unknown key: {other}");
            eprintln!("Supported: CAPSLOCK, SCROLLLOCK, PAUSE, INSERT, NUMLOCK, F13-F24, RIGHTALT, RIGHTCTRL, RIGHTSHIFT, RIGHTMETA");
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut key_name = "CAPSLOCK".to_string();
    let mut tail_ms: u64 = 300;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--key" => key_name = args.next().unwrap_or_else(|| "CAPSLOCK".into()),
            "--tail-ms" => {
                tail_ms = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(300);
            }
            _ => {}
        }
    }

    let target_key = resolve_key(&key_name);

    eprintln!(
        "Watching for {:?} on keyboards (tail: {}ms):",
        key_name.to_uppercase(),
        tail_ms
    );
    let mut devices = find_keyboards();

    if devices.is_empty() {
        eprintln!("No keyboards found. Is the service running with input group access?");
        std::process::exit(1);
    }

    let mut is_pressed = false;

    loop {
        let mut error_paths = Vec::new();

        for (path, device) in &mut devices {
            match device.fetch_events() {
                Ok(events) => {
                    for ev in events {
                        if let InputEventKind::Key(key) = ev.kind() {
                            if key != target_key {
                                continue;
                            }
                            match ev.value() {
                                1 if !is_pressed => {
                                    is_pressed = true;
                                    eprintln!("Key down — starting recording");
                                    let _ = Command::new("voxtype")
                                        .args(["record", "start"])
                                        .status();
                                }
                                0 if is_pressed => {
                                    is_pressed = false;
                                    if tail_ms > 0 {
                                        eprintln!("Key up — waiting {}ms then stopping", tail_ms);
                                        std::thread::sleep(std::time::Duration::from_millis(tail_ms));
                                    } else {
                                        eprintln!("Key up — stopping recording");
                                    }
                                    let _ = Command::new("voxtype")
                                        .args(["record", "stop"])
                                        .status();
                                }
                                _ => {} // repeat (2) or duplicate, ignore
                            }
                        }
                    }
                }
                Err(ref e) if e.raw_os_error() == Some(libc::ENODEV) => {
                    eprintln!("Device gone: {:?}", path);
                    error_paths.push(path.clone());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No events, normal for non-blocking
                }
                Err(_) => {}
            }
        }

        for path in error_paths {
            devices.remove(&path);
        }

        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
