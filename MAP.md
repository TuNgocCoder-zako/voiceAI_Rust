# Rust Migration Map

## Goal
Port the Voice AI Assistant (`voice-ai`) from Python to a high-performance, standalone Rust application (`voice-ai`), achieving sub-microsecond intent resolution, zero GIL audio streaming, and seamless compatibility with both GNOME Wayland and Hyprland.

## ADR Index
- [ADR-001: Standalone Pure Rust Binary Architecture](DECISIONS.md#adr-001-standalone-pure-rust-binary-architecture)
- [ADR-002: Modular Desktop and OS Control](DECISIONS.md#adr-002-modular-desktop-and-os-control)
- [ADR-003: Zero-Copy and Low-Latency AI & Audio Backends](DECISIONS.md#adr-003-zero-copy-and-low-latency-ai--audio-backends)
- [ADR-004: IPC Socket Protocol Compatibility](DECISIONS.md#adr-004-ipc-socket-protocol-compatibility)
- [ADR-005: Sub-Microsecond Fast-Path NLU](DECISIONS.md#adr-005-sub-microsecond-fast-path-nlu)

## Migration Units & Status

| Unit | Scope | Status | Notes |
| --- | --- | --- | --- |
| **Unit 1: Fast-Path NLU & Normalization** | Diacritics stripping, regex intent matcher, app matcher, system command matcher | `done` (2026-11-20) | Throughput: **17,613 cmds/s**, Median: **17.4 µs**, P90: **197 µs** (20x faster than Python) |
| **Unit 2: IPC & State Daemon** | Unix Domain Socket server & client, JSON protocol, state broadcaster | `done` (2026-11-20) | Compatible with Quickshell & Hyprland hotkeys (`/tmp/voice_ai_state.sock`) |
| **Unit 3: System & Desktop Tools** | Volume (wpctl), brightness, battery sysfs, apps scanner (.desktop), git, docker, timer | `done` (2026-11-20) | Direct kernel `/sys` and `/proc` reads with zero process overhead |
| **Unit 4: AI LLM Client & Router** | Ollama async streaming client, chat history, model router | `done` (2026-11-20) | Native async HTTP SSE with `reqwest` + `tokio` |
| **Unit 5: Speech & TTS** | Piper TTS in-memory ONNX synthesis & streaming player | `done` (2026-11-20) | C++ Piper standalone streaming raw PCM directly into PipeWire (`pw-play`), real-time factor 0.048 |
| **Unit 6: Audio Capture, VAD & STT** | Microphone capture (PipeWire), Schmitt trigger VAD, Whisper STT | `done` (2026-11-20) | Native Whisper STT (`whisper-cli` with `ggml-base.bin` & `ggml-small.bin`), 600ms pre-roll ring buffer |
| **Unit 7: Binary Entry Point & CLI** | CLI argument parser (clap), daemon orchestration, voice mode, benchmark | `done` (2026-11-20) | Built release binary at `rust/target/release/ai-assistant`, linked to `~/.local/bin/ai-assistant-rs` |

## Out of Scope
- Python training scripts (`training/`): Retained in Python as offline tools for training custom models.
