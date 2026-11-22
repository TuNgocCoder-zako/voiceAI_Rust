use clap::Parser;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use voice_ai::audio::{detect_wake_word, AudioRecorder, WhisperEngine};
use voice_ai::config::Config;
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::core::nlu::router::TIMER_MANAGER;
use voice_ai::core::state::{AssistantState, StateManager};
use voice_ai::ipc::client::{send_ipc_command, send_ipc_command_with_options};
use voice_ai::ipc::server::{ActionRequest, IpcServer};
use voice_ai::speech::{play_chime, ChimeType, PiperTts};
use voice_ai::tools::apps::{build_app_index, DesktopApp};

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

    #[arg(short = 'l', long, help = "Chế độ nghe qua Microphone liên tục (Always-on VAD Chat)")]
    listen: bool,

    #[arg(short = 'w', long, help = "Chế độ đánh thức bằng từ khóa Wake Word rảnh tay ('Alexa', 'Trợ lý ơi')")]
    wake_word: bool,

    #[arg(short = 'm', long, help = "Chỉ định thủ công mô hình LLM (Bỏ qua tự động định tuyến)")]
    model: Option<String>,

    #[arg(long, help = "Ép dùng mô hình suy luận sâu (Reasoning) khi dùng --query")]
    deep: bool,

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

    #[arg(long, help = "Đọc trực tiếp một đoạn văn bản qua TTS mà không gửi lên LLM")]
    speak: Option<String>,

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
                println!(
                    "✅ Trợ lý AI đang chạy nền! Trạng thái: {:?}",
                    resp.state.unwrap_or_default()
                );
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

    let tts = Arc::new(PiperTts::new());

    // 2.5. Direct TTS Speak Subcommand (dùng cho nút loa trên Overlay)
    if let Some(text) = args.speak {
        if config.socket_path.exists() {
            if send_ipc_command(&config.socket_path, "speak", Some(text.clone()))
                .await
                .is_ok()
            {
                return Ok(());
            }
        }
        if tts.is_available() {
            let tts_c = tts.clone();
            let _ = tokio::task::spawn_blocking(move || tts_c.speak(&text, None)).await;
        }
        return Ok(());
    }

    let app_index = Arc::new(build_app_index());
    let recorder = Arc::new(AudioRecorder::new());
    let whisper = Arc::new(WhisperEngine::new());
    let ollama = Arc::new(voice_ai::ai::OllamaClient::new(
        &config.ollama_url,
        &config.model,
    ));
    let model_router = Arc::new(voice_ai::ai::ModelRouter::new(&config.ollama_url));

    // 3. Single Query Execution (đã vá lỗi Double Execution)
    if let Some(q) = args.query {
        if config.socket_path.exists() {
            if send_ipc_command_with_options(
                &config.socket_path,
                "query",
                Some(q.clone()),
                Some(args.deep),
            )
            .await
            .is_ok()
            {
                println!("📨 Đã gửi câu lệnh tới Trợ lý AI đang chạy nền: \"{}\"", q);
                return Ok(());
            }
        }

        let (handled, reply) = if args.deep {
            (false, String::new())
        } else {
            fast_path_nlu(&q, &app_index, false)
        };

        if handled {
            println!("🤖 [Fast-Path]: {}", reply);
            if args.voice && tts.is_available() {
                let tts_c = tts.clone();
                let rep_c = reply.clone();
                let _ = tokio::task::spawn_blocking(move || tts_c.speak(&rep_c, None)).await;
            }
        } else {
            execute_llm_query(
                &q,
                args.model.as_deref(),
                args.deep,
                &model_router,
                &ollama,
                &config,
                &tts,
                None,
                args.voice,
            )
            .await;
        }
        return Ok(());
    }

    // 4. Dedicated Hands-Free Wake Word Mode
    if args.wake_word {
        let wake_whisper = Arc::new(WhisperEngine::fast_wake_engine());
        let state_manager = StateManager::new();
        TIMER_MANAGER.spawn_watcher(tts.clone(), state_manager.clone());
        ensure_overlay_running();
        run_wake_word_loop(
            recorder.clone(),
            wake_whisper,
            whisper.clone(),
            app_index.clone(),
            model_router.clone(),
            ollama.clone(),
            config.clone(),
            tts.clone(),
            state_manager,
            args.model.clone(),
        )
        .await;
        return Ok(());
    }

    // 5. Always-on Voice Listening Mode (Microphone Voice Loop without wake word)
    if args.listen {
        println!("🎙️ Chế độ đàm thoại giọng nói liên tục (Always-on VAD) - Nói 'tạm biệt' hoặc Ctrl+C để thoát.");
        println!("🔔 Đang sẵn sàng... Hãy nói vào microphone!");

        loop {
            play_chime(ChimeType::Wake);
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            println!("\n🔴 Đang nghe...");
            match recorder.record(8.0, None) {
                Ok(samples) => {
                    if samples.len() < 3200 {
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
                                        let tts_c = tts.clone();
                                        let _ = tokio::task::spawn_blocking(move || {
                                            tts_c.speak(bye, None)
                                        })
                                        .await;
                                    }
                                    break;
                                }

                                execute_voice_command(
                                    trimmed,
                                    &app_index,
                                    args.model.as_deref(),
                                    &model_router,
                                    &ollama,
                                    &config,
                                    &tts,
                                    None,
                                )
                                .await;
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

    // 6. Interactive CLI / Push-to-Talk Mode
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
                println!(
                    "🤖 Trợ lý ({:.2} µs): {}",
                    elapsed.as_secs_f64() * 1_000_000.0,
                    reply
                );
                if args.voice && tts.is_available() {
                    let tts_c = tts.clone();
                    let rep_c = reply.clone();
                    let _ = tokio::task::spawn_blocking(move || tts_c.speak(&rep_c, None)).await;
                }
            } else {
                execute_llm_query(
                    trimmed,
                    args.model.as_deref(),
                    args.deep,
                    &model_router,
                    &ollama,
                    &config,
                    &tts,
                    None,
                    args.voice,
                )
                .await;
            }
        }
        return Ok(());
    }

    // 7. Default: Run Daemon & IPC Server with Integrated Hands-Free Wake Word + Timer Watcher
    println!("🌟 Khởi chạy Voice AI Assistant Daemon (Rust Core v2.5.0)...");
    ensure_mic_levels();
    println!("🎧 Đang túc trực lắng nghe từ khóa: 'Alexa', 'Trợ lý ơi', 'Hey Jarvis'...");
    let state_manager = StateManager::new();
    let (action_tx, mut action_rx) = mpsc::channel::<ActionRequest>(64);

    // Kích hoạt luồng đếm ngược Hẹn giờ / Báo thức chạy nền
    TIMER_MANAGER.spawn_watcher(tts.clone(), state_manager.clone());

    let server = IpcServer::new(&config.socket_path, state_manager.clone(), action_tx);

    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Lỗi IPC Server: {}", e);
        }
    });

    // Giám sát và tự động duy trì giao diện Dynamic Island Overlay luôn chạy
    tokio::spawn(async {
        loop {
            let _ = tokio::task::spawn_blocking(ensure_overlay_running).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    let ww_recorder = recorder.clone();
    let ww_wake_whisper = Arc::new(WhisperEngine::fast_wake_engine());
    let ww_query_whisper = whisper.clone();
    let ww_app_index = app_index.clone();
    let ww_router = model_router.clone();
    let ww_ollama = ollama.clone();
    let ww_config = config.clone();
    let ww_tts = tts.clone();
    let ww_state_mgr = state_manager.clone();

    std::thread::Builder::new()
        .name("wake-word-worker".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(run_wake_word_loop(
                ww_recorder,
                ww_wake_whisper,
                ww_query_whisper,
                ww_app_index,
                ww_router,
                ww_ollama,
                ww_config,
                ww_tts,
                ww_state_mgr,
                None,
            ));
        })
        .expect("Failed to spawn wake-word-worker thread");

    println!("⚡ Trợ lý đã sẵn sàng nhận lệnh qua Giọng nói (Wake Word) hoặc IPC Socket.");
    state_manager.set_state(AssistantState::Idle, None).await;

    while let Some(action) = action_rx.recv().await {
        match action {
            ActionRequest::Trigger => {
                // state_manager.trigger_listen() đã được bật từ IPC Server -> wake-word-worker sẽ tự mở mic
                println!("🔔 Nhận sự kiện Trigger: Đang kích hoạt micro lắng nghe...");
            }
            ActionRequest::Stop => {
                println!("⏹️ Nhận sự kiện Stop: Đã dừng.");
                state_manager.stop_all();
                state_manager.set_state(AssistantState::Idle, None).await;
            }
            ActionRequest::Relisten => {
                println!("🔄 Nhận sự kiện Relisten: Dừng tác vụ cũ và mở mic nghe lượt mới...");
            }
            ActionRequest::Speak(text) => {
                // Ngắt âm cũ nếu đang đọc
                if tts.is_speaking() {
                    state_manager.stop_all();
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                }
                state_manager.reset_interrupt();

                let state_mgr = state_manager.clone();
                let tts_clone = tts.clone();
                tokio::spawn(async move {
                    state_mgr
                        .set_state_full(AssistantState::Speaking, Some(text.clone()), None)
                        .await;
                    if tts_clone.is_available() {
                        let tts_bg = tts_clone.clone();
                        let mgr_bg = state_mgr.clone();
                        let txt_bg = text.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            tts_bg.speak(&txt_bg, Some(&mgr_bg));
                        })
                        .await;
                    }
                    if !state_mgr.has_manual_trigger() {
                        state_mgr.set_state(AssistantState::Idle, None).await;
                    }
                });
            }
            ActionRequest::Query { text, deep } => {
                // Nếu đang nói hoặc đang thu, ngắt sạch trước khi xử lý truy vấn mới
                if tts.is_speaking() {
                    state_manager.stop_all();
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                }
                state_manager.reset_interrupt();

                let state_mgr = state_manager.clone();
                let app_idx = app_index.clone();
                let tts_clone = tts.clone();
                let router_clone = model_router.clone();
                let ollama_clone = ollama.clone();
                let cfg_clone = config.clone();

                tokio::spawn(async move {
                    state_mgr
                        .set_state_full(
                            AssistantState::Thinking,
                            Some("Đang xử lý...".to_string()),
                            Some(text.clone()),
                        )
                        .await;

                    let (handled, rep) = if deep {
                        (false, String::new())
                    } else {
                        let q_bg = text.clone();
                        let app_bg = app_idx.clone();
                        tokio::task::spawn_blocking(move || fast_path_nlu(&q_bg, &app_bg, false))
                            .await
                            .unwrap_or((false, String::new()))
                    };

                    if handled {
                        println!("🤖 [Fast-Path]: {}", rep);
                        state_mgr
                            .set_state_full(
                                AssistantState::Speaking,
                                Some(rep.clone()),
                                Some(text.clone()),
                            )
                            .await;

                        if tts_clone.is_available() && !rep.is_empty() {
                            let tts_bg = tts_clone.clone();
                            let mgr_bg = state_mgr.clone();
                            let rep_bg = rep.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                tts_bg.speak(&rep_bg, Some(&mgr_bg));
                            })
                            .await;
                        }
                        if !state_mgr.has_manual_trigger() {
                            state_mgr.set_state(AssistantState::Idle, None).await;
                        }
                    } else {
                        execute_llm_query(
                            &text,
                            None,
                            deep,
                            &router_clone,
                            &ollama_clone,
                            &cfg_clone,
                            &tts_clone,
                            Some(&state_mgr),
                            true,
                        )
                        .await;
                    }
                });
            }
        }
    }

    Ok(())
}

fn ensure_overlay_running() {
    let qml_check = std::process::Command::new("pgrep")
        .args(["-f", "VoiceOverlay.qml"])
        .output();
    if let Ok(out) = qml_check {
        if out.status.success() {
            return;
        }
    }

    let home = dirs::home_dir().unwrap_or_default();
    let qml_candidates = [
        home.join("Projects/rust/voice-ai/overlay/VoiceOverlay.qml"),
        std::env::current_dir()
            .unwrap_or_default()
            .join("overlay/VoiceOverlay.qml"),
        home.join(".config/voice-ai/overlay/VoiceOverlay.qml"),
    ];

    let qs_bin = if std::path::Path::new("/usr/bin/qs").exists() {
        Some("/usr/bin/qs")
    } else if std::path::Path::new("/usr/bin/quickshell").exists() {
        Some("/usr/bin/quickshell")
    } else {
        None
    };

    if let (Some(qs), Some(qml_path)) = (qs_bin, qml_candidates.iter().find(|p| p.exists())) {
        let mut cmd = std::process::Command::new(qs);
        cmd.args(["-p", qml_path.to_str().unwrap_or_default()]);

        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase()
            .contains("gnome");
        if is_gnome {
            cmd.env("QT_QPA_PLATFORM", "xcb");
        }
        if std::env::var("DISPLAY").is_err() {
            cmd.env("DISPLAY", ":0");
        }

        match cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                println!(
                    "🖥️ Đã khởi chạy giao diện Quickshell Dynamic Island Overlay: {:?}",
                    qml_path
                );
                return;
            }
            Err(e) => eprintln!("⚠️ Lỗi khởi chạy Quickshell overlay: {}", e),
        }
    }

    let check = std::process::Command::new("pgrep")
        .args(["-x", "voice-overlay"])
        .output();
    if let Ok(out) = check {
        if !out.status.success() {
            let candidates = [
                home.join(".local/bin/voice-overlay"),
                std::env::current_dir()
                    .unwrap_or_default()
                    .join("target/release/voice-overlay"),
                home.join("Projects/rust/voice-ai/target/release/voice-overlay"),
            ];
            if let Some(bin) = candidates.iter().find(|p| p.exists()) {
                let _ = std::process::Command::new(bin)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
    }
}

/// Thu câu lệnh chính sau khi Wake Word hoặc phím tắt Trigger kích hoạt
async fn capture_and_handle_followup(
    source_label: &str,
    recorder: &Arc<AudioRecorder>,
    query_whisper: &Arc<WhisperEngine>,
    app_index: &Arc<HashMap<String, DesktopApp>>,
    model_router: &Arc<voice_ai::ai::ModelRouter>,
    ollama: &Arc<voice_ai::ai::OllamaClient>,
    config: &Config,
    tts: &Arc<PiperTts>,
    state_manager: &StateManager,
    model_override: Option<&str>,
) {
    println!("\n🔔 [{}] Đang lắng nghe câu lệnh tiếp theo...", source_label);
    state_manager
        .set_state_full(
            AssistantState::Listening,
            Some("Đang lắng nghe...".to_string()),
            None,
        )
        .await;

    play_chime(ChimeType::Wake);
    // Chờ tiếng chime dứt hẳn để mic không thu lại tiếng loa
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

    if state_manager.is_interrupted() {
        return;
    }

    let followup_res = recorder
        .record(8.0, Some(state_manager))
        .map_err(|e| e.to_string());

    match followup_res {
        Ok(followup_samples) => {
            if state_manager.is_interrupted() || followup_samples.len() < 3200 {
                if !state_manager.has_manual_trigger() {
                    state_manager.set_state(AssistantState::Idle, None).await;
                }
                return;
            }
            let followup_wav = "/tmp/voice_input.wav";
            if AudioRecorder::save_wav(&followup_samples, followup_wav, 16000).is_ok() {
                state_manager
                    .set_state_full(
                        AssistantState::Thinking,
                        Some("Đang nhận diện giọng nói...".to_string()),
                        None,
                    )
                    .await;
                let stt_res = query_whisper
                    .transcribe_wav(followup_wav)
                    .map_err(|e| e.to_string());
                if let Ok(followup_text) = stt_res {
                    let q = followup_text.trim();
                    if !q.is_empty() {
                        println!("👤 Bạn: \"{}\"", q);
                        state_manager
                            .set_state_full(
                                AssistantState::Thinking,
                                Some("Đang xử lý...".to_string()),
                                Some(q.to_string()),
                            )
                            .await;
                        execute_voice_command(
                            q,
                            app_index,
                            model_override,
                            model_router,
                            ollama,
                            config,
                            tts,
                            Some(state_manager),
                        )
                        .await;
                        return;
                    }
                }
            }
            if !state_manager.has_manual_trigger() {
                state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
        Err(err) => {
            eprintln!("Lỗi thu âm tiếp nối: {}", err);
            if !state_manager.has_manual_trigger() {
                state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
    }
}

/// Vòng lặp lắng nghe từ khóa Wake Word rảnh tay + xử lý Trigger / Relisten tức thì
async fn run_wake_word_loop(
    recorder: Arc<AudioRecorder>,
    wake_whisper: Arc<WhisperEngine>,
    query_whisper: Arc<WhisperEngine>,
    app_index: Arc<HashMap<String, DesktopApp>>,
    model_router: Arc<voice_ai::ai::ModelRouter>,
    ollama: Arc<voice_ai::ai::OllamaClient>,
    config: Config,
    tts: Arc<PiperTts>,
    state_manager: StateManager,
    model_override: Option<String>,
) {
    println!("🎧 [Hands-Free Wake Word]: Đang túc trực... Hãy nói 'Alexa', 'Trợ lý ơi' hoặc 'Hey Jarvis'.");

    loop {
        // 1. Ưu tiên xử lý kích hoạt thủ công (--trigger / --relisten)
        if state_manager.has_manual_trigger() {
            // Chờ TTS cũ dừng hẳn (tối đa ~500ms) trước khi mở mic
            for _ in 0..20 {
                if !tts.is_speaking() {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            }
            state_manager.take_manual_trigger();
            state_manager.reset_interrupt();

            capture_and_handle_followup(
                "Manual Trigger",
                &recorder,
                &query_whisper,
                &app_index,
                &model_router,
                &ollama,
                &config,
                &tts,
                &state_manager,
                model_override.as_deref(),
            )
            .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
            continue;
        }

        // 2. Nếu vừa nhận lệnh --stop: chờ TTS dứt rồi khôi phục cờ để tiếp tục chờ wake word
        if state_manager.is_interrupted() {
            if tts.is_speaking() {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            if !state_manager.has_manual_trigger() {
                state_manager.reset_interrupt();
            }
            continue;
        }

        // 3. Tạm dừng thu wake word khi hệ thống đang bận (Thinking / Speaking từ IPC hoặc Timer)
        if tts.is_speaking() || state_manager.get_state_sync() != AssistantState::Idle {
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            continue;
        }

        // 4. Thu âm các đoạn ngắn (tối đa 3.5 giây), có thể bị ngắt ngay bởi --trigger / --stop
        let record_res = recorder
            .record(3.5, Some(&state_manager))
            .map_err(|e| e.to_string());

        match record_res {
            Ok(samples) => {
                // Nếu bị ngắt giữa chừng (do --trigger/--stop) hoặc hệ thống chuyển bận, quay lại đầu vòng lặp
                if state_manager.has_manual_trigger()
                    || state_manager.is_interrupted()
                    || state_manager.get_state_sync() != AssistantState::Idle
                {
                    continue;
                }

                if samples.len() < 3200 {
                    continue;
                }

                let temp_wav = "/tmp/wake_input.wav";
                if AudioRecorder::save_wav(&samples, temp_wav, 16000).is_ok() {
                    let transcript_res = wake_whisper
                        .transcribe_wav(temp_wav)
                        .map_err(|e| e.to_string());
                    if let Ok(transcript) = transcript_res {
                        let trimmed = transcript.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        println!("🎧 [Mic]: \"{}\"", trimmed);

                        if let Some((wake_word, cmd)) = detect_wake_word(trimmed) {
                            if cmd.is_empty() {
                                capture_and_handle_followup(
                                    &format!("Wake Word: {}", wake_word.name()),
                                    &recorder,
                                    &query_whisper,
                                    &app_index,
                                    &model_router,
                                    &ollama,
                                    &config,
                                    &tts,
                                    &state_manager,
                                    model_override.as_deref(),
                                )
                                .await;
                            } else {
                                play_chime(ChimeType::Wake);
                                println!(
                                    "\n🔔 [Wake Word: {}] Câu lệnh tức thì: \"{}\"",
                                    wake_word.name(),
                                    cmd
                                );
                                state_manager
                                    .set_state_full(
                                        AssistantState::Thinking,
                                        Some("Đang xử lý...".to_string()),
                                        Some(cmd.clone()),
                                    )
                                    .await;
                                execute_voice_command(
                                    &cmd,
                                    &app_index,
                                    model_override.as_deref(),
                                    &model_router,
                                    &ollama,
                                    &config,
                                    &tts,
                                    Some(&state_manager),
                                )
                                .await;
                            }

                            // Nghỉ 800ms sau khi trả lời để âm loa tắt vang phòng
                            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Lỗi ghi âm wake word: {}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }
        }
    }
}

async fn execute_voice_command(
    query: &str,
    app_index: &Arc<HashMap<String, DesktopApp>>,
    model_override: Option<&str>,
    model_router: &Arc<voice_ai::ai::ModelRouter>,
    ollama: &Arc<voice_ai::ai::OllamaClient>,
    config: &Config,
    tts: &Arc<PiperTts>,
    state_manager: Option<&StateManager>,
) {
    let start_nlu = Instant::now();
    let q_owned = query.to_string();
    let app_bg = app_index.clone();
    let (handled, reply) =
        tokio::task::spawn_blocking(move || fast_path_nlu(&q_owned, &app_bg, false))
            .await
            .unwrap_or((false, String::new()));
    let nlu_time = start_nlu.elapsed();

    if handled {
        println!(
            "🤖 Trợ lý ({:.2} µs): {}",
            nlu_time.as_secs_f64() * 1_000_000.0,
            reply
        );
        if let Some(mgr) = state_manager {
            mgr.set_state_full(
                AssistantState::Speaking,
                Some(reply.clone()),
                Some(query.to_string()),
            )
            .await;
        }

        if tts.is_available() && !reply.is_empty() {
            let tts_bg = tts.clone();
            let rep_bg = reply.clone();
            let mgr_bg = state_manager.cloned();
            let _ = tokio::task::spawn_blocking(move || {
                tts_bg.speak(&rep_bg, mgr_bg.as_ref());
            })
            .await;
        }

        if let Some(mgr) = state_manager {
            if !mgr.has_manual_trigger() {
                mgr.set_state(AssistantState::Idle, None).await;
            }
        }
    } else {
        execute_llm_query(
            query,
            model_override,
            false,
            model_router,
            ollama,
            config,
            tts,
            state_manager,
            true,
        )
        .await;
    }
}

async fn execute_llm_query(
    prompt: &str,
    model_override: Option<&str>,
    force_deep: bool,
    router: &Arc<voice_ai::ai::ModelRouter>,
    ollama: &Arc<voice_ai::ai::OllamaClient>,
    config: &Config,
    tts: &Arc<PiperTts>,
    state_manager: Option<&StateManager>,
    speak: bool,
) {
    let (target_model, role, options) = if let Some(m) = model_override {
        (
            m.to_string(),
            voice_ai::ai::TaskRole::General,
            voice_ai::ai::ChatOptions::default(),
        )
    } else {
        router
            .route_prompt_with_deep(prompt, force_deep, &config.model)
            .await
    };

    println!(
        "🌐 [Deep-Path LLM]: Điều hướng tự động -> model '{}' [{}]",
        target_model,
        role.label()
    );

    let messages = vec![
        voice_ai::ai::ChatMessage {
            role: "system".to_string(),
            content: voice_ai::ai::ollama::SYSTEM_PROMPT.to_string(),
        },
        voice_ai::ai::ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        },
    ];

    match ollama
        .chat_with_options(Some(&target_model), messages, Some(options), |chunk| {
            print!("{}", chunk);
            let _ = io::stdout().flush();
        })
        .await
        .map_err(|e| e.to_string())
    {
        Ok(raw_reply) => {
            println!();
            let (_thought, mut clean_reply) = voice_ai::ai::filter_thinking_content(&raw_reply);
            if clean_reply.trim().is_empty() {
                clean_reply =
                    "Dạ, tôi chưa tổng hợp kịp câu trả lời. Bạn hỏi lại ngắn gọn hơn giúp tôi nhé."
                        .to_string();
            }

            if let Some(mgr) = state_manager {
                mgr.set_state_full(
                    AssistantState::Speaking,
                    Some(clean_reply.clone()),
                    Some(prompt.to_string()),
                )
                .await;
            }

            if speak && tts.is_available() {
                let tts_bg = tts.clone();
                let rep_bg = clean_reply.clone();
                let mgr_bg = state_manager.cloned();
                let _ = tokio::task::spawn_blocking(move || {
                    tts_bg.speak(&rep_bg, mgr_bg.as_ref());
                })
                .await;
            }

            if let Some(mgr) = state_manager {
                if !mgr.has_manual_trigger() {
                    mgr.set_state(AssistantState::Idle, None).await;
                }
            }
        }
        Err(err_msg) => {
            let msg = format!(
                "Dạ, hiện tại dịch vụ Ollama chưa thể xử lý mô hình '{}': {}",
                target_model, err_msg
            );
            println!("⚠️ {}", msg);
            if speak && tts.is_available() {
                let tts_bg = tts.clone();
                let mgr_bg = state_manager.cloned();
                let _ = tokio::task::spawn_blocking(move || {
                    tts_bg.speak(
                        "Dạ, hiện tại dịch vụ trí tuệ nhân tạo chưa sẵn sàng.",
                        mgr_bg.as_ref(),
                    );
                })
                .await;
            }
            if let Some(mgr) = state_manager {
                if !mgr.has_manual_trigger() {
                    mgr.set_state(AssistantState::Idle, None).await;
                }
            }
        }
    }
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

fn is_exit_phrase(text: &str) -> bool {
    let t = text.to_lowercase();
    let t = t.trim().trim_end_matches(['.', '!', '?']);
    let exits = [
        "tạm biệt",
        "kết thúc",
        "dừng lại",
        "dừng cuộc trò chuyện",
        "tắt trợ lý",
        "đóng trợ lý",
        "thôi im",
        "im lặng",
        "hẹn gặp lại",
        "tạm biệt nhé",
    ];
    exits
        .iter()
        .any(|&e| t == e || t.starts_with(e) || t.ends_with(e))
}

fn detect_internal_mic_card() -> String {
    if let Ok(cards) = std::fs::read_to_string("/proc/asound/cards") {
        for line in cards.lines() {
            let trimmed = line.trim();
            if let Some(first_token) = trimmed.split_whitespace().next() {
                if first_token.chars().all(|c| c.is_ascii_digit()) {
                    let lower = trimmed.to_lowercase();
                    if lower.contains("pch")
                        || lower.contains("alc")
                        || lower.contains("realtek")
                        || lower.contains("sof-hda")
                    {
                        return first_token.to_string();
                    }
                }
            }
        }
    }
    "0".to_string()
}

fn ensure_mic_levels() {
    let card = detect_internal_mic_card();
    let _ = std::process::Command::new("amixer")
        .args(["-c", &card, "sset", "Internal Mic Boost", "1"])
        .output();
    let _ = std::process::Command::new("amixer")
        .args(["-c", &card, "sset", "Capture", "75%"])
        .output();
}
