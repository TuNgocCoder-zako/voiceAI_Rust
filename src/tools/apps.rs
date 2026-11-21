use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub desktop_file: PathBuf,
}

pub type AppIndex = HashMap<String, DesktopApp>;

pub fn build_app_index() -> AppIndex {
    let mut index = HashMap::new();
    let mut search_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".local/share/applications"));
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
                    index.entry(key).or_insert(app);
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
                    "Exec" if exec.is_none() => {
                        let clean_exec = v.split_whitespace()
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
    })
}

pub fn match_app<'a>(query: &str, index: &'a AppIndex) -> Option<&'a DesktopApp> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }

    // Exact name match
    if let Some(app) = index.get(&q) {
        return Some(app);
    }

    // Common aliases
    let alias_target = match q.as_str() {
        "vs code" | "vscode" | "visual studio code" | "code" => "visual studio code",
        "terminal" | "terminal emulator" | "bàn điều khiển" => "terminal",
        "chrome" | "google chrome" => "google chrome",
        "firefox" | "cáo lửa" => "firefox",
        "zalo" => "zalo",
        "spotify" => "spotify",
        "telegram" => "telegram",
        _ => &q,
    };

    if let Some(app) = index.get(alias_target) {
        return Some(app);
    }

    // Prefix / Substring match
    for (name, app) in index.iter() {
        if name.contains(&q) || name.contains(alias_target) {
            return Some(app);
        }
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

pub fn launch_desktop_app(app: &DesktopApp) -> bool {
    // Try gtk-launch first if available
    let res = Command::new("gtk-launch")
        .arg(app.desktop_file.file_name().unwrap_or_default())
        .spawn();

    if res.is_ok() {
        return true;
    }

    // Fallback: spawn the exec string directly
    Command::new("sh")
        .arg("-c")
        .arg(format!("{} &", app.exec))
        .spawn()
        .is_ok()
}

pub fn close_active_window(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng cửa sổ hiện tại cho bạn.".to_string());
    }

    // Hyprland: hyprctl dispatch killactive
    let hypr = Command::new("hyprctl")
        .args(["dispatch", "killactive"])
        .output();

    if let Ok(out) = hypr {
        if out.status.success() {
            return (true, "Đã đóng cửa sổ hiện tại.".to_string());
        }
    }

    // Generic X11/Wayland with xdotool / ydotool or GNOME
    let _ = Command::new("ydotool")
        .args(["key", "29:1", "56:1", "14:1", "14:0", "56:0", "29:0"]) // Alt+F4
        .output();

    (true, "Đã gửi lệnh đóng cửa sổ hiện tại.".to_string())
}

pub fn close_active_tab(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng thẻ trình duyệt hiện tại.".to_string());
    }

    let _ = Command::new("sh")
        .arg("-c")
        .arg("wtype -M ctrl -k w -m ctrl 2>/dev/null || ydotool key 29:1 17:1 17:0 29:0 2>/dev/null")
        .spawn();

    (true, "Đã đóng tab hiện tại cho bạn rồi nhé.".to_string())
}

pub fn close_all_open_apps(dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, "Đã đóng tất cả các ứng dụng đang mở.".to_string());
    }
    // Safe close non-essential apps
    let _ = Command::new("sh")
        .arg("-c")
        .arg("pkill -f 'chrome|firefox|spotify|code|discord|telegram' 2>/dev/null")
        .output();

    (true, "Đã gửi lệnh đóng toàn bộ ứng dụng đang mở.".to_string())
}

pub fn close_specific_app(app_target: &str, index: &AppIndex, dry_run: bool) -> (bool, String) {
    if dry_run {
        return (true, format!("Đã đóng ứng dụng {}.", app_target));
    }

    let bin_name = match app_target.to_lowercase().as_str() {
        "chrome" | "google chrome" => "chrome",
        "firefox" => "firefox",
        "spotify" => "spotify",
        "vs code" | "vscode" | "code" => "code",
        "zalo" => "zalo",
        "telegram" => "telegram",
        _ => {
            if let Some(app) = match_app(app_target, index) {
                app.exec.split_whitespace().next().unwrap_or(app_target)
            } else {
                app_target
            }
        }
    };

    let _ = Command::new("pkill")
        .args(["-f", bin_name])
        .output();

    (true, format!("Đã đóng ứng dụng {} cho bạn rồi nhé.", app_target))
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

pub fn set_window_fullscreen(enable: bool) {
    let arg = if enable { "1" } else { "0" };
    let _ = Command::new("hyprctl")
        .args(["dispatch", "fullscreen", arg])
        .output();
}

pub fn toggle_hyprland_split() -> (bool, String) {
    let _ = Command::new("hyprctl")
        .args(["dispatch", "togglesplit"])
        .output();

    (true, "Đã đổi hướng chia màn hình cho bạn rồi nhé.".to_string())
}

pub fn set_hyprland_split_ratio(ratio: &str) -> (bool, String) {
    let _ = Command::new("hyprctl")
        .args(["dispatch", "splitratio", ratio])
        .output();

    (true, format!("Đã điều chỉnh tỷ lệ chia màn hình {} cho bạn.", ratio))
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
