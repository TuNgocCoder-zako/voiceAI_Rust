use crate::core::state::StateManager;
use crate::speech::sanitize::sanitize_text_for_voice;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

pub struct PiperTts {
    model_path: PathBuf,
    espeak_data: PathBuf,
    piper_bin: PathBuf,
    is_speaking: Arc<AtomicBool>,
}

impl PiperTts {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let base_dir = std::env::current_dir().unwrap_or_default();

        let candidates = [
            std::env::var("AI_ASSISTANT_PIPER_MODEL").map(PathBuf::from).unwrap_or_default(),
            base_dir.join("models/piper/vi_VN-giong_nu_pho_thong-medium.onnx"),
            home.join("Projects/rust/voice-ai/models/piper/vi_VN-giong_nu_pho_thong-medium.onnx"),
            home.join("Projects/python/voice-ai/models/piper/vi_VN-giong_nu_pho_thong-medium.onnx"),
        ];

        let model_path = candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| {
            base_dir.join("models/piper/vi_VN-giong_nu_pho_thong-medium.onnx")
        });

        let espeak_data = home.join(".local/lib/piper/espeak-ng-data");
        let piper_bin = home.join(".local/bin/piper");

        Self {
            model_path,
            espeak_data,
            piper_bin,
            is_speaking: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_voice_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.model_path = path.as_ref().to_path_buf();
        self
    }

    pub fn is_available(&self) -> bool {
        self.piper_bin.exists() && self.model_path.exists()
    }

    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst)
    }

    /// Đọc văn bản ra loa theo luồng streaming zero-copy (không ghi file tạm ra đĩa).
    /// Hỗ trợ ngắt lời tức thì (Barge-in) nếu StateManager kích hoạt interrupt.
    pub fn speak(&self, text: &str, state_mgr: Option<&StateManager>) -> bool {
        let clean = sanitize_text_for_voice(text);
        if clean.is_empty() {
            return true;
        }

        if !self.is_available() {
            eprintln!("⚠️ Piper binary hoặc model không tìm thấy: {:?}", self.model_path);
            return false;
        }

        self.is_speaking.store(true, Ordering::SeqCst);

        // Khởi tạo tiến trình pw-cat nhận raw PCM từ stdin (--raw chế độ stream trực tiếp)
        let mut pw_play = match Command::new("pw-cat")
            .args(["-p", "--raw", "--rate", "22050", "--channels", "1", "--format", "s16", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("Không thể khởi chạy pw-play: {}", e);
                self.is_speaking.store(false, Ordering::SeqCst);
                return false;
            }
        };

        let pw_stdin = pw_play.stdin.take().expect("Failed to get pw-play stdin");

        // Khởi chạy Piper với --output_raw truyền thẳng vào pw-play stdin
        let mut piper_child = match Command::new(&self.piper_bin)
            .args([
                "--model",
                self.model_path.to_str().unwrap_or_default(),
                "--espeak_data",
                self.espeak_data.to_str().unwrap_or_default(),
                "--output_raw",
            ])
            .stdin(Stdio::piped())
            .stdout(pw_stdin)
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("Không thể khởi chạy piper: {}", e);
                let _ = pw_play.kill();
                self.is_speaking.store(false, Ordering::SeqCst);
                return false;
            }
        };

        // Ghi văn bản cần đọc vào stdin của piper
        if let Some(mut stdin) = piper_child.stdin.take() {
            let _ = writeln!(stdin, "{}", clean);
        }

        // Vòng lặp chờ phát âm thanh xong hoặc bị ngắt lời (Barge-in)
        let res = wait_with_interrupt(&mut piper_child, &mut pw_play, state_mgr);

        self.is_speaking.store(false, Ordering::SeqCst);
        res
    }

    /// Đọc chuỗi câu đến từ kênh streaming qua 1 tiến trình piper + pw-cat duy nhất.
    /// Chỉ khởi chạy tiến trình khi câu hợp lệ đầu tiên tới và hỗ trợ ngắt lời Barge-in tức thì.
    pub fn speak_stream(
        &self,
        rx: Receiver<String>,
        state_mgr: Option<&StateManager>,
    ) -> bool {
        if !self.is_available() {
            eprintln!("⚠️ Piper binary hoặc model không tìm thấy: {:?}", self.model_path);
            return false;
        }

        self.is_speaking.store(true, Ordering::SeqCst);

        let mut active: Option<(Child, Child, std::process::ChildStdin)> = None;
        let mut aborted = false;

        loop {
            if let Some(mgr) = state_mgr {
                if mgr.is_interrupted() {
                    aborted = true;
                    break;
                }
            }

            match rx.recv_timeout(Duration::from_millis(30)) {
                Ok(sentence) => {
                    let clean = sanitize_text_for_voice(&sentence);
                    if clean.is_empty() {
                        continue;
                    }

                    if active.is_none() {
                        let pw_play = match Command::new("pw-cat")
                            .args([
                                "-p", "--raw", "--rate", "22050", "--channels", "1", "--format",
                                "s16", "-",
                            ])
                            .stdin(Stdio::piped())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                        {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Không thể khởi chạy pw-cat: {}", e);
                                aborted = true;
                                break;
                            }
                        };

                        let mut pw_play = pw_play;
                        let pw_stdin = pw_play.stdin.take().expect("pw-cat stdin");

                        let mut piper_child = match Command::new(&self.piper_bin)
                            .args([
                                "--model",
                                self.model_path.to_str().unwrap_or_default(),
                                "--espeak_data",
                                self.espeak_data.to_str().unwrap_or_default(),
                                "--output_raw",
                            ])
                            .stdin(Stdio::piped())
                            .stdout(pw_stdin)
                            .stderr(Stdio::null())
                            .spawn()
                        {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Không thể khởi chạy piper: {}", e);
                                let _ = pw_play.kill();
                                aborted = true;
                                break;
                            }
                        };

                        let piper_stdin = piper_child.stdin.take().expect("piper stdin");
                        active = Some((piper_child, pw_play, piper_stdin));
                    }

                    if let Some((_, _, ref mut stdin)) = active {
                        if writeln!(stdin, "{}", clean).is_err() || stdin.flush().is_err() {
                            aborted = true;
                            break;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        let res = if let Some((mut piper, mut player, stdin)) = active {
            drop(stdin); // Đóng stdin để Piper đọc nốt câu cuối rồi thoát sạch
            if aborted {
                let _ = piper.kill();
                let _ = player.kill();
                false
            } else {
                wait_with_interrupt(&mut piper, &mut player, state_mgr)
            }
        } else {
            !aborted
        };

        self.is_speaking.store(false, Ordering::SeqCst);
        res
    }
}

fn wait_with_interrupt(
    piper: &mut Child,
    player: &mut Child,
    state_mgr: Option<&StateManager>,
) -> bool {
    let check_interval = std::time::Duration::from_millis(25);

    loop {
        // Kiểm tra xem có tín hiệu ngắt lời khẩn cấp không
        if let Some(mgr) = state_mgr {
            if mgr.is_interrupted() {
                let _ = piper.kill();
                let _ = player.kill();
                return false;
            }
        }

        // Kiểm tra xem trình phát âm đã kết thúc chưa
        match player.try_wait() {
            Ok(Some(_)) => {
                let _ = piper.wait();
                return true;
            }
            Ok(None) => {
                std::thread::sleep(check_interval);
            }
            Err(_) => {
                let _ = piper.kill();
                return false;
            }
        }
    }
}

impl Default for PiperTts {
    fn default() -> Self {
        Self::new()
    }
}
