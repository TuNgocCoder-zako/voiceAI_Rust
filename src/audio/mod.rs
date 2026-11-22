pub mod capture;
pub mod oww;
pub mod stt;
pub mod vad;
pub mod wakeword;

use std::sync::OnceLock;
use ort::environment::Environment;

pub fn get_ort_env() -> Result<&'static Environment, Box<dyn std::error::Error>> {
    static ENV: OnceLock<Environment> = OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    static INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = INIT_LOCK.lock().map_err(|e| e.to_string())?;
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init()
        .with_name("voice-ai")
        .build()?;
    let _ = ENV.set(env);
    Ok(ENV.get().expect("ENV initialized"))
}

pub use capture::AudioRecorder;
pub use oww::{OpenWakeWordConfig, OpenWakeWordDetector, OwwFrameResult};
pub use stt::WhisperEngine;
pub use vad::{SchmittTriggerVad, SileroVad};
pub use wakeword::{detect_wake_word, WakeWordType};
