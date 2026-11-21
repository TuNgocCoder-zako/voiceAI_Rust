# Voice AI - Native Rust Engine 🦀⚡

Dự án trợ lý ảo AI cục bộ siêu tốc độ cao (sub-microsecond Fast-Path NLU, zero-copy audio pipeline, native Whisper STT, Piper TTS và Ollama Docker).

## 1. Kiến trúc & Tính năng đã nâng cấp sang Rust
- **Fast-Path NLU (Regex Trie & Dynamic Levenshtein Fuzzy Router)**:
  - Tốc độ xử lý: **~18,000 queries/giây**.
  - Độ trễ trung vị (Median latency): **~17 µs** (nhanh hơn Python x20 - x50 lần).
  - Tự động chuẩn hóa dấu tiếng Việt, sửa lỗi phiên âm Whisper và chống lộ dữ liệu nhạy cảm.
- **Smart Multi-Model Auto-Discovery & Dynamic Router**:
  - Tự động nhận diện toàn bộ mô hình AI có sẵn trong Ollama qua API `/api/tags` (cache 30s).
  - Tự động phân tích ngữ nghĩa câu hỏi và điều hướng tới **mô hình tốt nhất** trên máy:
    - ⚡ **Hội thoại nhanh / Xã giao (Fast)**: Điều hướng tới `qwen3.8:2b` (hoặc các model 2B-4B) để phản hồi siêu tốc.
    - 💻 **Lập trình & Kỹ thuật (Coder)**: Điều hướng tới `qwen2.5:latest` (Qwen2.5-Coder-7B).
    - 🧠 **Suy luận sâu / Toán / Logic (Reasoning)**: Điều hướng tới `qwen3:4b-thinking` (kèm bộ lọc bóc tách suy nghĩ nội tâm `<think>` để giọng nói TTS tự nhiên).
- **Audio & Speech Pipeline**:
  - **VAD (Voice Activity Detection)**: Schmitt-trigger lọc nhiễu âm bản địa.
  - **Capture**: PipeWire native recording (`pw-record`) với 600ms pre-roll circular buffer và AGC.
  - **STT**: `whisper.cpp` C++ native runner (`ggml-base.bin` & `ggml-small.bin`).
  - **TTS**: Standalone C++ Piper (`vits-piper`) stream trực tiếp vào `pw-play` với tính năng Barge-In ngắt lập tức khi người dùng nói đè.
- **IPC Architecture**:
  - Unix Domain Socket Server tại `/tmp/voice_ai_state.sock` hỗ trợ giao tiếp với overlay UI, phím tắt hoặc CLI.
- **LLM Engine (Deep-Path)**:
  - Chạy Ollama qua Docker container (`localhost:11434`), tự động kết nối mô hình Qwen cục bộ không cần tải lại từ internet.

---

## 2. Cách chạy & Lệnh sử dụng

Đã tạo sẵn alias và binary tại `~/.local/bin/ai-assistant-rs`.

### Chạy truy vấn văn bản trực tiếp
```bash
# Fast-Path (xử lý ngay lập tức trong microsecond)
ai-assistant-rs --query "mấy giờ rồi"
ai-assistant-rs --query "pin còn bao nhiêu"
ai-assistant-rs --query "tăng âm lượng lên 70"

# Deep-Path (truy vấn Qwen qua Ollama Docker)
ai-assistant-rs --query "thủ đô của Việt Nam là gì"

# Đọc to kết quả bằng giọng nói (Piper TTS)
ai-assistant-rs --query "thời tiết hôm nay thế nào" --voice
```

### Chạy chế độ Push-to-Talk (Giữ phím để nói)
```bash
ai-assistant-rs --push-to-talk
```

### Chạy chế độ Đánh thức bằng giọng nói rảnh tay (Hands-Free Wake Word)
```bash
# Lắng nghe liên tục từ khóa 'Alexa', 'Trợ lý ơi', 'Hey Jarvis' (chỉ tốn ~15MB RAM)
ai-assistant-rs --wake-word

# Hoặc khởi chạy Daemon nền (vừa nhận lệnh IPC vừa túc trực Wake Word):
ai-assistant-rs
```

### Chạy chế độ Lắng nghe liên tục (Always-on VAD)
```bash
ai-assistant-rs --listen
```

### Chạy IPC Server làm nền (Daemon)
```bash
ai-assistant-rs
```

### Chạy Benchmark kiểm tra tốc độ NLU
```bash
ai-assistant-rs --benchmark
```

---

## 3. Dịch vụ Chạy Ngầm Tự Động (Systemd Background Service)

Trợ lý đã được cấu hình chạy ngầm vĩnh viễn dưới dạng dịch vụ `systemd --user` (chỉ tiêu thụ **~9.6 MB RAM** và tự động khởi động cùng hệ điều hành):

```bash
# Xem trạng thái trợ lý đang chạy ngầm
systemctl --user status voice-ai

# Xem nhật ký logs thời gian thực
journalctl --user -u voice-ai -f

# Tạm dừng / Khởi động lại dịch vụ
systemctl --user stop voice-ai
systemctl --user restart voice-ai
```

---

## 4. Quản lý Ollama Docker Backend

Container Ollama đã được thiết lập với cờ auto-restart:
```bash
# Kiểm tra trạng thái Ollama
docker ps -f name=ollama

# Kiểm tra các model đang có
curl http://localhost:11434/api/tags

# Khởi động lại container nếu cần
docker restart ollama
```
