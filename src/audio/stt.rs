use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WhisperEngine {
    cli_path: PathBuf,
    model_path: PathBuf,
    threads: usize,
}

impl WhisperEngine {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let base_dir = std::env::current_dir().unwrap_or_default();

        let cli_path = home.join(".local/bin/whisper-cli");

        let candidates = [
            std::env::var("AI_ASSISTANT_WHISPER_MODEL").map(PathBuf::from).unwrap_or_default(),
            base_dir.join("models/whisper/ggml-small.bin"),
            base_dir.join("models/whisper/ggml-base.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-small.bin"),
            home.join("Projects/rust/voice-ai/models/whisper/ggml-base.bin"),
            home.join("Projects/python/voice-ai/models/whisper/ggml-small.bin"),
            home.join("Projects/python/voice-ai/models/whisper/ggml-base.bin"),
        ];

        let model_path = candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| {
            base_dir.join("models/whisper/ggml-base.bin")
        });

        Self {
            cli_path,
            model_path,
            threads: 4,
        }
    }

    pub fn with_model_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.model_path = path.as_ref().to_path_buf();
        self
    }

    pub fn is_available(&self) -> bool {
        self.cli_path.exists() && self.model_path.exists()
    }

    /// Nhận diện giọng nói từ tệp âm thanh WAV
    pub fn transcribe_wav<P: AsRef<Path>>(&self, wav_path: P) -> Result<String, Box<dyn std::error::Error>> {
        if !self.is_available() {
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
                "-t",
                &self.threads.to_string(),
                "--prompt",
                "Alexa, trợ lý ảo giọng nói tiếng Việt",
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
            // Bỏ qua các dòng log debug của whisper.cpp
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
    if t == "." || t == ".." || t == "..." || t == "!" {
        return true;
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
