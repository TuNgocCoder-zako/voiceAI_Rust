use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const SYSTEM_PROMPT: &str = "\
Bạn là Alexa, một trợ lý ảo hệ điều hành AI thông minh, nhanh nhẹn và hữu ích trên máy tính Linux.\n\
Nguyên tắc trả lời:\n\
- Luôn trả lời bằng tiếng Việt tự nhiên, ngắn gọn, súc tích (1 đến 3 câu).\n\
- Vì câu trả lời sẽ được đọc thành tiếng qua giọng nói (TTS), KHÔNG sử dụng ký tự đặc biệt, định dạng markdown phức tạp, bảng biểu hay danh sách dài dòng.\n\
- Thân thiện, nhiệt tình và chính xác.\
";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    message: Option<ChunkMessage>,
    done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChunkMessage {
    content: String,
}

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: Client,
}

impl OllamaClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn chat<F>(
        &self,
        messages: Vec<ChatMessage>,
        mut chunk_callback: F,
    ) -> Result<String, Box<dyn std::error::Error>>
    where
        F: FnMut(&str) + Send,
    {
        let url = format!("{}/api/chat", self.base_url);
        let payload = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: true,
        };

        let response = self.client.post(&url).json(&payload).send().await?;
        let mut stream = response.bytes_stream();
        let mut full_text = String::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                if let Ok(data) = serde_json::from_str::<ChatChunk>(line) {
                    if let Some(msg) = data.message {
                        if !msg.content.is_empty() {
                            chunk_callback(&msg.content);
                            full_text.push_str(&msg.content);
                        }
                    }
                    if data.done == Some(true) {
                        break;
                    }
                }
            }
        }

        Ok(full_text)
    }
}
