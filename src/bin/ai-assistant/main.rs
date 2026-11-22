mod context;
mod daemon;
mod diagnostics;
mod pipeline;
mod platform;

use clap::Parser;
use context::{AppContext, Args};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;
use voice_ai::audio::WhisperEngine;
use voice_ai::config::Config;
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::core::nlu::router::TIMER_MANAGER;
use voice_ai::ipc::client::{send_ipc_command, send_ipc_command_with_options};
use voice_ai::speech::{play_chime, ChimeType, PiperTts};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config = Config::default();

    // 1. Nhóm lệnh điều khiển IPC nhanh (không cần nạp AppIndex / Whisper)
    if args.status {
        match send_ipc_command(&config.socket_path, "status", None).await {
            Ok(resp) => println!(
                "✅ Trợ lý AI đang chạy nền! Trạng thái: {:?}",
                resp.state.unwrap_or_default()
            ),
            Err(e) => println!("⚠️ Không kết nối được tới trợ lý đang chạy nền: {}", e),
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

    // 2. Nhóm công cụ chẩn đoán & đo hiệu năng
    if args.benchmark {
        diagnostics::run_nlu_benchmark(200);
        return Ok(());
    }
    if args.debug_audio {
        diagnostics::run_audio_debugger()?;
        return Ok(());
    }

    // 3. Đọc văn bản trực tiếp (--speak)
    if let Some(text) = args.speak {
        if config.socket_path.exists()
            && send_ipc_command(&config.socket_path, "speak", Some(text.clone()))
                .await
                .is_ok()
        {
            return Ok(());
        }
        let tts = Arc::new(PiperTts::new());
        if tts.is_available() {
            let _ = tokio::task::spawn_blocking(move || tts.speak(&text, None)).await;
        }
        return Ok(());
    }

    // 4. Truy vấn đơn lẻ (--query): ưu tiên chuyển sang daemon đang chạy
    if let Some(q) = args.query {
        if config.socket_path.exists()
            && send_ipc_command_with_options(
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

        let ctx = AppContext::new(config, args.model);
        let (handled, reply) = if args.deep {
            (false, String::new())
        } else {
            fast_path_nlu(&q, &ctx.app_index, false)
        };

        if handled {
            println!("🤖 [Fast-Path]: {}", reply);
            if args.voice && ctx.tts.is_available() {
                let tts_c = ctx.tts.clone();
                let rep_c = reply.clone();
                let _ = tokio::task::spawn_blocking(move || tts_c.speak(&rep_c, None)).await;
            }
        } else {
            pipeline::execute_llm_query(&ctx, &q, args.deep, false, args.voice).await;
        }
        return Ok(());
    }

    let ctx = AppContext::new(config, args.model);

    // 5. Chế độ Wake Word độc lập (--wake-word)
    if args.wake_word {
        let _ = ctx.whisper.preload();
        let wake_whisper = Arc::new(WhisperEngine::fast_wake_engine());
        TIMER_MANAGER.spawn_watcher(ctx.tts.clone(), ctx.state_manager.clone());
        platform::ensure_overlay_running();
        daemon::run_wake_word_loop(ctx, wake_whisper).await;
        return Ok(());
    }

    // 6. Chế độ Always-on VAD (--listen)
    if args.listen {
        let _ = ctx.whisper.preload();
        println!("🎙️ Chế độ đàm thoại liên tục (Always-on VAD) - Nói 'tạm biệt' hoặc Ctrl+C để thoát.");
        loop {
            play_chime(ChimeType::Wake);
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            ctx.recorder.flush();
            println!("\n🔴 Đang nghe...");
            match ctx.recorder.record(8.0, None) {
                Ok(samples) if samples.len() >= 3200 => {
                    let t0 = Instant::now();
                    if let Ok(text) = ctx.whisper.transcribe_samples(&samples) {
                        let trimmed = text.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        println!("👤 Bạn ({:.2?}): \"{}\"", t0.elapsed(), trimmed);
                        if pipeline::is_exit_phrase(trimmed) {
                            let bye = "Tạm biệt bạn nhé! Hẹn gặp lại bạn sau.";
                            println!("🤖 Trợ lý: {}", bye);
                            if ctx.tts.is_available() {
                                let tts_c = ctx.tts.clone();
                                let _ = tokio::task::spawn_blocking(move || tts_c.speak(bye, None))
                                    .await;
                            }
                            break;
                        }
                        pipeline::execute_voice_command(&ctx, trimmed, false).await;
                    }
                }
                Ok(_) => continue,
                Err(e) => {
                    eprintln!("Lỗi thu âm: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
        return Ok(());
    }

    // 7. Chế độ Push-to-Talk văn bản (--push-to-talk)
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
                break;
            }
            if trimmed.is_empty() {
                continue;
            }
            let start = Instant::now();
            let (handled, reply) = fast_path_nlu(trimmed, &ctx.app_index, false);
            if handled {
                println!(
                    "🤖 Trợ lý ({:.2} µs): {}",
                    start.elapsed().as_secs_f64() * 1_000_000.0,
                    reply
                );
                if args.voice && ctx.tts.is_available() {
                    let tts_c = ctx.tts.clone();
                    let rep_c = reply.clone();
                    let _ = tokio::task::spawn_blocking(move || tts_c.speak(&rep_c, None)).await;
                }
            } else {
                pipeline::execute_llm_query(&ctx, trimmed, args.deep, false, args.voice).await;
            }
        }
        return Ok(());
    }

    // 8. Chế độ mặc định: Full Daemon + IPC + Wake Word + Timer + Overlay Supervisor
    daemon::run_daemon(ctx).await
}
