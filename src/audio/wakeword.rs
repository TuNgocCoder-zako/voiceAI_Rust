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
// Bao quát toàn diện các biến thể ngữ âm học mà Whisper thường nhận diện khi người Việt gọi:
// Alexa, Alex, Alessia, A Lêsê, A lếch, A léc, A lek, A lếch xa, A léc xa, 2 xa, Hai xa, Ai xa, A xa, A lê sạch, A lai xa...
static UNACCENTED_ALEXA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:hey\s+|alo\s+|oi\s+|e\s+|nay\s+)?(?:alexa|alecxa|aleksa|alex|alec|alek|alisa|elisa|alipsa|alipxa|alica|alicka|alixa|alessia|alesia|aleccia|alesa|a\s*les\s*e|a\s*le\s*se|(?:a|e|o)\s*[-_]?\s*(?:lech|lec|lek|let|les|lip|lich|lic|lik|lai|lay)(?:\s*[-_]?\s*(?:xa|sa|ca|se|ce|xe))?|(?:a\s*|e\s*)?(?:le|lech|lec)\s*[-_]?\s*(?:xa|sa|ca|sach|say|se|ce|xe)|(?:2|hai|ai|ay|a|e)\s*[-_]?\s*(?:xa|sa|se)|tuyet\s*(?:soi|so|sa|xoi|xa)))\b").unwrap()
});

static UNACCENTED_JARVIS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:hey\s+|alo\s+|oi\s+|e\s+|nay\s+|he\s+|hai\s+|hi\s+)?(?:jarvis|za\s*vit|gia\s*vit|da\s*vit|za-vit|gia-vit|da\s*ria|gia\s*ria|cha\s*vit|tra\s*vit|ha\s*vit))\b").unwrap()
});

static UNACCENTED_ASSISTANT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:hey\s+|alo\s+|oi\s+|e\s+|nay\s+)?(?:tro|cho|co|do|chao)\s*(?:ly|lay|li|loi|le)(?:\s*(?:oi|o|a|e|ha|nhe|nao))?|troi\s*oi|(?:em|ban|chi)\s*oi)\b").unwrap()
});

static FILLER_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(ơi|à|a|ấy|ay|này|hãy|cho\s*mình\s*hỏi|cho\s*tôi\s*hỏi|làm\s*ơn|giúp\s*mình|giúp\s*tôi)[,\s]+").unwrap()
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

    // 3. Kiểm tra Trợ lý ơi / Trợ lý
    if let Some(m) = UNACCENTED_ASSISTANT_RE.find(&unaccented) {
        // Tránh tự kích hoạt lặp nếu câu chứa "trợ lý ảo" (TTS của bot tự xưng)
        let unacc_rem = unaccented[m.end()..].trim_start();
        if unacc_rem.starts_with("ao") {
            return None;
        }

        // Nếu từ khóa nằm ở giữa/cuối câu dài (m.start() > 0):
        // Chỉ chấp nhận nếu phía trước là từ gọi/chào (alo, hey, này, ê, ơi...)
        // hoặc câu ngắn chỉ có 1-2 từ (vd: "Trợ lý", "Alo trợ lý")
        let raw_words_count = raw.split_whitespace().count();
        if m.start() > 0 && raw_words_count > 2 {
            let prefix = unaccented[..m.start()].trim();
            let is_greeting = prefix == "hey"
                || prefix == "alo"
                || prefix == "oi"
                || prefix == "e"
                || prefix == "nay"
                || prefix == "chao";
            if !is_greeting {
                return None;
            }
        }

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

    let mut clean = clean
        .trim_matches(|c: char| c.is_ascii_punctuation() || c == ' ' || c == '，' || c == '。')
        .trim()
        .to_string();

    // Loại bỏ lặp từ khóa đánh thức nếu người dùng gọi liên tiếp (ví dụ: "Alexa, Alexa", "Trợ lý ơi, Trợ lý ơ")
    if !clean.is_empty() {
        let unacc_clean = strip_vietnamese_accents(&clean).to_lowercase();
        if UNACCENTED_ALEXA_RE.is_match(&unacc_clean)
            || UNACCENTED_JARVIS_RE.is_match(&unacc_clean)
            || UNACCENTED_ASSISTANT_RE.is_match(&unacc_clean)
        {
            if let Some((_, sub_cmd)) = detect_wake_word(&clean) {
                clean = sub_cmd;
            }
        }
    }

    clean
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_repeated_wake_word() {
        let (w1, c1) = detect_wake_word("Trợ lý ơi, Trợ lý ơ").unwrap();
        assert_eq!(w1, WakeWordType::Assistant);
        assert_eq!(c1, "");

        let (w2, c2) = detect_wake_word("Alexa, Alexa mấy giờ rồi").unwrap();
        assert_eq!(w2, WakeWordType::Alexa);
        assert_eq!(c2, "mấy giờ rồi");

        let (w3, c3) = detect_wake_word("Alexa Alexa").unwrap();
        assert_eq!(w3, WakeWordType::Alexa);
        assert_eq!(c3, "");
    }

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

        // Các biến thể phát âm khác của Alexa
        let cases = [
            ("A lếch", ""),
            ("A léc sa", ""),
            ("Alex à mấy giờ rồi", "mấy giờ rồi"),
            ("Alexa à mấy giờ rồi", "mấy giờ rồi"),
            ("Alex, mấy giờ rồi", "mấy giờ rồi"),
            ("A lếch, mở trình duyệt", "mở trình duyệt"),
            ("Aleksa ơi, pin còn bao nhiêu", "pin còn bao nhiêu"),
            ("Alo Alexa", ""),
            ("Ê lếch xa", ""),
            ("2 xa chạm pin còn bao nhiêu", "chạm pin còn bao nhiêu"),
            ("Ai xa mấy giờ rồi", "mấy giờ rồi"),
            ("A xa mở trình duyệt", "mở trình duyệt"),
            ("A lai xa mấy giờ rồi", "mấy giờ rồi"),
            ("A lê sạch", ""),
            ("Alessia", ""),
            ("Alessia mấy giờ rồi", "mấy giờ rồi"),
            ("A LÊSÊ!", ""),
            ("A LÊSÊ mấy giờ rồi", "mấy giờ rồi"),
            ("Lê xa", ""),
            ("Lê xa mấy giờ rồi", "mấy giờ rồi"),
        ];
        for (input, expected_cmd) in cases {
            let r = detect_wake_word(input);
            assert!(r.is_some(), "Không nhận diện được wake word từ '{}'", input);
            let (w, c) = r.unwrap();
            assert_eq!(w, WakeWordType::Alexa);
            assert_eq!(c, expected_cmd, "Sai cmd từ '{}'", input);
        }
    }

    #[test]
    fn test_detect_assistant() {
        let cases = [
            ("Trợ lý", ""),
            ("Trợ lý ơi", ""),
            ("Trợ lý ơi, kiểm tra pin", "kiểm tra pin"),
            ("Trợ lý ơ, mấy giờ rồi", "mấy giờ rồi"),
            ("Trợ lý à", ""),
            ("Trợ lý mấy giờ rồi", "mấy giờ rồi"),
            ("chợ lấy ơi, mở visual studio code", "mở visual studio code"),
            ("Cỡ lý ơi!", ""),
            ("Cơ lý", ""),
            ("Cô lấy", ""),
            ("Đó lý", ""),
            ("trò lấy", ""),
            ("Chào lời", ""),
            ("Trời ơi!", ""),
            ("Alo trợ lý", ""),
            ("Em ơi mấy giờ rồi", "mấy giờ rồi"),
        ];
        for (input, expected_cmd) in cases {
            let res = detect_wake_word(input);
            assert!(res.is_some(), "Không nhận diện được assistant từ '{}'", input);
            let (ww, cmd) = res.unwrap();
            assert_eq!(ww, WakeWordType::Assistant);
            assert_eq!(cmd, expected_cmd, "Sai cmd từ '{}'", input);
        }
    }

    #[test]
    fn test_detect_jarvis() {
        let cases = [
            ("Hey Jarvis, mở visual studio code", "mở visual studio code"),
            ("Hey Jarvis", ""),
            ("Hai Zavit", ""),
            ("Đà rìa mấy giờ rồi", "mấy giờ rồi"),
            ("Za vít", ""),
        ];
        for (input, expected_cmd) in cases {
            let res = detect_wake_word(input);
            assert!(res.is_some(), "Không nhận diện được jarvis từ '{}'", input);
            let (ww, cmd) = res.unwrap();
            assert_eq!(ww, WakeWordType::Jarvis);
            assert_eq!(cmd, expected_cmd, "Sai cmd từ '{}'", input);
        }
    }

    #[test]
    fn test_non_wake_word_ignored() {
        assert_eq!(detect_wake_word("Hôm nay trời đẹp quá bạn nhỉ"), None);
        assert_eq!(detect_wake_word("Tôi muốn đi uống cà phê"), None);
        assert_eq!(detect_wake_word("Tôi là trợ lý ảo của bạn"), None);
        assert_eq!(detect_wake_word("giám đốc và trợ lý"), None);
        assert_eq!(detect_wake_word("Đây là trợ lý"), None);
        assert_eq!(detect_wake_word("Tôi muốn gặp trợ lý"), None);
    }
}
