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

    pub async fn set_state(&self, new_state: AssistantState, text: Option<String>) {
        {
            let mut state = self.current_state.write().await;
            *state = new_state;
        }
        let event = StateEvent {
            state: new_state,
            text,
            visual: None,
        };
        let _ = self.broadcaster.send(event);
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
