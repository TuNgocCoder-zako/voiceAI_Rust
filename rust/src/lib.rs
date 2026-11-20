pub mod config;
pub mod core;
pub mod ipc;
pub mod tools;
pub mod ai;
pub mod speech;
pub mod audio;

pub use audio::{AudioRecorder, WhisperEngine};
pub use config::Config;
pub use core::nlu::{clean_user_input, fast_path_nlu, strip_vietnamese_accents};
pub use core::state::{AssistantState, StateManager};
pub use ipc::client::send_ipc_command;
pub use ipc::server::IpcServer;
pub use speech::{play_chime, ChimeType, PiperTts};
pub use tools::apps::build_app_index;
