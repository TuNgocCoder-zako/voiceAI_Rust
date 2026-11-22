use crate::ai::router::ChatOptions;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    message: Option<ChunkMessage>,
    done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChunkMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    #[allow(dead_code)]
    thinking: Option<String>,
}

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    default_model: String,
    client: Client,
}

impl OllamaClient {
    pub fn new(base_url: &str, default_model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub async fn chat<F>(
        &self,
        messages: Vec<ChatMessage>,
        chunk_callback: F,
    ) -> Result<String, Box<dyn std::error::Error>>
    where
        F: FnMut(&str) + Send,
    {
        self.chat_with_options(None, messages, None, chunk_callback).await
    }

    pub async fn chat_with_options<F>(
        &self,
        model_override: Option<&str>,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
        mut chunk_callback: F,
    ) -> Result<String, Box<dyn std::error::Error>>
    where
        F: FnMut(&str) + Send,
    {
        let target_model = model_override
            .unwrap_or(&self.default_model)
            .to_string();

        let url = format!("{}/api/chat", self.base_url);
        let payload = ChatRequest {
            model: target_model,
            messages,
            stream: true,
            options,
        };

        let response = self.client.post(&url).json(&payload).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama HTTP {}: {}", status, err_body.trim()).into());
        }

        let mut stream = response.bytes_stream();
        let mut byte_buf: Vec<u8> = Vec::new();
        let mut full_content = String::new();
        let mut is_done = false;

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res?;
            byte_buf.extend_from_slice(&chunk);

            // Chỉ parse khi đã gom đủ trọn vẹn 1 dòng kết thúc bằng '\n'
            while let Some(newline_pos) = byte_buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = byte_buf.drain(..=newline_pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Ok(data) = serde_json::from_str::<ChatChunk>(trimmed) {
                    if let Some(msg) = data.message {
                        if !msg.content.is_empty() {
                            chunk_callback(&msg.content);
                            full_content.push_str(&msg.content);
                        }
                    }
                    if data.done == Some(true) {
                        is_done = true;
                        break;
                    }
                }
            }

            if is_done {
                break;
            }
        }

        // Xử lý dòng cuối nếu server đóng kết nối không kèm '\n'
        if !is_done && !byte_buf.is_empty() {
            let line = String::from_utf8_lossy(&byte_buf);
            if let Ok(data) = serde_json::from_str::<ChatChunk>(line.trim()) {
                if let Some(msg) = data.message {
                    if !msg.content.is_empty() {
                        chunk_callback(&msg.content);
                        full_content.push_str(&msg.content);
                    }
                }
            }
        }

        if full_content.trim().is_empty() {
            let fallback = "Dạ, tôi chưa tổng hợp kịp câu trả lời. Bạn hỏi lại ngắn gọn hơn giúp tôi nhé.";
            chunk_callback(fallback);
            return Ok(fallback.to_string());
        }

        Ok(full_content)
    }
}
