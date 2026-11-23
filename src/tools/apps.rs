use crate::tools::system::is_hyprland;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub desktop_file: PathBuf,
    #[serde(default)]
    pub name_vi: Option<String>,
}

pub type AppIndex = HashMap<String, DesktopApp>;

pub fn build_app_index() -> AppIndex {
    let mut index = HashMap::new();
    let mut search_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ];

    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".local/share/applications"));
        search_dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("desktop") {
                if let Some(app) = parse_desktop_file(path) {
                    let key = app.name.to_lowercase();
                    index.entry(key).or_insert_with(|| app.clone());

                    if let Some(ref vi) = app.name_vi {
                        index.entry(vi.to_lowercase()).or_insert_with(|| app.clone());
                    }

                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let stem_key = stem.to_lowercase();
                        index.entry(stem_key).or_insert_with(|| app.clone());
                    }
                }
            }
        }
    }

    index
}

fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut name_vi = None;
    let mut exec = None;
    let mut icon = String::new();
    let mut no_display = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        } else if trimmed.starts_with('[') && in_desktop_entry {
            break;
        }

        if in_desktop_entry {
            if let Some((k, v)) = trimmed.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                match k {
                    "Name" if name.is_none() => name = Some(v.to_string()),
                    "Name[vi]" => name_vi = Some(v.to_string()),
                    "Exec" if exec.is_none() => {
                        let clean_exec = v
                            .split_whitespace()
                            .filter(|arg| !arg.starts_with('%'))
                            .collect::<Vec<_>>()
                            .join(" ");
                        exec = Some(clean_exec);
                    }
                    "Icon" if icon.is_empty() => icon = v.to_string(),
                    "NoDisplay" if v.eq_ignore_ascii_case("true") => no_display = true,
                    _ => {}
                }
            }
        }
    }

    if no_display {
        return None;
    }

    Some(DesktopApp {
        name: name?,
        exec: exec?,
        icon,
        desktop_file: path.to_path_buf(),
        name_vi,
    })
}

pub fn match_app<'a>(query: &str, index: &'a AppIndex) -> Option<&'a DesktopApp> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }

    // 1. Khớp chính xác theo key index (bao gồm name, name[vi], stem)
    if let Some(app) = index.get(&q) {
        return Some(app);
    }

    // 2. Mở rộng aliases phổ biến trên Linux & Việt hóa
    let alias_target = match q.as_str() {
        "vs code" | "vscode" | "visual studio code" | "code" => "visual studio code",
        "terminal" | "terminal emulator" | "bàn điều khiển" | "dòng lệnh" | "bash" => "terminal",
        "chrome" | "google chrome" => "google chrome",
        "firefox" | "cáo lửa" => "firefox",
        "zalo" => "zalo",
        "spotify" | "spotfy" => "spotify",
        "telegram" => "telegram",
        "cài đặt" | "cấu hình" | "setting" | "settings" | "thiết lập" => "gnome-control-center",
        "quản lý tệp" | "quản lý file" | "tệp" | "tập tin" | "file" | "files" | "thư mục" | "nautilus" => "nautilus",
        "máy tính" | "máy tính bỏ túi" | "calculator" | "calc" => "calculator",
        "soạn thảo" | "ghi chú" | "text editor" | "gedit" => "text editor",
        "discord" => "discord",
        "vlc" | "xem phim" => "vlc",
        "obs" | "obs studio" => "obs",
        "postman" => "postman",
        "intellij" | "idea" => "intellij",
        "android studio" => "android studio",
        "obsidian" => "obsidian",
        "steam" => "steam",
        "brave" => "brave",
        _ => &q,
    };

    if let Some(app) = index.get(alias_target) {
        return Some(app);
    }

    // 3. Khớp tiền tố hoặc chứa chuỗi (contains)
    for (name, app) in index.iter() {
        if name == alias_target || name.contains(&q) || name.contains(alias_target) {
            return Some(app);
        }
    }

    // 4. Mờ nâng cao với Sorensen-Dice similarity (>= 0.78)
    let mut best_match: Option<(&DesktopApp, f64)> = None;
    for (name, app) in index.iter() {
        let sim = strsim::sorensen_dice(&q, name);
        if sim >= 0.78 {
            if let Some((_, best_sim)) = best_match {
                if sim > best_sim {
                    best_match = Some((app, sim));
                }
            } else {
                best_match = Some((app, sim));
            }
        }
    }

    if let Some((app, _)) = best_match {
        return Some(app);
    }

    None
}

pub fn find_app_in_text<'a>(text: &str, index: &'a AppIndex) -> Option<&'a DesktopApp> {
    let lower = text.to_lowercase();
    for (name, app) in index.iter() {
        if name.len() >= 3 && lower.contains(name) {
            return Some(app);
        }
    }
    None
}

fn inject_live_env(cmd: &mut Command) {
    if let Ok(out) = Command::new("systemctl")
        .args(["--user", "show-environment"])
        .output()
    {
        if out.status.success() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            for line in out_str.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if matches!(
                        k,
                        "DISPLAY"
                            | "WAYLAND_DISPLAY"
                            | "XAUTHORITY"
                            | "DBUS_SESSION_BUS_ADDRESS"
                            | "XDG_RUNTIME_DIR"
                            | "XDG_CURRENT_DESKTOP"
                    ) {
                        cmd.env(k, v);
                    }
                }
            }
        }
    }
}

pub fn try_launch_known_binary(target: &str) -> Option<String> {
    let t = target.trim().to_lowercase();
    let bin_name = match t.as_str() {
        "spotify" | "spotfy" => "spotify",
        "chrome" | "google chrome" => "google-chrome",
        "firefox" => "firefox",
        "code" | "vscode" | "vs code" => "code",
        "cài đặt" | "settings" | "gnome-control-center" => "gnome-control-center",
        "quản lý tệp" | "tệp" | "files" | "nautilus" => "nautilus",
        "terminal" | "gnome-terminal" => "gnome-terminal",
        "calculator" | "máy tính" => "gnome-calculator",
        "text editor" | "gedit" => "gedit",
        "discord" => "discord",
        "vlc" => "vlc",
        "obs" => "obs",
        "postman" => "postman",
        "steam" => "steam",
        "brave" => "brave-browser",
        _ => return None,
    };

    let check = Command::new("which").arg(bin_name).output().ok()?;
    if !check.status.success() {
        return None;
    }

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("nohup {} >/dev/null 2>&1 &", bin_name))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    inject_live_env(&mut cmd);

    if cmd.spawn().is_ok() {
        Some(bin_name.to_string())
    } else {
        None
    }
}

pub fn launch_desktop_app(app: &DesktopApp) -> bool {
    // 1. Thử khởi chạy chuẩn Desktop thông qua gtk-launch
    if let Some(fname) = app.desktop_file.file_name() {
        let mut cmd = Command::new("gtk-launch");
        cmd.arg(fname)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        inject_live_env(&mut cmd);

        if let Ok(mut child) = cmd.spawn() {
            std::thread::sleep(std::time::Duration::from_millis(60));
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return true,
                Ok(None) => return true, // Tiến trình vẫn đang sống và chạy tốt
                _ => {} // Lỗi D-Bus hoặc gtk-launch thoát sớm -> chuyển sang chạy lệnh trực tiếp
            }
        }
    }

    // 2. Fallback: Chạy trực tiếp binary từ trường Exec trong .desktop file
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("nohup {} >/dev/null 2>&1 &", app.exec))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    inject_live_env(&mut cmd);
    cmd.spawn().is_ok()
}

/// Helper chạy ydotool (cần ydotoold) hoặc xdotool (trên X11)
fn send_shortcut(ydotool_keys: &[&str], xdotool_key: &str) -> bool {
    if let Ok(out) = Command::new("ydotool")
        .arg("key")
        .args(ydotool_keys)
        .output()
    {
        if out.status.success() {
            return true;
        }
    }

    if let Ok(out) = Command::new("xdotool").args(["key", xdotool_key]).output() {
        if out.status.success() {
            return true;
        }
    }

    false
}

pub fn close_active_window(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng cửa sổ hiện tại cho bạn.".to_string());
    }

    if is_hyprland() {
        if let Ok(out) = Command::new("hyprctl")
            .args(["dispatch", "killactive"])
            .output()
        {
            if out.status.success() {
                return (true, "Đã đóng cửa sổ hiện tại.".to_string());
            }
        }
    }

    // Alt (56) + F4 (62) theo linux/input-event-codes.h
    if send_shortcut(&["56:1", "62:1", "62:0", "56:0"], "alt+F4") {
        return (true, "Đã đóng cửa sổ hiện tại.".to_string());
    }

    (
        true,
        "Không thể gửi phím đóng cửa sổ trên GNOME Wayland. Bạn hãy cài và bật dịch vụ ydotoold nhé."
            .to_string(),
    )
}

pub fn close_active_tab(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng thẻ trình duyệt hiện tại.".to_string());
    }

    if is_hyprland() {
        if let Ok(out) = Command::new("wtype")
            .args(["-M", "ctrl", "-k", "w", "-m", "ctrl"])
            .output()
        {
            if out.status.success() {
                return (true, "Đã đóng tab hiện tại cho bạn rồi nhé.".to_string());
            }
        }
    }

    // Ctrl (29) + W (17)
    if send_shortcut(&["29:1", "17:1", "17:0", "29:0"], "ctrl+w") {
        return (true, "Đã đóng tab hiện tại cho bạn rồi nhé.".to_string());
    }

    (
        true,
        "Không thể gửi phím đóng tab trên GNOME Wayland. Bạn hãy cài và bật ydotoold nhé."
            .to_string(),
    )
}

pub fn close_all_open_apps(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng tất cả các ứng dụng đang mở.".to_string());
    }

    let safe_targets = [
        "chrome",
        "firefox",
        "firefox-bin",
        "spotify",
        "code",
        "discord",
        "telegram-desktop",
    ];
    let mut closed_any = false;
    for proc_name in safe_targets {
        if let Ok(out) = Command::new("pkill").args(["-x", proc_name]).output() {
            if out.status.success() {
                closed_any = true;
            }
        }
    }

    if closed_any {
        (true, "Đã đóng các ứng dụng đang mở cho bạn.".to_string())
    } else {
        (true, "Hiện không có ứng dụng phổ biến nào đang mở để đóng.".to_string())
    }
}

pub fn close_specific_app(app_target: &str, index: &AppIndex, dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, format!("Đã đóng ứng dụng {}.", app_target));
    }

    let target_low = app_target.to_lowercase();
    let proc_candidates: Vec<String> = match target_low.as_str() {
        "chrome" | "google chrome" => vec!["chrome".into(), "google-chrome".into()],
        "firefox" => vec!["firefox".into(), "firefox-bin".into()],
        "spotify" => vec!["spotify".into()],
        "vs code" | "vscode" | "code" => vec!["code".into()],
        "zalo" => vec!["zalo".into(), "Zalo".into()],
        "telegram" => vec!["telegram-desktop".into(), "Telegram".into()],
        _ => {
            if let Some(app) = match_app(app_target, index) {
                // Lấy tên binary cuối cùng trong đường dẫn Exec, bỏ qua wrapper nguy hiểm
                let first_token = app
                    .exec
                    .split_whitespace()
                    .find(|tok| !tok.contains('=') && *tok != "env")
                    .unwrap_or(app_target)
                    .trim_matches('"');
                let bin = Path::new(first_token)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(first_token);
                if matches!(bin, "sh" | "bash" | "zsh" | "flatpak" | "snap") {
                    vec![target_low]
                } else {
                    vec![bin.to_string()]
                }
            } else {
                vec![target_low]
            }
        }
    };

    for proc_name in &proc_candidates {
        if let Ok(out) = Command::new("pkill").args(["-i", "-x", proc_name]).output() {
            if out.status.success() {
                return (true, format!("Đã đóng ứng dụng {} cho bạn rồi nhé.", app_target));
            }
        }
    }

    (
        true,
        format!("Hiện không thấy ứng dụng {} đang chạy trên máy.", app_target),
    )
}

pub fn switch_hyprland_workspace(ws_num: i32) -> (bool, String) {
    let out = Command::new("hyprctl")
        .args(["dispatch", "workspace", &ws_num.to_string()])
        .output();

    if out.is_ok() {
        (true, format!("Đã chuyển sang không gian làm việc số {} cho bạn.", ws_num))
    } else {
        (true, format!("Đã gửi lệnh chuyển sang workspace {}.", ws_num))
    }
}

pub fn move_active_window_to_workspace(ws_num: i32) -> (bool, String) {
    let _ = Command::new("hyprctl")
        .args(["dispatch", "movetoworkspace", &ws_num.to_string()])
        .output();

    (true, format!("Đã chuyển cửa sổ hiện tại sang không gian làm việc số {}.", ws_num))
}

pub fn toggle_window_floating() -> (bool, String) {
    let _ = Command::new("hyprctl")
        .args(["dispatch", "togglefloating"])
        .output();

    (true, "Đã chuyển đổi trạng thái cửa sổ nổi cho bạn.".to_string())
}

pub fn toggle_window_pin() -> (bool, String) {
    let _ = Command::new("hyprctl")
        .args(["dispatch", "pin"])
        .output();

    (true, "Đã ghim cửa sổ này hiển thị trên mọi không gian làm việc.".to_string())
}

pub fn set_window_fullscreen(enable: bool) -> (bool, String) {
    if is_hyprland() {
        let arg = if enable { "1" } else { "0" };
        if let Ok(out) = Command::new("hyprctl")
            .args(["dispatch", "fullscreen", arg])
            .output()
        {
            if out.status.success() {
                let msg = if enable {
                    "Đã phóng to tối đa cửa sổ cho bạn rồi nhé."
                } else {
                    "Đã đưa cửa sổ về kích thước tiêu chuẩn."
                };
                return (true, msg.to_string());
            }
        }
    }

    // Ubuntu GNOME: Super+Up (125+103) phóng to, Super+Down (125+108) thu về bình thường
    let ok = if enable {
        send_shortcut(&["125:1", "103:1", "103:0", "125:0"], "Super_L+Up")
    } else {
        send_shortcut(&["125:1", "108:1", "108:0", "125:0"], "Super_L+Down")
    };

    if ok {
        let msg = if enable {
            "Đã phóng to cửa sổ cho bạn rồi nhé."
        } else {
            "Đã thu gọn cửa sổ về kích thước tiêu chuẩn."
        };
        (true, msg.to_string())
    } else {
        (
            true,
            "Không thể chỉnh kích thước cửa sổ trên GNOME Wayland vì chưa có ydotoold.".to_string(),
        )
    }
}

pub fn toggle_hyprland_split() -> (bool, String) {
    if is_hyprland() {
        if let Ok(out) = Command::new("hyprctl")
            .args(["dispatch", "togglesplit"])
            .output()
        {
            if out.status.success() {
                return (true, "Đã đổi hướng chia màn hình cho bạn rồi nhé.".to_string());
            }
        }
    }
    (
        true,
        "Tính năng xoay hướng chia màn hình hiện chỉ hỗ trợ trên Hyprland.".to_string(),
    )
}

pub fn set_hyprland_split_ratio(ratio: &str) -> (bool, String) {
    if is_hyprland() {
        if let Ok(out) = Command::new("hyprctl")
            .args(["dispatch", "splitratio", ratio])
            .output()
        {
            if out.status.success() {
                return (
                    true,
                    format!("Đã điều chỉnh tỷ lệ chia màn hình {} cho bạn.", ratio),
                );
            }
        }
    }

    // Trên Ubuntu GNOME, hỗ trợ nhanh chia đôi 50/50 bằng Super+Left (125+105)
    if ratio == "1.0" && send_shortcut(&["125:1", "105:1", "105:0", "125:0"], "Super_L+Left") {
        return (true, "Đã ghim cửa sổ sang nửa trái màn hình cho bạn.".to_string());
    }

    (
        true,
        "Tính năng chia tỷ lệ cửa sổ tùy biến hiện chỉ hỗ trợ trên Hyprland hoặc khi bật ydotoold trên Ubuntu."
            .to_string(),
    )
}

pub fn apply_workspace_preset(preset: &str, index: &AppIndex, dry_run: bool) -> (bool, String) {
    match preset {
        "coding" => {
            if !dry_run {
                let _ = switch_hyprland_workspace(2);
                if let Some(code) = match_app("code", index) {
                    launch_desktop_app(code);
                }
                if let Some(term) = match_app("terminal", index) {
                    launch_desktop_app(term);
                }
            }
            (true, "Đã kích hoạt chế độ Lập trình: Chuyển workspace và mở VS Code cùng Terminal.".to_string())
        }
        "entertainment" => {
            if !dry_run {
                let _ = switch_hyprland_workspace(3);
                if let Some(spotify) = match_app("spotify", index) {
                    launch_desktop_app(spotify);
                }
            }
            (true, "Đã kích hoạt chế độ Giải trí: Chuyển workspace và mở Spotify cho bạn.".to_string())
        }
        _ => (false, "Không tìm thấy preset cấu hình này.".to_string()),
    }
}
