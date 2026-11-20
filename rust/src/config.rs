use std::collections::HashMap;
use std::path::PathBuf;

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/voice_ai_state.sock";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "qwen2.5";
pub const SAMPLE_RATE: u32 = 16000;
pub const CHUNK_SIZE: usize = 1280;

#[derive(Debug, Clone)]
pub struct Config {
    pub socket_path: PathBuf,
    pub ollama_url: String,
    pub model: String,
    pub sample_rate: u32,
    pub chunk_size: usize,
    pub url_shortcuts: HashMap<String, String>,
    pub url_display_names: HashMap<String, String>,
    pub app_aliases: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let socket_path = std::env::var("VOICE_AI_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET_PATH));

        let ollama_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let model = std::env::var("AI_ASSISTANT_LLM_MODEL")
            .unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        let mut url_shortcuts = HashMap::new();
        url_shortcuts.insert("youtube".into(), "https://www.youtube.com".into());
        url_shortcuts.insert("gmail".into(), "https://mail.google.com".into());
        url_shortcuts.insert("facebook".into(), "https://www.facebook.com".into());
        url_shortcuts.insert("github".into(), "https://github.com".into());
        url_shortcuts.insert("zalo".into(), "https://chat.zalo.me".into());
        url_shortcuts.insert("spotify".into(), "https://open.spotify.com".into());
        url_shortcuts.insert("google".into(), "https://www.google.com".into());
        url_shortcuts.insert("chatgpt".into(), "https://chatgpt.com".into());
        url_shortcuts.insert("claude".into(), "https://claude.ai".into());
        url_shortcuts.insert("gemini".into(), "https://gemini.google.com".into());

        let mut url_display_names = HashMap::new();
        url_display_names.insert("youtube".into(), "YouTube".into());
        url_display_names.insert("gmail".into(), "Gmail".into());
        url_display_names.insert("facebook".into(), "Facebook".into());
        url_display_names.insert("github".into(), "GitHub".into());
        url_display_names.insert("zalo".into(), "Zalo".into());
        url_display_names.insert("spotify".into(), "Spotify".into());
        url_display_names.insert("google".into(), "Google".into());

        let mut app_aliases = HashMap::new();
        app_aliases.insert("vs code".into(), "code".into());
        app_aliases.insert("vscode".into(), "code".into());
        app_aliases.insert("visual studio code".into(), "code".into());
        app_aliases.insert("terminal".into(), "terminal".into());
        app_aliases.insert("terminal emulator".into(), "terminal".into());
        app_aliases.insert("trình duyệt".into(), "browser".into(),);
        app_aliases.insert("firefox".into(), "firefox".into());
        app_aliases.insert("chrome".into(), "google-chrome".into());
        app_aliases.insert("google chrome".into(), "google-chrome".into());
        app_aliases.insert("brave".into(), "brave-browser".into());
        app_aliases.insert("spotify".into(), "spotify".into());
        app_aliases.insert("zalo".into(), "zalo".into());
        app_aliases.insert("telegram".into(), "telegram".into());

        Self {
            socket_path,
            ollama_url,
            model,
            sample_rate: SAMPLE_RATE,
            chunk_size: CHUNK_SIZE,
            url_shortcuts,
            url_display_names,
            app_aliases,
        }
    }
}
