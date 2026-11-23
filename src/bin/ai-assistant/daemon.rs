use super::context::AppContext;
use std::sync::Arc;
use std::time::Instant;
use voice_ai::audio::{
    detect_wake_word, AudioRecorder, SileroVad, UnifiedWakeWordEngine, WhisperEngine,
};
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::core::nlu::router::TIMER_MANAGER;
use voice_ai::core::state::AssistantState;
use voice_ai::ipc::server::{ActionRequest, IpcServer};
use voice_ai::speech::{play_chime, ChimeType};
use voice_ai::tools::system::is_media_playing;

pub async fn run_daemon(ctx: AppContext) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌟 Khởi chạy Voice AI Assistant Daemon (Rust Core v3.0.0)...");
    crate::platform::ensure_mic_levels();
    if let Err(e) = ctx.whisper.preload() {
        eprintln!("⚠️ Chưa nạp sẵn được Whisper RAM: {}", e);
    }
    println!("🎧 Đang túc trực lắng nghe từ khóa: 'Alexa', 'Trợ lý ơi', 'Hey Jarvis'...");

    let (action_tx, mut action_rx) = tokio::sync::mpsc::channel::<ActionRequest>(64);

    // Kích hoạt luồng đếm ngược Hẹn giờ / Báo thức chạy nền
    TIMER_MANAGER.spawn_watcher(ctx.tts.clone(), ctx.state_manager.clone());

    let server = IpcServer::new(&ctx.config.socket_path, ctx.state_manager.clone(), action_tx);

    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Lỗi IPC Server: {}", e);
        }
    });

    // Giám sát và tự động duy trì giao diện Dynamic Island Overlay luôn chạy
    tokio::spawn(async {
        loop {
            let _ = tokio::task::spawn_blocking(crate::platform::ensure_overlay_running).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    let ww_ctx = ctx.clone();
    let wake_whisper = Arc::new(WhisperEngine::fast_wake_engine());

    std::thread::Builder::new()
        .name("wake-word-worker".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(run_wake_word_loop(ww_ctx, wake_whisper));
        })
        .expect("Failed to spawn wake-word-worker thread");

    println!("⚡ Trợ lý đã sẵn sàng nhận lệnh qua Giọng nói (Wake Word) hoặc IPC Socket.");
    ctx.state_manager.set_state(AssistantState::Idle, None).await;

    while let Some(action) = action_rx.recv().await {
        match action {
            ActionRequest::Trigger => {
                println!("🔔 Nhận sự kiện Trigger: Đang kích hoạt micro lắng nghe...");
            }
            ActionRequest::Stop => {
                println!("⏹️ Nhận sự kiện Stop: Đã dừng.");
                ctx.state_manager.stop_all();
                ctx.state_manager.set_state(AssistantState::Idle, None).await;
            }
            ActionRequest::Relisten => {
                println!("🔄 Nhận sự kiện Relisten: Dừng tác vụ cũ và mở mic nghe lượt mới...");
            }
            ActionRequest::Speak(text) => {
                if ctx.tts.is_speaking() {
                    ctx.state_manager.stop_all();
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                }
                ctx.state_manager.reset_interrupt();

                let act_ctx = ctx.clone();
                tokio::spawn(async move {
                    act_ctx
                        .state_manager
                        .set_state_full(AssistantState::Speaking, Some(text.clone()), None)
                        .await;
                    if act_ctx.tts.is_available() {
                        let tts_bg = act_ctx.tts.clone();
                        let mgr_bg = act_ctx.state_manager.clone();
                        let txt_bg = text.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            tts_bg.speak(&txt_bg, Some(&mgr_bg));
                        })
                        .await;
                    }
                    if !act_ctx.state_manager.has_manual_trigger() {
                        act_ctx.state_manager.set_state(AssistantState::Idle, None).await;
                    }
                });
            }
            ActionRequest::Query { text, deep } => {
                if ctx.tts.is_speaking() || ctx.state_manager.get_state_sync() != AssistantState::Idle {
                    ctx.state_manager.stop_all();
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                }
                ctx.state_manager.reset_interrupt();

                let act_ctx = ctx.clone();
                tokio::spawn(async move {
                    act_ctx
                        .state_manager
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
                        let app_bg = act_ctx.app_index.clone();
                        tokio::task::spawn_blocking(move || fast_path_nlu(&q_bg, &app_bg, false))
                            .await
                            .unwrap_or((false, String::new()))
                    };

                    if handled {
                        println!("🤖 [Fast-Path]: {}", rep);
                        act_ctx
                            .state_manager
                            .set_state_full(
                                AssistantState::Speaking,
                                Some(rep.clone()),
                                Some(text.clone()),
                            )
                            .await;

                        if act_ctx.tts.is_available() && !rep.is_empty() {
                            let tts_bg = act_ctx.tts.clone();
                            let mgr_bg = act_ctx.state_manager.clone();
                            let rep_bg = rep.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                tts_bg.speak(&rep_bg, Some(&mgr_bg));
                            })
                            .await;
                        }
                        if !act_ctx.state_manager.has_manual_trigger() {
                            act_ctx.state_manager.set_state(AssistantState::Idle, None).await;
                        }
                    } else {
                        crate::pipeline::execute_llm_query(
                            &act_ctx,
                            &text,
                            deep,
                            true,
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

pub async fn run_wake_word_loop(
    ctx: AppContext,
    wake_whisper: Arc<WhisperEngine>,
) {
    let mut silero_vad = SileroVad::load_default();
    let mut wake_engine = UnifiedWakeWordEngine::load_default();

    if let Some(engine) = wake_engine.as_ref() {
        println!(
            "🎧 [Hands-Free]: {} + Silero VAD đã sẵn sàng ('Alexa', 'Hey Jarvis').",
            engine.backend_name()
        );
    } else {
        println!("⚠️ [Fallback]: Chưa nạp được Wake Word Engine, tạm dùng Whisper Wake Word.");
    }

    let mut last_music_check = Instant::now() - std::time::Duration::from_secs(5);
    let mut cached_music_playing = false;

    loop {
        // 1. Ưu tiên xử lý kích hoạt thủ công (--trigger / --relisten)
        if ctx.state_manager.has_manual_trigger() {
            for _ in 0..20 {
                if !ctx.tts.is_speaking() {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            }
            ctx.state_manager.take_manual_trigger();
            ctx.state_manager.reset_interrupt();

            crate::pipeline::capture_and_handle_followup(
                &ctx,
                "Manual Trigger",
                silero_vad.as_mut(),
                false,
            )
            .await;

            if let Some(eng) = wake_engine.as_mut() {
                eng.reset();
            }
            ctx.recorder.flush();
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
            continue;
        }

        // 2. Xử lý tín hiệu --stop
        if ctx.state_manager.is_interrupted() {
            if ctx.tts.is_speaking() {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            if !ctx.state_manager.has_manual_trigger() {
                ctx.state_manager.reset_interrupt();
            }
            if let Some(eng) = wake_engine.as_mut() {
                eng.reset();
            }
            ctx.recorder.flush();
            continue;
        }

        // 3. Tạm nghỉ khi hệ thống đang bận xử lý / phát TTS
        if ctx.tts.is_speaking() || ctx.state_manager.get_state_sync() != AssistantState::Idle {
            tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
            if let Some(eng) = wake_engine.as_mut() {
                eng.reset();
            }
            ctx.recorder.flush();
            continue;
        }

        // 4A. NHÁNH CHÍNH: UnifiedWakeWordEngine (Porcupine 32ms hoặc openWakeWord ONNX)
        if let Some(engine) = wake_engine.as_mut() {
            if last_music_check.elapsed() >= std::time::Duration::from_secs(2) {
                cached_music_playing = is_media_playing();
                last_music_check = Instant::now();
            }

            let mut frame_512 = [0.0f32; SileroVad::CHUNK_SAMPLES];
            match ctx.recorder.read_frame_f32(&mut frame_512, Some(&ctx.state_manager)) {
                Ok(true) => {
                    let vad_prob = silero_vad
                        .as_mut()
                        .and_then(|v| v.process_chunk(&frame_512).ok())
                        .unwrap_or(1.0);

                    match engine.process_frame_512(&frame_512, vad_prob, cached_music_playing) {
                        Ok(res) => {
                            if let Some(hit) = res.detected {
                                println!(
                                    "\n✨ [{}: {}] Độ khớp: {:.2} (Ngưỡng: {:.2}{})",
                                    hit.backend,
                                    hit.wake_word.name(),
                                    hit.score,
                                    res.active_threshold,
                                    if cached_music_playing { " 🎵" } else { "" }
                                );

                                crate::pipeline::capture_and_handle_followup(
                                    &ctx,
                                    &format!("{}: {}", hit.backend, hit.wake_word.name()),
                                    silero_vad.as_mut(),
                                    true, // Cho phép nói liền 1 hơi!
                                )
                                .await;

                                if !ctx.state_manager.has_manual_trigger() && !ctx.tts.is_speaking() {
                                    ctx.state_manager.set_state(AssistantState::Idle, None).await;
                                }

                                engine.reset();
                                if let Some(v) = silero_vad.as_mut() {
                                    v.reset();
                                }
                                ctx.recorder.flush();
                                tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
                            }
                        }
                        Err(e) => eprintln!("⚠️ Lỗi Wake Word ({}): {}", engine.backend_name(), e),
                    }
                }
                Ok(false) => continue,
                Err(e) => {
                    eprintln!("Lỗi đọc luồng audio cpal: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                }
            }
            continue;
        }

        // 4B. NHÁNH DỰ PHÒNG (khi máy chưa tải tệp .onnx): dùng lại vòng lặp Whisper cũ
        let record_res = ctx
            .recorder
            .record(3.5, Some(&ctx.state_manager))
            .map_err(|e| e.to_string());
        if let Ok(samples) = record_res {
            if ctx.state_manager.has_manual_trigger()
                || ctx.state_manager.is_interrupted()
                || ctx.state_manager.get_state_sync() != AssistantState::Idle
                || samples.len() < 3200
            {
                continue;
            }
            let temp_wav = "/tmp/wake_input.wav";
            if AudioRecorder::save_wav(&samples, temp_wav, 16000).is_ok() {
                if let Ok(transcript) = wake_whisper.transcribe_wav(temp_wav) {
                    let trimmed = transcript.trim();
                    if !trimmed.is_empty() {
                        println!("🎧 [Mic]: \"{}\"", trimmed);
                        if let Some((wake_word, cmd)) = detect_wake_word(trimmed) {
                            if cmd.is_empty() {
                                crate::pipeline::capture_and_handle_followup(
                                    &ctx,
                                    &format!("Wake Word: {}", wake_word.name()),
                                    silero_vad.as_mut(),
                                    false,
                                )
                                .await;

                                if !ctx.state_manager.has_manual_trigger() && !ctx.tts.is_speaking() {
                                    ctx.state_manager.set_state(AssistantState::Idle, None).await;
                                }
                            } else {
                                play_chime(ChimeType::Wake);
                                ctx.state_manager
                                    .set_state_full(
                                        AssistantState::Thinking,
                                        Some("Đang xử lý...".to_string()),
                                        Some(cmd.clone()),
                                    )
                                    .await;
                                crate::pipeline::execute_voice_command(
                                    &ctx,
                                    &cmd,
                                    true,
                                )
                                .await;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                        }
                    }
                }
            }
        }
    }
}
