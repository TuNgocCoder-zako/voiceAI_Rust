# Architectural Decision Records (ADRs)

## ADR-001: Standalone Pure Rust Binary Architecture
- **Status**: accepted (2026-11-20)
- **Context**: The existing Python codebase suffers from GIL contention, garbage collection pauses, and interpreter latency in audio streaming, VAD, and NLU matching. The user requested maximum speed optimization for the voice assistant.
- **Decision**: Port the application into a standalone, native Rust binary (`voice-ai` / `ai-assistant`). The daemon, Unix Domain Socket IPC server, Fast-Path NLU, state machine, and audio processing pipeline will run natively in Rust with async Tokio. Python is retained strictly for offline model training scripts.
- **Consequences**: Zero GIL, sub-microsecond intent resolution, minimal memory footprint, and reliable real-time audio processing without buffer dropouts.

## ADR-002: Modular Desktop and OS Control
- **Status**: accepted (2026-11-20)
- **Context**: The original codebase was tightly coupled to Arch Linux and Hyprland (`hyprctl`). The current runtime host is Ubuntu 26.04 LTS running GNOME on Wayland with PipeWire (`wpctl`).
- **Decision**: Implement an OS abstraction layer (`system` / `desktop_manager`) that detects the current desktop environment (`XDG_CURRENT_DESKTOP`, `HYPRLAND_INSTANCE_SIGNATURE`). It defaults to generic Linux standards (PipeWire via `wpctl`, D-Bus, `brightnessctl`, `nmcli`) while transparently using `hyprctl` when running inside Hyprland.
- **Consequences**: Runs seamlessly on both Ubuntu GNOME and Arch Linux / Hyprland out of the box without requiring manual reconfiguration.

## ADR-003: Zero-Copy and Low-Latency AI & Audio Backends
- **Status**: accepted (2026-11-20)
- **Context**: In Python, Piper TTS required spawning subprocesses or saving temporary `.wav` files to disk, and openWakeWord ran through Python ONNX wrappers with NumPy array copying.
- **Decision**:
  - **Audio Capture & Playback**: Native Rust audio using `cpal` / `rodio` with lock-free ring buffers.
  - **Wake Word & VAD**: ONNX Runtime via `ort` crate for openWakeWord and Silero VAD.
  - **STT**: `whisper-rs` (native Whisper.cpp) or ONNX / CT2 bindings for Vietnamese speech-to-text.
  - **TTS**: Piper neural TTS running ONNX inference in-memory and streaming audio directly to the output stream without intermediate disk I/O.
  - **LLM**: Async HTTP streaming via `reqwest` to Ollama.
- **Consequences**: Eliminates disk I/O bottlenecks and eliminates audio buffer copying overhead.

## ADR-004: IPC Socket Protocol Compatibility
- **Status**: accepted (2026-11-20)
- **Context**: The project uses an IPC Unix Domain Socket (`/tmp/voice_ai_state.sock` or `~/.cache/voice_ai_state.sock`) to communicate with the Quickshell Dynamic Island overlay (`overlay/VoiceOverlay.qml`) and CLI hotkeys.
- **Decision**: Retain exact wire-format compatibility for the Unix Domain Socket (JSON command/event stream: `{"command": ...}`, `{"state": ...}`).
- **Consequences**: The existing Quickshell overlay, Hyprland/GNOME hotkeys, and CLI flags (`--trigger`, `--stop`, `--relisten`, `--push-to-talk`) work without modifying external UI scripts.

## ADR-005: Sub-Microsecond Fast-Path NLU
- **Status**: accepted (2026-11-20)
- **Context**: Fast-Path NLU is responsible for immediate local actions (volume, brightness, wifi, time, date, opening apps, closing windows) without invoking the heavy LLM. In Python it ran in ~0.02ms - 4ms.
- **Decision**: Implement Fast-Path NLU in Rust using compiled `regex`, Aho-Corasick automaton, Vietnamese diacritics normalization, and exact prefix/suffix matchers.
- **Consequences**: Reduces NLU latency from milliseconds to microseconds (< 10µs), delivering near-instant response for daily assistant commands.
