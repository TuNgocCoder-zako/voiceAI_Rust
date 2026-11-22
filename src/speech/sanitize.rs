use regex::Regex;
use std::sync::LazyLock;

static CJK_EMOJI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef\x{10000}-\x{10ffff}]").unwrap()
});

static THINK_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<think>.*?</think>").unwrap()
});

static UNCLOSED_THINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<think>.*$").unwrap()
});

static DA_TYPO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[Dd]à\b").unwrap()
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

    // Loại bỏ thẻ suy nghĩ <think>...</think> (kể cả khi bị cắt cụt chưa kịp đóng)
    t = THINK_TAG_RE.replace_all(&t, "").to_string();
    t = UNCLOSED_THINK_RE.replace_all(&t, "").to_string();

    // Chuẩn hóa 'Dà' -> 'Dạ'
    t = DA_TYPO_RE.replace_all(&t, "Dạ").to_string();

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

/// Tách luồng token từ Ollama thành từng câu hoàn chỉnh theo thời gian thực để đọc TTS ngay,
/// đồng thời lọc bỏ khối `<think>...</think>` ngay cả khi thẻ bị tách ngang qua nhiều chunk.
pub struct SentenceStreamSplitter {
    raw_pending: String,
    visible_buf: String,
    in_think: bool,
}

impl SentenceStreamSplitter {
    pub fn new() -> Self {
        Self {
            raw_pending: String::new(),
            visible_buf: String::new(),
            in_think: false,
        }
    }

    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        self.raw_pending.push_str(chunk);
        self.consume_think_tags();
        self.extract_ready_sentences()
    }

    pub fn flush(&mut self) -> Vec<String> {
        // Nếu còn ký tự bình thường trong raw_pending và không nằm trong <think>
        if !self.in_think && !self.raw_pending.is_empty() {
            if !self.raw_pending.starts_with("<think") {
                self.visible_buf.push_str(&self.raw_pending);
            }
            self.raw_pending.clear();
        }

        let mut out = self.extract_ready_sentences();
        let rem = self.visible_buf.trim().to_string();
        self.visible_buf.clear();
        if !rem.is_empty() {
            out.push(rem);
        }
        out
    }

    fn consume_think_tags(&mut self) {
        loop {
            if self.in_think {
                if let Some(end_pos) = self.raw_pending.find("</think>") {
                    self.raw_pending.drain(..end_pos + "</think>".len());
                    self.in_think = false;
                } else {
                    // Chỉ giữ lại tối đa 8 byte cuối phòng trường hợp "</think>" đang về dở
                    if self.raw_pending.len() > 8 {
                        let keep_from = self
                            .raw_pending
                            .char_indices()
                            .rev()
                            .take(8)
                            .last()
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);
                        self.raw_pending.drain(..keep_from);
                    }
                    break;
                }
            } else if let Some(start_pos) = self.raw_pending.find("<think>") {
                self.visible_buf.push_str(&self.raw_pending[..start_pos]);
                self.raw_pending.drain(..start_pos + "<think>".len());
                self.in_think = true;
            } else {
                // Nếu đuôi chuỗi có thể là tiền tố của "<think>" (vd: "<thi"), giữ phần từ '<' trở đi
                if let Some(lt_pos) = self.raw_pending.rfind('<') {
                    let tail = &self.raw_pending[lt_pos..];
                    if "<think>".starts_with(tail) {
                        self.visible_buf.push_str(&self.raw_pending[..lt_pos]);
                        self.raw_pending.drain(..lt_pos);
                        break;
                    }
                }
                self.visible_buf.push_str(&self.raw_pending);
                self.raw_pending.clear();
                break;
            }
        }
    }

    fn extract_ready_sentences(&mut self) -> Vec<String> {
        let mut sentences = Vec::new();
        loop {
            let mut split_idx = None;
            let chars: Vec<(usize, char)> = self.visible_buf.char_indices().collect();

            for i in 0..chars.len() {
                let (byte_idx, ch) = chars[i];
                let is_boundary = match ch {
                    '\n' => byte_idx >= 6,
                    '!' | '?' | ';' => byte_idx >= 12,
                    '.' => {
                        let prev_digit = i > 0 && chars[i - 1].1.is_ascii_digit();
                        let next_digit = i + 1 < chars.len() && chars[i + 1].1.is_ascii_digit();
                        let next_is_space_or_nl =
                            i + 1 < chars.len() && chars[i + 1].1.is_whitespace();
                        !prev_digit && !next_digit && next_is_space_or_nl && byte_idx >= 18
                    }
                    _ => false,
                };

                if is_boundary {
                    split_idx = Some(byte_idx + ch.len_utf8());
                    break;
                }
            }

            if let Some(idx) = split_idx {
                let piece: String = self.visible_buf.drain(..idx).collect();
                let trimmed = piece.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
            } else {
                break;
            }
        }
        sentences
    }
}

impl Default for SentenceStreamSplitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentence_stream_splitter_think_and_decimals() {
        let mut sp = SentenceStreamSplitter::new();
        let mut collected = Vec::new();
        for chunk in [
            "<thi",
            "nk>Đang suy luận nội bộ.</thi",
            "nk>RAM hiện còn 3.5 GB trống. ",
            "Hệ thống chạy rất mượt mà!",
        ] {
            collected.extend(sp.push(chunk));
        }
        collected.extend(sp.flush());
        assert_eq!(
            collected,
            vec![
                "RAM hiện còn 3.5 GB trống.",
                "Hệ thống chạy rất mượt mà!"
            ]
        );
    }

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
