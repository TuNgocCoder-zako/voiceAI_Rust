use crate::config::Config;
use crate::core::memory::MemoryManager;
use crate::core::nlu::fuzzy::fuzzy_match_canonical_action;
use crate::core::nlu::normalize::{clean_user_input, MULTI_SPACE_RE};
use crate::core::nlu::patterns::*;
use crate::tools::apps::*;
use crate::tools::dev::*;
use crate::tools::system::*;
use crate::tools::timer::TimerManager;
use crate::tools::web::*;
use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

pub static TIMER_MANAGER: LazyLock<TimerManager> = LazyLock::new(TimerManager::new);
static MEMORY_MANAGER: LazyLock<MemoryManager> = LazyLock::new(MemoryManager::new);
static GLOBAL_CONFIG: LazyLock<Config> = LazyLock::new(Config::default);

static SIMPLE_MATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:tính|bao nhiêu là)?\s*(\d+(?:\.\d+)?)\s*([\+\-\*\/x×÷]|cộng|trừ|nhân|chia)\s*(\d+(?:\.\d+)?)\s*(?:bằng bao nhiêu|\?)?$").unwrap()
});

pub fn fast_path_nlu(
    user_text: &str,
    app_index: &AppIndex,
    dry_run: bool,
) -> (bool, String) {
    fast_path_nlu_internal(user_text, app_index, dry_run, false)
}

fn fast_path_nlu_internal(
    user_text: &str,
    app_index: &AppIndex,
    dry_run: bool,
    is_retry: bool,
) -> (bool, String) {
    if user_text.trim().is_empty() {
        return (false, String::new());
    }

    let cleaned = clean_user_input(user_text);
    let norm_text: String = cleaned.nfkc().collect();
    let mut t = norm_text.to_lowercase().trim().trim_end_matches(['.', '!', '?']).to_string();
    t = MULTI_SPACE_RE.replace_all(&t, " ").to_string();

    // 0. Barge-in / Stop
    if BARGE_IN_RE.is_match(&t) {
        if !dry_run {
            media_pause();
        }
        return (true, String::new());
    }

    // 0.5. Identity & Greetings
    if IDENTITY_RE.is_match(&t) {
        return (
            true,
            "Tôi là Alexa, trợ lý ảo giọng nói tiếng Việt tối ưu bằng Rust. Tôi có thể giúp bạn mở ứng dụng, nghe nhạc, điều khiển máy tính, lập trình và giải đáp mọi thắc mắc.".to_string(),
        );
    }
    if GREETING_RE.is_match(&t) {
        return (
            true,
            "Chào bạn! Tôi có thể giúp gì cho bạn hôm nay?".to_string(),
        );
    }
    if HOW_ARE_YOU_RE.is_match(&t) {
        return (
            true,
            "Tôi luôn khỏe và sẵn sàng hỗ trợ bạn bất cứ lúc nào!".to_string(),
        );
    }

    // 1. Time & Date
    if TIME_RE.is_match(&t) {
        return (true, get_vietnamese_time());
    }
    if DATE_RE.is_match(&t) {
        return (true, get_vietnamese_date());
    }

    // 2. Battery
    if BATTERY_RE.is_match(&t) {
        if dry_run {
            return (true, "Pin hiện tại còn 85%.".to_string());
        }
        return (true, get_battery_info());
    }

    // 2.5. Git & Clipboard
    if GIT_STATUS_RE.is_match(&t) {
        if dry_run {
            return (true, "Trạng thái Git: Nhánh main sạch sẽ.".to_string());
        }
        return (true, get_git_status_summary());
    }
    if CLIPBOARD_RE.is_match(&t) {
        if dry_run {
            return (true, "Nội dung trong bộ nhớ tạm.".to_string());
        }
        return (true, get_clipboard_content());
    }

    // 2.6. Network speed
    if NETWORK_SPEED_RE.is_match(&t) {
        if dry_run {
            return (true, "Tốc độ mạng hiện tại: Độ trễ ping 35 ms, tải xuống 120 Mbps.".to_string());
        }
        return (true, get_network_speed_info());
    }

    // 3. Volume
    if let Some(caps) = VOLUME_PCT_RE.captures(&t) {
        if !t.contains("tăng") && !t.contains("giảm") {
            if let Some(m) = caps.get(1) {
                if let Ok(pct) = m.as_str().parse::<u32>() {
                    let clamped = pct.min(100);
                    if !dry_run {
                        set_system_volume(clamped);
                    }
                    return (true, format!("Đã đặt âm lượng ở mức {} phần trăm.", clamped));
                }
            }
        }
    }
    if !t.contains("cửa sổ") && !t.contains("màn hình") && VOLUME_UP_RE.is_match(&t) {
        if !dry_run {
            adjust_system_volume("+10%");
        }
        return (true, "Đã tăng âm lượng lên rồi nhé.".to_string());
    }
    if !t.contains("cửa sổ") && !t.contains("màn hình") && VOLUME_DOWN_RE.is_match(&t) {
        if !dry_run {
            adjust_system_volume("-10%");
        }
        return (true, "Đã giảm âm lượng cho bạn.".to_string());
    }
    if VOLUME_MUTE_RE.is_match(&t) {
        if !dry_run {
            toggle_system_mute();
        }
        return (true, "Đã chuyển đổi trạng thái âm thanh.".to_string());
    }

    // 4. Media
    if MEDIA_INFO_RE.is_match(&t) {
        if dry_run {
            return (true, "Bài hát đang phát là Shape of You của Ed Sheeran.".to_string());
        }
        return (true, get_current_media_track());
    }
    if MEDIA_PAUSE_RE.is_match(&t) || t == "dừng lại" || t == "tạm dừng" || t == "tắt nhạc" {
        if !dry_run {
            media_pause();
        }
        return (true, "Đã tạm dừng phát nhạc.".to_string());
    }
    if MEDIA_PLAY_RE.is_match(&t) {
        if !dry_run {
            media_play();
        }
        return (true, "Đã tiếp tục phát nhạc nhé.".to_string());
    }
    if MEDIA_NEXT_RE.is_match(&t) {
        if !dry_run {
            media_next();
        }
        return (true, "Đã chuyển sang bài tiếp theo cho bạn.".to_string());
    }
    if MEDIA_PREV_RE.is_match(&t) {
        if !dry_run {
            media_previous();
        }
        return (true, "Đã quay lại bài trước rồi nhé.".to_string());
    }

    // 4.1. Hardware Controls (Wi-Fi & Bluetooth)
    if WIFI_ON_RE.is_match(&t) {
        if !dry_run {
            set_wifi_state(true);
        }
        return (true, "Đã bật kết nối Wi-Fi cho bạn rồi nhé.".to_string());
    }
    if WIFI_OFF_RE.is_match(&t) {
        if !dry_run {
            set_wifi_state(false);
        }
        return (true, "Đã tắt Wi-Fi.".to_string());
    }
    if BT_ON_RE.is_match(&t) {
        if !dry_run {
            set_bluetooth_state(true);
        }
        return (true, "Đã bật Bluetooth.".to_string());
    }
    if BT_OFF_RE.is_match(&t) {
        if !dry_run {
            set_bluetooth_state(false);
        }
        return (true, "Đã tắt Bluetooth.".to_string());
    }

    // 4.2. ASUS Fan Profiles
    if ASUS_TURBO_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã chuyển sang chế độ Hiệu năng cao Turbo cho bạn.".to_string());
        }
        return (true, set_asus_profile("Performance"));
    }
    if ASUS_QUIET_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã chuyển sang chế độ Yên tĩnh tiết kiệm pin.".to_string());
        }
        return (true, set_asus_profile("Quiet"));
    }
    if ASUS_BALANCED_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã chuyển về chế độ Cân bằng mượt mà.".to_string());
        }
        return (true, set_asus_profile("Balanced"));
    }

    // 5. Brightness
    if BRIGHTNESS_UP_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã tăng độ sáng màn hình rồi nhé.".to_string());
        }
        if adjust_brightness("+10%") {
            return (true, "Đã tăng độ sáng màn hình rồi nhé.".to_string());
        }
        return (true, "Không thể chỉnh độ sáng màn hình trên thiết bị này.".to_string());
    }
    if BRIGHTNESS_DOWN_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã giảm độ sáng màn hình cho bạn.".to_string());
        }
        if adjust_brightness("10%-") {
            return (true, "Đã giảm độ sáng màn hình cho bạn.".to_string());
        }
        return (true, "Không thể chỉnh độ sáng màn hình trên thiết bị này.".to_string());
    }

    // 6. Lock screen & Screenshot
    if LOCK_SCREEN_RE.is_match(&t) {
        if !dry_run {
            lock_screen();
        }
        return (true, "Đang khóa màn hình máy tính cho bạn nhé.".to_string());
    }
    if SCREENSHOT_RE.is_match(&t) {
        let mode = if t.contains("toàn") { "full" } else { "region" };
        if dry_run {
            return (true, "Đã kích hoạt chụp ảnh màn hình cho bạn.".to_string());
        }
        return (true, take_screenshot(mode));
    }

    // 6.1. Close all apps
    if CLOSE_ALL_APPS_RE.is_match(&t) {
        return close_all_open_apps(dry_run);
    }
    // 6.2. Close tab
    if CLOSE_ACTIVE_TAB_RE.is_match(&t) {
        return close_active_tab(dry_run);
    }
    // 6.2.1. Close window
    if CLOSE_ACTIVE_WINDOW_RE.is_match(&t) {
        return close_active_window(dry_run);
    }

    // 6.5. Close specific app by name
    if let Some(caps) = CLOSE_APP_MATCH_RE.captures(&t) {
        if let Some(m) = caps.get(1) {
            let target = m.as_str().trim();
            let excluded = [
                "hẹn giờ", "báo thức", "nhạc", "wifi", "bluetooth", "âm thanh", "tiếng",
                "màn hình", "máy", "cửa sổ", "nó", "cái này", "lại", "đi", "nhé", "nha",
            ];
            if !excluded.iter().any(|&e| target.contains(e)) {
                return close_specific_app(target, app_index, dry_run);
            }
        }
    }

    // 6.6. Fullscreen / Maximize
    if FULLSCREEN_ON_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã phóng to tối đa cửa sổ cho bạn rồi nhé.".to_string());
        }
        return set_window_fullscreen(true);
    }
    if FULLSCREEN_OFF_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã đưa cửa sổ về kích thước tiêu chuẩn.".to_string());
        }
        return set_window_fullscreen(false);
    }

    // 6.7. Split layout
    if SPLIT_TOGGLE_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã đổi hướng chia màn hình cho bạn rồi nhé.".to_string());
        }
        return toggle_hyprland_split();
    }
    if SPLIT_5050_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã chia đều màn hình tỷ lệ 50 50 cho bạn rồi nhé.".to_string());
        }
        return set_hyprland_split_ratio("1.0");
    }
    if SPLIT_7030_RE.is_match(&t) {
        if dry_run {
            return (true, "Đã điều chỉnh cửa sổ chiếm 70% màn hình cho bạn rồi nhé.".to_string());
        }
        return set_hyprland_split_ratio("1.4");
    }

    // 6.8. Presets
    if t.contains("lập trình") || t.contains("code") || t.contains("coding") {
        if t.contains("chế độ") || t.contains("bật chế độ") {
            return apply_workspace_preset("coding", app_index, dry_run);
        }
    }
    if t.contains("giải trí") || t.contains("thư giãn") {
        if t.contains("chế độ") || t.contains("bật chế độ") {
            return apply_workspace_preset("entertainment", app_index, dry_run);
        }
    }

    // 7. Hardware & Dev
    if let Some(caps) = KILL_PORT_RE.captures(&t) {
        if let Some(p) = caps.get(1) {
            if let Ok(port) = p.as_str().parse::<u16>() {
                if dry_run {
                    return (true, format!("Đã giải phóng cổng {}.", port));
                }
                return (true, kill_port_process(port));
            }
        }
    }
    if let Some(caps) = CHECK_PORT_RE.captures(&t) {
        if let Some(p) = caps.get(1) {
            if let Ok(port) = p.as_str().parse::<u16>() {
                if dry_run {
                    return (true, format!("Cổng {} đang trống.", port));
                }
                return (true, check_port_status(port));
            }
        }
    }
    if DOCKER_RE.is_match(&t) {
        if dry_run {
            return (true, "Docker hiện có 2 container đang chạy.".to_string());
        }
        return (true, check_docker_containers());
    }
    if JAVA_VERSION_RE.is_match(&t) {
        if dry_run {
            return (true, "Hệ thống đang cài đặt OpenJDK 21 LTS.".to_string());
        }
        return (true, check_java_version());
    }
    if CPU_INFO_RE.is_match(&t) {
        if dry_run {
            return (true, "CPU Intel Core i5 đang hoạt động ổn định.".to_string());
        }
        return (true, get_cpu_info());
    }
    if HARDWARE_INFO_RE.is_match(&t) {
        if dry_run {
            return (true, "RAM đang sử dụng 4.2 GB / 16.0 GB.".to_string());
        }
        return (true, get_system_hardware_info());
    }

    // 8. Math calculation
    if let Some(math_ans) = parse_simple_math(&t) {
        return (true, math_ans);
    }

    // 9. Weather
    if WEATHER_RE.is_match(&t) {
        if dry_run {
            return (true, "Thời tiết hiện tại 28 độ C, trời quang mây tạnh.".to_string());
        }
        let loc = t
            .split_once("ở ")
            .or_else(|| t.split_once("tại "))
            .map(|(_, rest)| rest.trim())
            .unwrap_or("");
        return (true, get_weather_info(loc));
    }

    // 10. Web URL Shortcuts
    for (site, link) in GLOBAL_CONFIG.url_shortcuts.iter() {
        let open_phrases = [
            format!("mở {}", site),
            format!("mà {}", site),
            format!("mờ {}", site),
            format!("bật {}", site),
            format!("vào {}", site),
            format!("truy cập {}", site),
            site.to_string(),
        ];
        if open_phrases.iter().any(|p| t == *p) {
            if !dry_run {
                open_url(link);
            }
            let name = GLOBAL_CONFIG.url_display_names.get(site).cloned().unwrap_or_else(|| site.clone());
            return (true, format!("Đã mở {} cho bạn rồi nhé.", name));
        }
    }

    // 11. YouTube Search
    if let Some(caps) = YT_SEARCH_RE1.captures(&t).or_else(|| YT_SEARCH_RE2.captures(&t)) {
        if let Some(q) = caps.get(1) {
            let query = q.as_str().trim();
            if !query.is_empty() {
                if !dry_run {
                    return (true, open_youtube_search(query));
                }
                return (true, format!("Đang mở {} trên YouTube cho bạn.", query));
            }
        }
    }

    // 12. Google Search
    if let Some(caps) = GOOGLE_SEARCH_RE.captures(&t) {
        if let Some(q) = caps.get(1) {
            let query = q.as_str().trim();
            if !query.is_empty() && query != "thông tin" {
                if !dry_run {
                    return (true, open_google_search(query));
                }
                return (true, format!("Đang tìm kiếm {} trên Google cho bạn.", query));
            }
        }
    }

    // 13. App Launcher (Solo or with open verb)
    if let Some(app) = match_app(&t, app_index) {
        if !dry_run {
            launch_desktop_app(app);
        }
        return (true, format!("Đã mở {} cho bạn rồi nhé.", app.name));
    }
    if OPEN_TRIGGER_RE.is_match(&t) {
        if let Some(caps) = OPEN_APP_MATCH_RE.captures(&t) {
            if let Some(m) = caps.get(1) {
                let target = m.as_str().trim();
                if let Some(app) = match_app(target, app_index) {
                    if !dry_run {
                        launch_desktop_app(app);
                    }
                    return (true, format!("Đã mở {} cho bạn rồi nhé.", app.name));
                }
            }
        }
        if let Some(app) = find_app_in_text(&t, app_index) {
            if !dry_run {
                launch_desktop_app(app);
            }
            return (true, format!("Đã mở {} cho bạn rồi nhé.", app.name));
        }
    }

    // 15. Timers & Alarms
    if TIMER_CANCEL_RE.is_match(&t) {
        return (true, TIMER_MANAGER.cancel_all());
    }
    if TIMER_CHECK_RE.is_match(&t) {
        return (true, TIMER_MANAGER.check_timers());
    }
    if let Some(caps) = ALARM_ADD_RE.captures(&t) {
        if let Some(h) = caps.get(1) {
            if let Ok(mut hr) = h.as_str().parse::<u32>() {
                let mn = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .unwrap_or(0);
                if let Some(period) = caps.get(3).map(|m| m.as_str()) {
                    match period {
                        "chiều" | "tối" if hr < 12 => hr += 12,
                        "sáng" if hr == 12 => hr = 0,
                        _ => {}
                    }
                }
                if hr > 23 || mn > 59 {
                    return (
                        true,
                        "Giờ báo thức không hợp lệ, bạn nói lại giúp tôi nhé.".to_string(),
                    );
                }
                let label = caps.get(4).map(|m| m.as_str()).unwrap_or("");
                if dry_run {
                    return (true, format!("Đã cài báo thức lúc {} giờ {} phút.", hr, mn));
                }
                return (true, TIMER_MANAGER.add_alarm(hr, mn, label));
            }
        }
    }
    if let Some(caps) = TIMER_ADD_RE.captures(&t).or_else(|| TIMER_SHORT_RE.captures(&t)) {
        if let Some(v) = caps.get(1) {
            if let Ok(val) = v.as_str().parse::<f64>() {
                let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("phút");
                let label = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                let mins = match unit {
                    "tiếng" | "giờ" => val * 60.0,
                    "giây" => val / 60.0,
                    _ => val,
                };
                if dry_run {
                    return (true, format!("Đã đặt hẹn giờ {} {}.", val, unit));
                }
                return (true, TIMER_MANAGER.add_timer(mins, label));
            }
        }
    }

    // 16. Notes & Facts
    if t.contains("đọc ghi chú") || t.contains("xem ghi chú") {
        return (true, MEMORY_MANAGER.read_notes());
    }
    if let Some((_, rest)) = t.split_once("ghi chú ") {
        let note = rest.trim();
        if !note.is_empty() {
            if dry_run {
                return (true, format!("Đã lưu ghi chú '{}'.", note));
            }
            return (true, MEMORY_MANAGER.add_note(note));
        }
    }
    if let Some((_, rest)) = t
        .split_once("hãy nhớ ")
        .or_else(|| t.split_once("ghi nhớ "))
        .or_else(|| t.split_once("nhớ "))
    {
        let fact = rest.trim();
        if !fact.is_empty() {
            if dry_run {
                return (true, format!("Tôi đã ghi nhớ '{}'.", fact));
            }
            return (true, MEMORY_MANAGER.add_fact(fact));
        }
    }

    // 17.5. Fuzzy Homophone Fallback
    if !is_retry {
        if let Some((canon, _)) = fuzzy_match_canonical_action(&t, 0.82) {
            let (ok, rep) = fast_path_nlu_internal(canon, app_index, dry_run, true);
            if ok {
                return (true, rep);
            }
        }
    }

    // 18. Smart Anti-Leak Guard
    let question_signals = [
        "là gì", "tại sao", "như thế nào", "sao lại", "thế nào", "bao nhiêu", "ở đâu", "khi nào",
        "mấy", "giải thích", "hướng dẫn", "phân tích", "so sánh", "nghĩa là", "có phải", "viết",
        "tạo", "làm sao", "code", "thuật toán", "lập trình", "ai là", "thế nào là",
    ];
    let is_knowledge = question_signals.iter().any(|&s| t.contains(s))
        || [" không", " hả", " nhỉ", " chứ", " thế", " vậy"].iter().any(|&e| t.ends_with(e));

    if !is_knowledge {
        let words: Vec<&str> = t.split_whitespace().collect();
        let action_verbs = ["mở", "bật", "chạy", "vào", "nghe", "tắt", "đóng", "thoát", "kill", "phóng to", "thu nhỏ"];
        if let Some(&first) = words.first() {
            // Chỉ chặn khi câu chỉ có 1-2 từ cụt lủn (vd: "mở", "mở cái", "bật"), còn lại để LLM xử lý
            if action_verbs.contains(&first) && words.len() <= 2 {
                return (true, "Dạ, tôi chưa nghe rõ tên ứng dụng hoặc câu lệnh bạn yêu cầu. Bạn nói lại giúp tôi nhé?".to_string());
            }
        }
    }

    (false, String::new())
}

fn parse_simple_math(text: &str) -> Option<String> {
    let t = text.trim();
    let caps = SIMPLE_MATH_RE.captures(t)?;
    let a: f64 = caps.get(1)?.as_str().parse().ok()?;
    let op = caps.get(2)?.as_str();
    let b: f64 = caps.get(3)?.as_str().parse().ok()?;

    let res = match op {
        "+" | "cộng" => a + b,
        "-" | "trừ" => a - b,
        "*" | "x" | "×" | "nhân" => a * b,
        "/" | "÷" | "chia" => {
            if b.abs() < 1e-9 {
                return Some("Không thể chia cho số không bạn nhé.".to_string());
            }
            a / b
        }
        _ => return None,
    };

    if (res - res.round()).abs() < 1e-9 {
        Some(format!("Kết quả của phép tính là {}.", res.round() as i64))
    } else {
        Some(format!("Kết quả của phép tính là {:.2}.", res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_slices_and_alarm_period() {
        let app_index = build_app_index();
        // Các câu này từng gây panic vì cắt sai byte UTF-8
        let (ok1, _) = fast_path_nlu("thời tiết ở đà lạt", &app_index, true);
        let (ok2, _) = fast_path_nlu("thời tiết tại nha trang", &app_index, true);
        let (ok3, r3) = fast_path_nlu("ghi chú mua sữa tươi", &app_index, true);
        let (ok4, r4) = fast_path_nlu("hãy nhớ sinh nhật mẹ", &app_index, true);
        let (ok5, r5) = fast_path_nlu("đặt báo thức 7 giờ tối", &app_index, true);

        assert!(ok1 && ok2 && ok3 && ok4 && ok5);
        assert!(r3.contains("mua sữa tươi"));
        assert!(r4.contains("sinh nhật mẹ"));
        assert!(r5.contains("19 giờ 0 phút"), "Got: {}", r5);
    }

    #[test]
    fn test_all_benchmark_commands() {
        let app_index = build_app_index();
        let commands = [
            "tăng âm lượng 20%",
            "giảm âm lượng",
            "tắt tiếng",
            "bật tiếng",
            "tăng độ sáng màn hình",
            "giảm độ sáng",
            "bật wifi",
            "tắt bluetooth",
            "chế độ turbo",
            "chế độ im lặng",
            "pin còn bao nhiêu",
            "kiểm tra ram",
            "bây giờ là mấy giờ",
            "hôm nay ngày mấy",
            "thời tiết hà nội thế nào",
            "mở visual studio code",
            "mở google chrome",
            "đóng cửa sổ",
            "phóng to màn hình",
            "giải phóng cổng 8080",
            "kiểm tra trạng thái git",
            "kiểm tra docker",
            "phiên bản java",
            "hẹn giờ 5 phút",
            "hủy hẹn giờ",
        ];

        for &cmd in &commands {
            let (handled, reply) = fast_path_nlu(cmd, &app_index, true);
            assert!(
                handled,
                "Command failed to match fast_path_nlu: '{}' (reply: '{}')",
                cmd, reply
            );
        }
    }

    #[test]
    fn test_polite_phrasing_handling() {
        let app_index = build_app_index();
        let (h1, r1) = fast_path_nlu("mở youtube giúp tôi", &app_index, true);
        assert!(h1, "Failed 'mở youtube giúp tôi'");
        assert!(r1.contains("YouTube"));

        let (h2, r2) = fast_path_nlu("mở facebook lên đi", &app_index, true);
        assert!(h2, "Failed 'mở facebook lên đi'");
        assert!(r2.contains("Facebook"));

        let (h3, r3) = fast_path_nlu("làm ơn mở visual studio code giúp mình nhé", &app_index, true);
        assert!(h3, "Failed 'làm ơn mở visual studio code giúp mình nhé'");
        assert!(r3.contains("Visual Studio Code") || r3.contains("code"));
    }
}
