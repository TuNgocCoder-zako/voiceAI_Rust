use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssistantState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Disabled,
}

impl std::fmt::Display for AssistantState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssistantState::Idle => write!(f, "idle"),
            AssistantState::Listening => write!(f, "listening"),
            AssistantState::Thinking => write!(f, "thinking"),
            AssistantState::Speaking => write!(f, "speaking"),
            AssistantState::Disabled => write!(f, "disabled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    pub state: AssistantState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlexaUiState {
    pub status: String,
    pub text: String,
    pub subtext: String,
    pub query: String,
    pub is_long_text: bool,
    pub long_answer: String,
    pub updated_at: f64,
}

fn write_ui_state_atomic(state: &AlexaUiState) {
    if let Ok(json_str) = serde_json::to_string(state) {
        let targets = [
            std::env::var("XDG_RUNTIME_DIR")
                .map(|d| format!("{}/alexa_state.json", d))
                .unwrap_or_else(|_| "/tmp/alexa_state.json".to_string()),
            "/tmp/alexa_state.json".to_string(),
        ];

        for path in targets {
            let tmp_path = format!("{}.tmp.{}", path, std::process::id());
            if std::fs::write(&tmp_path, &json_str).is_ok() {
                let _ = std::fs::rename(&tmp_path, &path);
            }
        }
    }
}

#[derive(Clone)]
pub struct StateManager {
    current_state: Arc<tokio::sync::RwLock<AssistantState>>,
    broadcaster: broadcast::Sender<StateEvent>,
    is_interrupted: Arc<AtomicBool>,
}

impl StateManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            current_state: Arc::new(tokio::sync::RwLock::new(AssistantState::Idle)),
            broadcaster: tx,
            is_interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn get_state(&self) -> AssistantState {
        *self.current_state.read().await
    }

    pub async fn set_state_full(
        &self,
        new_state: AssistantState,
        text: Option<String>,
        query: Option<String>,
    ) {
        {
            let mut state = self.current_state.write().await;
            *state = new_state;
        }

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        let t = text.clone().unwrap_or_default();
        let q = query.unwrap_or_default();
        let is_long = t.len() > 60;

        let ui_state = AlexaUiState {
            status: new_state.to_string(),
            text: t.clone(),
            subtext: String::new(),
            query: q,
            is_long_text: is_long,
            long_answer: if is_long { t.clone() } else { String::new() },
            updated_at: now_sec,
        };

        write_ui_state_atomic(&ui_state);

        let event = StateEvent {
            state: new_state,
            text,
            visual: None,
        };
        let _ = self.broadcaster.send(event);
    }

    pub async fn set_state(&self, new_state: AssistantState, text: Option<String>) {
        self.set_state_full(new_state, text, None).await;
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.broadcaster.subscribe()
    }

    pub fn interrupt(&self) {
        self.is_interrupted.store(true, Ordering::SeqCst);
    }

    pub fn reset_interrupt(&self) {
        self.is_interrupted.store(false, Ordering::SeqCst);
    }

    pub fn is_interrupted(&self) -> bool {
        self.is_interrupted.load(Ordering::SeqCst)
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}
