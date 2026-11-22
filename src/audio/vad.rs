pub struct VadConfig {
    pub min_speech_ms: u32,
    pub min_silence_ms: u32,
    pub max_wait_ms: u32,
    pub sample_rate: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            min_speech_ms: 150,
            min_silence_ms: 600,
            max_wait_ms: 2500,
            sample_rate: 16000,
        }
    }
}

pub struct SchmittTriggerVad {
    pub ambient_noise: f32,
    pub speech_trigger_threshold: f32,
    pub silence_threshold: f32,
    pub has_spoken: bool,
    pub silence_duration_ms: u32,
    pub spoken_duration_ms: u32,
}

impl SchmittTriggerVad {
    pub fn new() -> Self {
        Self {
            ambient_noise: 0.010,
            speech_trigger_threshold: 0.022,
            silence_threshold: 0.015,
            has_spoken: false,
            silence_duration_ms: 0,
            spoken_duration_ms: 0,
        }
    }

    /// Hiệu chuẩn độ ồn môi trường nền từ các khung đầu tiên
    pub fn calibrate(&mut self, ambient_rms_samples: &[f32]) {
        if ambient_rms_samples.is_empty() {
            return;
        }
        let avg_rms: f32 = ambient_rms_samples.iter().sum::<f32>() / ambient_rms_samples.len() as f32;
        // Giới hạn độ ồn môi trường tối đa 0.040 để tiếng ồn nền/loa không làm điếc micro
        self.ambient_noise = avg_rms.max(0.005).min(0.040);
        self.speech_trigger_threshold = (self.ambient_noise * 1.4)
            .max(self.ambient_noise + 0.010)
            .max(0.022)
            .min(0.055);
        self.silence_threshold = (self.ambient_noise * 1.15)
            .max(self.ambient_noise + 0.005)
            .max(0.015)
            .min(0.040);
    }

    /// Tính toán RMS năng lượng của khung âm thanh i16 PCM (chuẩn hóa về [-1.0, 1.0])
    pub fn compute_rms(samples: &[i16]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = samples
            .iter()
            .map(|&s| {
                let norm = s as f64 / 32768.0;
                norm * norm
            })
            .sum();
        (sum_sq / samples.len() as f64).sqrt() as f32
    }

    /// Cập nhật trạng thái VAD cho từng khung 100ms
    /// Trả về (has_spoken, should_stop)
    pub fn process_frame(&mut self, frame_rms: f32, chunk_ms: u32) -> (bool, bool) {
        if !self.has_spoken {
            if frame_rms >= self.speech_trigger_threshold {
                self.has_spoken = true;
                self.spoken_duration_ms += chunk_ms;
                self.silence_duration_ms = 0;
            }
            (false, false)
        } else {
            self.spoken_duration_ms += chunk_ms;
            if frame_rms <= self.silence_threshold {
                self.silence_duration_ms += chunk_ms;
                if self.silence_duration_ms >= 600 && self.spoken_duration_ms >= 200 {
                    // Đã phát hiện khoảng lặng tự nhiên kết thúc câu nói (600ms)
                    return (true, true);
                }
            } else {
                self.silence_duration_ms = 0;
            }
            (true, false)
        }
    }
}

impl Default for SchmittTriggerVad {
    fn default() -> Self {
        Self::new()
    }
}
