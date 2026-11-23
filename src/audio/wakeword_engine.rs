use crate::audio::oww::OpenWakeWordDetector;
use crate::audio::vad::SileroVad;
use crate::audio::wakeword::WakeWordType;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WakeHit {
    pub wake_word: WakeWordType,
    pub score: f32,
    pub backend: &'static str,
}

#[derive(Debug, Clone)]
pub struct WakeFrameStatus {
    pub detected: Option<WakeHit>,
    pub debug_summary: String,
    pub active_threshold: f32,
}

/// Bộ dò Wake Word thống nhất trên nhịp khung 512 mẫu (32ms @ 16kHz):
/// - Ưu tiên 1: Picovoice Porcupine (thu xa 2-4m, < 2% CPU, 32ms/khung) khi có PICOVOICE_ACCESS_KEY
/// - Dự phòng: openWakeWord ONNX (tích hợp VAD-Gating chống báo giả nhạc/phím cơ)
pub enum UnifiedWakeWordEngine {
    #[cfg(feature = "porcupine")]
    Porcupine {
        engine: porcupine::Porcupine,
        keywords: Vec<WakeWordType>,
        sensitivity: f32,
        last_trigger: Option<Instant>,
        cooldown: Duration,
    },
    OpenWakeWord {
        detector: OpenWakeWordDetector,
        pcm_queue: VecDeque<f32>,
        vad_voice_window: u32,
        last_summary: String,
    },
}

impl UnifiedWakeWordEngine {
    pub fn load_default() -> Option<Self> {
        #[cfg(feature = "porcupine")]
        {
            if let Ok(access_key) = std::env::var("PICOVOICE_ACCESS_KEY") {
                let key = access_key.trim();
                if !key.is_empty() {
                    let sensitivity = std::env::var("AI_ASSISTANT_PORCUPINE_SENSITIVITY")
                        .ok()
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(0.65)
                        .clamp(0.1, 0.95);

                    let builtins = [
                        porcupine::BuiltinKeywords::Alexa,
                        porcupine::BuiltinKeywords::Jarvis,
                    ];
                    let mapped = vec![WakeWordType::Alexa, WakeWordType::Jarvis];
                    let sens = vec![sensitivity; builtins.len()];

                    match porcupine::PorcupineBuilder::new_with_keywords(key, &builtins)
                        .sensitivities(&sens)
                        .init()
                    {
                        Ok(engine) => {
                            println!(
                                "🦔 [Porcupine]: Đã nạp thành công ('Alexa', 'Jarvis', sensitivity={:.2}, frame={} mẫu)",
                                sensitivity,
                                engine.frame_length()
                            );
                            return Some(Self::Porcupine {
                                engine,
                                keywords: mapped,
                                sensitivity,
                                last_trigger: None,
                                cooldown: Duration::from_secs_f32(1.5),
                            });
                        }
                        Err(e) => {
                            eprintln!(
                                "⚠️ Không khởi tạo được Porcupine ({:?}) -> Tự động chuyển sang openWakeWord.",
                                e
                            );
                        }
                    }
                }
            }
        }

        OpenWakeWordDetector::load_default().map(|detector| Self::OpenWakeWord {
            detector,
            pcm_queue: VecDeque::with_capacity(2048),
            vad_voice_window: 0,
            last_summary: String::new(),
        })
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            #[cfg(feature = "porcupine")]
            Self::Porcupine { .. } => "Porcupine",
            Self::OpenWakeWord { .. } => "openWakeWord",
        }
    }

    pub fn reset(&mut self) {
        match self {
            #[cfg(feature = "porcupine")]
            Self::Porcupine { .. } => {}
            Self::OpenWakeWord {
                detector,
                pcm_queue,
                vad_voice_window,
                last_summary,
            } => {
                detector.reset();
                pcm_queue.clear();
                *vad_voice_window = 0;
                last_summary.clear();
            }
        }
    }

    /// Xử lý đúng 512 mẫu f32 [-1.0, 1.0] (32ms @ 16kHz), kèm xác suất Silero VAD hiện tại
    pub fn process_frame_512(
        &mut self,
        frame_512: &[f32],
        vad_prob: f32,
        is_music_playing: bool,
    ) -> Result<WakeFrameStatus, Box<dyn std::error::Error>> {
        if frame_512.len() != SileroVad::CHUNK_SAMPLES {
            return Err(format!(
                "UnifiedWakeWordEngine yêu cầu {} mẫu, nhận {}",
                SileroVad::CHUNK_SAMPLES,
                frame_512.len()
            )
            .into());
        }

        match self {
            #[cfg(feature = "porcupine")]
            Self::Porcupine {
                engine,
                keywords,
                sensitivity,
                last_trigger,
                cooldown,
            } => {
                let mut pcm_i16 = [0i16; SileroVad::CHUNK_SAMPLES];
                for (dst, &src) in pcm_i16.iter_mut().zip(frame_512.iter()) {
                    *dst = (src * 32767.0).clamp(-32768.0, 32767.0) as i16;
                }

                let in_cooldown = last_trigger
                    .map(|t| t.elapsed() < *cooldown)
                    .unwrap_or(false);

                let idx = engine
                    .process(&pcm_i16)
                    .map_err(|e| format!("Lỗi suy luận Porcupine: {:?}", e))?;

                let detected = if !in_cooldown && idx >= 0 && (idx as usize) < keywords.len() {
                    *last_trigger = Some(Instant::now());
                    Some(WakeHit {
                        wake_word: keywords[idx as usize],
                        score: 1.0,
                        backend: "Porcupine",
                    })
                } else {
                    None
                };

                Ok(WakeFrameStatus {
                    detected,
                    debug_summary: format!("Porcupine (sens={:.2})", sensitivity),
                    active_threshold: *sensitivity,
                })
            }

            Self::OpenWakeWord {
                detector,
                pcm_queue,
                vad_voice_window,
                last_summary,
            } => {
                // Giữ cửa sổ giọng người ~960ms (30 khung x 32ms) để mở cổng cho OWW
                // Ngưỡng vad_prob >= 0.15 bắt trọn giọng nói nhỏ, thì thầm từ xa
                if vad_prob >= 0.15 {
                    *vad_voice_window = 30;
                } else {
                    *vad_voice_window = vad_voice_window.saturating_sub(1);
                }

                pcm_queue.extend(frame_512.iter().copied());
                let mut last_threshold = 0.35;

                while pcm_queue.len() >= OpenWakeWordDetector::FRAME_SAMPLES {
                    let frame_1280: Vec<f32> = pcm_queue
                        .drain(..OpenWakeWordDetector::FRAME_SAMPLES)
                        .collect();
                    let res = detector.process_frame(&frame_1280, is_music_playing)?;
                    last_threshold = res.active_threshold;

                    if !res.scores.is_empty() {
                        *last_summary = res
                            .scores
                            .iter()
                            .map(|(t, s)| format!("{}: {:.2}", t.name(), s))
                            .collect::<Vec<_>>()
                            .join(" | ");
                    }

                    if let Some((ww_type, score)) = res.detected {
                        // VAD-Gate: chỉ chấp nhận khi có giọng người gần đây (hoặc điểm cực cao >= 0.60)
                        if *vad_voice_window > 0 || score >= 0.60 {
                            return Ok(WakeFrameStatus {
                                detected: Some(WakeHit {
                                    wake_word: ww_type,
                                    score,
                                    backend: "openWakeWord",
                                }),
                                debug_summary: last_summary.clone(),
                                active_threshold: last_threshold,
                            });
                        } else {
                            // Bị chặn bởi VAD-Gate: hủy cooldown để không nuốt nhịp gọi thực của người dùng
                            detector.cancel_cooldown();
                        }
                    }
                }

                Ok(WakeFrameStatus {
                    detected: None,
                    debug_summary: last_summary.clone(),
                    active_threshold: last_threshold,
                })
            }
        }
    }
}
