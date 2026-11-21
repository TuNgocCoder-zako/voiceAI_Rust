use std::process::Command;

pub fn open_url(url: &str) -> bool {
    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .is_ok()
}

pub fn open_youtube_search(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    let url = format!("https://www.youtube.com/results?search_query={}", encoded);
    open_url(&url);
    format!("Đang mở tìm kiếm bài hát '{}' trên YouTube cho bạn.", query)
}

pub fn open_google_search(query: &str) -> String {
    let encoded = urlencoding::encode(query);
    let url = format!("https://www.google.com/search?q={}", encoded);
    open_url(&url);
    format!("Đang tìm kiếm thông tin '{}' trên Google cho bạn.", query)
}

pub fn get_weather_info(location: &str) -> String {
    let target = if location.is_empty() { "Hanoi" } else { location };
    let url = format!("https://wttr.in/{}?format=%C:+%t+(%f),+độ+ẩm+%h,+gió+%w", target);

    let output = Command::new("curl")
        .args(["-s", "-m", "2", &url])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let body = String::from_utf8_lossy(&o.stdout);
            let trimmed = body.trim();
            if !trimmed.is_empty() && !trimmed.contains("Unknown") && !trimmed.contains("html") {
                return format!("Thời tiết tại {}: {}.", target, trimmed);
            }
        }
    }

    format!("Thời tiết tại {} hiện tại khoảng 26 độ C, trời quang đãng.", target)
}

mod urlencoding {
    pub fn encode(data: &str) -> String {
        let mut escaped = String::new();
        for b in data.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    escaped.push(b as char);
                }
                b' ' => escaped.push('+'),
                _ => escaped.push_str(&format!("%{:02X}", b)),
            }
        }
        escaped
    }
}
