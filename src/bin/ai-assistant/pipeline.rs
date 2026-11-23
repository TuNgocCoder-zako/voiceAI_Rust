use super::context::AppContext;
use std::io::{self, Write};
use std::time::Instant;
use voice_ai::audio::{AudioRecorder, SileroVad};
use voice_ai::core::nlu::fast_path_nlu;
use voice_ai::core::state::AssistantState;
use voice_ai::speech::{play_chime, ChimeType, SentenceStreamSplitter};

pub fn is_exit_phrase(text: &str) -> bool {
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

pub async fn capture_and_handle_followup(
    ctx: &AppContext,
    source_label: &str,
    mut silero_vad: Option<&mut SileroVad>,
) {
    println!("\n🔔 [{}] Đang lắng nghe câu lệnh tiếp theo...", source_label);
    ctx.state_manager
        .set_state_full(
            AssistantState::Listening,
            Some("Đang lắng nghe...".to_string()),
            None,
        )
        .await;

    play_chime(ChimeType::Wake);
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    ctx.recorder.flush();

    if ctx.state_manager.is_interrupted() {
        return;
    }

    let followup_res = if let Some(vad) = silero_vad.as_deref_mut() {
        ctx.recorder
            .record_command_with_silero(vad, 4.5, 10.0, Some(&ctx.state_manager))
            .map_err(|e| e.to_string())
    } else {
        ctx.recorder
            .record(8.0, Some(&ctx.state_manager))
            .map_err(|e| e.to_string())
    };

    match followup_res {
        Ok(followup_samples) => {
            if ctx.state_manager.is_interrupted() {
                return;
            }
            if followup_samples.len() < 3200 {
                println!("⚠️ [VAD]: Không phát hiện giọng nói người dùng (đoạn âm thanh quá ngắn hoặc im lặng).");
                if !ctx.state_manager.has_manual_trigger() {
                    ctx.state_manager.set_state(AssistantState::Idle, None).await;
                }
                return;
            }

            ctx.state_manager
                .set_state_full(
                    AssistantState::Thinking,
                    Some("Đang nhận diện giọng nói...".to_string()),
                    None,
                )
                .await;

            let stt_start = Instant::now();
            let _ = AudioRecorder::save_wav(&followup_samples, "/tmp/last_stt_debug.wav", 16000);
            println!(
                "🎙️ Đã thu {} mẫu ({:.2}s). Bắt đầu nhận diện Whisper...",
                followup_samples.len(),
                followup_samples.len() as f32 / 16000.0
            );

            let stt_res = ctx
                .whisper
                .transcribe_samples(&followup_samples)
                .map_err(|e| e.to_string());

            match stt_res {
                Ok(followup_text) => {
                    let q = followup_text.trim();
                    if !q.is_empty() {
                        println!("👤 Bạn ({:.2?}): \"{}\"", stt_start.elapsed(), q);
                        ctx.state_manager
                            .set_state_full(
                                AssistantState::Thinking,
                                Some("Đang xử lý...".to_string()),
                                Some(q.to_string()),
                            )
                            .await;
                        execute_voice_command(ctx, q, true).await;
                        return;
                    } else {
                        println!(
                            "⚠️ [Whisper STT ({:.2?})]: Không nhận diện được chữ nào (kết quả rỗng hoặc lọc nhiễu).",
                            stt_start.elapsed()
                        );
                        play_chime(ChimeType::Error);
                    }
                }
                Err(e) => {
                    eprintln!("❌ [Whisper STT]: Lỗi nhận diện giọng nói: {}", e);
                    play_chime(ChimeType::Error);
                }
            }
            if !ctx.state_manager.has_manual_trigger() {
                ctx.state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
        Err(err) => {
            eprintln!("❌ Lỗi thu âm tiếp nối: {}", err);
            if !ctx.state_manager.has_manual_trigger() {
                ctx.state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
    }
}

pub async fn execute_voice_command(
    ctx: &AppContext,
    query: &str,
    update_ui: bool,
) {
    let start_nlu = Instant::now();
    let q_owned = query.to_string();
    let app_bg = ctx.app_index.clone();
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
        if update_ui {
            ctx.state_manager
                .set_state_full(
                    AssistantState::Speaking,
                    Some(reply.clone()),
                    Some(query.to_string()),
                )
                .await;
        }

        if ctx.tts.is_available() && !reply.is_empty() {
            let tts_bg = ctx.tts.clone();
            let rep_bg = reply.clone();
            let mgr_bg = if update_ui {
                Some(ctx.state_manager.clone())
            } else {
                None
            };
            let _ = tokio::task::spawn_blocking(move || {
                tts_bg.speak(&rep_bg, mgr_bg.as_ref());
            })
            .await;
        }

        if update_ui {
            if !ctx.state_manager.has_manual_trigger() {
                ctx.state_manager.set_state(AssistantState::Idle, None).await;
            }
        }
    } else {
        execute_llm_query(ctx, query, false, update_ui, true).await;
    }
}

pub async fn execute_llm_query(
    ctx: &AppContext,
    prompt: &str,
    force_deep: bool,
    update_ui: bool,
    speak: bool,
) {
    let (target_model, role, options) = if let Some(ref m) = ctx.model_override {
        (
            m.to_string(),
            voice_ai::ai::TaskRole::General,
            voice_ai::ai::ChatOptions::default(),
        )
    } else {
        ctx.model_router
            .route_prompt_with_deep(prompt, force_deep, &ctx.config.model)
            .await
    };

    println!(
        "🌐 [Deep-Path LLM]: Điều hướng tự động -> model '{}' [{}]",
        target_model,
        role.label()
    );

    let messages = {
        let mut hist = ctx.history.lock().unwrap();
        hist.build_messages(voice_ai::ai::ollama::SYSTEM_PROMPT, prompt)
    };

    let mut tts_tx_opt: Option<std::sync::mpsc::Sender<String>> = None;
    let mut tts_handle: Option<tokio::task::JoinHandle<bool>> = None;

    if speak && ctx.tts.is_available() {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let tts_bg = ctx.tts.clone();
        let mgr_bg = if update_ui {
            Some(ctx.state_manager.clone())
        } else {
            None
        };
        let handle = tokio::task::spawn_blocking(move || tts_bg.speak_stream(rx, mgr_bg.as_ref()));
        tts_tx_opt = Some(tx);
        tts_handle = Some(handle);
    }

    // Kênh cập nhật UI tuần tự để đảm bảo không bao giờ ghi đè Speaking sau khi đã về Idle
    let (ui_tx, mut ui_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let ui_handle = if update_ui {
        let mgr_ui = ctx.state_manager.clone();
        let q_ui = prompt.to_string();
        Some(tokio::spawn(async move {
            while let Some(text_snap) = ui_rx.recv().await {
                if mgr_ui.is_interrupted() {
                    break;
                }
                mgr_ui
                    .set_state_full(AssistantState::Speaking, Some(text_snap), Some(q_ui.clone()))
                    .await;
            }
        }))
    } else {
        None
    };

    let mut splitter = SentenceStreamSplitter::new();
    let mut visible_acc = String::new();
    let was_cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let was_cancelled_cb = was_cancelled.clone();

    let stream_res = ctx
        .ollama
        .chat_with_cancel(
            Some(&target_model),
            messages,
            Some(options),
            |chunk| {
                if update_ui && ctx.state_manager.is_interrupted() {
                    was_cancelled_cb.store(true, std::sync::atomic::Ordering::SeqCst);
                    tts_tx_opt = None;
                    return;
                }

                print!("{}", chunk);
                let _ = io::stdout().flush();

                let ready_sentences = splitter.push(chunk);
                for sent in ready_sentences {
                    if !visible_acc.is_empty() {
                        visible_acc.push(' ');
                    }
                    visible_acc.push_str(&sent);

                    if let Some(ref tx) = tts_tx_opt {
                        let _ = tx.send(sent);
                    }

                    if update_ui {
                        let _ = ui_tx.send(visible_acc.clone());
                    }
                }
            },
            {
                let was_cancelled_poll = was_cancelled.clone();
                let mgr = ctx.state_manager.clone();
                move || {
                    if update_ui && mgr.is_interrupted() {
                        was_cancelled_poll.store(true, std::sync::atomic::Ordering::SeqCst);
                        true
                    } else {
                        false
                    }
                }
            },
        )
        .await
        .map_err(|e| e.to_string());

    drop(ui_tx);
    if let Some(h) = ui_handle {
        let _ = h.await;
    }

    for last_sent in splitter.flush() {
        if !visible_acc.is_empty() {
            visible_acc.push(' ');
        }
        visible_acc.push_str(&last_sent);
        if let Some(ref tx) = tts_tx_opt {
            let _ = tx.send(last_sent);
        }
    }
    drop(tts_tx_opt);

    match stream_res {
        Ok(raw_reply) => {
            println!();
            // Nếu đã bị ngắt bởi --stop / --relisten: chờ TTS dứt rồi thoát sạch, không ghi đè UI hay lịch sử
            if was_cancelled.load(std::sync::atomic::Ordering::SeqCst)
                || (update_ui && ctx.state_manager.is_interrupted())
            {
                if let Some(handle) = tts_handle {
                    let _ = handle.await;
                }
                return;
            }

            let (_thought, mut clean_reply) = voice_ai::ai::filter_thinking_content(&raw_reply);
            if clean_reply.trim().is_empty() {
                clean_reply =
                    "Dạ, tôi chưa tổng hợp kịp câu trả lời. Bạn hỏi lại ngắn gọn hơn giúp tôi nhé."
                        .to_string();
            }

            {
                let mut hist = ctx.history.lock().unwrap();
                hist.push_turn(prompt, &clean_reply);
            }

            if update_ui {
                ctx.state_manager
                    .set_state_full(
                        AssistantState::Speaking,
                        Some(clean_reply.clone()),
                        Some(prompt.to_string()),
                    )
                    .await;
            }

            if let Some(handle) = tts_handle {
                let _ = handle.await;
            }

            if update_ui {
                if !ctx.state_manager.has_manual_trigger() {
                    ctx.state_manager.set_state(AssistantState::Idle, None).await;
                }
            }
        }
        Err(err_msg) => {
            if let Some(handle) = tts_handle {
                let _ = handle.await;
            }
            let msg = format!(
                "Dạ, hiện tại dịch vụ Ollama chưa thể xử lý mô hình '{}': {}",
                target_model, err_msg
            );
            println!("⚠️ {}", msg);
            if speak && ctx.tts.is_available() {
                let tts_bg = ctx.tts.clone();
                let mgr_bg = if update_ui {
                    Some(ctx.state_manager.clone())
                } else {
                    None
                };
                let _ = tokio::task::spawn_blocking(move || {
                    tts_bg.speak(
                        "Dạ, hiện tại dịch vụ trí tuệ nhân tạo chưa sẵn sàng.",
                        mgr_bg.as_ref(),
                    );
                })
                .await;
            }
            if update_ui {
                if !ctx.state_manager.has_manual_trigger() {
                    ctx.state_manager.set_state(AssistantState::Idle, None).await;
                }
            }
        }
    }
}
