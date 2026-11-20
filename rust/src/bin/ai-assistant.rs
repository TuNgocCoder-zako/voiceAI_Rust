use clap::Parser;
use std::io::{self, BufRead, Write};
use std::time::Instant;
use tokio::sync::mpsc;
use voice_ai::audio::{AudioRecorder, WhisperEngine};
use voice_ai::config::Config;
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::core::state::{AssistantState, StateManager};
use voice_ai::ipc::client::send_ipc_command;
use voice_ai::ipc::server::{ActionRequest, IpcServer};
use voice_ai::speech::{play_chime, ChimeType, PiperTts};
use voice_ai::tools::apps::build_app_index;

#[derive(Parser, Debug)]
#[command(name = "ai-assistant")]
#[command(author = "TuNgocCoder <tudm.dev@gmail.com>")]
#[command(version = "2.5.0")]
#[command(about = "High-Performance Voice AI Assistant Daemon & Fast-Path NLU in Rust", long_about = None)]
struct Args {
    #[arg(short, long, help = "Chế độ bấm Enter để nhập lệnh văn bản (Push-to-Talk)")]
    push_to_talk: bool,

    #[arg(short, long, help = "Bật giọng nói phản hồi (Piper TTS offline)")]
    voice: bool,

    #[arg(short = 'l', long, help = "Chế độ nghe qua Microphone thời gian thực (Voice Chat)")]
    listen: bool,

    #[arg(long, help = "Gửi tín hiệu đánh thức tới trợ lý đang chạy nền")]
    trigger: bool,

    #[arg(long, help = "Gửi tín hiệu ngắt lời / dừng phát âm tới trợ lý đang chạy nền")]
    stop: bool,

    #[arg(long, help = "Dừng ngay và kích hoạt micro lắng nghe ngay lượt mới")]
    relisten: bool,

    #[arg(long, help = "Kiểm tra trạng thái của trợ lý đang chạy nền qua IPC")]
    status: bool,

    #[arg(long, help = "Chạy câu lệnh đơn lẻ rồi thoát ngay")]
    query: Option<String>,

    #[arg(long, help = "Chạy đo kiểm hiệu năng Fast-Path NLU siêu tốc")]
    benchmark: bool,
}

const BENCHMARK_COMMANDS: &[&str] = &[
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config = Config::default();

    // 1. IPC Client subcommands
    if args.status {
        match send_ipc_command(&config.socket_path, "status", None).await {
            Ok(resp) => {
                println!("✅ Trợ lý AI đang chạy nền! Trạng thái: {:?}", resp.state.unwrap_or_default());
            }
            Err(e) => {
                println!("⚠️ Không kết nối được tới trợ lý đang chạy nền: {}", e);
            }
        }
        return Ok(());
    }

    if args.trigger {
        match send_ipc_command(&config.socket_path, "trigger", None).await {
            Ok(resp) => println!("Tín hiệu kích hoạt đã gửi: {:?}", resp.message),
            Err(e) => eprintln!("Lỗi gửi trigger: {}", e),
        }
        return Ok(());
    }

    if args.stop {
        match send_ipc_command(&config.socket_path, "stop", None).await {
            Ok(resp) => println!("Tín hiệu dừng đã gửi: {:?}", resp.message),
            Err(e) => eprintln!("Lỗi gửi stop: {}", e),
        }
        return Ok(());
    }

    if args.relisten {
        match send_ipc_command(&config.socket_path, "relisten", None).await {
            Ok(resp) => println!("Tín hiệu relisten đã gửi: {:?}", resp.message),
            Err(e) => eprintln!("Lỗi gửi relisten: {}", e),
        }
        return Ok(());
    }

    // 2. Benchmark Subcommand
    if args.benchmark {
        run_nlu_benchmark(200);
        return Ok(());
    }

    let app_index = build_app_index();
    let tts = PiperTts::new();
    let recorder = AudioRecorder::new();
    let whisper = WhisperEngine::new();
    let ollama = voice_ai::ai::OllamaClient::new(&config.ollama_url, &config.model);

    // 3. Single Query Execution
    if let Some(q) = args.query {
        let (handled, reply) = fast_path_nlu(&q, &app_index, false);
        if handled {
            println!("🤖 [Fast-Path]: {}", reply);
            if args.voice && tts.is_available() {
                tts.speak(&reply, None);
            }
        } else {
            println!("🌐 [Deep-Path LLM]: Đang truy vấn Ollama ({})...", config.model);
            let messages = vec![
                voice_ai::ai::ChatMessage {
                    role: "system".to_string(),
                    content: voice_ai::ai::ollama::SYSTEM_PROMPT.to_string(),
                },
                voice_ai::ai::ChatMessage {
                    role: "user".to_string(),
                    content: q,
                },
            ];
            match ollama.chat(messages, |chunk| {
                print!("{}", chunk);
                let _ = io::stdout().flush();
            }).await {
                Ok(reply) => {
                    println!();
                    if args.voice && tts.is_available() {
                        tts.speak(&reply, None);
                    }
                }
                Err(_) => {
                    let msg = "Dạ, hiện tại dịch vụ Ollama LLM chưa được khởi chạy trên máy của bạn.";
                    println!("⚠️ {}", msg);
                    if args.voice && tts.is_available() {
                        tts.speak(msg, None);
                    }
                }
            }
        }
        return Ok(());
    }

    // 4. Voice Listening Mode (Microphone Voice Loop)
    if args.listen {
        println!("🎙️ Chế độ đàm thoại giọng nói (Voice Chat) - Nói 'tạm biệt' hoặc Ctrl+C để thoát.");
        println!("🔔 Đang sẵn sàng... Hãy nói vào microphone!");

        loop {
            play_chime(ChimeType::Wake);
            println!("\n🔴 Đang nghe...");
            match recorder.record(8.0, None) {
                Ok(samples) => {
                    if samples.len() < 3200 { // Dưới 200ms
                        continue;
                    }
                    let temp_wav = "/tmp/voice_input.wav";
                    if AudioRecorder::save_wav(&samples, temp_wav, 16000).is_ok() {
                        println!("⏳ Đang nhận diện qua Whisper...");
                        let start_stt = Instant::now();
                        match whisper.transcribe_wav(temp_wav) {
                            Ok(user_text) => {
                                let stt_time = start_stt.elapsed();
                                let trimmed = user_text.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                println!("👤 Bạn ({:.2?}): \"{}\"", stt_time, trimmed);

                                if is_exit_phrase(trimmed) {
                                    let bye = "Tạm biệt bạn nhé! Hẹn gặp lại bạn sau.";
                                    println!("🤖 Trợ lý: {}", bye);
                                    if tts.is_available() {
                                        tts.speak(bye, None);
                                    }
                                    break;
                                }

                                let start_nlu = Instant::now();
                                let (handled, reply) = fast_path_nlu(trimmed, &app_index, false);
                                let nlu_time = start_nlu.elapsed();

                                if handled {
                                    println!("🤖 Trợ lý ({:.2} µs): {}", nlu_time.as_secs_f64() * 1_000_000.0, reply);
                                    if tts.is_available() {
                                        tts.speak(&reply, None);
                                    }
                                } else {
                                    println!("🌐 [Deep-Path LLM]: Đang suy luận qua Ollama...");
                                    let messages = vec![
                                        voice_ai::ai::ChatMessage {
                                            role: "system".to_string(),
                                            content: voice_ai::ai::ollama::SYSTEM_PROMPT.to_string(),
                                        },
                                        voice_ai::ai::ChatMessage {
                                            role: "user".to_string(),
                                            content: trimmed.to_string(),
                                        },
                                    ];
                                    match ollama.chat(messages, |chunk| {
                                        print!("{}", chunk);
                                        let _ = io::stdout().flush();
                                    }).await {
                                        Ok(llm_reply) => {
                                            println!();
                                            if tts.is_available() {
                                                tts.speak(&llm_reply, None);
                                            }
                                        }
                                        Err(_) => {
                                            let msg = "Dạ, hiện tại tôi chưa kết nối được với mô hình Ollama.";
                                            println!("⚠️ {}", msg);
                                            if tts.is_available() {
                                                tts.speak(msg, None);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => eprintln!("Lỗi STT: {}", e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Lỗi thu âm: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
        return Ok(());
    }

    // 5. Interactive CLI / Push-to-Talk Mode
    if args.push_to_talk {
        println!("🎙️ Chế độ tương tác dòng lệnh (Push-to-Talk) - Nhập 'exit' để thoát.");
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("\n👤 Bạn: ");
            stdout.flush()?;
            let mut line = String::new();
            if stdin.lock().read_line(&mut line)? == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
                println!("Tạm biệt bạn!");
                break;
            }
            if trimmed.is_empty() {
                continue;
            }

            let start = Instant::now();
            let (handled, reply) = fast_path_nlu(trimmed, &app_index, false);
            let elapsed = start.elapsed();

            if handled {
                println!("🤖 Trợ lý ({:.2} µs): {}", elapsed.as_secs_f64() * 1_000_000.0, reply);
                if args.voice && tts.is_available() {
                    tts.speak(&reply, None);
                }
            } else {
                println!("🌐 [Deep-Path]: Đang chuyển sang Ollama LLM ({:.2} µs)...", elapsed.as_secs_f64() * 1_000_000.0);
            }
        }
        return Ok(());
    }

    // 6. Default: Run Daemon & IPC Server
    println!("🌟 Khởi chạy Voice AI Assistant Daemon (Rust Core v2.5.0)...");
    let state_manager = StateManager::new();
    let (action_tx, mut action_rx) = mpsc::channel::<ActionRequest>(64);

    let server = IpcServer::new(&config.socket_path, state_manager.clone(), action_tx);

    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Lỗi IPC Server: {}", e);
        }
    });

    println!("⚡ Trợ lý đã sẵn sàng nhận lệnh qua IPC Unix Domain Socket.");
    state_manager.set_state(AssistantState::Idle, None).await;

    while let Some(action) = action_rx.recv().await {
        match action {
            ActionRequest::Trigger => {
                println!("🔔 Nhận sự kiện Trigger: Đang lắng nghe...");
                state_manager.set_state(AssistantState::Listening, None).await;
                play_chime(ChimeType::Wake);
            }
            ActionRequest::Stop => {
                println!("⏹️ Nhận sự kiện Stop: Đã dừng.");
                state_manager.set_state(AssistantState::Idle, None).await;
            }
            ActionRequest::Relisten => {
                println!("🔄 Nhận sự kiện Relisten: Lắng nghe lượt mới...");
                state_manager.set_state(AssistantState::Listening, None).await;
                play_chime(ChimeType::Wake);
            }
            ActionRequest::Query(text) => {
                state_manager.set_state(AssistantState::Thinking, None).await;
                let (handled, rep) = fast_path_nlu(&text, &app_index, false);
                if handled {
                    println!("🤖 [Fast-Path]: {}", rep);
                    state_manager.set_state(AssistantState::Speaking, Some(rep.clone())).await;
                    if tts.is_available() {
                        tts.speak(&rep, Some(&state_manager));
                    }
                }
                state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
    }

    Ok(())
}

fn run_nlu_benchmark(iterations: usize) {
    let total_runs = BENCHMARK_COMMANDS.len() * iterations;
    println!(
        "🚀 Bắt đầu Benchmark Fast-Path NLU bằng Rust ({} câu lệnh x {} vòng = {} lượt đo)...",
        BENCHMARK_COMMANDS.len(),
        iterations,
        total_runs
    );

    let app_index = build_app_index();

    // Warmup
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
    println!("- Tốc độ xử lý (Throughput): {:.0} lệnh/giây", total_runs as f64 / total_duration.as_secs_f64());
    println!("- Độ trễ Tối thiểu (Min) : {:.3} µs ({:.6} ms)", min_us, min_us / 1000.0);
    println!("- Độ trễ Trung vị (Median): {:.3} µs ({:.6} ms)", median_us, median_us / 1000.0);
    println!("- Độ trễ Trung bình (Mean): {:.3} µs ({:.6} ms)", avg_us, avg_us / 1000.0);
    println!("- Phân vị P90             : {:.3} µs ({:.6} ms)", p90_us, p90_us / 1000.0);
    println!("- Phân vị P99             : {:.3} µs ({:.6} ms)", p99_us, p99_us / 1000.0);
    println!("- Độ trễ Tối đa (Max)     : {:.3} µs ({:.6} ms)", max_us, max_us / 1000.0);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

fn is_exit_phrase(text: &str) -> bool {
    let t = text.to_lowercase();
    let t = t.trim().trim_end_matches(['.', '!', '?']);
    let exits = [
        "tạm biệt", "kết thúc", "dừng lại", "dừng cuộc trò chuyện",
        "tắt trợ lý", "đóng trợ lý", "thôi im", "im lặng", "hẹn gặp lại", "tạm biệt nhé"
    ];
    exits.iter().any(|&e| t == e || t.starts_with(e) || t.ends_with(e))
}
