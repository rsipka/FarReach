//! Telnet sessions: a TCP connection driven by [`protocol::TelnetCodec`], with
//! output streamed to the terminal as `telnet-data-{id}` events and the end of
//! the connection as `telnet-exit-{id}`.
//!
//! The terminal connects only once it is listening, so nothing the device
//! sends first (a banner, a login prompt) is lost.

pub mod protocol;

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use protocol::{AutoLogin, LoginStep, TelnetCodec, TelnetOptions};

/// How long to wait for the device to answer the connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long after connecting a login prompt is answered from the saved login.
const LOGIN_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelnetConnectParams {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub options: TelnetOptions,
    /// Typed at the device's login prompt, if it shows one.
    #[serde(default)]
    pub username: Option<String>,
    /// Typed at the device's password prompt.
    #[serde(default)]
    pub password: Option<String>,
    /// A password manager entry used instead of `password`.
    #[serde(default)]
    pub password_ref: Option<crate::pwmanager::SecretRef>,
}

enum Command {
    Input(Vec<u8>),
    Resize(u16, u16),
    Close,
}

#[derive(Default)]
pub struct TelnetManager {
    connections: HashMap<String, mpsc::UnboundedSender<Command>>,
}

impl TelnetManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run a connection made by [`open`] until either side closes it.
    pub fn start(&mut self, app: tauri::AppHandle, params: TelnetConnectParams, stream: TcpStream) {
        let (tx, rx) = mpsc::unbounded_channel();
        // A connection made again under the same id replaces the old one.
        if let Some(old) = self.connections.insert(params.id.clone(), tx) {
            let _ = old.send(Command::Close);
        }
        tauri::async_runtime::spawn(run(app, params, stream, rx));
    }

    pub fn send(&self, id: &str, data: Vec<u8>) -> Result<(), String> {
        self.command(id, Command::Input(data))
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        self.command(id, Command::Resize(cols, rows))
    }

    pub fn close(&mut self, id: &str) {
        if let Some(tx) = self.connections.remove(id) {
            let _ = tx.send(Command::Close);
        }
    }

    fn command(&self, id: &str, cmd: Command) -> Result<(), String> {
        self.connections
            .get(id)
            .ok_or_else(|| format!("Telnet connection not found: {id}"))?
            .send(cmd)
            .map_err(|_| "The Telnet connection is closed".to_string())
    }
}

/// Connect to the device.
pub async fn open(host: &str, port: u16) -> Result<TcpStream, String> {
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| {
            format!(
                "{host}:{port} did not answer within {} seconds",
                CONNECT_TIMEOUT.as_secs()
            )
        })?
        .map_err(|e| format!("Could not connect to {host}:{port}: {e}"))?;
    // Keystrokes go out as they are typed.
    let _ = stream.set_nodelay(true);
    Ok(stream)
}

async fn run(
    app: tauri::AppHandle,
    params: TelnetConnectParams,
    stream: TcpStream,
    mut commands: mpsc::UnboundedReceiver<Command>,
) {
    let id = params.id.clone();
    let data_event = format!("telnet-data-{id}");
    let exit_event = format!("telnet-exit-{id}");
    let (mut reader, mut writer) = stream.into_split();
    let mut codec = TelnetCodec::new(params.options, params.cols, params.rows);
    let mut login = AutoLogin::new(params.username.clone(), params.password.clone());
    let login_until = tokio::time::Instant::now() + LOGIN_WINDOW;
    let mut decoder = crate::text::Utf8Stream::new();
    let mut buf = vec![0u8; 8192];

    let start = codec.start();
    let mut error: Option<String> = None;
    if !start.is_empty() {
        if let Err(e) = writer.write_all(&start).await {
            error = Some(e.to_string());
        }
    }

    while error.is_none() {
        tokio::select! {
            read = reader.read(&mut buf) => {
                let n = match read {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                };
                let received = codec.receive(&buf[..n]);
                let mut reply = received.reply;
                if login.active() {
                    if tokio::time::Instant::now() > login_until {
                        login.stop();
                    } else if let Some(step) = login.feed(&received.data) {
                        let mut typed = match step {
                            LoginStep::Username(u) => u.into_bytes(),
                            LoginStep::Password(p) => p.into_bytes(),
                        };
                        typed.push(b'\r');
                        reply.extend_from_slice(&codec.encode(&typed));
                    }
                }
                if !reply.is_empty() {
                    if let Err(e) = writer.write_all(&reply).await {
                        error = Some(e.to_string());
                        break;
                    }
                }
                let text = decoder.push(&received.data);
                if !text.is_empty() && app.emit(&data_event, text).is_err() {
                    break;
                }
            }
            cmd = commands.recv() => {
                let bytes = match cmd {
                    Some(Command::Input(data)) => codec.encode(&data),
                    Some(Command::Resize(cols, rows)) => codec.resize(cols, rows),
                    Some(Command::Close) | None => break,
                };
                if !bytes.is_empty() {
                    if let Err(e) = writer.write_all(&bytes).await {
                        error = Some(e.to_string());
                    }
                }
            }
        }
    }

    if let Some(tail) = decoder.flush() {
        let _ = app.emit(&data_event, tail);
    }
    if let Some(e) = &error {
        tracing::info!("Telnet {id}: connection ended: {e}");
    }
    let _ = writer.shutdown().await;
    let _ = app.emit(&exit_event, serde_json::json!({ "error": error }));
}
