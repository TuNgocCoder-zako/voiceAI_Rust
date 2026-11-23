use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRole {
    Fast,      // Hội thoại nhanh, chào hỏi, kiến thức ngắn -> Qwen 3.8 2B / Qwen 2.5 3B
    Coder,     // Lập trình, thuật toán, bash, docker, git -> Qwen2.5-Coder 7B
    Reasoning, // Suy luận phức tạp, giải thích sâu, logic -> Qwen3 Thinking / DeepSeek R1
    General,   // Mô hình tổng quát tốt nhất
}

impl TaskRole {
    pub fn label(&self) -> &'static str {
        match self {
            TaskRole::Fast => "Phản xạ nhanh (Fast)",
            TaskRole::Coder => "Lập trình & Kỹ thuật (Coder)",
            TaskRole::Reasoning => "Suy luận chuyên sâu (Reasoning)",
            TaskRole::General => "Tổng quát tốt nhất (General Best)",
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModelDetails {
    pub parent_model: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub families: Option<Vec<String>>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelTagInfo {
    pub name: String,
    pub model: Option<String>,
    pub size: Option<u64>,
    pub details: Option<ModelDetails>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Option<Vec<ModelTagInfo>>,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct ChatOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

struct CachedModels {
    models: Vec<ModelTagInfo>,
    last_fetched: Instant,
}

pub struct ModelRouter {
    ollama_url: String,
    client: Client,
    cache: RwLock<Option<CachedModels>>,
    ttl: Duration,
}

impl ModelRouter {
    pub fn new(ollama_url: &str) -> Self {
        Self {
            ollama_url: ollama_url.trim_end_matches('/').to_string(),
            client: Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_default(),
            cache: RwLock::new(None),
            ttl: Duration::from_secs(30),
        }
    }

    /// Lấy danh sách các model hiện có trong Ollama với cache 30s
    pub async fn get_available_models(&self) -> Vec<ModelTagInfo> {
        if let Ok(guard) = self.cache.read() {
            if let Some(cached) = guard.as_ref() {
                if cached.last_fetched.elapsed() < self.ttl && !cached.models.is_empty() {
                    return cached.models.clone();
                }
            }
        }

        let url = format!("{}/api/tags", self.ollama_url);
        let models = match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<TagsResponse>().await {
                    body.models.unwrap_or_default()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        if !models.is_empty() {
            if let Ok(mut guard) = self.cache.write() {
                *guard = Some(CachedModels {
                    models: models.clone(),
                    last_fetched: Instant::now(),
                });
            }
        }

        models
    }

    /// Tự động phát hiện vai trò của câu hỏi dựa trên từ khóa ngữ nghĩa
    pub fn detect_role(&self, prompt: &str) -> TaskRole {
        let p_low = prompt.to_lowercase();

        // 1. Kiểm tra tác vụ lập trình / kỹ thuật
        let coder_keywords = [
            "code", "lập trình", "viết code", "viết hàm", "viết script", "thuật toán",
            "debug", "sửa lỗi", "syntax", "python", "rust", "javascript", "typescript",
            "c++", "golang", "java", "sql", "docker", "dockerfile", "git", "bash",
            "shell", "regex", "hàm", "function", "class", "struct", "async", "api",
            "endpoint", "terminal", "compile", "biên dịch", "json", "html", "css",
        ];
        if coder_keywords.iter().any(|&kw| p_low.contains(kw)) {
            return TaskRole::Coder;
        }

        // 2. Kiểm tra tác vụ suy luận sâu, giải thích nguyên lý, toán học
        let reasoning_keywords = [
            "tại sao", "vì sao", "nguyên nhân", "giải thích chi tiết", "khái niệm",
            "định nghĩa", "phân tích", "so sánh", "kể về", "tóm tắt", "nguồn gốc",
            "lịch sử", "ý nghĩa", "nguyên lý", "cơ chế", "ưu nhược điểm", "khác nhau",
            "chứng minh", "logic", "bài toán", "đạo hàm", "tích phân", "vật lý",
            "hóa học", "suy luận", "tư duy", "triết học", "nghiên cứu",
        ];
        if reasoning_keywords.iter().any(|&kw| p_low.contains(kw)) {
            return TaskRole::Reasoning;
        }

        // 3. Mặc định: Phản xạ nhanh (Fast)
        TaskRole::Fast
    }

    /// Tự động lựa chọn mô hình TỐT NHẤT hiện có trên máy phù hợp với vai trò
    pub fn resolve_best_model(
        &self,
        role: TaskRole,
        available: &[ModelTagInfo],
        default_fallback: &str,
    ) -> (String, TaskRole, ChatOptions) {
        if available.is_empty() {
            return (
                default_fallback.to_string(),
                role,
                self.make_options(role, default_fallback),
            );
        }

        let names: Vec<String> = available.iter().map(|m| m.name.clone()).collect();

        match role {
            TaskRole::Coder => {
                // Ưu tiên các model coder chuyên biệt
                for prefix in &["qwen2.5-coder", "deepseek-coder", "codellama", "starcoder"] {
                    if let Some(m) = names.iter().find(|n| n.to_lowercase().contains(prefix)) {
                        return (m.clone(), TaskRole::Coder, self.make_options(TaskRole::Coder, m));
                    }
                }
                // Nếu parent_model có chứa coder (như qwen2.5:latest tạo từ file coder)
                for m in available {
                    if let Some(details) = &m.details {
                        if let Some(parent) = &details.parent_model {
                            if parent.to_lowercase().contains("coder") {
                                return (m.name.clone(), TaskRole::Coder, self.make_options(TaskRole::Coder, &m.name));
                            }
                        }
                    }
                }
                // Fallback: Model có dung lượng lớn nhất
                let best = self.find_highest_param_model(available);
                (best.clone(), TaskRole::Coder, self.make_options(TaskRole::Coder, &best))
            }

            TaskRole::Reasoning => {
                // Ưu tiên các model Thinking / DeepSeek R1 / QwQ
                for prefix in &["thinking", "think", "r1", "qwq", "reasoning"] {
                    if let Some(m) = names.iter().find(|n| n.to_lowercase().contains(prefix)) {
                        return (m.clone(), TaskRole::Reasoning, self.make_options(TaskRole::Reasoning, m));
                    }
                }
                // Fallback: Model lớn nhất hiện có
                let best = self.find_highest_param_model(available);
                (best.clone(), TaskRole::Reasoning, self.make_options(TaskRole::Reasoning, &best))
            }

            TaskRole::Fast => {
                // Ưu tiên model chính do người dùng mong muốn: qwen3.8 2b, tiếp đến 2b-4b
                for prefix in &[
                    "qwen3.8:2b",
                    "qwen3.8",
                    "qwen3:4b",
                    "qwen2.5:3b",
                    "llama3.2:3b",
                    "qwen2.5:1.5b",
                    "gemma:2b",
                    "gemma2:2b",
                ] {
                    if let Some(m) = names.iter().find(|n| n.to_lowercase().contains(prefix)) {
                        return (m.clone(), TaskRole::Fast, self.make_options(TaskRole::Fast, m));
                    }
                }
                // Nếu không có model nhỏ, chọn model đầu tiên có sẵn
                let first = names.first().cloned().unwrap_or_else(|| default_fallback.to_string());
                (first.clone(), TaskRole::Fast, self.make_options(TaskRole::Fast, &first))
            }

            TaskRole::General => {
                let best = self.find_highest_param_model(available);
                (best.clone(), TaskRole::General, self.make_options(TaskRole::General, &best))
            }
        }
    }

    /// Định tuyến từ câu hỏi, cho phép ép chế độ Suy luận sâu từ UI
    pub async fn route_prompt_with_deep(
        &self,
        prompt: &str,
        force_deep: bool,
        default_fallback: &str,
    ) -> (String, TaskRole, ChatOptions) {
        let role = if force_deep {
            TaskRole::Reasoning
        } else {
            self.detect_role(prompt)
        };
        let available = self.get_available_models().await;
        self.resolve_best_model(role, &available, default_fallback)
    }

    /// Định tuyến hoàn chỉnh từ câu hỏi người dùng
    pub async fn route_prompt(
        &self,
        prompt: &str,
        default_fallback: &str,
    ) -> (String, TaskRole, ChatOptions) {
        let role = self.detect_role(prompt);
        let available = self.get_available_models().await;
        self.resolve_best_model(role, &available, default_fallback)
    }

    /// Tìm model có số lượng tham số lớn nhất / chất lượng cao nhất
    fn find_highest_param_model(&self, models: &[ModelTagInfo]) -> String {
        let mut best_name = models[0].name.clone();
        let mut max_score = 0;

        for m in models {
            let mut score = 0;
            let n_low = m.name.to_lowercase();

            // Điểm theo kiến trúc dòng họ
            if n_low.contains("qwen3") {
                score += 500;
            } else if n_low.contains("qwen2.5") {
                score += 400;
            } else if n_low.contains("deepseek") {
                score += 450;
            } else if n_low.contains("llama3") {
                score += 350;
            }

            // Điểm theo dung lượng file
            if let Some(sz) = m.size {
                score += (sz / (1024 * 1024 * 100)) as i32; // Mỗi 100MB +1 điểm
            }

            if score > max_score {
                max_score = score;
                best_name = m.name.clone();
            }
        }

        best_name
    }

    /// Tự động sinh tham số tối ưu (temperature, token limit) tương ứng với vai trò và model
    fn make_options(&self, role: TaskRole, model_name: &str) -> ChatOptions {
        let m_low = model_name.to_lowercase();
        let is_thinking = m_low.contains("thinking") || m_low.contains("r1") || m_low.contains("qwq");

        match role {
            TaskRole::Reasoning => ChatOptions {
                temperature: Some(0.6),
                num_predict: Some(if is_thinking { 2048 } else { 1024 }),
                num_ctx: Some(2048),
                top_p: Some(0.95),
            },
            TaskRole::Coder => ChatOptions {
                temperature: Some(0.2), // Nhiệt độ thấp cho kết quả chính xác, chuẩn cú pháp
                num_predict: Some(1200),
                num_ctx: Some(2048),
                top_p: Some(0.9),
            },
            TaskRole::Fast => ChatOptions {
                temperature: Some(0.3),
                num_predict: Some(512), // Đủ dài cho câu trả lời tự nhiên và đầy đủ ý
                num_ctx: Some(2048),
                top_p: Some(0.9),
            },
            TaskRole::General => ChatOptions {
                temperature: Some(0.4),
                num_predict: Some(768),
                num_ctx: Some(2048),
                top_p: Some(0.9),
            },
        }
    }
}

/// Trích xuất suy nghĩ nội tâm <think>...</think> (an toàn cả khi thẻ bị cắt chưa kịp đóng)
pub fn filter_thinking_content(text: &str) -> (Option<String>, String) {
    if let Some(start) = text.find("<think>") {
        let after_open = start + "<think>".len();
        if let Some(rel_end) = text[after_open..].find("</think>") {
            let end = after_open + rel_end;
            let thought = text[after_open..end].trim().to_string();
            let mut final_text = String::new();
            final_text.push_str(&text[..start]);
            final_text.push_str(&text[end + "</think>".len()..]);
            return (Some(thought), final_text.trim().to_string());
        } else {
            let thought = text[after_open..].trim().to_string();
            let prefix = text[..start].trim().to_string();
            return (Some(thought), prefix);
        }
    }
    (None, text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_role() {
        let router = ModelRouter::new("http://localhost:11434");
        assert_eq!(
            router.detect_role("viết cho tôi một script bash sao lưu dữ liệu"),
            TaskRole::Coder
        );
        assert_eq!(
            router.detect_role("tại sao lá cây lại có màu xanh"),
            TaskRole::Reasoning
        );
        assert_eq!(
            router.detect_role("hôm nay là thứ mấy"),
            TaskRole::Fast
        );
    }

    #[test]
    fn test_filter_thinking() {
        let raw = "<think>\nTôi cần trả lời thủ đô của Pháp.\n</think>\nThủ đô của nước Pháp là Paris.";
        let (thought, clean) = filter_thinking_content(raw);
        assert_eq!(thought.as_deref(), Some("Tôi cần trả lời thủ đô của Pháp."));
        assert_eq!(clean, "Thủ đô của nước Pháp là Paris.");
    }
}
