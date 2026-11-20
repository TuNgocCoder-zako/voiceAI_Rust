use regex::Regex;
use std::sync::LazyLock;

static CJK_EMOJI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef\x{10000}-\x{10ffff}]").unwrap()
});

static THINK_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<think>.*?</think>").unwrap()
});

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://\S+").unwrap()
});

static MARKDOWN_BOLD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\*\*([^*]+)\*\*").unwrap()
});

static MARKDOWN_ITALIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\*([^*]+)\*").unwrap()
});

static MARKDOWN_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"`([^`]+)`").unwrap()
});

static MARKDOWN_SYMBOLS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[#*_~>]").unwrap()
});

static PERCENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+)\s*%").unwrap()
});

static TEMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+)\s*°[Cc]").unwrap()
});

static TIME_COLON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{1,2}):(\d{2})\b").unwrap()
});

static MEM_GB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*GB\b").unwrap()
});

static MEM_MB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*MB\b").unwrap()
});

static DECIMAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+)\.(\d+)").unwrap()
});

static BRACKETS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\[\]\(\)\{\}]").unwrap()
});

static MULTI_COMMAS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r",\s*,+").unwrap()
});

static MULTI_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+").unwrap()
});

/// Chuẩn hóa văn bản thành tiếng Việt ngữ âm tự nhiên trước khi đưa vào TTS:
/// - Chuyển đổi số học, phần trăm (100% -> 100 phần trăm).
/// - Chuyển đổi đơn vị (GB -> ghi ga, °C -> độ C).
/// - Chuyển đổi thời gian (14:30 -> 14 giờ 30 phút).
/// - Chuyển đổi số thập phân (5.1 -> 5 phẩy 1).
/// - Loại bỏ markdown, URL, ký tự CJK / emoji.
pub fn sanitize_text_for_voice(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut t = text.to_string();

    // Loại bỏ thẻ suy nghĩ <think>...</think> của các mô hình reasoning
    t = THINK_TAG_RE.replace_all(&t, "").to_string();

    // Chuẩn hóa 'Dà' -> 'Dạ'
    t = Regex::new(r"\b[Dd]à\b").unwrap().replace_all(&t, "Dạ").to_string();

    // Xóa CJK và emoji
    t = CJK_EMOJI_RE.replace_all(&t, "").to_string();

    // Xóa URL
    t = URL_RE.replace_all(&t, "").to_string();

    // Markdown
    t = MARKDOWN_BOLD_RE.replace_all(&t, "$1").to_string();
    t = MARKDOWN_ITALIC_RE.replace_all(&t, "$1").to_string();
    t = MARKDOWN_CODE_RE.replace_all(&t, "$1").to_string();
    t = MARKDOWN_SYMBOLS_RE.replace_all(&t, "").to_string();

    // Phần trăm & nhiệt độ & thời gian
    t = PERCENT_RE.replace_all(&t, "$1 phần trăm").to_string();
    t = TEMP_RE.replace_all(&t, "$1 độ C").to_string();
    t = TIME_COLON_RE.replace_all(&t, "$1 giờ $2 phút").to_string();

    // RAM / Bộ nhớ
    t = MEM_GB_RE.replace_all(&t, "$1 ghi ga").to_string();
    t = MEM_MB_RE.replace_all(&t, "$1 mê ga").to_string();

    // Số thập phân
    t = DECIMAL_RE.replace_all(&t, "$1 phẩy $2").to_string();

    // Dấu ngoặc
    t = BRACKETS_RE.replace_all(&t, ", ").to_string();
    t = MULTI_COMMAS_RE.replace_all(&t, ",").to_string();

    MULTI_SPACE_RE.replace_all(&t, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_text_for_voice() {
        let input = "Pin hiện tại còn 85% và RAM trống 4.2 GB lúc 14:30, nhiệt độ 28°C.";
        let out = sanitize_text_for_voice(input);
        assert_eq!(
            out,
            "Pin hiện tại còn 85 phần trăm và RAM trống 4 phẩy 2 ghi ga lúc 14 giờ 30 phút, nhiệt độ 28 độ C."
        );
    }

    #[test]
    fn test_sanitize_strips_think_tags() {
        let input = "<think>\nĐang suy luận logic...\n</think>\nChào bạn, tôi có thể giúp gì?";
        let out = sanitize_text_for_voice(input);
        assert_eq!(out, "Chào bạn, tôi có thể giúp gì?");
    }
}
