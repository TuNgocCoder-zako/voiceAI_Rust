use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn load_live_user_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    if let Ok(out) = Command::new("systemctl")
        .args(["--user", "show-environment"])
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some((k, v)) = line.split_once('=') {
                    env.insert(k.to_string(), v.to_string());
                }
            }
        }
    }
    env
}

fn find_xwayland_auth(runtime_dir: &str) -> Option<String> {
    if let Ok(entries) = std::fs::read_dir(runtime_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if s.starts_with(".mutter-Xwaylandauth.") {
                return Some(entry.path().to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn ensure_overlay_running() {
    let qml_check = Command::new("pgrep")
        .args(["-f", "VoiceOverlay.qml"])
        .output();
    if let Ok(out) = qml_check {
        if out.status.success() {
            return;
        }
    }

    let live_env = load_live_user_env();
    let runtime_dir = live_env
        .get("XDG_RUNTIME_DIR")
        .cloned()
        .or_else(|| std::env::var("XDG_RUNTIME_DIR").ok())
        .unwrap_or_else(|| "/run/user/1000".to_string());

    let display = live_env
        .get("DISPLAY")
        .cloned()
        .or_else(|| std::env::var("DISPLAY").ok());
    let wayland = live_env
        .get("WAYLAND_DISPLAY")
        .cloned()
        .or_else(|| std::env::var("WAYLAND_DISPLAY").ok());

    // Chưa vào phiên đồ họa hoàn chỉnh (mới boot) -> bỏ qua để vòng lặp 5s thử lại
    if display.is_none() && wayland.is_none() {
        return;
    }

    let is_gnome = live_env
        .get("XDG_CURRENT_DESKTOP")
        .cloned()
        .or_else(|| std::env::var("XDG_CURRENT_DESKTOP").ok())
        .unwrap_or_default()
        .to_lowercase()
        .contains("gnome")
        || Command::new("pgrep")
            .args(["-x", "gnome-shell"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

    let xauth = live_env
        .get("XAUTHORITY")
        .cloned()
        .or_else(|| std::env::var("XAUTHORITY").ok())
        .or_else(|| find_xwayland_auth(&runtime_dir));

    // Trên GNOME cần chạy XCB (XWayland) -> đợi đến khi DISPLAY sẵn sàng
    if is_gnome && display.is_none() {
        return;
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
        cmd.env("XDG_RUNTIME_DIR", &runtime_dir);

        if let Some(ref d) = display {
            cmd.env("DISPLAY", d);
        }
        if let Some(ref w) = wayland {
            cmd.env("WAYLAND_DISPLAY", w);
        }
        if let Some(ref xa) = xauth {
            cmd.env("XAUTHORITY", xa);
        }
        if let Some(dbus) = live_env.get("DBUS_SESSION_BUS_ADDRESS") {
            cmd.env("DBUS_SESSION_BUS_ADDRESS", dbus);
        }
        if is_gnome {
            cmd.env("QT_QPA_PLATFORM", "xcb");
        }

        match cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                println!(
                    "🖥️ Đã khởi chạy Quickshell Overlay (display={:?}, gnome={}): {:?}",
                    display, is_gnome, qml_path
                );
                // Thu dọn tiến trình nếu qs thoát sớm để tránh sinh zombie
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
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

    // 1. Cấu hình volume PipeWire cho micro cơ sở (Built-in Audio)
    // Mức 0.20 tương ứng khoảng +18dB trên ALSA, vừa đủ thu nhạy mà tuyệt đối không kích hoạt Boost làm rè
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
                    // Bỏ qua nguồn EchoCancel hoặc VoiceAI để không kéo ngược volume của EchoCancel xuống
                    if line.contains("EchoCancel") || line.contains("VoiceAI") {
                        continue;
                    }
                    if let Some(token) = line.split('.').next() {
                        let id = token.trim().trim_start_matches(|c: char| !c.is_ascii_digit());
                        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                            let _ = Command::new("wpctl").args(["set-volume", id, "0.20"]).output();
                            let _ = Command::new("wpctl").args(["set-mute", id, "0"]).output();
                        }
                    }
                }
            }
        }
    }

    // 2. Khóa cứng ALSA mixer để chống vỡ tiếng ADC Realtek và triệt tiêu tiếng ù cơ học 50-70Hz
    let _ = Command::new("amixer")
        .args(["-c", &card, "sset", "Internal Mic Boost", "0"])
        .output();
    let _ = Command::new("amixer")
        .args(["-c", &card, "sset", "Capture", "40"])
        .output();

    // 3. Đồng bộ mức âm lượng 1.0 và bỏ mute cho PipeWire EchoCancel_Source
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SOURCE@", "1.0"])
        .output();
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "0"])
        .output();
}
