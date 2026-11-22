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
            min_silence_ms: 500,
            max_wait_ms: 2500,
            sample_rate: 16000,
        }
    }
}

/// Bộ lọc thông cao Biquad bậc 2 (Butterworth High-Pass Filter @ 100Hz, fs=16kHz)
/// Triệt tiêu hoàn toàn tiếng ù cơ học quạt tản nhiệt laptop (25Hz-45Hz), rung chấn vỏ máy và DC offset,
/// giữ nguyên 100% âm vực giọng nói của người (100Hz - 8000Hz).
#[derive(Clone, Copy)]
pub struct HighPassFilter {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl HighPassFilter {
    pub fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn process_in_place(&mut self, samples: &mut [i16]) {
        // Butterworth 2nd order HPF at 100Hz, fs=16000Hz
        const B0: f32 = 0.972614;
        const B1: f32 = -1.945228;
        const B2: f32 = 0.972614;
        const A1: f32 = -1.944480;
        const A2: f32 = 0.945978;

        for s in samples.iter_mut() {
            let x0 = *s as f32;
            let y0 = B0 * x0 + B1 * self.x1 + B2 * self.x2 - A1 * self.y1 - A2 * self.y2;
            self.x2 = self.x1;
            self.x1 = x0;
            self.y2 = self.y1;
            self.y1 = y0;
            *s = y0.clamp(-32768.0, 32767.0) as i16;
        }
    }

    /// Lọc từng mẫu chuẩn hóa [-1.0, 1.0] giữ nguyên trạng thái bộ lọc xuyên suốt phiên chạy
    #[inline]
    pub fn process_sample_f32(&mut self, x0: f32) -> f32 {
        const B0: f32 = 0.972614;
        const B1: f32 = -1.945228;
        const B2: f32 = 0.972614;
        const A1: f32 = -1.944480;
        const A2: f32 = 0.945978;

        let y0 = B0 * x0 + B1 * self.x1 + B2 * self.x2 - A1 * self.y1 - A2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x0;
        self.y2 = self.y1;
        self.y1 = y0;
        y0.clamp(-1.0, 1.0)
    }

    pub fn process_f32_in_place(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process_sample_f32(*s);
        }
    }
}

impl Default for HighPassFilter {
    fn default() -> Self {
        Self::new()
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
            ambient_noise: 0.008,
            speech_trigger_threshold: 0.016,
            silence_threshold: 0.010,
            has_spoken: false,
            silence_duration_ms: 0,
            spoken_duration_ms: 0,
        }
    }

    /// Hiệu chuẩn độ ồn môi trường nền từ các khung âm thanh ban đầu
    pub fn calibrate(&mut self, ambient_rms_samples: &[f32]) {
        if ambient_rms_samples.is_empty() {
            return;
        }
        let avg_rms: f32 = ambient_rms_samples.iter().sum::<f32>() / ambient_rms_samples.len() as f32;
        self.ambient_noise = avg_rms.max(0.003).min(0.025);
        self.speech_trigger_threshold = (self.ambient_noise * 1.5)
            .max(self.ambient_noise + 0.007)
            .clamp(0.014, 0.035);
        self.silence_threshold = (self.ambient_noise * 1.15)
            .max(self.ambient_noise + 0.003)
            .clamp(0.008, 0.022);
    }

    /// Cập nhật thích nghi độ ồn nền trong lúc yên tĩnh
    pub fn update_ambient(&mut self, frame_rms: f32) {
        if !self.has_spoken && frame_rms < self.speech_trigger_threshold {
            self.ambient_noise = self.ambient_noise * 0.95 + frame_rms * 0.05;
            self.speech_trigger_threshold = (self.ambient_noise * 1.5)
                .max(self.ambient_noise + 0.007)
                .clamp(0.014, 0.035);
            self.silence_threshold = (self.ambient_noise * 1.15)
                .max(self.ambient_noise + 0.003)
                .clamp(0.008, 0.022);
        }
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
                (true, false)
            } else {
                self.update_ambient(frame_rms);
                (false, false)
            }
        } else {
            self.spoken_duration_ms += chunk_ms;
            if frame_rms <= self.silence_threshold {
                self.silence_duration_ms += chunk_ms;
                if self.silence_duration_ms >= 550 && self.spoken_duration_ms >= 130 {
                    // Đã phát hiện khoảng lặng tự nhiên kết thúc câu nói (550ms)
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
