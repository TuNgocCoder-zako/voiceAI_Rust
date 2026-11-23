use clap::Parser;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use voice_ai::ai::{ModelRouter, OllamaClient};
use voice_ai::audio::{AudioRecorder, WhisperEngine};
use voice_ai::config::Config;
use voice_ai::core::memory::ConversationHistory;
use voice_ai::core::state::StateManager;
use voice_ai::speech::PiperTts;
use voice_ai::tools::apps::{build_app_index, DesktopApp};

#[derive(Parser, Debug)]
#[command(name = "ai-assistant")]
#[command(author = "TuNgocCoder <tudm.dev@gmail.com>")]
#[command(version = "3.0.0")]
#[command(about = "High-Performance Local Voice AI Assistant Daemon for Ubuntu (Rust)")]
pub struct Args {
    #[arg(short, long, help = "Chế độ bấm Enter để nhập lệnh văn bản (Push-to-Talk)")]
    pub push_to_talk: bool,

    #[arg(short, long, help = "Bật giọng nói phản hồi (Piper TTS offline)")]
    pub voice: bool,

    #[arg(short = 'l', long, help = "Chế độ nghe qua Microphone liên tục (Always-on VAD Chat)")]
    pub listen: bool,

    #[arg(short = 'w', long, help = "Chế độ đánh thức bằng từ khóa Wake Word rảnh tay")]
    pub wake_word: bool,

    #[arg(short = 'm', long, help = "Chỉ định thủ công mô hình LLM (Bỏ qua tự động định tuyến)")]
    pub model: Option<String>,

    #[arg(long, help = "Ép dùng mô hình suy luận sâu (Reasoning) khi dùng --query")]
    pub deep: bool,

    #[arg(long, help = "Gửi tín hiệu đánh thức tới trợ lý đang chạy nền")]
    pub trigger: bool,

    #[arg(long, help = "Gửi tín hiệu ngắt lời / dừng phát âm tới trợ lý đang chạy nền")]
    pub stop: bool,

    #[arg(long, help = "Dừng ngay và kích hoạt micro lắng nghe ngay lượt mới")]
    pub relisten: bool,

    #[arg(long, help = "Kiểm tra trạng thái của trợ lý đang chạy nền qua IPC")]
    pub status: bool,

    #[arg(long, help = "Chạy câu lệnh đơn lẻ rồi thoát ngay")]
    pub query: Option<String>,

    #[arg(long, help = "Đọc trực tiếp một đoạn văn bản qua TTS mà không gửi lên LLM")]
    pub speak: Option<String>,

    #[arg(long, help = "Chạy đo kiểm hiệu năng Fast-Path NLU siêu tốc")]
    pub benchmark: bool,

    #[arg(long, help = "Hiển thị trực tiếp điểm Silero VAD & openWakeWord theo thời gian thực")]
    pub debug_audio: bool,

    #[arg(long, help = "Mở giao diện gõ văn bản trực tiếp (Text Mode / Chat) trên Overlay")]
    pub chat: bool,

    #[arg(long, help = "Chế độ yên lặng khi truy vấn --query (không phát âm TTS)")]
    pub silent: bool,
}

#[derive(Clone)]
pub struct AppContext {
    pub config: Config,
    pub app_index: Arc<HashMap<String, DesktopApp>>,
    pub tts: Arc<PiperTts>,
    pub recorder: Arc<AudioRecorder>,
    pub whisper: Arc<WhisperEngine>,
    pub ollama: Arc<OllamaClient>,
    pub model_router: Arc<ModelRouter>,
    pub history: Arc<Mutex<ConversationHistory>>,
    pub state_manager: StateManager,
    pub model_override: Option<String>,
}

impl AppContext {
    pub fn new(config: Config, model_override: Option<String>) -> Self {
        let ollama = Arc::new(OllamaClient::new(&config.ollama_url, &config.model));
        let model_router = Arc::new(ModelRouter::new(&config.ollama_url));
        Self {
            config,
            app_index: Arc::new(build_app_index()),
            tts: Arc::new(PiperTts::new()),
            recorder: Arc::new(AudioRecorder::new()),
            whisper: Arc::new(WhisperEngine::new()),
            ollama,
            model_router,
            history: Arc::new(Mutex::new(ConversationHistory::new(3, 120))),
            state_manager: StateManager::new(),
            model_override,
        }
    }
}
