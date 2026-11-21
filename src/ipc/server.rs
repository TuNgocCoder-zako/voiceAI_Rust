use crate::core::state::StateManager;
use crate::ipc::client::{IpcCommand, IpcResponse};
use std::fs;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

pub enum ActionRequest {
    Trigger,
    Stop,
    Relisten,
    Query(String),
}

pub struct IpcServer {
    socket_path: std::path::PathBuf,
    state_manager: StateManager,
    action_tx: mpsc::Sender<ActionRequest>,
}

impl IpcServer {
    pub fn new<P: AsRef<Path>>(
        socket_path: P,
        state_manager: StateManager,
        action_tx: mpsc::Sender<ActionRequest>,
    ) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            state_manager,
            action_tx,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        println!("🚀 IPC Server listening on {:?}", self.socket_path);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_mgr = self.state_manager.clone();
                    let tx = self.action_tx.clone();
                    tokio::spawn(async move {
                        let (read_half, mut write_half) = stream.into_split();
                        let mut reader = BufReader::new(read_half);
                        let mut line = String::new();

                        if let Ok(n) = reader.read_line(&mut line).await {
                            if n > 0 {
                                if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&line) {
                                    let resp = match cmd.command.as_str() {
                                        "status" => {
                                            let current = state_mgr.get_state().await;
                                            IpcResponse {
                                                status: "ok".to_string(),
                                                message: Some("AI Assistant is running".to_string()),
                                                state: Some(current.to_string()),
                                            }
                                        }
                                        "trigger" => {
                                            let _ = tx.send(ActionRequest::Trigger).await;
                                            IpcResponse {
                                                status: "ok".to_string(),
                                                message: Some("Triggered listening".to_string()),
                                                state: Some("listening".to_string()),
                                            }
                                        }
                                        "stop" => {
                                            state_mgr.interrupt();
                                            let _ = tx.send(ActionRequest::Stop).await;
                                            IpcResponse {
                                                status: "ok".to_string(),
                                                message: Some("Stopped speech/inference".to_string()),
                                                state: Some("idle".to_string()),
                                            }
                                        }
                                        "relisten" => {
                                            state_mgr.interrupt();
                                            let _ = tx.send(ActionRequest::Relisten).await;
                                            IpcResponse {
                                                status: "ok".to_string(),
                                                message: Some("Relisten requested".to_string()),
                                                state: Some("listening".to_string()),
                                            }
                                        }
                                        "query" => {
                                            if let Some(text) = cmd.text {
                                                let _ = tx.send(ActionRequest::Query(text)).await;
                                            }
                                            IpcResponse {
                                                status: "ok".to_string(),
                                                message: Some("Query enqueued".to_string()),
                                                state: None,
                                            }
                                        }
                                        _ => IpcResponse {
                                            status: "error".to_string(),
                                            message: Some("Unknown command".to_string()),
                                            state: None,
                                        },
                                    };

                                    if let Ok(mut resp_str) = serde_json::to_string(&resp) {
                                        resp_str.push('\n');
                                        let _ = write_half.write_all(resp_str.as_bytes()).await;
                                    }
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("IPC Accept error: {}", e);
                }
            }
        }
    }
}
