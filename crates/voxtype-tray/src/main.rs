use std::process::Stdio;

use ksni::TrayMethods;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
enum VoxtypeState {
    Idle,
    Recording,
    Transcribing,
    Unknown,
}

impl VoxtypeState {
    fn from_alt(alt: &str) -> Self {
        match alt {
            "idle" => Self::Idle,
            "recording" => Self::Recording,
            "transcribing" => Self::Transcribing,
            _ => Self::Unknown,
        }
    }

    fn icon_name(&self) -> &str {
        match self {
            Self::Idle => "audio-input-microphone",
            Self::Recording => "media-record",
            Self::Transcribing => "preferences-desktop-locale",
            Self::Unknown => "dialog-question",
        }
    }

    fn tooltip(&self) -> &str {
        match self {
            Self::Idle => "Voxtype: ready",
            Self::Recording => "Voxtype: recording...",
            Self::Transcribing => "Voxtype: transcribing...",
            Self::Unknown => "Voxtype: unknown state",
        }
    }

    fn status(&self) -> ksni::Status {
        match self {
            Self::Idle => ksni::Status::Passive,
            Self::Recording => ksni::Status::NeedsAttention,
            Self::Transcribing => ksni::Status::Active,
            Self::Unknown => ksni::Status::Passive,
        }
    }
}

struct VoxtypeTray {
    state: VoxtypeState,
    notifier: mpsc::UnboundedSender<TrayAction>,
}

enum TrayAction {
    ToggleRecording,
    RestartDaemon,
    Quit,
}

impl std::fmt::Debug for VoxtypeTray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxtypeTray")
            .field("state", &self.state)
            .finish()
    }
}

impl ksni::Tray for VoxtypeTray {
    fn id(&self) -> String {
        "voxtype-tray".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "Voxtype".into()
    }

    fn icon_name(&self) -> String {
        self.state.icon_name().into()
    }

    fn status(&self) -> ksni::Status {
        self.state.status()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.state.tooltip().into(),
            description: String::new(),
            icon_name: self.state.icon_name().into(),
            icon_pixmap: vec![],
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.notifier.send(TrayAction::ToggleRecording);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let state_label = match &self.state {
            VoxtypeState::Idle => "Status: Ready",
            VoxtypeState::Recording => "Status: Recording",
            VoxtypeState::Transcribing => "Status: Transcribing",
            VoxtypeState::Unknown => "Status: Unknown",
        };

        vec![
            StandardItem {
                label: state_label.into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Toggle Recording".into(),
                icon_name: "media-record".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.notifier.send(TrayAction::ToggleRecording);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Restart Daemon".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.notifier.send(TrayAction::RestartDaemon);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit Tray".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.notifier.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn `voxtype status --follow --format json` and yield state strings.
async fn spawn_status_follower(tx: mpsc::UnboundedSender<VoxtypeState>) {
    loop {
        let result = Command::new("voxtype")
            .args(["status", "--follow", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match result {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to spawn voxtype status: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(alt) = json.get("alt").and_then(|v| v.as_str()) {
                    let state = VoxtypeState::from_alt(alt);
                    if tx.send(state).is_err() {
                        // Receiver dropped, exit
                        let _ = child.kill().await;
                        return;
                    }
                }
            }
        }

        // Process exited — daemon probably restarted. Wait and retry.
        let _ = child.wait().await;
        eprintln!("voxtype status exited, reconnecting in 2s...");
        let _ = tx.send(VoxtypeState::Unknown);
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (action_tx, mut action_rx) = mpsc::unbounded_channel();
    let (state_tx, mut state_rx) = mpsc::unbounded_channel();

    let tray = VoxtypeTray {
        state: VoxtypeState::Unknown,
        notifier: action_tx,
    };

    let handle = tray.spawn().await.expect("failed to create tray icon");

    // Spawn the status follower
    tokio::spawn(spawn_status_follower(state_tx));

    loop {
        tokio::select! {
            Some(state) = state_rx.recv() => {
                handle.update(|tray: &mut VoxtypeTray| {
                    tray.state = state;
                }).await;
            }
            Some(action) = action_rx.recv() => {
                match action {
                    TrayAction::ToggleRecording => {
                        let _ = Command::new("voxtype")
                            .args(["record", "toggle"])
                            .status()
                            .await;
                    }
                    TrayAction::RestartDaemon => {
                        let _ = Command::new("systemctl")
                            .args(["--user", "restart", "voxtype"])
                            .status()
                            .await;
                    }
                    TrayAction::Quit => {
                        break;
                    }
                }
            }
            else => break,
        }
    }
}
