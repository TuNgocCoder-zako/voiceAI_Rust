use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Loại bỏ toàn bộ dấu tiếng Việt (đ/Đ -> d/D, NFD diacritics removal -> NFC).
pub fn strip_vietnamese_accents(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let replaced = text.replace('đ', "d").replace('Đ', "D");
    let nfd: String = replaced
        .nfd()
        .filter(|&c| !is_combining_mark(c))
        .collect();
    nfd.nfc().collect::<String>().trim().to_string()
}

fn is_combining_mark(c: char) -> bool {
    // Unicode Nonspacing Mark (Mn) range check for common Vietnamese diacritics
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

pub struct ReplacementRule {
    pub pattern: Regex,
    pub replacement: &'static str,
}

static UNACCENTED_INTENT_MAP: LazyLock<Vec<ReplacementRule>> = LazyLock::new(|| {
    let rules: &[(&str, &str)] = &[
        // Hyprland & màn hình
        (r"(?i)\bchia\s+doi\s+man\s+hinh\b", "chia đôi màn hình"),
        (r"(?i)\bchia\s+doi\b", "chia đôi"),
        (r"(?i)\bchia\s+ngang\b", "chia ngang"),
        (r"(?i)\bchia\s+doc\b", "chia dọc"),
        (r"(?i)\bphong\s+to\s+len\b", "phóng to lên"),
        (r"(?i)\bphong\s+to\s+cua\s+so\b", "phóng to cửa sổ"),
        (r"(?i)\bphong\s+to\s+ung\s+dung\b", "phóng to ứng dụng"),
        (r"(?i)\bphong\s+to\b", "phóng to"),
        (r"(?i)\bthu\s+nho\s+cua\s+so\b", "thu nhỏ cửa sổ"),
        (r"(?i)\bthu\s+nho\s+ung\s+dung\b", "thu nhỏ ứng dụng"),
        (r"(?i)\bthu\s+nho\b", "thu nhỏ"),
        (r"(?i)\bchuyen\s+workspace\b", "chuyển workspace"),
        (r"(?i)\bchuyen\s+khong\s+gian\s+lam\s+viec\b", "chuyển không gian làm việc"),
        (r"(?i)\bkhong\s+gian\s+lam\s+viec\b", "không gian làm việc"),
        (r"(?i)\bchup\s+man\s+hinh\b", "chụp màn hình"),
        (r"(?i)\btoan\s+man\s+hinh\b", "toàn màn hình"),
        // Cửa sổ & ứng dụng
        (r"(?i)\bdong\s+cua\s+so\b", "đóng cửa sổ"),
        (r"(?i)\btat\s+cua\s+so\b", "tắt cửa sổ"),
        (r"(?i)\btat\s+ung\s+dung\b", "tắt ứng dụng"),
        (r"(?i)\bdong\s+ung\s+dung\b", "đóng ứng dụng"),
        (r"(?i)\bthoat\s+ung\s+dung\b", "thoát ứng dụng"),
        (r"(?i)\btat\s+spotify\b", "tắt spotify"),
        (r"(?i)\bdong\s+spotify\b", "đóng spotify"),
        (r"(?i)\btat\s+spotfy\b", "tắt spotify"),
        (r"(?i)\bdong\s+spotfy\b", "đóng spotify"),
        (r"(?i)\bthoat\s+spotfy\b", "thoát spotify"),
        (r"(?i)\btat\s+no(?:\s+di)?\b", "tắt nó"),
        (r"(?i)\bdong\s+no(?:\s+lai)?\b", "đóng nó"),
        (r"(?i)\btat\s+cai\s+nay\b", "tắt cái này"),
        (r"(?i)\bdong\s+cai\s+nay\b", "đóng cái này"),
        (r"(?i)\bphong\s+to\s+no(?:\s+len)?\b", "phóng to nó lên"),
        (r"(?i)\bcho\s+no\s+to\s+len\b", "cho nó to lên"),
        (r"(?i)\bthu\s+nho\s+no(?:\s+lai)?\b", "thu nhỏ nó lại"),
        (r"(?i)\bchuyen\s+no\s+sang\b", "chuyển nó sang"),
        (r"(?i)\bthoat\b", "thoát"),
        (r"(?i)\btrinh\s+duyet\b", "trình duyệt"),
        // Thời gian & Thời tiết
        (r"(?i)\bmay\s+gio\s+roi\b", "mấy giờ rồi"),
        (r"(?i)\bmay\s+gio\b", "mấy giờ"),
        (r"(?i)\bbay\s+gio\s+la\s+may\s+gio\b", "bây giờ là mấy giờ"),
        (r"(?i)\bngay\s+may\b", "ngày mấy"),
        (r"(?i)\bhom\s+nay\b", "hôm nay"),
        (r"(?i)\bthoi\s+tiet\b", "thời tiết"),
        // Pin & Hệ thống & Mạng
        (r"(?i)\bkiem\s+tra\s+pin\b", "kiểm tra pin"),
        (r"(?i)\bpin\s+con\s+bao\s+nhieu\b", "pin còn bao nhiêu"),
        (r"(?i)\bkiem\s+tra\s+mang\b", "kiểm tra mạng"),
        (r"(?i)\btoc\s+do\s+mang\b", "tốc độ mạng"),
        (r"(?i)\bkiem\s+tra\s+toc\s+do\s+mang\b", "kiểm tra tốc độ mạng"),
        (r"(?i)\bdoc\s+clipboard\b", "đọc clipboard"),
        (r"(?i)\bbo\s+nho\s+tam\b", "bộ nhớ tạm"),
        (r"(?i)\bkiem\s+tra\s+git\b", "kiểm tra git"),
        // Âm lượng & Độ sáng
        (r"(?i)\btang\s+am\s+luong\b", "tăng âm lượng"),
        (r"(?i)\bgiam\s+am\s+luong\b", "giảm âm lượng"),
        (r"(?i)\bam\s+luong\b", "âm lượng"),
        (r"(?i)\btat\s+tieng\b", "tắt tiếng"),
        (r"(?i)\bbat\s+lai\s+tieng\b", "bật lại tiếng"),
        (r"(?i)\btang\s+do\s+sang\b", "tăng độ sáng"),
        (r"(?i)\bgiam\s+do\s+sang\b", "giảm độ sáng"),
        (r"(?i)\bdo\s+sang\b", "độ sáng"),
        // Media
        (r"(?i)\bbat\s+nhac\b", "bật nhạc"),
        (r"(?i)\bphat\s+nhac\b", "phát nhạc"),
        (r"(?i)\btat\s+nhac\b", "tắt nhạc"),
        (r"(?i)\bdung\s+nhac\b", "dừng nhạc"),
        (r"(?i)\btam\s+dung\s+nhac\b", "tạm dừng nhạc"),
        (r"(?i)\btam\s+dung\b", "tạm dừng"),
        (r"(?i)\btiep\s+tuc\s+phat\s+nhac\b", "tiếp tục phát nhạc"),
        (r"(?i)\btiep\s+tuc\b", "tiếp tục"),
        (r"(?i)\bchuyen\s+bai\s+hat\b", "chuyển bài hát"),
        (r"(?i)\bchuyen\s+bai\b", "chuyển bài"),
        (r"(?i)\bdoi\s+bai\b", "đổi bài"),
        (r"(?i)\bbai\s+truoc\b", "bài trước"),
        (r"(?i)\bbai\s+tiep\s+theo\b", "bài tiếp theo"),
        // Hẹn giờ
        (r"(?i)\bhen\s+gio\b", "hẹn giờ"),
        (r"(?i)\bhuy\s+hen\s+gio\b", "hủy hẹn giờ"),
        (r"(?i)\bdat\s+bao\s+thuc\b", "đặt báo thức"),
        (r"(?i)\bbao\s+thuc\b", "báo thức"),
        // Từ đơn hành động
        (r"(?i)\bmo\b", "mở"),
        (r"(?i)\bbat\b", "bật"),
        (r"(?i)\btat\b", "tắt"),
        (r"(?i)\bdong\b", "đóng"),
        (r"(?i)\bchay\b", "chạy"),
        (r"(?i)\bchuyen\b", "chuyển"),
        (r"(?i)\bdoi\b", "đổi"),
        (r"(?i)\bchup\b", "chụp"),
        (r"(?i)\btang\b", "tăng"),
        (r"(?i)\bgiam\b", "giảm"),
        (r"(?i)\bnhac\b", "nhạc"),
        (r"(?i)\bcua\s+so\b", "cửa sổ"),
        (r"(?i)\bman\s+hinh\b", "màn hình"),
        (r"(?i)\bung\s+dung\b", "ứng dụng"),
        (r"(?i)\bphut\b", "phút"),
        (r"(?i)\bgiay\b", "giây"),
        (r"(?i)\bgio\b", "giờ"),
    ];

    rules
        .iter()
        .map(|&(p, r)| ReplacementRule {
            pattern: Regex::new(p).expect("Valid regex"),
            replacement: r,
        })
        .collect()
});

static PHONETIC_CLEANUP_RULES: LazyLock<Vec<ReplacementRule>> = LazyLock::new(|| {
    let rules: &[(&str, &str)] = &[
        // Wake-word removal
        (r"(?i)^(?:ê\s+|này\s+|ơi\s+|chào\s+)*alexa(?:\s+ơi)?[\s,\.!\?]+", ""),
        (r"(?i)^(?:ê\s+|này\s+|ơi\s+|chào\s+)*jarvis(?:\s+ơi)?[\s,\.!\?]+", ""),
        (r"(?i)^(?:à|ừm|ừ|này|ê)[\s,\.!\?]+", ""),
        // Diacritics "của sổ" -> "cửa sổ"
        (r"(?i)\bc[ủũụùú]a\s+s[ổỗộồốo]\b", "cửa sổ"),
        // "m ở", "em ở", "ê ở" -> "mở"
        (r"(?i)\b(?:m\s+[ởờỡớơ]|em\s+[ởờỡớơ]|ê\s+[ởờỡớơ])\b", "mở"),
        // CJK removal (Whisper hallucinations)
        (r"[\u4e00-\u9fff\u3040-\u30ff]", ""),
        // "mỡ/mớ/mờ" -> "mở"
        (r"(?i)\b(?:mỡ|mớ|mờ|mợ|mơ|mỗi|mởn|mỡn|mớn|mờn|mợn|mởng|mẫn|mởm)\b", "mở"),
        // App names & confusion
        (r"(?i)\b(?:gour\s*miel|gourmiel|gour\s*meo|gour\s*mail)\b", "gmail"),
        (r"(?i)\b(?:g[ií]t\s*(?:h[uú]p|h[aá]p|hub|hup|hap|h[uú]c|h[oó]p|h[oớ]p|h[aặ]p)|gít\s*húp|gh)\b", "github"),
        (r"(?i)\b(?:ph[âe]y\s*(?:b[uú][ckpt]|book|booc|bô|bô\s*k)?|phe\s*b[uú][ckpt]|fb|phai\s*bước)\b", "facebook"),
        (r"(?i)\b(?:du\s*túp|dút\s*túp|diu\s*túp|diu\s*tup|du\s*tup|dút\s*tup|iu\s*túp|iu\s*tup|du\s*tốc|dưu\s*tốc|diu\s*tốc|vô\s*tấp|vô\s*túp|dô\s*túp|dô\s*tấp|zu\s*túp|giữ\s*tập|giu\s*tập|du\s*tập|diu\s*tập)\b", "youtube"),
        (r"(?i)\b(?:da\s*lô|za\s*lô|dép\s*lào)\b", "zalo"),
        (r"(?i)\b(?:spotfy|spoty|spotifi|supportify|xì\s*po\s*ti\s*phai|sì\s*po\s*ti\s*phai|bót\s*ti\s*phai|sờ\s*po\s*ti)\b", "spotify"),
        (r"(?i)\b(?:téc\s*mi\s*nan|tẹc\s*mi\s*nan|tẹt\s*mi\s*nan|téc\s*min\s*nồ)\b", "terminal"),
        (r"(?i)\b(?:gu\s*gồ|gúc\s*gồ|gô\s*gồ)\b", "google"),
        (r"(?i)\b(?:vai\s*phai|quai\s*phai|quy\s*phuy)\b", "wifi"),
        (r"(?i)\b(?:lu\s*tút|bu\s*tút|bờ\s*lu\s*tút)\b", "bluetooth"),
        (r"(?i)\b(?:đốc\s*cơ|đốc\s*kơ)\b", "docker"),
        (r"(?i)\b(?:vi\s*ét\s*cốt|vi\s*ét\s*code|vê\s*ét\s*cốt|vé\s*code|vét\s*cốt|phi\s*ét\s*cốt)\b", "vs code"),
        (r"(?i)\b(?:phai|fai|phay|fay|phi|pha|fire|file)[-\s]*(?:fox|fóc|phóc|phốc|phót|phó|phọt|phốt|phot|phoc|phóp|pho)\b", "firefox"),
        (r"(?i)\b(?:cáo\s*lửa|cáo\s*đỏ|hỏa\s*hồ)\b", "firefox"),
        // Chuẩn hóa "má / mà" -> "mở" CHỈ KHI đứng ngay trước tên app/web phổ biến
        (r"(?i)\b(?:má|mà|mả|mã)\s+(youtube|facebook|google|gmail|github|zalo|spotify|chrome|firefox|vs\s*code|terminal)\b", "mở $1"),
        // Whisper hearing "Firefox" as "5 phút"
        (r"(?i)\b(?:mở|bật|chạy|vào|khởi động)\s+(?:5\s*phút|năm\s*phút|một\s*5\s*phút|1\s*5\s*phút|5p|năm\s*p|5\s*phúc|năm\s*phúc)\b", "mở firefox"),
        // Âm lượng & độ sáng
        (r"(?i)\b(?:tính|tinh|thanh|tênh|tánh|thính|tăn|tắng|tằng|tây|tay)\s*(?:,\s*)?(?:âm|ấm|ngâm|ôm)\s*(?:nượng|nương|lương|lường|lưỡng|lượng|lửa|lồ|lộ|lỗ)\b", "tăng âm lượng"),
        (r"(?i)\b(?:giảm|dảm|dàm)\s*(?:âm\s*nượng|âm\s*nương|âm\s*lương|âm\s*lường|âm\s*lưỡng)\b", "giảm âm lượng"),
        (r"(?i)\b(?:âm\s*nượng|âm\s*nương)\b", "âm lượng"),
        (r"(?i)\b(?:độ\s*xáng)\b", "độ sáng"),
        (r"(?i)\b(?:tua\s*bô|tua\s*bo)\b", "turbo"),
        (r"(?i)\b(?:bát\s*tơ\s*ri|bét\s*tờ\s*ri)\b", "pin"),
        (r"(?i)\b(?:tạm\s*dân|tạm\s*dâng|tạm\s*dung|tạm\s*zừng|tạm\s*zung|tạm\s*dừn|tám\s*dừng)\b", "tạm dừng"),
        (r"(?i)\b(?:dương\s*nhiệt|dừng\s*nhiệt|dừng\s*nhạt|dừng\s*nhẹt)\b", "dừng nhạc"),
        (r"(?i)\b(?:chiếm\s*bay\s*hát|chiếm\s*bài\s*hát|chuyển\s*bay\s*hát|chuyển\s*bài\s*hắc|chiếm\s*bài)\b", "chuyển bài hát"),
        (r"(?i)\b(?:bye|bai|bay)\s*,?\s*tiếp\s*theo\b", "bài tiếp theo"),
        (r"(?i)\b(?:bạch\s*nhạc|bịch\s*nhạc|bình\s*nhạc|bọc\s*nhạc|bấm\s*nhạc)\b", "bật nhạc"),
        (r"(?i)\b(?:tất\s*nhiệt|tắt\s*nhiệt|tắt\s*nhạt)\b", "tắt nhạc"),
        (r"(?i)\b(?:kiểm\s*chạy\s*tìm|kiểm\s*tra\s*tinh|kiểm\s*tra\s*tiên|kiểm\s*tra\s*bên|kiểm\s*tra\s*bin|kiểm\s*tra\s*đi)\b", "kiểm tra pin"),
        (r"(?i)\b(?:tin|bin|bên)\s*còn\s*bao\s*nhiêu\b", "pin còn bao nhiêu"),
        (r"(?i)\b(?:bây\s*giờ\s*rồi|mấy\s*rửa\s*rồi|mấy\s*dở\s*rồi|mấy\s*sợ\s*(?:rồi|sôi)|mấy\s*giờ\s*sôi)\b", "mấy giờ rồi"),
        (r"(?i)\bhôm\s+nay,?\s+ngày\s+mỹ\b", "hôm nay ngày mấy"),
        (r"(?i)\b(?:chúc\s*màn\s*hình)\b", "chụp màn hình"),
        (r"(?i)\b(?:quý\s*vị\s*hẹn\s*giờ)\b", "hủy hẹn giờ"),
        (r"(?i)\b(?:uốc\s*(?:xây|bay|sờ\s*bây)|quốc\s*(?:xây|bay|sờ\s*bây))\b", "workspace"),
        (r"(?i)\b(?:híp\s*(?:len|lần|lan|lơn)|hai\s*pơ\s*(?:len|lan))\b", "hyprland"),
        // Gọt từ mở đầu lịch sự
        (r"(?i)^(?:hãy|làm\s+ơn|cho\s+(?:tôi|mình|anh|em)|giúp\s+(?:tôi|mình))\s+", ""),
        // Gọt hậu tố lịch sự/khẩu ngữ ở cuối câu
        (r"(?i)\s+(?:giúp\s+(?:tôi|mình|nhé|nha)|hộ\s+(?:tôi|mình|cái)|cho\s+(?:tôi|mình)|lên\s+đi|lên\s+nhé|lên|đi|nhé|nha|với|ạ)$", ""),
    ];

    rules
        .iter()
        .map(|&(p, r)| ReplacementRule {
            pattern: Regex::new(p).expect("Valid regex"),
            replacement: r,
        })
        .collect()
});

pub static MULTI_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Làm sạch và chuẩn hóa văn bản đầu vào:
/// - Chuẩn hóa NFKC
/// - Tự động khôi phục dấu tiếng Việt từ unaccented map
/// - Lọc nhiễu âm Whisper
pub fn clean_user_input(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let norm = text.nfkc().collect::<String>();
    let mut t = norm.trim().to_string();

    // Khôi phục dấu từ bảng Unaccented Intent
    for rule in UNACCENTED_INTENT_MAP.iter() {
        if rule.pattern.is_match(&t) {
            t = rule.pattern.replace_all(&t, rule.replacement).to_string();
        }
    }

    // Làm sạch dị âm phát âm tiếng Việt
    for rule in PHONETIC_CLEANUP_RULES.iter() {
        if rule.pattern.is_match(&t) {
            t = rule.pattern.replace_all(&t, rule.replacement).to_string();
        }
    }

    let cleaned = MULTI_SPACE_RE.replace_all(&t, " ").trim().to_string();
    if cleaned.is_empty() {
        norm
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_vietnamese_accents() {
        assert_eq!(strip_vietnamese_accents("tăng âm lượng 20%"), "tang am luong 20%");
        assert_eq!(strip_vietnamese_accents("mở visual studio code"), "mo visual studio code");
        assert_eq!(strip_vietnamese_accents("đóng cửa sổ"), "dong cua so");
        assert_eq!(strip_vietnamese_accents("bây giờ là mấy giờ"), "bay gio la may gio");
    }

    #[test]
    fn test_clean_user_input() {
        assert_eq!(clean_user_input("alexa ơi tăng âm lượng"), "tăng âm lượng");
        assert_eq!(clean_user_input("tang am luong"), "tăng âm lượng");
        assert_eq!(clean_user_input("dong cua so"), "đóng cửa sổ");
        assert_eq!(clean_user_input("m ở visual studio code"), "mở visual studio code");
        assert_eq!(clean_user_input("Má giữ tập"), "mở youtube");
    }
}
