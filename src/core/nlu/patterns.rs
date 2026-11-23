use regex::Regex;
use std::sync::LazyLock;

pub static BARGE_IN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(im lặng|dừng lại|dừng nói|thôi im|tắt tiếng đi|im đi)\b").unwrap()
});

pub static IDENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:bạn\s+là\s+ai|bạn\s+tên\s+(?:là\s+)?gì|mày\s+là\s+ai|ai\s+đấy|ai\s+đó|giới\s+thiệu\s+bản\s+thân|bạn\s+là\s+trợ\s+lý\s+gì|bạn\s+làm\s+được\s+gì|bạn\s+có\s+thể\s+làm\s+gì)\b").unwrap()
});

pub static GREETING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:chào\s+bạn|xin\s+chào|chào\s+alexa|hello|hi|hế\s+lô)(?:\s+(?:bạn|nhé|nha|ạ))?$").unwrap()
});

pub static HOW_ARE_YOU_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:bạn\s+khỏe\s+không|dạo\s+này\s+thế\s+nào)(?:\s+(?:alexa|ạ|nhỉ|hả))?$").unwrap()
});

// Time & Date
pub static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(mấy giờ|bây giờ là mấy giờ|xem giờ|thời gian hiện tại|mấy giờ rồi)\b").unwrap()
});

pub static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(hôm nay ngày mấy|hôm nay là ngày mấy|hôm nay ngày bao nhiêu|ngày mấy tháng mấy|hôm nay thứ mấy|hôm nay là thứ mấy|thứ mấy hôm nay|ngày bao nhiêu)\b").unwrap()
});

// Battery
pub static BATTERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(pin còn bao nhiêu|kiểm tra pin|xem pin|tình trạng pin|sạc pin chưa|còn mấy phần trăm pin|mức pin)\b").unwrap()
});

// Git & Clipboard & Net
pub static GIT_STATUS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(kiểm tra git|git status|tình trạng git|trạng thái git|nhánh git hiện tại|nhánh git)\b").unwrap()
});

pub static CLIPBOARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(đọc clipboard|đọc bộ nhớ tạm|bộ nhớ tạm có gì|clipboard có gì|trong clipboard có gì)\b").unwrap()
});

pub static NETWORK_SPEED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(kiểm tra tốc độ mạng|đo tốc độ mạng|tốc độ mạng|tốc độ internet|kiểm tra mạng|đo mạng|đo băng thông|ping mạng)\b").unwrap()
});

// Volume
pub static VOLUME_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:đặt|chỉnh|cài)?\s*âm lượng\s*(?:ở\s*mức\s*|về\s*|lên\s*)?(\d{1,3})\s*(?:%|phần trăm)?").unwrap()
});

pub static VOLUME_UP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(tăng âm lượng|cho to lên|bật to lên|to tiếng hơn|tăng loa|cho to tí|to hơn nữa|bật to loa|cho to loa|to loa lên|tăng âm)\b").unwrap()
});

pub static VOLUME_DOWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(giảm âm lượng|cho nhỏ lại|bật nhỏ lại|nhỏ tiếng hơn|giảm loa|cho nhỏ tí|bé hơn|nhỏ lại|nhỏ bớt|cho nhỏ loa|nhỏ loa lại|giảm âm)\b").unwrap()
});

pub static VOLUME_MUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(tắt tiếng|tắt âm|mute|bật tiếng|bật lại tiếng|ngắt tiếng|mở lại tiếng)\b").unwrap()
});

// Media
pub static MEDIA_INFO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bài\s*(?:hát|này)?\s*(?:tên\s*là\s*gì|tên\s*gì|là\s*bài\s*gì)|đang\s*(?:phát|nghe|bật|chạy)\s*(?:bài|nhạc|bài\s*hát|bài\s*nhạc)?(?:\s*gì)?|tên\s*(?:của\s*)?bài\s*hát|bài\s*hát\s*gì\s*(?:đây|đấy|thế)|bài\s*gì\s*(?:đang\s*phát|thế|đây)|ai\s*đang\s*hát|ca\s*sĩ\s*nào\s*hát|thông\s*tin\s*bài\s*hát|nhạc\s*tên\s*gì|bài\s*tên\s*gì)\b").unwrap()
});

pub static MEDIA_PAUSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(tạm\s*dừng(?:\s*(?:nhạc|bài\s*hát|phát|nha|nhé|lại|cho\s*tôi|hộ))?|dừng\s*(?:nhạc|bài\s*hát|phát|bài|lại|nghe\s*nhạc)?|ngừng\s*(?:nhạc|phát|bài\s*hát)?|tắt\s*(?:nhạc|bài\s*hát)|pause(?:\s*nhạc)?|nghỉ\s*nghe\s*nhạc)\b").unwrap()
});

pub static MEDIA_PLAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(phát\s*tiếp(?:\s*(?:nhạc|bài\s*hát))?|tiếp\s*tục\s*(?:nhạc|phát|nghe\s*nhạc|bài\s*hát)|phát\s*(?:bài\s*)?nhạc\s*tiếp|phát\s*bài\s*tiếp|bật\s*(?:lại\s*)?nhạc(?:\s*tiếp)?|mở\s*(?:lại\s*)?nhạc(?:\s*tiếp)?|play\s*nhạc|phát\s*nhạc|bật\s*nhạc(?:\s*lên)?|nghe\s*tiếp|nghe\s*nhạc\s*tiếp|tiếp\s*tục\s*nghe)\b").unwrap()
});

pub static MEDIA_NEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(chuyển\s*(?:bài(?:\s*hát)?|nhạc)|đổi\s*(?:bài(?:\s*hát)?|nhạc)|bài\s*tiếp\s*(?:theo|kế)?|qua\s*bài|next\s*bài|bài\s*khác|bỏ\s*qua\s*bài|chuyển\s*sang\s*bài\s*tiếp)\b").unwrap()
});

pub static MEDIA_PREV_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bài\s*trước|quay\s*lại\s*bài\s*trước|lùi\s*bài|back\s*bài|bài\s*vừa\s*rồi|nghe\s*lại\s*bài\s*trước|phát\s*lại\s*bài\s*trước|bài\s*lúc\s*nãy|bài\s*vừa\s*phát)\b").unwrap()
});

// Hardware Controls
pub static WIFI_ON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(bật wifi|mở wifi|kết nối wifi)\b").unwrap());
pub static WIFI_OFF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(tắt wifi|ngắt wifi|ngắt kết nối wifi)\b").unwrap());
pub static BT_ON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(bật bluetooth|mở bluetooth)\b").unwrap());
pub static BT_OFF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(tắt bluetooth|ngắt bluetooth)\b").unwrap());

// Asus Fan Profiles
pub static ASUS_TURBO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chế độ turbo|chế độ hiệu năng|bật turbo|quạt mạnh|tối đa hiệu năng)\b").unwrap());
pub static ASUS_QUIET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chế độ yên tĩnh|chế độ im lặng|quạt êm|chế độ tiết kiệm pin|quạt yên tĩnh)\b").unwrap());
pub static ASUS_BALANCED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chế độ cân bằng|quạt bình thường|chế độ bình thường)\b").unwrap());

// Brightness
pub static BRIGHTNESS_UP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(tăng độ sáng|cho sáng lên|sáng màn hình hơn|tăng sáng|sáng thêm|sáng lên|màn hình tối quá|cho sáng màn hình|sáng màn hình lên)\b").unwrap());
pub static BRIGHTNESS_DOWN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(giảm độ sáng|cho tối bớt|tối màn hình lại|giảm sáng|màn hình chói quá|chói mắt quá|cho tối màn hình)\b").unwrap());

// Lock & Screenshot
pub static LOCK_SCREEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(khóa màn hình|khóa máy|lock máy|lock màn hình|tôi đi ra ngoài)\b").unwrap());
pub static SCREENSHOT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chụp màn hình|chụp ảnh màn hình|chụp vùng|chụp lại màn hình|chụp một góc)\b").unwrap());

// Window Management
pub static CLOSE_ALL_APPS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:đóng|tắt)\s+(?:hết|tất cả|các|mọi|toàn bộ)\s+(?:ứng dụng|cửa sổ|phần mềm|app|tab)\b").unwrap());
pub static CLOSE_ACTIVE_TAB_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(tắt tab này|đóng tab này|đóng tab hiện tại|tắt tab hiện tại|đóng tab|tắt tab)\b").unwrap());
pub static CLOSE_ACTIVE_WINDOW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:đóng|tắt)\s+(?:cửa sổ|ứng dụng|phần mềm|app)(?:\s+(?:này|hiện tại))?$|\b(tắt ứng dụng này|đóng ứng dụng này|tắt cửa sổ này|đóng cửa sổ này|tắt nó đi|đóng nó lại|tắt nó|đóng nó|tắt cái này|đóng cái này)\b").unwrap());

// Fullscreen
pub static FULLSCREEN_ON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(phóng to cửa sổ|cửa sổ to ra|to cửa sổ|to toàn màn hình|fullscreen cửa sổ|to hết cỡ|phóng to ứng dụng|phóng to lên|cho ứng dụng to lên|cho nó to lên|phóng to nó lên|phóng to nó|phóng to trong workspace|maximize cửa sổ|maximize ứng dụng|phóng to màn hình)\b").unwrap());
pub static FULLSCREEN_OFF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(thoát toàn màn hình|hủy toàn màn hình|hủy phóng to|bỏ phóng to|thu nhỏ toàn màn hình|thu nhỏ phóng to|hủy maximize|thu nhỏ nó|thu nhỏ nó lại)\b").unwrap());

// Layout
pub static SPLIT_TOGGLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(đổi hướng chia màn hình|đổi hướng chia|chia ngang|chia dọc|đổi chiều chia|xoay hướng chia|toggle split)\b").unwrap());
pub static SPLIT_5050_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chia\s*(?:lại\s*)?(?:(?:cho\s*)?đều\s*(?:màn\s*hình|cửa\s*sổ)|màn\s*hình\s*(?:cho\s*)?đều|đôi(?:\s*50\s*50)?|50\s*50|đều)|cân\s*bằng\s*màn\s*hình)\b").unwrap());
pub static SPLIT_7030_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(chia màn hình 70 30|chia 70 30|tỷ lệ 70 30|tỉ lệ 70 30|cửa sổ 70|70 phần trăm)\b").unwrap());

// Dev Tools
pub static KILL_PORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:kill|giải phóng|xóa|tắt|đóng)\s+(?:cổng|port)\s+(\d+)\b").unwrap());
pub static CHECK_PORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:cổng|port)\s+(\d+)\b").unwrap());
pub static DOCKER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(kiểm tra docker|docker có gì|trạng thái docker|container nào đang chạy|xem docker)\b").unwrap());
pub static JAVA_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(kiểm tra java|java version|phiên bản java|máy đang cài java mấy|java mấy)\b").unwrap());

// Hardware Stats
pub static CPU_INFO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:kiểm tra|xem|cho xem|báo|thông báo)?\s*(?:thông số|tình trạng|mức tải|nhiệt độ|xung nhịp)?\s*(?:của\s+)?(?:cpu|vi xử lý|chip)\b|\b(?:cpu|vi xử lý)\s*(?:của\s+máy|thế nào|chạy thế nào|ra sao|đang thế nào)\b").unwrap());
pub static HARDWARE_INFO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:kiểm tra|xem|cho xem|báo|thông báo)?\s*(?:thông số|tình trạng|cấu hình|thông tin)?\s*(?:của\s+)?(?:máy|máy tính|hệ thống|thiết bị|laptop|pc|phần cứng)\b|\b(?:kiểm tra|xem)?\s*(?:dung lượng|bộ nhớ)?\s*(?:ram)\b|\b(?:thông số|kiểm tra)\s+(?:cpu\s+(?:và|với)\s+ram|ram\s+(?:và|với)\s+cpu)\b|\b(máy tính chạy bao lâu|thời gian bật máy)\b").unwrap());

// Weather
pub static WEATHER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(thời tiết|nhiệt độ ngoài trời|trời có mưa không|mưa hay nắng)\b").unwrap());

// Timers & Alarms
pub static TIMER_CANCEL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:hủy|tắt|xóa|dừng)\s+(?:hẹn giờ|báo thức|đếm ngược)\b").unwrap());
pub static TIMER_CHECK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:kiểm tra|xem|còn bao nhiêu|còn mấy|còn bao lâu)\s+(?:hẹn giờ|báo thức|đếm ngược)\b|^hẹn giờ còn bao lâu$|^kiểm tra hẹn giờ$|^xem hẹn giờ$|^báo thức$").unwrap());
pub static ALARM_ADD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:đặt\s+)?báo thức\s+(?:lúc\s+)?(\d{1,2})\s*(?:giờ|h)\s*(?:(\d{1,2})\s*(?:phút|p)?)?\s*(sáng|chiều|tối)?(?:\s+(?:để|cho)?\s*(.+))?\b").unwrap());
pub static TIMER_ADD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:hẹn giờ|đếm ngược|hẹn|báo thức)\s+(\d+(?:\.\d+)?)\s*(phút|giây|tiếng|giờ|p)(?:\s+(?:để|cho)?\s*(.+))?\b").unwrap());
pub static TIMER_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:đặt\s+)?(\d+(?:\.\d+)?)\s*(phút|giây|tiếng|giờ|p)$").unwrap());

// Web & YouTube & Google
pub static YT_SEARCH_RE1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:mở|phát|bật|nghe)?\s*(?:bài hát|nhạc|video|clip)?\s*(.+?)\s*trên\s*youtube").unwrap());
pub static YT_SEARCH_RE2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:mở|lên|vào)\s+youtube\s+(?:và\s+|để\s+)?(?:tìm|xem|nghe|phát|mở|bài hát|nhạc|video)\s+(.+)").unwrap()
});
pub static GOOGLE_SEARCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:tìm kiếm|tra cứu|tìm)\s+(.+?)(?:\s+(?:trên|ở|bằng)\s+(?:google|mạng|trình duyệt))?$").unwrap());

// App / Web Launching
pub static OPEN_TRIGGER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(mở|mà|mờ|bật|chạy|khởi động|vào|nghe|lên|truy cập)\b").unwrap());
pub static OPEN_APP_MATCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:mở|mà|mờ|bật|chạy|khởi động|vào|nghe|lên|truy cập)\s+(?:ứng dụng\s+|phần mềm\s+|app\s+)?(.+)").unwrap());
pub static CLOSE_APP_MATCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:đóng|tắt|thoát|kill)\s+(?:ứng dụng\s+|phần mềm\s+|app\s+|trang web\s+|trang\s+|web\s+)?([a-zA-Z0-9\s_àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]+)").unwrap());
pub static MULTI_LAUNCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:,|\bvà\b|\bvới\b|\bvới\s+cả\b|\bcùng\s+với\b|\bcùng\b|\blẫn\b|\brồi\b|\bđồng\s+thời\b|\bvào\b)").unwrap());
