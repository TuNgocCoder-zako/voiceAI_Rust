use crate::audio::wakeword::WakeWordType;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct OpenWakeWordConfig {
    pub base_threshold: f32,
    pub music_boost: f32,
    pub patience_frames: u32,
    pub cooldown_secs: f32,
}

impl Default for OpenWakeWordConfig {
    fn default() -> Self {
        let base_threshold = std::env::var("AI_ASSISTANT_WAKE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.35);
        Self {
            base_threshold,
            music_boost: 0.05,
            patience_frames: 1, // Kích hoạt tức thì ở khung đỉnh (80ms), bắt nhạy giọng nói tự nhiên
            cooldown_secs: 1.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OwwFrameResult {
    pub detected: Option<(WakeWordType, f32)>,
    pub scores: Vec<(WakeWordType, f32)>,
    pub active_threshold: f32,
}

/// Bộ theo dõi ngưỡng động + Patience + Cooldown tách riêng để kiểm thử độc lập
pub struct TriggerGate {
    config: OpenWakeWordConfig,
    patience_counts: Vec<u32>,
    last_trigger: Option<Instant>,
}

impl TriggerGate {
    pub fn new(num_models: usize, config: OpenWakeWordConfig) -> Self {
        Self {
            config,
            patience_counts: vec![0; num_models],
            last_trigger: None,
        }
    }

    pub fn active_threshold(&self, is_music_playing: bool) -> f32 {
        if is_music_playing {
            (self.config.base_threshold + self.config.music_boost).min(0.95)
        } else {
            self.config.base_threshold
        }
    }

    pub fn reset(&mut self) {
        self.patience_counts.fill(0);
    }

    pub fn mark_triggered(&mut self) {
        self.last_trigger = Some(Instant::now());
        self.patience_counts.fill(0);
    }

    pub fn evaluate(
        &mut self,
        scores: &[(WakeWordType, f32)],
        is_music_playing: bool,
    ) -> Option<(WakeWordType, f32)> {
        let threshold = self.active_threshold(is_music_playing);
        let fast_peak_threshold = (threshold + 0.22).min(0.85);

        if let Some(last) = self.last_trigger {
            if last.elapsed() < Duration::from_secs_f32(self.config.cooldown_secs) {
                self.patience_counts.fill(0);
                return None;
            }
        }

        let mut best_hit: Option<(WakeWordType, f32)> = None;
        for (idx, &(ww_type, score)) in scores.iter().enumerate() {
            if idx >= self.patience_counts.len() {
                break;
            }
            if score >= fast_peak_threshold {
                match best_hit {
                    Some((_, prev)) if prev >= score => {}
                    _ => best_hit = Some((ww_type, score)),
                }
            } else if score >= threshold {
                self.patience_counts[idx] += 1;
                if self.patience_counts[idx] >= self.config.patience_frames {
                    match best_hit {
                        Some((_, prev)) if prev >= score => {}
                        _ => best_hit = Some((ww_type, score)),
                    }
                }
            } else {
                self.patience_counts[idx] = 0;
            }
        }

        if best_hit.is_some() {
            self.mark_triggered();
        }
        best_hit
    }
}

/// Pipeline openWakeWord 3 tầng ONNX chạy 100% trên CPU:
/// PCM 16kHz (1280 mẫu = 80ms) -> melspectrogram.onnx -> embedding_model.onnx -> wakeword.onnx
pub struct OpenWakeWordDetector {
    melspec: Session,
    embedding: Session,
    classifiers: Vec<(WakeWordType, Session)>,
    prev_raw: [f32; 480],
    mel_buffer: VecDeque<[f32; 32]>,
    feature_buffer: VecDeque<[f32; 96]>,
    gate: TriggerGate,
}

impl OpenWakeWordDetector {
    pub const FRAME_SAMPLES: usize = 1280; // 80ms @ 16kHz
    const OVERLAP_SAMPLES: usize = 480;    // 30ms gối đầu (3 * hop 160)
    const MEL_WINDOW: usize = 76;          // 76 khung mel cho embedding_model
    const EMB_WINDOW: usize = 16;          // 16 vector embedding cho classifier

    pub fn load_from_dir<P: AsRef<Path>>(
        dir: P,
        config: OpenWakeWordConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = dir.as_ref();
        let mel_path = dir.join("melspectrogram.onnx");
        let emb_path = dir.join("embedding_model.onnx");

        if !mel_path.exists() || !emb_path.exists() {
            return Err(format!(
                "Thiếu melspectrogram.onnx hoặc embedding_model.onnx trong {:?}",
                dir
            )
            .into());
        }

        let env = crate::audio::get_ort_env()?;

        let melspec = Session::builder(env)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(1)?
            .commit_from_file(&mel_path)?;

        let embedding = Session::builder(env)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(1)?
            .commit_from_file(&emb_path)?;

        let model_specs = [
            (WakeWordType::Alexa, "alexa_v0.1.onnx"),
            (WakeWordType::Jarvis, "hey_jarvis_v0.1.onnx"),
        ];

        let mut classifiers = Vec::new();
        for (ww_type, filename) in model_specs {
            let p = dir.join(filename);
            if p.exists() {
                let sess = Session::builder(env)?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(1)?
                    .commit_from_file(&p)?;
                classifiers.push((ww_type, sess));
            }
        }

        if classifiers.is_empty() {
            return Err(format!(
                "Không tìm thấy tệp wake word (.onnx) nào trong {:?}",
                dir
            )
            .into());
        }

        let num_models = classifiers.len();
        Ok(Self {
            melspec,
            embedding,
            classifiers,
            prev_raw: [0.0f32; Self::OVERLAP_SAMPLES],
            mel_buffer: VecDeque::with_capacity(128),
            feature_buffer: VecDeque::with_capacity(32),
            gate: TriggerGate::new(num_models, config),
        })
    }

    pub fn load_default() -> Option<Self> {
        let home = dirs::home_dir().unwrap_or_default();
        let cwd = std::env::current_dir().unwrap_or_default();

        let candidates = [
            std::env::var("AI_ASSISTANT_WAKEWORD_DIR")
                .map(PathBuf::from)
                .unwrap_or_default(),
            cwd.join("models/wakeword"),
            home.join("Projects/rust/voice-ai/models/wakeword"),
            home.join(".local/share/voice-ai/models/wakeword"),
        ];

        let dir = candidates
            .into_iter()
            .find(|d| d.join("melspectrogram.onnx").exists())?;

        match Self::load_from_dir(&dir, OpenWakeWordConfig::default()) {
            Ok(det) => {
                let names: Vec<&str> = det.classifiers.iter().map(|(t, _)| t.name()).collect();
                println!(
                    "🎯 [openWakeWord ONNX]: Đã nạp {} mô hình từ {:?}: {:?}",
                    names.len(),
                    dir,
                    names
                );
                Some(det)
            }
            Err(e) => {
                eprintln!("⚠️ Không nạp được openWakeWord ({:?}): {}", dir, e);
                None
            }
        }
    }

    /// Xóa bộ đệm nội bộ sau khi thu lệnh hoặc phát TTS để tránh dữ liệu gián đoạn
    pub fn reset(&mut self) {
        self.prev_raw.fill(0.0);
        self.mel_buffer.clear();
        self.feature_buffer.clear();
        self.gate.reset();
    }

    /// Đưa vào đúng 1280 mẫu f32 [-1.0, 1.0] (80ms @ 16kHz)
    pub fn process_frame(
        &mut self,
        frame_1280: &[f32],
        is_music_playing: bool,
    ) -> Result<OwwFrameResult, Box<dyn std::error::Error>> {
        if frame_1280.len() != Self::FRAME_SAMPLES {
            return Err(format!(
                "openWakeWord yêu cầu {} mẫu, nhận được {}",
                Self::FRAME_SAMPLES,
                frame_1280.len()
            )
            .into());
        }

        let active_threshold = self.gate.active_threshold(is_music_playing);

        // 1. Chuyển sang thang int16 float [-32768, 32767] + ghép 480 mẫu gối đầu
        let mut raw_1760 = Vec::with_capacity(Self::OVERLAP_SAMPLES + Self::FRAME_SAMPLES);
        raw_1760.extend_from_slice(&self.prev_raw);
        for &s in frame_1280 {
            raw_1760.push((s * 32767.0).clamp(-32768.0, 32767.0));
        }
        let tail_start = raw_1760.len() - Self::OVERLAP_SAMPLES;
        self.prev_raw.copy_from_slice(&raw_1760[tail_start..]);

        // 2. Melspectrogram: [1, 1760] -> [1, 1, T, 32]
        let mel_in = Tensor::from_array(([1usize, raw_1760.len()], raw_1760))?;
        let mel_out = self.melspec.run(ort::inputs![mel_in])?;
        let (_, mel_raw): (&ort::value::Shape, &[f32]) = mel_out[0].try_extract_tensor::<f32>()?;

        for frame in mel_raw.chunks_exact(32) {
            let mut bin = [0.0f32; 32];
            for (dst, &v) in bin.iter_mut().zip(frame.iter()) {
                *dst = v / 10.0 + 2.0; // Công thức chuẩn hóa bắt buộc của openWakeWord
            }
            self.mel_buffer.push_back(bin);
            if self.mel_buffer.len() > 128 {
                self.mel_buffer.pop_front();
            }
        }

        if self.mel_buffer.len() < Self::MEL_WINDOW {
            return Ok(OwwFrameResult {
                detected: None,
                scores: Vec::new(),
                active_threshold,
            });
        }

        // 3. Embedding: lấy 76 khung mel mới nhất -> [1, 76, 32, 1] -> [1, 1, 1, 96]
        let mel_start = self.mel_buffer.len() - Self::MEL_WINDOW;
        let mut emb_input = Vec::with_capacity(Self::MEL_WINDOW * 32);
        for frame in self.mel_buffer.iter().skip(mel_start) {
            emb_input.extend_from_slice(frame);
        }

        let emb_in = Tensor::from_array(([1usize, Self::MEL_WINDOW, 32, 1], emb_input))?;
        let emb_out = self.embedding.run(ort::inputs![emb_in])?;
        let (_, emb_raw): (&ort::value::Shape, &[f32]) = emb_out[0].try_extract_tensor::<f32>()?;

        if emb_raw.len() < 96 {
            return Ok(OwwFrameResult {
                detected: None,
                scores: Vec::new(),
                active_threshold,
            });
        }

        let mut vec96 = [0.0f32; 96];
        vec96.copy_from_slice(&emb_raw[..96]);
        self.feature_buffer.push_back(vec96);
        if self.feature_buffer.len() > 32 {
            self.feature_buffer.pop_front();
        }

        if self.feature_buffer.len() < Self::EMB_WINDOW {
            return Ok(OwwFrameResult {
                detected: None,
                scores: Vec::new(),
                active_threshold,
            });
        }

        // 4. Classifiers: lấy 16 khung embedding mới nhất -> [1, 16, 96] -> [1, 1]
        let feat_start = self.feature_buffer.len() - Self::EMB_WINDOW;
        let mut cls_input = Vec::with_capacity(Self::EMB_WINDOW * 96);
        for feat in self.feature_buffer.iter().skip(feat_start) {
            cls_input.extend_from_slice(feat);
        }

        let mut scores = Vec::with_capacity(self.classifiers.len());
        for (ww_type, session) in &mut self.classifiers {
            let cls_in = Tensor::from_array(([1usize, Self::EMB_WINDOW, 96], cls_input.clone()))?;
            let cls_out = session.run(ort::inputs![cls_in])?;
            let (_, prob_raw): (&ort::value::Shape, &[f32]) = cls_out[0].try_extract_tensor::<f32>()?;
            let prob = prob_raw.first().copied().unwrap_or(0.0);
            scores.push((*ww_type, prob));
        }

        let detected = self.gate.evaluate(&scores, is_music_playing);

        Ok(OwwFrameResult {
            detected,
            scores,
            active_threshold,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_gate_patience_and_music_boost() {
        let mut gate = TriggerGate::new(
            2,
            OpenWakeWordConfig {
                base_threshold: 0.55,
                music_boost: 0.05,
                patience_frames: 2,
                cooldown_secs: 1.5,
            },
        );

        // 1 khung gai nhọn đơn lẻ (0.62) không được kích hoạt do dưới fast_peak_threshold (0.77) và yêu cầu patience = 2
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.62), (WakeWordType::Jarvis, 0.05)], false),
            None
        );
        // Rớt xuống dưới ngưỡng -> reset bộ đếm patience
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.20), (WakeWordType::Jarvis, 0.05)], false),
            None
        );

        // Khi đang mở nhạc: ngưỡng tăng từ 0.55 lên 0.60 -> điểm 0.58 liên tiếp vẫn bị chặn
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.58), (WakeWordType::Jarvis, 0.05)], true),
            None
        );
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.58), (WakeWordType::Jarvis, 0.05)], true),
            None
        );

        // 2 khung liên tiếp >= 0.60 khi mở nhạc -> kích hoạt thành công
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.65), (WakeWordType::Jarvis, 0.05)], true),
            None
        );
        let hit = gate.evaluate(&[(WakeWordType::Alexa, 0.78), (WakeWordType::Jarvis, 0.05)], true);
        assert_eq!(hit, Some((WakeWordType::Alexa, 0.78)));

        // Ngay sau khi kích hoạt, cooldown 1.5s chặn gọi đúp
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.95), (WakeWordType::Jarvis, 0.05)], false),
            None
        );
    }

    #[test]
    fn test_fast_peak_single_frame() {
        let mut gate = TriggerGate::new(
            2,
            OpenWakeWordConfig {
                base_threshold: 0.48,
                music_boost: 0.05,
                patience_frames: 2,
                cooldown_secs: 1.5,
            },
        );
        // Ngưỡng fast peak = (0.48 + 0.22) = 0.70. Điểm 0.82 kích hoạt ngay tức thì ở khung đơn lẻ
        let hit = gate.evaluate(&[(WakeWordType::Alexa, 0.82)], false);
        assert_eq!(hit, Some((WakeWordType::Alexa, 0.82)));
    }

    #[test]
    fn test_single_frame_wake_trigger() {
        let mut gate = TriggerGate::new(1, OpenWakeWordConfig::default());
        // Khung đơn lẻ đạt ngưỡng nhạy (0.35) phải kích hoạt ngay lập tức
        assert_eq!(
            gate.evaluate(&[(WakeWordType::Alexa, 0.45)], false),
            Some((WakeWordType::Alexa, 0.45))
        );
    }
}
