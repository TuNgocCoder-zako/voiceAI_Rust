use crate::audio::vad::{HighPassFilter, SchmittTriggerVad, SileroVad};
use crate::core::state::StateManager;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const TARGET_SAMPLE_RATE: u32 = 16000;

/// Bộ lấy mẫu lại có trạng thái từ tần số phần cứng (48kHz / 44.1kHz / 16kHz) về chuẩn 16kHz mono.
pub struct StreamResampler {
    in_rate: u32,
    out_rate: u32,
    // Trạng thái cho nội suy tuyến tính (44.1kHz hoặc tần số bất kỳ)
    phase: f64,
    prev_sample: f32,
    has_prev: bool,
    // Bộ đệm lọc chống răng cưa cho tỷ lệ nguyên (vd: 48kHz -> 16kHz = 3:1)
    decim_accum: f32,
    decim_count: u32,
}

impl StreamResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self {
            in_rate: in_rate.max(1),
            out_rate: out_rate.max(1),
            phase: 0.0,
            prev_sample: 0.0,
            has_prev: false,
            decim_accum: 0.0,
            decim_count: 0,
        }
    }

    pub fn push_sample(&mut self, sample: f32, out: &mut Vec<f32>) {
        // Trường hợp 1: Đã đúng 16kHz
        if self.in_rate == self.out_rate {
            out.push(sample);
            return;
        }

        // Trường hợp 2: Tỷ lệ chia hết nguyên (chuẩn PipeWire 48000 -> 16000 = 3:1)
        // Dùng trung bình khung (boxcar low-pass) để chống aliasing trước khi giảm mẫu
        if self.in_rate % self.out_rate == 0 {
            let factor = self.in_rate / self.out_rate;
            self.decim_accum += sample;
            self.decim_count += 1;
            if self.decim_count >= factor {
                out.push(self.decim_accum / factor as f32);
                self.decim_accum = 0.0;
                self.decim_count = 0;
            }
            return;
        }

        // Trường hợp 3: Tần số không chia hết (vd: 44100 -> 16000) bằng nội suy tuyến tính
        if !self.has_prev {
            self.prev_sample = sample;
            self.has_prev = true;
            return;
        }

        let step = self.in_rate as f64 / self.out_rate as f64;
        while self.phase < 1.0 {
            let frac = self.phase as f32;
            let interp = self.prev_sample * (1.0 - frac) + sample * frac;
            out.push(interp);
            self.phase += step;
        }
        self.phase -= 1.0;
        self.prev_sample = sample;
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_sample = 0.0;
        self.has_prev = false;
        self.decim_accum = 0.0;
        self.decim_count = 0;
    }
}

/// Bộ khuếch đại mềm theo luồng (Streaming Soft-AGC) tối ưu cho thu giọng khoảng cách xa (2–4m).
/// Giữ im lặng khi chỉ có nhiễu nền thấp (noise gate), tăng chậm khi giọng xa và giảm nhanh khi giọng gần.
pub struct SoftAgc {
    target_rms: f32,
    noise_floor_rms: f32,
    max_gain: f32,
    min_gain: f32,
    attack: f32,
    release: f32,
    current_gain: f32,
}

impl SoftAgc {
    pub fn new() -> Self {
        Self {
            target_rms: 0.08,
            noise_floor_rms: 0.004,
            max_gain: 3.5,
            min_gain: 0.5,
            attack: 0.35,
            release: 0.06,
            current_gain: 1.0,
        }
    }

    pub fn current_gain(&self) -> f32 {
        self.current_gain
    }

    pub fn reset(&mut self) {
        self.current_gain = 1.0;
    }

    /// Khuếch đại tại chỗ 1 khung f32 [-1.0, 1.0] và trả về mức RMS sau xử lý
    pub fn process_frame(&mut self, frame: &mut [f32]) -> f32 {
        if frame.is_empty() {
            return 0.0;
        }

        let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / frame.len() as f32).sqrt();

        let desired_gain = if rms < self.noise_floor_rms {
            // Không khuếch đại tiếng ồn phòng yên tĩnh, kéo gain từ từ về 1.0
            1.0
        } else {
            (self.target_rms / rms).clamp(self.min_gain, self.max_gain)
        };

        let alpha = if desired_gain < self.current_gain {
            self.attack
        } else {
            self.release
        };
        self.current_gain += alpha * (desired_gain - self.current_gain);

        let mut out_sq = 0.0f32;
        for s in frame.iter_mut() {
            // Soft-clipping (tanh nhẹ) khi tín hiệu vượt 0.90 để không vỡ tiếng khi nói gần
            let scaled = *s * self.current_gain;
            let limited = if scaled.abs() > 0.90 {
                scaled.tanh()
            } else {
                scaled
            };
            *s = limited.clamp(-0.98, 0.98);
            out_sq += *s * *s;
        }
        (out_sq / frame.len() as f32).sqrt()
    }
}

impl Default for SoftAgc {
    fn default() -> Self {
        Self::new()
    }
}

/// Luồng thu âm cpal sống suốt vòng đời daemon, không bao giờ spawn/kill tiến trình ngoài.
pub struct ContinuousAudioStream {
    consumer: HeapCons<f32>,
    resampler: StreamResampler,
    hpf: HighPassFilter,
    agc: SoftAgc,
    pending_16k: VecDeque<f32>,
    native_rate: u32,
    stop_flag: Arc<AtomicBool>,
    _worker: Option<JoinHandle<()>>,
}

impl ContinuousAudioStream {
    pub fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop_flag.clone();
        let (init_tx, init_rx) =
            mpsc::sync_channel::<Result<(u32, HeapCons<f32>), String>>(1);

        let handle = thread::Builder::new()
            .name("cpal-audio-capture".to_string())
            .spawn(move || {
                let host = cpal::default_host();
                let device = match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        let _ = init_tx.send(Err("Không tìm thấy thiết bị micro mặc định".into()));
                        return;
                    }
                };

                let supported_config = match device.default_input_config() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = init_tx.send(Err(format!("Không đọc được cấu hình micro: {}", e)));
                        return;
                    }
                };

                let sample_format = supported_config.sample_format();
                let fallback_config: cpal::StreamConfig = supported_config.into();
                let preferred_config = cpal::StreamConfig {
                    channels: 1,
                    sample_rate: cpal::SampleRate(TARGET_SAMPLE_RATE),
                    buffer_size: cpal::BufferSize::Default,
                };

                let err_fn = |err| {
                    eprintln!("⚠️ Lỗi luồng âm thanh cpal: {}", err);
                };

                // Thử mở thẳng 16kHz Mono trước; nếu thiết bị không hỗ trợ mới dùng fallback_config
                let build_for = |cfg: &cpal::StreamConfig, mut prod: HeapProd<f32>| {
                    let ch = (cfg.channels as usize).max(1);
                    match sample_format {
                        cpal::SampleFormat::F32 => device.build_input_stream(
                            cfg,
                            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                if ch == 1 {
                                    for &s in data {
                                        let _ = prod.try_push(s);
                                    }
                                } else {
                                    // Chỉ lấy kênh 0 để tránh triệt tiêu pha trên mic stereo laptop ASUS
                                    for frame in data.chunks_exact(ch) {
                                        let _ = prod.try_push(frame[0]);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        ),
                        cpal::SampleFormat::I16 => device.build_input_stream(
                            cfg,
                            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                let inv = 1.0 / 32768.0;
                                if ch == 1 {
                                    for &s in data {
                                        let _ = prod.try_push(s as f32 * inv);
                                    }
                                } else {
                                    for frame in data.chunks_exact(ch) {
                                        let _ = prod.try_push(frame[0] as f32 * inv);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        ),
                        cpal::SampleFormat::U16 => device.build_input_stream(
                            cfg,
                            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                                if ch == 1 {
                                    for &s in data {
                                        let _ = prod.try_push((s as f32 / 32768.0) - 1.0);
                                    }
                                } else {
                                    for frame in data.chunks_exact(ch) {
                                        let _ = prod.try_push((frame[0] as f32 / 32768.0) - 1.0);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        ),
                        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
                    }
                };

                let rb_pref = HeapRb::<f32>::new((preferred_config.sample_rate.0 as usize) * 3);
                let (prod_pref, cons_pref) = rb_pref.split();

                let (stream, active_config, cons) = match build_for(&preferred_config, prod_pref) {
                    Ok(s) => (s, preferred_config, cons_pref),
                    Err(_) => {
                        let rb_fb = HeapRb::<f32>::new((fallback_config.sample_rate.0 as usize) * 3);
                        let (prod_fb, cons_fb) = rb_fb.split();
                        match build_for(&fallback_config, prod_fb) {
                            Ok(s) => (s, fallback_config, cons_fb),
                            Err(e) => {
                                let _ = init_tx.send(Err(format!("Không mở được luồng cpal: {}", e)));
                                return;
                            }
                        }
                    }
                };

                let native_rate = active_config.sample_rate.0;
                let channels = (active_config.channels as usize).max(1);

                if let Err(e) = stream.play() {
                    let _ = init_tx.send(Err(format!("Không chạy được luồng thu âm: {}", e)));
                    return;
                }

                let device_name = device.name().unwrap_or_else(|_| "default".into());
                println!(
                    "🎙️ [cpal]: Đã mở luồng thu âm liên tục '{}' ({} Hz, {} kênh, {:?})",
                    device_name, native_rate, channels, sample_format
                );

                let _ = init_tx.send(Ok((native_rate, cons)));

                // Giữ cpal::Stream sống trong thread riêng cho đến khi daemon tắt
                while !stop_for_thread.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(50));
                }
                drop(stream);
            })?;

        let (native_rate, consumer) = init_rx
            .recv()
            .map_err(|e| format!("Lỗi đồng bộ khởi tạo luồng âm thanh: {}", e))?
            .map_err(|e| e)?;

        let mut stream = Self {
            consumer,
            resampler: StreamResampler::new(native_rate, TARGET_SAMPLE_RATE),
            hpf: HighPassFilter::new(),
            agc: SoftAgc::new(),
            pending_16k: VecDeque::with_capacity(TARGET_SAMPLE_RATE as usize),
            native_rate,
            stop_flag,
            _worker: Some(handle),
        };

        // Warm-up bộ lọc HPF 120ms một lần duy nhất lúc khởi động để bỏ tiếng tách mở mic
        let mut warmup = vec![0.0f32; 1920];
        let _ = stream.read_exact_f32(&mut warmup, None);

        Ok(stream)
    }

    pub fn native_rate(&self) -> u32 {
        self.native_rate
    }

    /// Xả sạch âm thanh tồn đọng (gọi sau khi TTS nói xong hoặc sau tiếng chime)
    /// mà vẫn giữ nguyên trạng thái bộ lọc HPF để không gây xung transient.
    pub fn flush(&mut self) {
        let mut temp = Vec::with_capacity(1024);
        while let Some(s) = self.consumer.try_pop() {
            self.resampler.push_sample(s, &mut temp);
            for sample_16k in temp.drain(..) {
                let _ = self.hpf.process_sample_f32(sample_16k);
            }
        }
        self.pending_16k.clear();
        self.agc.reset();
    }

    /// Trả về Ok(true) khi đọc đủ mẫu, Ok(false) khi bị ngắt bởi StateManager,
    /// Err khi luồng cpal không có dữ liệu quá 2 giây (cần khởi tạo lại).
    pub fn read_exact_f32_checked(
        &mut self,
        out: &mut [f32],
        state_mgr: Option<&StateManager>,
    ) -> Result<bool, &'static str> {
        let mut raw_buf = [0.0f32; 512];
        let mut resampled = Vec::with_capacity(512);
        let mut stalled_ms: u32 = 0;

        while self.pending_16k.len() < out.len() {
            if let Some(mgr) = state_mgr {
                if mgr.is_interrupted() {
                    return Ok(false);
                }
            }

            let n = self.consumer.pop_slice(&mut raw_buf);
            if n == 0 {
                thread::sleep(Duration::from_millis(4));
                stalled_ms += 4;
                if stalled_ms >= 2000 {
                    return Err("Luồng âm thanh cpal bị treo quá 2 giây");
                }
                continue;
            }

            stalled_ms = 0;
            resampled.clear();
            for &s in &raw_buf[..n] {
                self.resampler.push_sample(s, &mut resampled);
            }
            for &s16 in &resampled {
                let clean = self.hpf.process_sample_f32(s16);
                self.pending_16k.push_back(clean);
            }
        }

        for slot in out.iter_mut() {
            *slot = self.pending_16k.pop_front().unwrap_or(0.0);
        }
        if out.len() >= 128 {
            self.agc.process_frame(out);
        }
        Ok(true)
    }

    /// Đọc đủ `out.len()` mẫu 16kHz mono f32 [-1.0, 1.0] đã qua resample + HPF 100Hz.
    /// Trả về `false` nếu bị ngắt bởi `StateManager::is_interrupted()` hoặc lỗi treo luồng.
    pub fn read_exact_f32(
        &mut self,
        out: &mut [f32],
        state_mgr: Option<&StateManager>,
    ) -> bool {
        self.read_exact_f32_checked(out, state_mgr).unwrap_or(false)
    }

    /// Đọc đủ `out.len()` mẫu 16kHz mono i16 kèm kiểm tra lỗi watchdog >2s.
    pub fn read_exact_i16_checked(
        &mut self,
        out: &mut [i16],
        state_mgr: Option<&StateManager>,
    ) -> Result<bool, &'static str> {
        let mut f32_buf = vec![0.0f32; out.len()];
        let ok = self.read_exact_f32_checked(&mut f32_buf, state_mgr)?;
        if !ok {
            return Ok(false);
        }
        for (dst, &src) in out.iter_mut().zip(f32_buf.iter()) {
            *dst = (src * 32767.0).clamp(-32768.0, 32767.0) as i16;
        }
        Ok(true)
    }

    /// Đọc đủ `out.len()` mẫu 16kHz mono i16 đã qua resample + HPF 100Hz.
    pub fn read_exact_i16(
        &mut self,
        out: &mut [i16],
        state_mgr: Option<&StateManager>,
    ) -> bool {
        self.read_exact_i16_checked(out, state_mgr).unwrap_or(false)
    }
}

impl Drop for ContinuousAudioStream {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self._worker.take() {
            let _ = handle.join();
        }
    }
}

pub struct AudioRecorder {
    sample_rate: u32,
    chunk_samples: usize,
    stream: Mutex<Option<ContinuousAudioStream>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            sample_rate: TARGET_SAMPLE_RATE,
            chunk_samples: 1600, // 100ms @ 16kHz
            stream: Mutex::new(None),
        }
    }

    /// Đảm bảo luồng cpal đang chạy (chỉ mở thiết bị 1 lần duy nhất)
    pub fn ensure_started(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = self.stream.lock().unwrap();
        if guard.is_none() {
            let s = ContinuousAudioStream::start()
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            *guard = Some(s);
        }
        Ok(())
    }

    /// Xả sạch bộ đệm micro tồn đọng (rất cần thiết ngay sau khi chime/TTS vừa dừng)
    pub fn flush(&self) {
        let mut guard = self.stream.lock().unwrap();
        if let Some(s) = guard.as_mut() {
            s.flush();
        }
    }

    /// Đọc 1 khung f32 16kHz (dùng trực tiếp cho Silero VAD 512 mẫu / openWakeWord 1280 mẫu ở Chặng 2.2)
    pub fn read_frame_f32(
        &self,
        out: &mut [f32],
        state_mgr: Option<&StateManager>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        self.ensure_started()?;
        let mut guard = self.stream.lock().unwrap();
        let s = guard.as_mut().ok_or("Audio stream chưa khởi tạo")?;
        match s.read_exact_f32_checked(out, state_mgr) {
            Ok(ok) => Ok(ok),
            Err(reason) => {
                eprintln!("🔄 [cpal]: {} -> Tự động mở lại thiết bị thu âm...", reason);
                *guard = None; // Drop luồng cũ để lượt gọi kế tiếp tự start() mới
                Err(reason.into())
            }
        }
    }

    /// Thu âm giọng nói từ luồng liên tục cho đến khi phát hiện khoảng lặng hoặc hết thời gian tối đa.
    pub fn record(
        &self,
        max_duration_secs: f32,
        state_mgr: Option<&StateManager>,
    ) -> Result<Vec<i16>, Box<dyn std::error::Error>> {
        self.ensure_started()?;

        let max_speech_chunks =
            (7.0f32 * (self.sample_rate as f32 / self.chunk_samples as f32)) as usize;
        let max_chunks = ((max_duration_secs.max(2.5))
            * (self.sample_rate as f32 / self.chunk_samples as f32)) as usize;
        let mut preroll_buffer: VecDeque<Vec<i16>> = VecDeque::with_capacity(6);
        let mut recorded_chunks: Vec<Vec<i16>> = Vec::new();
        let mut vad = SchmittTriggerVad::new();

        let initial_silence_timeout_ms: u32 = if max_duration_secs >= 4.0 {
            4500
        } else {
            2200
        };

        let mut guard = self.stream.lock().unwrap();
        if guard.is_none() {
            return Err("Audio stream chưa khởi tạo".into());
        }

        for i in 0..max_chunks {
            let mut chunk = vec![0i16; self.chunk_samples];
            let read_res = {
                let stream = guard.as_mut().ok_or("Audio stream chưa khởi tạo")?;
                stream.read_exact_i16_checked(&mut chunk, state_mgr)
            };
            match read_res {
                Ok(true) => {}
                Ok(false) => return Ok(Vec::new()),
                Err(reason) => {
                    eprintln!("🔄 [cpal]: {} -> Reset luồng thu âm...", reason);
                    *guard = None;
                    return Err(reason.into());
                }
            }

            let rms = SchmittTriggerVad::compute_rms(&chunk);
            let (has_spoken, should_stop) = vad.process_frame(rms, 100);

            if !has_spoken {
                preroll_buffer.push_back(chunk);
                if preroll_buffer.len() > 6 {
                    preroll_buffer.pop_front();
                }
                if (i * 100) as u32 >= initial_silence_timeout_ms {
                    break;
                }
            } else {
                if recorded_chunks.is_empty() {
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

        drop(guard);

        if vad.spoken_duration_ms < 130 {
            return Ok(Vec::new());
        }

        let mut full_samples: Vec<i16> = Vec::new();
        for chunk in recorded_chunks {
            full_samples.extend(chunk);
        }

        let max_abs = full_samples
            .iter()
            .map(|&s| (s as i32).abs())
            .max()
            .unwrap_or(0);
        if max_abs < 250 {
            return Ok(Vec::new());
        }

        normalize_audio(&mut full_samples, vad.ambient_noise);
        Ok(full_samples)
    }

    pub fn record_command_with_silero(
        &self,
        vad: &mut SileroVad,
        initial_wait_secs: f32,
        max_speech_secs: f32,
        state_mgr: Option<&StateManager>,
    ) -> Result<Vec<i16>, Box<dyn std::error::Error>> {
        self.record_command_with_silero_seeded(
            vad,
            &[],
            initial_wait_secs,
            max_speech_secs,
            state_mgr,
        )
    }

    /// Thu câu lệnh bằng Silero VAD v5; nếu `seed_frames` không rỗng (chế độ nói liền 1 hơi),
    /// giữ nguyên toàn bộ các khung đầu câu đã nói ngay sau Wake Word.
    pub fn record_command_with_silero_seeded(
        &self,
        vad: &mut SileroVad,
        seed_frames: &[Vec<f32>],
        initial_wait_secs: f32,
        max_speech_secs: f32,
        state_mgr: Option<&StateManager>,
    ) -> Result<Vec<i16>, Box<dyn std::error::Error>> {
        self.ensure_started()?;
        vad.reset();

        const SPEECH_START_PROB: f32 = 0.42;
        const SPEECH_CONT_PROB: f32 = 0.28;
        const START_MIN_FRAMES: u32 = 2;    // 64ms liên tiếp
        const END_SILENCE_FRAMES: u32 = 34; // ~1088ms im lặng để chốt câu (không ngắt sớm khi ngập ngừng)
        const MIN_SPEECH_FRAMES: u32 = 8;   // ~256ms tối thiểu
        const PREROLL_FRAMES: usize = 20;   // ~640ms đệm đầu câu

        let max_wait_frames = ((initial_wait_secs * 1000.0) / 32.0).round() as u32;
        let max_speech_frames = ((max_speech_secs * 1000.0) / 32.0).round() as u32;

        let mut preroll: VecDeque<Vec<f32>> = VecDeque::with_capacity(PREROLL_FRAMES + 1);
        let mut recorded_f32: Vec<f32> = Vec::new();

        let mut waiting_frames: u32 = 0;
        let mut speech_onset_streak: u32 = 0;
        let mut has_spoken = false;
        let mut spoken_frames: u32 = 0;
        let mut silence_frames: u32 = 0;

        // Nạp khung mồi (One-Breath) và làm ấm trạng thái ẩn của Silero VAD
        if !seed_frames.is_empty() {
            has_spoken = true;
            for frame in seed_frames {
                if frame.len() == SileroVad::CHUNK_SAMPLES {
                    let _ = vad.process_chunk(frame);
                }
                recorded_f32.extend_from_slice(frame);
                spoken_frames += 1;
            }
        }

        let mut guard = self.stream.lock().unwrap();

        loop {
            let mut chunk = vec![0.0f32; SileroVad::CHUNK_SAMPLES];
            let read_res = {
                let stream = guard.as_mut().ok_or("Audio stream chưa khởi tạo")?;
                stream.read_exact_f32_checked(&mut chunk, state_mgr)
            };
            match read_res {
                Ok(true) => {}
                Ok(false) => return Ok(Vec::new()),
                Err(reason) => {
                    eprintln!("🔄 [cpal]: {} -> Reset luồng thu âm...", reason);
                    *guard = None;
                    return Err(reason.into());
                }
            }

            let prob = vad.process_chunk(&chunk).unwrap_or(0.0);

            if !has_spoken {
                preroll.push_back(chunk);
                if preroll.len() > PREROLL_FRAMES {
                    preroll.pop_front();
                }

                if prob >= SPEECH_START_PROB {
                    speech_onset_streak += 1;
                    if speech_onset_streak >= START_MIN_FRAMES {
                        has_spoken = true;
                        spoken_frames = speech_onset_streak;
                        silence_frames = 0;
                        for frame in preroll.drain(..) {
                            recorded_f32.extend_from_slice(&frame);
                        }
                    }
                } else {
                    speech_onset_streak = 0;
                    waiting_frames += 1;
                    if waiting_frames >= max_wait_frames {
                        break;
                    }
                }
            } else {
                recorded_f32.extend_from_slice(&chunk);
                spoken_frames += 1;

                if prob < SPEECH_CONT_PROB {
                    silence_frames += 1;
                    if silence_frames >= END_SILENCE_FRAMES && spoken_frames >= MIN_SPEECH_FRAMES {
                        break;
                    }
                } else {
                    silence_frames = 0;
                }

                if spoken_frames >= max_speech_frames {
                    break;
                }
            }
        }

        drop(guard);

        if !has_spoken || spoken_frames < MIN_SPEECH_FRAMES {
            return Ok(Vec::new());
        }

        let mut full_samples: Vec<i16> = recorded_f32
            .into_iter()
            .map(|s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        let max_abs = full_samples
            .iter()
            .map(|&s| (s as i32).abs())
            .max()
            .unwrap_or(0);
        if max_abs < 200 {
            return Ok(Vec::new());
        }

        normalize_audio(&mut full_samples, 0.015);
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

        file.write_all(b"RIFF")?;
        file.write_all(&chunk_size.to_le_bytes())?;
        file.write_all(b"WAVE")?;

        file.write_all(b"fmt ")?;
        file.write_all(&16u32.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;

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

fn normalize_audio(samples: &mut [i16], ambient_noise: f32) {
    if samples.is_empty() {
        return;
    }
    let max_abs = samples
        .iter()
        .map(|&s| (s as i32).abs())
        .max()
        .unwrap_or(0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_48k_to_16k() {
        let mut r = StreamResampler::new(48000, 16000);
        let mut out = Vec::new();
        for i in 0..4800 {
            let s = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 48000.0)).sin();
            r.push_sample(s, &mut out);
        }
        assert_eq!(out.len(), 1600);
    }

    #[test]
    fn test_resampler_44100_to_16k() {
        let mut r = StreamResampler::new(44100, 16000);
        let mut out = Vec::new();
        for i in 0..4410 {
            let s = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 44100.0)).sin();
            r.push_sample(s, &mut out);
        }
        assert!((out.len() as i32 - 1600).abs() <= 2, "Got {}", out.len());
    }

    #[test]
    fn test_soft_agc_far_field_and_noise_gate() {
        let mut agc = SoftAgc::new();
        // Nhiễu nền cực nhỏ (< 0.004) không bị khuếch đại
        let mut quiet = vec![0.001f32; 512];
        agc.process_frame(&mut quiet);
        assert!((agc.current_gain() - 1.0).abs() < 0.05);

        // Giọng nói xa yếu (0.015) được nâng dần lên rõ rệt
        for _ in 0..30 {
            let mut far_voice = vec![0.015f32; 512];
            agc.process_frame(&mut far_voice);
        }
        assert!(agc.current_gain() > 2.0 && agc.current_gain() <= 3.5);
    }
}
