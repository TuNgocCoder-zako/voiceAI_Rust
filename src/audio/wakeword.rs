use crate::core::nlu::strip_vietnamese_accents;
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeWordType {
    Alexa,
    Jarvis,
    Assistant,
}

impl WakeWordType {
    pub fn name(&self) -> &'static str {
        match self {
            WakeWordType::Alexa => "Alexa",
            WakeWordType::Jarvis => "Hey Jarvis",
            WakeWordType::Assistant => "Trợ lý",
        }
    }
}

// Regex trên chuỗi đã chuẩn hóa bỏ dấu tiếng Việt (lowercase)
static UNACCENTED_ALEXA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(a\s*lech\s*xa|a\s*lec\s*xa|a\s*lech\s*sa|alexa|alecxa|a-lech-xa|e\s*alexa|hey\s*alexa)\b").unwrap()
});

static UNACCENTED_JARVIS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(hey\s*jarvis|jarvis|gia\s*vit|za\s*vit|gia-vit)\b").unwrap()
});

static UNACCENTED_ASSISTANT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(tro\s*ly\s*oi|oi\s*tro\s*ly|cho\s*ly\s*oi|tro\s*ly|e\s*tro\s*ly)\b").unwrap()
});

static FILLER_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(ơi|này|hãy|cho\s*mình\s*hỏi|cho\s*tôi\s*hỏi|làm\s*ơn|giúp\s*mình|giúp\s*tôi)[,\s]+").unwrap()
});

/// Kiểm tra văn bản nhận diện từ microphone có chứa từ khóa đánh thức hay không.
/// Trả về `Some((WakeWordType, stripped_command))` nếu phát hiện từ khóa:
/// - Nếu `stripped_command` rỗng: Người dùng chỉ mới gọi tên (ví dụ: "Alexa"), trợ lý sẽ ting và lắng nghe lượt tiếp.
/// - Nếu `stripped_command` có nội dung (ví dụ: "Alexa, mấy giờ rồi"): Trợ lý lập tức thực thi câu lệnh mà không cần đợi.
pub fn detect_wake_word(text: &str) -> Option<(WakeWordType, String)> {
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }

    let unaccented = strip_vietnamese_accents(raw).to_lowercase();

    // 1. Kiểm tra Alexa
    if let Some(m) = UNACCENTED_ALEXA_RE.find(&unaccented) {
        let remainder = clean_remainder_by_unaccented(raw, &unaccented, m.start(), m.end());
        return Some((WakeWordType::Alexa, remainder));
    }

    // 2. Kiểm tra Jarvis
    if let Some(m) = UNACCENTED_JARVIS_RE.find(&unaccented) {
        let remainder = clean_remainder_by_unaccented(raw, &unaccented, m.start(), m.end());
        return Some((WakeWordType::Jarvis, remainder));
    }

    // 3. Kiểm tra Trợ lý ơi
    if let Some(m) = UNACCENTED_ASSISTANT_RE.find(&unaccented) {
        let remainder = clean_remainder_by_unaccented(raw, &unaccented, m.start(), m.end());
        return Some((WakeWordType::Assistant, remainder));
    }

    None
}

/// Tách câu lệnh còn lại dựa trên vị trí phát hiện từ khóa
fn clean_remainder_by_unaccented(
    raw: &str,
    unaccented: &str,
    _unacc_start: usize,
    unacc_end: usize,
) -> String {
    let raw_words: Vec<&str> = raw.split_whitespace().collect();
    let unacc_words: Vec<&str> = unaccented.split_whitespace().collect();
    if raw_words.is_empty() {
        return String::new();
    }

    let mut current_offset = 0;
    let mut matched_end_word = 0;

    for (idx, word) in unacc_words.iter().enumerate() {
        let word_start = unaccented[current_offset..]
            .find(word)
            .map(|pos| pos + current_offset)
            .unwrap_or(current_offset);
        let word_end = word_start + word.len();
        current_offset = word_end;

        if word_start < unacc_end {
            matched_end_word = idx + 1;
        }
    }

    let remainder = if matched_end_word < raw_words.len() {
        raw_words[matched_end_word..].join(" ")
    } else {
        String::new()
    };

    let mut clean = remainder
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == ' ' || c == '，' || c == '。')
        .to_string();

    // Loại bỏ các từ đệm mở đầu câu sau từ khóa đánh thức
    let mut loop_count = 0;
    while loop_count < 3 {
        let trimmed = clean.trim().to_string();
        if let Some(m) = FILLER_PREFIX_RE.find(&trimmed) {
            clean = trimmed[m.end()..].trim().to_string();
            loop_count += 1;
        } else {
            break;
        }
    }

    clean
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == ' ' || c == '，' || c == '。')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_alexa_standalone() {
        let res = detect_wake_word("Alexa");
        assert!(res.is_some());
        let (ww, cmd) = res.unwrap();
        assert_eq!(ww, WakeWordType::Alexa);
        assert_eq!(cmd, "");
    }

    #[test]
    fn test_detect_alexa_compound() {
        let res = detect_wake_word("Alexa mấy giờ rồi");
        assert!(res.is_some());
        let (ww, cmd) = res.unwrap();
        assert_eq!(ww, WakeWordType::Alexa);
        assert_eq!(cmd, "mấy giờ rồi");
    }

    #[test]
    fn test_detect_vietnamese_accent_homophone() {
        let res = detect_wake_word("A lếch xa ơi, thời tiết hôm nay thế nào");
        assert!(res.is_some());
        let (ww, cmd) = res.unwrap();
        assert_eq!(ww, WakeWordType::Alexa);
        assert_eq!(cmd, "thời tiết hôm nay thế nào");
    }

    #[test]
    fn test_detect_assistant() {
        let res = detect_wake_word("Trợ lý ơi, kiểm tra pin");
        assert!(res.is_some());
        let (ww, cmd) = res.unwrap();
        assert_eq!(ww, WakeWordType::Assistant);
        assert_eq!(cmd, "kiểm tra pin");
    }

    #[test]
    fn test_detect_jarvis() {
        let res = detect_wake_word("Hey Jarvis, mở visual studio code");
        assert!(res.is_some());
        let (ww, cmd) = res.unwrap();
        assert_eq!(ww, WakeWordType::Jarvis);
        assert_eq!(cmd, "mở visual studio code");
    }

    #[test]
    fn test_non_wake_word_ignored() {
        assert_eq!(detect_wake_word("Hôm nay trời đẹp quá bạn nhỉ"), None);
        assert_eq!(detect_wake_word("Tôi muốn đi uống cà phê"), None);
    }
}
