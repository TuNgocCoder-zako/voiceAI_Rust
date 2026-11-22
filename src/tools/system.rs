use chrono::{Datelike, Local, Timelike};
use std::fs;
use std::process::{Command, Stdio};

pub fn is_hyprland() -> bool {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}

pub fn get_vietnamese_time() -> String {
    let now = Local::now();
    format!("Bây giờ là {} giờ {} phút.", now.hour(), now.minute())
}

pub fn get_vietnamese_date() -> String {
    let now = Local::now();
    let weekday_str = match now.weekday() {
        chrono::Weekday::Mon => "Thứ Hai",
        chrono::Weekday::Tue => "Thứ Ba",
        chrono::Weekday::Wed => "Thứ Tư",
        chrono::Weekday::Thu => "Thứ Năm",
        chrono::Weekday::Fri => "Thứ Sáu",
        chrono::Weekday::Sat => "Thứ Bảy",
        chrono::Weekday::Sun => "Chủ Nhật",
    };
    format!(
        "Hôm nay là {}, ngày {} tháng {} năm {}.",
        weekday_str,
        now.day(),
        now.month(),
        now.year()
    )
}

pub fn get_battery_info() -> String {
    for entry in walkdir::WalkDir::new("/sys/class/power_supply")
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let cap_file = path.join("capacity");
        let status_file = path.join("status");
        if cap_file.exists() {
            if let Ok(cap_raw) = fs::read_to_string(&cap_file) {
                let cap = cap_raw.trim();
                let status = fs::read_to_string(&status_file)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| "Discharging".to_string());
                let vi_status = match status.as_str() {
                    "Charging" => "đang sạc pin",
                    "Full" => "đã sạc đầy",
                    _ => "đang dùng pin",
                };
                return format!("Pin hiện tại còn {} phần trăm, máy {}.", cap, vi_status);
            }
        }
    }
    "Không tìm thấy pin laptop hoặc máy tính đang dùng nguồn điện trực tiếp.".to_string()
}

pub fn get_clipboard_content() -> String {
    let commands: &[(&str, &[&str])] = &[
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-o", "-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--output"]),
    ];

    for &(bin, args) in commands {
        if let Ok(out) = Command::new(bin).args(args).output() {
            if out.status.success() {
                let content = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if content.is_empty() {
                    return "Bộ nhớ tạm hiện đang trống.".to_string();
                }
                // Giới hạn độ dài để TTS không đọc quá lâu nếu clipboard chứa log/code lớn
                let preview: String = content.chars().take(240).collect();
                let suffix = if content.chars().count() > 240 { "..." } else { "" };
                return format!("Nội dung trong bộ nhớ tạm là: {}{}", preview, suffix);
            }
        }
    }

    "Không thể đọc bộ nhớ tạm. Bạn hãy cài thêm wl-clipboard hoặc xclip trên Ubuntu nhé.".to_string()
}

pub fn get_cpu_info() -> String {
    let loadavg = fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(", "))
        .unwrap_or_else(|_| "0.5, 0.4, 0.3".to_string());

    let model = fs::read_to_string("/proc/cpuinfo")
        .map(|s| {
            for line in s.lines() {
                if line.starts_with("model name") {
                    if let Some(pos) = line.find(':') {
                        return line[pos + 1..].trim().to_string();
                    }
                }
            }
            "CPU Đa nhân".to_string()
        })
        .unwrap_or_else(|_| "CPU Đa nhân".to_string());

    format!("CPU {} đang hoạt động ổn định, mức tải trung bình là: {}.", model, loadavg)
}

pub fn get_system_hardware_info() -> String {
    let mut mem_total_kb = 16000000u64;
    let mut mem_avail_kb = 8000000u64;

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    mem_total_kb = parts[1].parse().unwrap_or(mem_total_kb);
                }
            } else if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    mem_avail_kb = parts[1].parse().unwrap_or(mem_avail_kb);
                }
            }
        }
    }

    let mem_used_gb = (mem_total_kb.saturating_sub(mem_avail_kb)) as f64 / 1024.0 / 1024.0;
    let mem_total_gb = mem_total_kb as f64 / 1024.0 / 1024.0;

    format!(
        "Bộ nhớ RAM đang sử dụng {:.1} GB trên tổng số {:.1} GB.",
        mem_used_gb, mem_total_gb
    )
}

pub fn get_network_speed_info() -> String {
    let output = Command::new("ping")
        .args(["-c", "1", "-W", "2", "8.8.8.8"])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(idx) = text.find("time=") {
                let rest = &text[idx + 5..];
                let time_ms = rest.split_whitespace().next().unwrap_or("0");
                return format!("Độ trễ mạng Internet là {} mili giây, kết nối ổn định.", time_ms);
            }
            return "Kết nối mạng Internet đang hoạt động bình thường.".to_string();
        }
    }
    "Hiện tại không thể kết nối tới Internet. Bạn hãy kiểm tra lại Wi-Fi hoặc dây mạng nhé.".to_string()
}

pub fn set_system_volume(percent: u32) {
    let pct_str = format!("{}%", percent.min(100));
    // Pipewire wpctl support
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &pct_str])
        .output();
}

pub fn adjust_system_volume(delta: &str) {
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", delta])
        .output();
}

pub fn toggle_system_mute() {
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .output();
}

pub fn adjust_brightness(delta: &str) -> bool {
    if let Ok(out) = Command::new("brightnessctl").args(["set", delta]).output() {
        if out.status.success() {
            return true;
        }
    }

    // Fallback chuẩn trên Ubuntu GNOME qua SettingsDaemon D-Bus (không cần quyền nhóm video)
    let method = if delta.contains('-') {
        "org.gnome.SettingsDaemon.Power.Screen.StepDown"
    } else {
        "org.gnome.SettingsDaemon.Power.Screen.StepUp"
    };

    Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.SettingsDaemon.Power",
            "--object-path",
            "/org/gnome/SettingsDaemon/Power",
            "--method",
            method,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn set_wifi_state(enable: bool) {
    let arg = if enable { "on" } else { "off" };
    let _ = Command::new("nmcli")
        .args(["radio", "wifi", arg])
        .output();
}

pub fn set_bluetooth_state(enable: bool) {
    let arg = if enable { "on" } else { "off" };
    let _ = Command::new("bluetoothctl")
        .args(["power", arg])
        .output();
}

pub fn set_asus_profile(profile: &str) -> String {
    let (ppd_name, label, sysfs_code) = match profile {
        "Quiet" => ("power-saver", "Yên tĩnh tiết kiệm pin", "2"),
        "Performance" => ("performance", "Hiệu năng cao Turbo", "1"),
        _ => ("balanced", "Cân bằng mượt mà", "0"),
    };

    // 1. Chuẩn Ubuntu GNOME (không cần quyền root)
    if let Ok(out) = Command::new("powerprofilesctl")
        .args(["set", ppd_name])
        .output()
    {
        if out.status.success() {
            return format!("Đã chuyển sang chế độ {} cho bạn.", label);
        }
    }

    // 2. Nếu máy có cài asusctl
    if let Ok(out) = Command::new("asusctl")
        .args(["profile", "-P", profile])
        .output()
    {
        if out.status.success() {
            return format!("Đã chuyển sang chế độ {} qua asusctl.", label);
        }
    }

    // 3. Fallback ghi sysfs trực tiếp (chỉ chạy nếu có quyền udev/root)
    let sys_path = "/sys/devices/platform/asus-nb-wmi/throttle_thermal_policy";
    if fs::write(sys_path, sysfs_code).is_ok() {
        return format!("Đã chuyển sang chế độ {} cho bạn.", label);
    }

    "Không thể đổi chế độ hiệu năng. Bạn hãy kiểm tra dịch vụ power-profiles-daemon trên Ubuntu nhé.".to_string()
}

pub fn lock_screen() {
    let ok = Command::new("loginctl")
        .arg("lock-session")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok {
        let _ = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.gnome.ScreenSaver",
                "--object-path",
                "/org/gnome/ScreenSaver",
                "--method",
                "org.gnome.ScreenSaver.Lock",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

pub fn take_screenshot(mode: &str) -> String {
    let dest = dirs::picture_dir()
        .map(|p| p.join("Screenshots"))
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Pictures/Screenshots"));
    let _ = fs::create_dir_all(&dest);
    let filename = format!("screenshot_{}.png", Local::now().format("%Y%m%d_%H%M%S"));
    let path = dest.join(filename);

    // 1. Hyprland / wlroots
    if is_hyprland() {
        let spawned = if mode == "region" {
            Command::new("sh")
                .arg("-c")
                .arg(format!("grim -g \"$(slurp)\" \"{}\"", path.display()))
                .spawn()
                .is_ok()
        } else {
            Command::new("grim").arg(&path).spawn().is_ok()
        };
        if spawned {
            return "Đã kích hoạt chụp ảnh màn hình cho bạn.".to_string();
        }
    }

    // 2. Ubuntu GNOME: ưu tiên gnome-screenshot nếu đã cài
    let mut gs = Command::new("gnome-screenshot");
    if mode == "region" {
        gs.arg("-a");
    }
    gs.arg("-f").arg(&path);
    if gs.stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok() {
        return "Đã kích hoạt chụp ảnh màn hình và lưu vào thư mục Pictures/Screenshots.".to_string();
    }

    // 3. Fallback chuẩn Wayland mọi bản Ubuntu GNOME qua XDG Desktop Portal (không cần cài thêm app)
    let interactive = if mode == "region" { "true" } else { "false" };
    let options = format!("{{'interactive': <{}>}}", interactive);
    let portal_ok = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Screenshot.Screenshot",
            "",
            &options,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok();

    if portal_ok {
        return "Đã mở công cụ chụp ảnh màn hình của Ubuntu cho bạn.".to_string();
    }

    "Không tìm thấy công cụ chụp màn hình. Bạn có thể cài thêm gnome-screenshot nhé.".to_string()
}

pub fn media_pause() {
    let _ = Command::new("playerctl").arg("pause").output();
}

pub fn media_play() {
    let _ = Command::new("playerctl").arg("play").output();
}

pub fn media_next() {
    let _ = Command::new("playerctl").arg("next").output();
}

pub fn media_previous() {
    let _ = Command::new("playerctl").arg("previous").output();
}

pub fn is_media_playing() -> bool {
    Command::new("playerctl")
        .arg("status")
        .output()
        .map(|out| {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            s.contains("playing")
        })
        .unwrap_or(false)
}

pub fn get_current_media_track() -> String {
    let out = Command::new("playerctl")
        .args(["metadata", "--format", "{{artist}} - {{title}}"])
        .output();

    if let Ok(o) = out {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                return format!("Bài hát đang phát là: {}.", s);
            }
        }
    }
    "Hiện tại máy không có bài hát nào đang phát qua trình đa phương tiện.".to_string()
}
