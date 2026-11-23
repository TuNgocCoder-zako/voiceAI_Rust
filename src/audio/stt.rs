use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperEngine {
    cli_path: PathBuf,
    model_path: PathBuf,
    threads: usize,
    ctx: Mutex<Option<WhisperContext>>,
}

impl WhisperEngine {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let base_dir = std::env::current_dir().unwrap_or_default();
        let cli_path = home.join(".local/bin/whisper-cli");

        let candidates = [
            std::env::var("AI_ASSISTANT_WHISPER_MODEL")
                .map(PathBuf::from)
                .unwrap_or_default(),
            base_dir.join("models/whisper/ggml-large-v3-turbo-q5_0.bin"),
            base_dir.join("models/whisper/ggml-large-v3-turbo.bin"),
            base_dir.join("models/whisper/ggml-small.bin"),
            base_dir.join("models/whisper/ggml-base.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-large-v3-turbo-q5_0.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-large-v3-turbo.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-small.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-base.bin"),
            home.join("Projects/python/voice-ai/models/whisper/ggml-small.bin"),
            home.join("Projects/python/voice-ai/models/whisper/ggml-base.bin"),
        ];

        let model_path = candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| base_dir.join("models/whisper/ggml-base.bin"));

        let num_cpus = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(4).min(8))
            .unwrap_or(6);

        Self {
            cli_path,
            model_path,
            threads: num_cpus,
            ctx: Mutex::new(None),
        }
    }

    /// Khởi tạo engine nhẹ (ưu tiên ggml-base) cho chế độ dự phòng khi chưa có ONNX
    pub fn fast_wake_engine() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let base_dir = std::env::current_dir().unwrap_or_default();
        let cli_path = home.join(".local/bin/whisper-cli");

        let candidates = [
            std::env::var("AI_ASSISTANT_WAKE_MODEL")
                .map(PathBuf::from)
                .unwrap_or_default(),
            base_dir.join("models/whisper/ggml-base.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-base.bin"),
            base_dir.join("models/whisper/ggml-small.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-small.bin"),
        ];

        let model_path = candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| base_dir.join("models/whisper/ggml-base.bin"));

        let num_cpus = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(4).min(8))
            .unwrap_or(6);

        Self {
            cli_path,
            model_path,
            threads: num_cpus,
            ctx: Mutex::new(None),
        }
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn with_model_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.model_path = path.as_ref().to_path_buf();
        self
    }

    pub fn is_available(&self) -> bool {
        self.model_path.exists()
    }

    /// Nạp sẵn mô hình Whisper vào RAM một lần duy nhất khi khởi động daemon
    pub fn preload(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.model_path.exists() {
            return Err(format!("Không tìm thấy model Whisper tại {:?}", self.model_path).into());
        }
        let mut guard = self.ctx.lock().unwrap();
        if guard.is_none() {
            let model_str = self
                .model_path
                .to_str()
                .ok_or("Đường dẫn model Whisper không phải UTF-8 hợp lệ")?;
            let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
                .map_err(|e| format!("Lỗi nạp WhisperContext ({:?}): {:?}", self.model_path, e))?;
            println!("⚡ [Whisper Native RAM]: Đã nạp sẵn model {:?} vào bộ nhớ.", self.model_path);
            *guard = Some(ctx);
        }
        Ok(())
    }

    /// Nhận diện giọng nói trực tiếp từ bộ đệm PCM 16kHz mono i16 trong RAM (0 disk I/O)
    pub fn transcribe_samples(
        &self,
        samples_i16: &[i16],
    ) -> Result<String, Box<dyn std::error::Error>> {
        if samples_i16.is_empty() {
            return Ok(String::new());
        }

        self.preload()?;

        let mut audio_f32 = vec![0.0f32; samples_i16.len()];
        whisper_rs::convert_integer_to_float_audio(samples_i16, &mut audio_f32)
            .map_err(|e| format!("Lỗi chuyển đổi PCM -> f32: {:?}", e))?;

        let guard = self.ctx.lock().unwrap();
        let ctx = guard.as_ref().ok_or("WhisperContext chưa sẵn sàng")?;
        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Không tạo được WhisperState: {:?}", e))?;

        const WHISPER_PROMPT: &str = "Xin chào, bây giờ là mấy giờ, thời tiết hôm nay thế nào, \
            mở YouTube, mở Google Chrome, mở Firefox, mở VS Code, mở Spotify, \
            mở Facebook, mở GitHub, mở Zalo, mở Gmail, mở Terminal, \
            tăng âm lượng, giảm âm lượng, tắt tiếng, tăng độ sáng, kiểm tra pin, hẹn giờ.";

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 2,
            patience: -1.0,
        });
        params.set_n_threads(self.threads as i32);
        params.set_language(Some("vi"));
        params.set_translate(false);
        params.set_initial_prompt(WHISPER_PROMPT);
        params.set_no_context(true);
        params.set_temperature(0.0);
        params.set_temperature_inc(0.0);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, &audio_f32)
            .map_err(|e| format!("Lỗi suy luận Whisper: {:?}", e))?;

        let num_segments = state.full_n_segments();
        let mut parts: Vec<String> = Vec::with_capacity(num_segments.max(0) as usize);
        for i in 0..num_segments {
            if let Some(seg) = state.get_segment(i) {
                if let Ok(seg_text) = seg.to_str_lossy() {
                    let trimmed = seg_text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                }
            }
        }

        let text = parts.join(" ").trim().to_string();
        if is_hallucination(&text) {
            return Ok(String::new());
        }
        Ok(text)
    }

    /// Giữ lại tương thích khi cần phiên âm trực tiếp từ tệp WAV ngoài
    pub fn transcribe_wav<P: AsRef<Path>>(
        &self,
        wav_path: P,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if !self.cli_path.exists() || !self.model_path.exists() {
            return Err("Whisper CLI hoặc model không khả dụng".into());
        }

        let output = Command::new(&self.cli_path)
            .args([
                "-m",
                self.model_path.to_str().unwrap_or_default(),
                "-f",
                wav_path.as_ref().to_str().unwrap_or_default(),
                "-l",
                "vi",
                "-nt",
                "-np",
                "-nf",
                "--suppress-nst",
                "-t",
                &self.threads.to_string(),
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Lỗi chạy Whisper: {}", err).into());
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let mut clean_lines: Vec<&str> = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("read_audio_data:") || trimmed.is_empty() {
                continue;
            }
            clean_lines.push(trimmed);
        }

        let text = clean_lines.join(" ").trim().to_string();
        if is_hallucination(&text) {
            return Ok(String::new());
        }
        Ok(text)
    }
}

fn is_hallucination(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.len() < 2 {
        return true;
    }
    if t == "." || t == ".." || t == "..." || t == "!" || t == "?" {
        return true;
    }

    if (t.starts_with('(') && t.ends_with(')'))
        || (t.starts_with('[') && t.ends_with(']'))
        || (t.starts_with('*') && t.ends_with('*'))
        || t.eq_ignore_ascii_case("*s*")
    {
        return true;
    }

    let lower = t.to_lowercase();
    const YOUTUBE_HALLUCINATIONS: &[&str] = &[
        "video hấp dẫn",
        "subscribe",
        "đăng ký kênh",
        "ghiền mình",
        "cảm ơn các bạn",
        "hẹn gặp lại",
        "chúc các bạn",
        "theo dõi kênh",
        "nhấn chuông",
        "like và share",
        "đóng hộng",
    ];
    for &pattern in YOUTUBE_HALLUCINATIONS {
        if lower.contains(pattern) {
            return true;
        }
    }

    let words: Vec<&str> = t.split_whitespace().collect();
    if words.len() >= 4 {
        let mut unique = std::collections::HashSet::new();
        for &w in &words {
            unique.insert(w.to_lowercase());
        }
        let ratio = unique.len() as f64 / words.len() as f64;
        if ratio < 0.4 {
            return true;
        }
    }

    false
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}
