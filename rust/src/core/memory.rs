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
