use crate::audio::vad::SchmittTriggerVad;
use crate::core::state::StateManager;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct AudioRecorder {
    sample_rate: u32,
    chunk_samples: usize,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            sample_rate: 16000,
            chunk_samples: 1600, // 100ms at 16kHz
        }
    }

    /// Thu âm giọng nói người dùng cho đến khi phát hiện khoảng lặng hoặc đạt thời lượng tối đa.
    /// Hỗ trợ ngắt khẩn cấp qua StateManager.
    pub fn record(
        &self,
        max_duration_secs: f32,
        state_mgr: Option<&StateManager>,
    ) -> Result<Vec<i16>, Box<dyn std::error::Error>> {
        let chunk_bytes = self.chunk_samples * 2; // 16-bit PCM = 2 bytes/sample
        let max_speech_chunks = (7.0f32 * (self.sample_rate as f32 / self.chunk_samples as f32)) as usize;
        let max_chunks = ((max_duration_secs.max(2.5)) * (self.sample_rate as f32 / self.chunk_samples as f32)) as usize;
        let mut preroll_buffer: VecDeque<Vec<i16>> = VecDeque::with_capacity(6);
        let mut recorded_chunks: Vec<Vec<i16>> = Vec::new();
        let mut ambient_rms_samples: Vec<f32> = Vec::new();
        let mut vad = SchmittTriggerVad::new();

        // Khởi chạy tiến trình pw-record (PipeWire)
        let mut child: Child = Command::new("pw-record")
            .args([
                "--rate",
                &self.sample_rate.to_string(),
                "--channels",
                "1",
                "--format",
                "s16",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .or_else(|_| {
                // Fallback qua arecord nếu không có PipeWire recorder
                Command::new("arecord")
                    .args([
                        "-r",
                        &self.sample_rate.to_string(),
                        "-c",
                        "1",
                        "-f",
                        "S16_LE",
                        "-t",
                        "raw",
                        "-q",
                        "-",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
            })?;

        let mut stdout = child.stdout.take().ok_or("Failed to capture audio stdout")?;
        let mut buf = vec![0u8; chunk_bytes];

        for i in 0..max_chunks {
            // Kiểm tra ngắt lời / dừng
            if let Some(mgr) = state_mgr {
                if mgr.is_interrupted() {
                    let _ = child.kill();
                    return Ok(Vec::new());
                }
            }

            stdout.read_exact(&mut buf)?;

            let mut chunk = Vec::with_capacity(self.chunk_samples);
            for bytes in buf.chunks_exact(2) {
                let sample = i16::from_le_bytes([bytes[0], bytes[1]]);
                chunk.push(sample);
            }

            let rms = SchmittTriggerVad::compute_rms(&chunk);

            // 4 khung đầu tiên dùng để đo độ ồn nền (Ambient Noise Calibration)
            if i < 4 {
                if rms < 0.035 {
                    ambient_rms_samples.push(rms);
                }
                preroll_buffer.push_back(chunk);
                if preroll_buffer.len() > 6 {
                    preroll_buffer.pop_front();
                }
                continue;
            } else if i == 4 {
                preroll_buffer.push_back(chunk);
                if !ambient_rms_samples.is_empty() {
                    vad.calibrate(&ambient_rms_samples);
                }
                continue;
            }

            let (has_spoken, should_stop) = vad.process_frame(rms, 100);

            if !has_spoken {
                preroll_buffer.push_back(chunk);
                if preroll_buffer.len() > 6 {
                    preroll_buffer.pop_front();
                }

                // Nếu người dùng chưa bắt đầu nói gì trong 2.0 giây thì dừng để vòng lặp túc trực xoay vòng
                if i * 100 >= 2000 {
                    break;
                }
            } else {
                if recorded_chunks.is_empty() {
                    // Nạp toàn bộ 600ms pre-roll buffer để không bị nuốt âm đầu câu
                    for p in preroll_buffer.drain(..) {
                        recorded_chunks.push(p);
                    }
                }
                recorded_chunks.push(chunk);

                if should_stop || recorded_chunks.len() >= max_speech_chunks {
                    break;
                }
            }
        }

        let _ = child.kill();
        let _ = child.wait();

        // Nếu người dùng nói dưới 250ms (chỉ là tiếng click chuột, ho nhẹ hoặc tiếng gõ phím)
        if vad.spoken_duration_ms < 250 {
            return Ok(Vec::new());
        }

        // Ghép các chunk thành chuỗi âm thanh hoàn chỉnh
        let mut full_samples: Vec<i16> = Vec::new();
        for chunk in recorded_chunks {
            full_samples.extend(chunk);
        }

        let max_abs = full_samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        if max_abs < 900 {
            // Âm lượng quá nhỏ dưới ngưỡng nghe được, bỏ qua
            return Ok(Vec::new());
        }

        // Tự động chuẩn hóa biên độ âm thanh (Automatic Gain Control)
        normalize_audio(&mut full_samples, vad.ambient_noise);

        Ok(full_samples)
    }

    /// Lưu các mẫu âm thanh i16 PCM thành tệp chuẩn WAV 16kHz mono
    pub fn save_wav<P: AsRef<Path>>(
        samples: &[i16],
        path: P,
        sample_rate: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::create(path)?;
        let num_samples = samples.len() as u32;
        let byte_rate = sample_rate * 2;
        let block_align = 2u16;
        let bits_per_sample = 16u16;
        let data_size = num_samples * 2;
        let chunk_size = 36 + data_size;

        // RIFF Header
        file.write_all(b"RIFF")?;
        file.write_all(&chunk_size.to_le_bytes())?;
        file.write_all(b"WAVE")?;

        // fmt chunk
        file.write_all(b"fmt ")?;
        file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size
        file.write_all(&1u16.to_le_bytes())?;  // AudioFormat (PCM = 1)
        file.write_all(&1u16.to_le_bytes())?;  // NumChannels (1 = Mono)
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;

        // data chunk
        file.write_all(b"data")?;
        file.write_all(&data_size.to_le_bytes())?;

        let mut byte_buf = Vec::with_capacity(samples.len() * 2);
        for &s in samples {
            byte_buf.extend_from_slice(&s.to_le_bytes());
        }
        file.write_all(&byte_buf)?;

        Ok(())
    }
}

/// Chuẩn hóa âm lượng thông minh với trần giới hạn tiếng ồn
fn normalize_audio(samples: &mut [i16], ambient_noise: f32) {
    if samples.is_empty() {
        return;
    }
    let max_abs = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
    if max_abs > 150 {
        let max_val_f = max_abs as f32 / 32768.0;
        let target_peak = 0.90f32;
        let max_gain = 5.0f32;
        let noise_gain_cap = 0.22 / ambient_noise.max(0.01);
        let gain = (target_peak / max_val_f).min(max_gain).min(noise_gain_cap);

        if (gain - 1.0).abs() > 0.05 {
            for s in samples.iter_mut() {
                let scaled = (*s as f32 * gain).clamp(-32768.0, 32767.0);
                *s = scaled as i16;
            }
        }
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}
