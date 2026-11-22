use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcCommand {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

pub async fn send_ipc_command<P: AsRef<Path>>(
    socket_path: P,
    command: &str,
    text: Option<String>,
) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    send_ipc_command_with_options(socket_path, command, text, None).await
}

pub async fn send_ipc_command_with_options<P: AsRef<Path>>(
    socket_path: P,
    command: &str,
    text: Option<String>,
    deep: Option<bool>,
) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path).await?;
    let payload = IpcCommand {
        command: command.to_string(),
        text,
        deep,
    };

    let mut line = serde_json::to_string(&payload)?;
    line.push('\n');
    stream.write_all(line.as_bytes()).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).await?;

    let resp: IpcResponse = serde_json::from_str(&resp_line)?;
    Ok(resp)
}
