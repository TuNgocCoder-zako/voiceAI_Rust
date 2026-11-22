use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::Instant;
use voice_ai::audio::{AudioRecorder, OpenWakeWordDetector, SileroVad};
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::tools::apps::build_app_index;
use voice_ai::tools::system::is_media_playing;

pub const BENCHMARK_COMMANDS: &[&str] = &[
    "tăng âm lượng 20%",
    "giảm âm lượng",
    "tắt tiếng",
    "bật tiếng",
    "tăng độ sáng màn hình",
    "giảm độ sáng",
    "bật wifi",
    "tắt bluetooth",
    "chế độ turbo",
    "chế độ im lặng",
    "pin còn bao nhiêu",
    "kiểm tra ram",
    "bây giờ là mấy giờ",
    "hôm nay ngày mấy",
    "thời tiết hà nội thế nào",
    "mở visual studio code",
    "mở google chrome",
    "đóng cửa sổ",
    "phóng to màn hình",
    "giải phóng cổng 8080",
    "kiểm tra trạng thái git",
    "kiểm tra docker",
    "phiên bản java",
    "hẹn giờ 5 phút",
    "hủy hẹn giờ",
];

pub fn run_nlu_benchmark(iterations: usize) {
    let total_runs = BENCHMARK_COMMANDS.len() * iterations;
    println!(
        "🚀 Bắt đầu Benchmark Fast-Path NLU bằng Rust ({} câu lệnh x {} vòng = {} lượt đo)...",
        BENCHMARK_COMMANDS.len(),
        iterations,
        total_runs
    );

    let app_index = build_app_index();

    for &cmd in BENCHMARK_COMMANDS.iter().take(5) {
        let _ = fast_path_nlu(cmd, &app_index, true);
    }

    let mut latencies_us: Vec<f64> = Vec::with_capacity(total_runs);

    let total_start = Instant::now();
    for _ in 0..iterations {
        for &cmd in BENCHMARK_COMMANDS {
            let t0 = Instant::now();
            let (handled, _) = fast_path_nlu(cmd, &app_index, true);
            let t1 = Instant::now();
            assert!(handled, "Benchmark command must be handled: {}", cmd);
            latencies_us.push(t1.duration_since(t0).as_secs_f64() * 1_000_000.0);
        }
    }
    let total_duration = total_start.elapsed();

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min_us = latencies_us.first().copied().unwrap_or(0.0);
    let max_us = latencies_us.last().copied().unwrap_or(0.0);
    let median_us = latencies_us[latencies_us.len() / 2];
    let p90_us = latencies_us[(latencies_us.len() as f64 * 0.90) as usize];
    let p99_us = latencies_us[(latencies_us.len() as f64 * 0.99) as usize];
    let avg_us = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;

    println!("\n📊 KẾT QUẢ HIỆU NĂNG FAST-PATH NLU (RUST NATIVE):");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("- Tổng số lượt thực thi : {} lượt", total_runs);
    println!("- Tổng thời gian đo     : {:.2?}", total_duration);
    println!(
        "- Tốc độ xử lý (Throughput): {:.0} lệnh/giây",
        total_runs as f64 / total_duration.as_secs_f64()
    );
    println!(
        "- Độ trễ Tối thiểu (Min) : {:.3} µs ({:.6} ms)",
        min_us,
        min_us / 1000.0
    );
    println!(
        "- Độ trễ Trung vị (Median): {:.3} µs ({:.6} ms)",
        median_us,
        median_us / 1000.0
    );
    println!(
        "- Độ trễ Trung bình (Mean): {:.3} µs ({:.6} ms)",
        avg_us,
        avg_us / 1000.0
    );
    println!(
        "- Phân vị P90             : {:.3} µs ({:.6} ms)",
        p90_us,
        p90_us / 1000.0
    );
    println!(
        "- Phân vị P99             : {:.3} µs ({:.6} ms)",
        p99_us,
        p99_us / 1000.0
    );
    println!(
        "- Độ trễ Tối đa (Max)     : {:.3} µs ({:.6} ms)",
        max_us,
        max_us / 1000.0
    );
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

pub fn run_audio_debugger() -> Result<(), Box<dyn std::error::Error>> {
    let recorder = AudioRecorder::new();
    let mut vad = SileroVad::load_default()
        .ok_or("Chưa thấy models/vad/silero_vad.onnx")?;
    let mut oww = OpenWakeWordDetector::load_default()
        .ok_or("Chưa thấy thư mục models/wakeword/")?;

    println!("🔬 Chế độ kiểm tra âm thanh trực tiếp (Ctrl+C để thoát)...");
    println!("   Thử gõ phím, mở nhạc và gọi 'Alexa' / 'Hey Jarvis' để quan sát chỉ số.\n");

    let mut oww_queue: VecDeque<f32> = VecDeque::with_capacity(2048);
    let mut last_music_check = Instant::now() - std::time::Duration::from_secs(5);
    let mut music_playing = false;

    loop {
        if last_music_check.elapsed() >= std::time::Duration::from_secs(2) {
            music_playing = is_media_playing();
            last_music_check = Instant::now();
        }

        let mut frame = [0.0f32; SileroVad::CHUNK_SAMPLES];
        if !recorder.read_frame_f32(&mut frame, None)? {
            break;
        }

        let vad_prob = vad.process_chunk(&frame)?;
        oww_queue.extend(frame.iter().copied());

        if oww_queue.len() >= OpenWakeWordDetector::FRAME_SAMPLES {
            let chunk_1280: Vec<f32> = oww_queue
                .drain(..OpenWakeWordDetector::FRAME_SAMPLES)
                .collect();
            let res = oww.process_frame(&chunk_1280, music_playing)?;

            let score_str = res
                .scores
                .iter()
                .map(|(t, s)| format!("{}: {:.2}", t.name(), s))
                .collect::<Vec<_>>()
                .join(" | ");

            let bar_len = (vad_prob * 10.0).round() as usize;
            let bar = format!(
                "{}{}",
                "█".repeat(bar_len.min(10)),
                "░".repeat(10usize.saturating_sub(bar_len))
            );

            print!(
                "\r🎙️ VAD: [{}] {:.2} | {} (Ngưỡng: {:.2}{})   ",
                bar,
                vad_prob,
                if score_str.is_empty() { "warmup..." } else { &score_str },
                res.active_threshold,
                if music_playing { " 🎵" } else { "" }
            );
            let _ = io::stdout().flush();

            if let Some((ww, sc)) = res.detected {
                println!("\n🔔 PHÁT HIỆN WAKE WORD: {} (score = {:.2})!", ww.name(), sc);
            }
        }
    }
    Ok(())
}
