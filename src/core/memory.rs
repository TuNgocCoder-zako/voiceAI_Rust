use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserMemory {
    pub notes: Vec<String>,
    pub facts: Vec<String>,
}

pub struct MemoryManager {
    file_path: PathBuf,
    memory: Mutex<UserMemory>,
}

impl MemoryManager {
    pub fn new() -> Self {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"))
            .join("voice-ai");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("memory.json");

        let memory = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|data| serde_json::from_str(&data).ok())
                .unwrap_or_default()
        } else {
            UserMemory::default()
        };

        Self {
            file_path,
            memory: Mutex::new(memory),
        }
    }

    pub fn add_note(&self, note: &str) -> String {
        let mut mem = self.memory.lock().unwrap();
        mem.notes.push(note.to_string());
        self.save(&mem);
        format!("Đã lưu ghi chú '{}' cho bạn rồi nhé.", note)
    }

    pub fn read_notes(&self) -> String {
        let mem = self.memory.lock().unwrap();
        if mem.notes.is_empty() {
            "Hiện tại bạn chưa có ghi chú nào được lưu.".to_string()
        } else {
            let list = mem.notes.iter()
                .enumerate()
                .map(|(i, n)| format!("{}. {}", i + 1, n))
                .collect::<Vec<_>>()
                .join("\n");
            format!("Danh sách các ghi chú của bạn:\n{}", list)
        }
    }

    pub fn add_fact(&self, fact: &str) -> String {
        let mut mem = self.memory.lock().unwrap();
        mem.facts.push(fact.to_string());
        self.save(&mem);
        format!("Tôi đã ghi nhớ thông tin '{}' vào bộ nhớ dài hạn.", fact)
    }

    fn save(&self, mem: &UserMemory) {
        if let Ok(data) = serde_json::to_string_pretty(mem) {
            let _ = fs::write(&self.file_path, data);
        }
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

use crate::ai::ChatMessage;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Bộ nhớ ngữ cảnh hội thoại ngắn hạn (trượt N tin nhắn gần nhất, tự hết hạn theo TTL)
pub struct ConversationHistory {
    messages: VecDeque<ChatMessage>,
    max_messages: usize,
    ttl: Duration,
    last_updated: Option<Instant>,
}

impl ConversationHistory {
    pub fn new(max_turns: usize, ttl_secs: u64) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_turns * 2),
            max_messages: max_turns * 2,
            ttl: Duration::from_secs(ttl_secs),
            last_updated: None,
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.last_updated = None;
    }

    fn expire_if_stale(&mut self) {
        if let Some(last) = self.last_updated {
            if last.elapsed() > self.ttl {
                self.messages.clear();
                self.last_updated = None;
            }
        }
    }

    pub fn build_messages(&mut self, system_prompt: &str, current_prompt: &str) -> Vec<ChatMessage> {
        self.expire_if_stale();
        let mut out = Vec::with_capacity(self.messages.len() + 2);
        out.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        });
        for m in &self.messages {
            out.push(m.clone());
        }
        out.push(ChatMessage {
            role: "user".to_string(),
            content: current_prompt.to_string(),
        });
        out
    }

    pub fn push_turn(&mut self, user_msg: &str, assistant_msg: &str) {
        self.expire_if_stale();
        self.messages.push_back(ChatMessage {
            role: "user".to_string(),
            content: user_msg.to_string(),
        });
        self.messages.push_back(ChatMessage {
            role: "assistant".to_string(),
            content: assistant_msg.to_string(),
        });
        while self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
        self.last_updated = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_history_sliding_window() {
        let mut hist = ConversationHistory::new(2, 120);
        hist.push_turn("Câu 1", "Đáp 1");
        hist.push_turn("Câu 2", "Đáp 2");
        hist.push_turn("Câu 3", "Đáp 3");

        let msgs = hist.build_messages("SYS", "Câu 4");
        // 1 system + 4 history (Câu 2, Đáp 2, Câu 3, Đáp 3) + 1 current user = 6
        assert_eq!(msgs.len(), 6);
        assert_eq!(msgs[1].content, "Câu 2");
        assert_eq!(msgs[5].content, "Câu 4");
    }
}
