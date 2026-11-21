use crate::core::nlu::normalize::strip_vietnamese_accents;
use strsim::sorensen_dice;

pub static CANONICAL_ACTIONS: &[(&str, &str)] = &[
    ("bat nhac", "bật nhạc"),
    ("phat nhac", "phát nhạc"),
    ("tat nhac", "tắt nhạc"),
    ("dung nhac", "dừng nhạc"),
    ("tam dung nhac", "tạm dừng nhạc"),
    ("tam dung", "tạm dừng"),
    ("tiep tuc", "tiếp tục"),
    ("tiep tuc phat nhac", "tiếp tục phát nhạc"),
    ("bai tiep theo", "bài tiếp theo"),
    ("chuyen bai", "chuyển bài"),
    ("chuyen bai hat", "chuyển bài hát"),
    ("doi bai", "đổi bài"),
    ("bai truoc", "bài trước"),
    ("tang am luong", "tăng âm lượng"),
    ("giam am luong", "giảm âm lượng"),
    ("tat am thanh", "tắt tiếng"),
    ("tat tieng", "tắt tiếng"),
    ("bat lai tieng", "bật lại tiếng"),
    ("bat lai am thanh", "bật lại tiếng"),
    ("tang do sang", "tăng độ sáng"),
    ("giam do sang", "giảm độ sáng"),
    ("may gio roi", "mấy giờ rồi"),
    ("hom nay ngay may", "hôm nay ngày mấy"),
    ("kiem tra pin", "kiểm tra pin"),
    ("pin con bao nhieu", "pin còn bao nhiêu"),
    ("kiem tra mang", "kiểm tra mạng"),
    ("toc do mang", "tốc độ mạng"),
    ("chup man hinh", "chụp màn hình"),
    ("dong cua so", "đóng cửa sổ"),
    ("tat ung dung", "tắt ứng dụng"),
    ("phong to", "phóng to"),
    ("thu nho", "thu nhỏ"),
    ("chia doi man hinh", "chia đôi màn hình"),
    ("chia doc", "chia dọc"),
    ("chia ngang", "chia ngang"),
    ("huy hen gio", "hủy hẹn giờ"),
    ("kiem tra hen gio", "kiểm tra hẹn giờ"),
    ("doc clipboard", "đọc clipboard"),
];

pub fn fuzzy_match_canonical_action(text: &str, threshold: f64) -> Option<(&'static str, f64)> {
    let unacc = strip_vietnamese_accents(text).to_lowercase();
    let unacc = unacc.trim();
    if unacc.len() < 4 {
        return None;
    }

    let mut best_match: Option<&'static str> = None;
    let mut best_ratio = 0.0;

    for &(canon_unacc, canon_acc) in CANONICAL_ACTIONS {
        let ratio = sorensen_dice(unacc, canon_unacc);
        if ratio > best_ratio {
            best_ratio = ratio;
            best_match = Some(canon_acc);
        }
    }

    if best_ratio >= threshold {
        best_match.map(|m| (m, best_ratio))
    } else {
        None
    }
}
