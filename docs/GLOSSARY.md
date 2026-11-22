# Glossary: Python to Rust Domain Mappings

| Python Domain Term | Python Location | Rust Term / Type | Rust Crate / Module | Description |
| --- | --- | --- | --- | --- |
| `fast_path_nlu` | `ai_assistant.core.nlu` | `fast_path_nlu` / `FastPathNlu` | `nlu` / `crate::core::nlu` | Deterministic intent parser & dispatcher for instant commands (<10µs) |
| `strip_vietnamese_accents` | `ai_assistant.core.nlu` | `strip_vietnamese_accents` | `crate::core::nlu::normalize` | Unicode NFD diacritic stripper for Vietnamese text |
| `assistant_state` | `ai_assistant.core.state` | `AssistantState` | `crate::core::state` | Enum of states: `Idle`, `Listening`, `Thinking`, `Speaking`, `Disabled` |
| `send_ipc_command` | `ai_assistant.ipc.client` | `send_ipc_command` | `crate::ipc::client` | Sends JSON command to Unix domain socket |
| `start_ipc_server` | `ai_assistant.ipc.server` | `start_ipc_server` | `crate::ipc::server` | Async Tokio Unix Domain Socket server managing clients |
| `build_app_index` | `ai_assistant.tools.apps` | `AppIndex` | `crate::tools::apps` | Scans and indexes `.desktop` files in `/usr/share/applications` and `~/.local/share/applications` |
| `set_system_volume` | `ai_assistant.tools.system` | `set_system_volume` | `crate::tools::system` | Controls volume via PipeWire (`wpctl`) or PulseAudio |
| `adjust_brightness` | `ai_assistant.tools.system` | `adjust_brightness` | `crate::tools::system` | Adjusts display backlight via `brightnessctl` |
| `query_ollama_streaming` | `ai_assistant.ai.ollama_client` | `OllamaClient::chat_stream` | `crate::ai::ollama` | Async HTTP streaming client for Ollama LLM API |
| `speak_piper_tts` | `ai_assistant.speech.tts` | `PiperEngine::speak` | `crate::speech::tts` | Synthesizes Vietnamese speech using Piper ONNX model |
| `load_whisper_model` | `ai_assistant.audio.stt` | `WhisperEngine` | `crate::audio::stt` | Transcribes audio buffers to Vietnamese text |
| `openWakeWord` | `ai_assistant.audio.wakeword` | `WakeWordDetector` | `crate::audio::wakeword` | Real-time wake word detection on audio frames |
| `timer_manager` | `ai_assistant.tools.timer` | `TimerManager` | `crate::tools::timer` | Background timer and alarm schedule manager |
