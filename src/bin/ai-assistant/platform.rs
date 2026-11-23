use std::path::Path;
use std::process::Command;

pub fn ensure_overlay_running() {
    let qml_check = Command::new("pgrep")
        .args(["-f", "VoiceOverlay.qml"])
        .output();
    if let Ok(out) = qml_check {
        if out.status.success() {
            return;
        }
    }

    let home = dirs::home_dir().unwrap_or_default();
    let qml_candidates = [
        home.join("Projects/rust/voice-ai/overlay/VoiceOverlay.qml"),
        std::env::current_dir()
            .unwrap_or_default()
            .join("overlay/VoiceOverlay.qml"),
        home.join(".config/voice-ai/overlay/VoiceOverlay.qml"),
    ];

    let qs_bin = if Path::new("/usr/bin/qs").exists() {
        Some("/usr/bin/qs")
    } else if Path::new("/usr/bin/quickshell").exists() {
        Some("/usr/bin/quickshell")
    } else {
        None
    };

    if let (Some(qs), Some(qml_path)) = (qs_bin, qml_candidates.iter().find(|p| p.exists())) {
        let mut cmd = Command::new(qs);
        cmd.args(["-p", qml_path.to_str().unwrap_or_default()]);

        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase()
            .contains("gnome");
        if is_gnome {
            cmd.env("QT_QPA_PLATFORM", "xcb");
        }
        if std::env::var("DISPLAY").is_err() {
            cmd.env("DISPLAY", ":0");
        }

        match cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                println!(
                    "🖥️ Đã khởi chạy giao diện Quickshell Dynamic Island Overlay: {:?}",
                    qml_path
                );
                return;
            }
            Err(e) => eprintln!("⚠️ Lỗi khởi chạy Quickshell overlay: {}", e),
        }
    }

    let check = Command::new("pgrep")
        .args(["-x", "voice-overlay"])
        .output();
    if let Ok(out) = check {
        if !out.status.success() {
            let candidates = [
                home.join(".local/bin/voice-overlay"),
                std::env::current_dir()
                    .unwrap_or_default()
                    .join("target/release/voice-overlay"),
                home.join("Projects/rust/voice-ai/target/release/voice-overlay"),
            ];
            if let Some(bin) = candidates.iter().find(|p| p.exists()) {
                let _ = Command::new(bin)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
    }
}

pub fn detect_internal_mic_card() -> String {
    if let Ok(cards) = std::fs::read_to_string("/proc/asound/cards") {
        for line in cards.lines() {
            let trimmed = line.trim();
            if let Some(first_token) = trimmed.split_whitespace().next() {
                if first_token.chars().all(|c| c.is_ascii_digit()) {
                    let lower = trimmed.to_lowercase();
                    if lower.contains("pch")
                        || lower.contains("alc")
                        || lower.contains("realtek")
                        || lower.contains("sof-hda")
                    {
                        return first_token.to_string();
                    }
                }
            }
        }
    }
    "0".to_string()
}

pub fn ensure_mic_levels() {
    let card = detect_internal_mic_card();
    // Bật phần cứng Internal Mic Boost trên ALSA nếu có
    let _ = Command::new("amixer")
        .args(["-c", &card, "sset", "Internal Mic Boost", "1"])
        .output();

    // Nâng gain cho micro phần cứng cơ sở trong PipeWire (chống bị giảm âm lượng nguồn)
    if let Ok(out) = Command::new("wpctl").arg("status").output() {
        if let Ok(text) = String::from_utf8(out.stdout) {
            let mut in_sources = false;
            for line in text.lines() {
                if line.contains("Sources:") {
                    in_sources = true;
                    continue;
                }
                if in_sources && (line.contains("Filters:") || line.contains("Streams:") || line.contains("Sinks:") || line.contains("Video")) {
                    in_sources = false;
                }
                if in_sources && line.contains("[vol:") {
                    // Bỏ qua nguồn EchoCancel hoặc VoiceAI để không kéo ngược volume của EchoCancel xuống 0.85
                    if line.contains("EchoCancel") || line.contains("VoiceAI") {
                        continue;
                    }
                    if let Some(token) = line.split('.').next() {
                        let id = token.trim().trim_start_matches(|c: char| !c.is_ascii_digit());
                        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                            let _ = Command::new("wpctl").args(["set-volume", id, "0.85"]).output();
                            let _ = Command::new("wpctl").args(["set-mute", id, "0"]).output();
                        }
                    }
                }
            }
        }
    }

    // Đồng bộ mức âm lượng 1.0 và bỏ mute cho PipeWire EchoCancel_Source (chạy sau cùng để đảm bảo không bị ghi đè)
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SOURCE@", "1.0"])
        .output();
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "0"])
        .output();
}
