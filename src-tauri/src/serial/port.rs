//! Open serial connections. The port is driven by two threads, since the
//! serialport API blocks: one reads with a short timeout and streams to the
//! terminal, one writes what is typed (and can hold a break).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use tauri::Emitter;

use super::options::{FlowControl, Parity, SerialOptions};

/// How long a read waits before checking whether the connection was closed.
const READ_TIMEOUT: Duration = Duration::from_millis(100);
/// How long a break holds the line (Cisco and Juniper want at least 250 ms).
const BREAK_LENGTH: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialConnectParams {
    pub id: String,
    /// `COM3`, `/dev/ttyUSB0`, `/dev/cu.usbserial-1410`.
    pub port: String,
    #[serde(default)]
    pub options: SerialOptions,
}

/// A port the system knows about, for the session editor's picker.
#[derive(Debug, Clone, Serialize)]
pub struct SerialPortInfo {
    pub name: String,
    /// What is plugged in, as far as the system says: "USB (FT232R, 0403:6001)".
    pub description: String,
}

enum Command {
    Input(Vec<u8>),
    Break,
    Close,
}

struct Connection {
    commands: mpsc::Sender<Command>,
    /// Set when a newer connection took this one's id: its events now
    /// belong to that one, so this one ends without reporting anything.
    replaced: Arc<AtomicBool>,
    /// The reader and the writer; the port is free once both have ended.
    threads: Vec<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
pub struct SerialManager {
    connections: HashMap<String, Connection>,
}

impl SerialManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run a port opened by [`open`] until it is closed or fails.
    pub fn start(&mut self, app: tauri::AppHandle, params: SerialConnectParams, port: Box<dyn SerialPort>) -> Result<(), String> {
        let writer = port.try_clone().map_err(|e| format!("Could not use {}: {e}", params.port))?;
        let (tx, rx) = mpsc::channel();
        let replaced = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None::<String>));
        let data_event = format!("serial-data-{}", params.id);
        let writer_thread = {
            let (app, stop, failure, data_event) = (app.clone(), stop.clone(), failure.clone(), data_event.clone());
            let options = params.options;
            let id = params.id.clone();
            std::thread::spawn(move || write_loop(app, id, writer, options, rx, stop, failure, data_event))
        };
        let id = params.id.clone();
        let reader_thread = {
            let replaced = replaced.clone();
            std::thread::spawn(move || read_loop(app, params.id, port, stop, failure, replaced, data_event))
        };
        let connection = Connection { commands: tx, replaced, threads: vec![writer_thread, reader_thread] };
        if let Some(old) = self.connections.insert(id, connection) {
            old.replaced.store(true, Ordering::SeqCst);
            let _ = old.commands.send(Command::Close);
        }
        Ok(())
    }

    pub fn send(&self, id: &str, data: Vec<u8>) -> Result<(), String> {
        self.command(id, Command::Input(data))
    }

    pub fn send_break(&self, id: &str) -> Result<(), String> {
        self.command(id, Command::Break)
    }

    /// Close the connection. Wait on what this returns (blocking, briefly)
    /// before opening the same port again.
    pub fn close(&mut self, id: &str) -> Vec<std::thread::JoinHandle<()>> {
        match self.connections.remove(id) {
            Some(connection) => {
                let _ = connection.commands.send(Command::Close);
                connection.threads
            }
            None => Vec::new(),
        }
    }

    fn command(&self, id: &str, cmd: Command) -> Result<(), String> {
        self.connections
            .get(id)
            .ok_or_else(|| format!("Serial connection not found: {id}"))?
            .commands
            .send(cmd)
            .map_err(|_| "The serial port is closed".to_string())
    }
}

/// Open the port with the session's settings. Blocking.
pub fn open(port: &str, options: &SerialOptions) -> Result<Box<dyn SerialPort>, String> {
    options.check()?;
    let data_bits = match options.data_bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    };
    let parity = match options.parity {
        Parity::None => serialport::Parity::None,
        Parity::Odd => serialport::Parity::Odd,
        Parity::Even => serialport::Parity::Even,
    };
    let stop_bits = if options.stop_bits == 2 { serialport::StopBits::Two } else { serialport::StopBits::One };
    let flow = match options.flow_control {
        FlowControl::None => serialport::FlowControl::None,
        FlowControl::Software => serialport::FlowControl::Software,
        FlowControl::Hardware => serialport::FlowControl::Hardware,
    };
    serialport::new(port, options.baud_rate)
        .data_bits(data_bits)
        .parity(parity)
        .stop_bits(stop_bits)
        .flow_control(flow)
        .timeout(READ_TIMEOUT)
        .open()
        .map_err(|e| open_error(port, e))
}

fn open_error(port: &str, e: serialport::Error) -> String {
    let denied = matches!(e.kind, serialport::ErrorKind::Io(std::io::ErrorKind::PermissionDenied))
        || e.description.contains("Permission denied");
    if denied && cfg!(target_os = "linux") {
        return format!(
            "Could not open {port}: permission denied. Add yourself to the group that owns it \
             (usually dialout: sudo usermod -aG dialout $USER), then sign out and in again"
        );
    }
    if matches!(e.kind, serialport::ErrorKind::NoDevice) {
        return format!("Could not open {port}: no such port, or another program is using it");
    }
    format!("Could not open {port}: {e}")
}

fn read_loop(
    app: tauri::AppHandle,
    id: String,
    mut port: Box<dyn SerialPort>,
    stop: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    replaced: Arc<AtomicBool>,
    data_event: String,
) {
    let mut decoder = crate::text::Utf8Stream::new();
    let mut buf = vec![0u8; 4096];
    while !stop.load(Ordering::SeqCst) {
        match port.read(&mut buf) {
            Ok(0) => continue,
            Ok(n) => {
                let text = decoder.push(&buf[..n]);
                if text.is_empty() {
                    continue;
                }
                crate::history::output(&id, &text);
                if app.emit(&data_event, text).is_err() {
                    break;
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) => {}
            Err(e) => {
                // Unplugged, mostly.
                failure.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(e.to_string());
                break;
            }
        }
    }
    stop.store(true, Ordering::SeqCst);
    let error = failure.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(e) = &error {
        tracing::info!("Serial {id}: ended: {e}");
    }
    if replaced.load(Ordering::SeqCst) {
        return;
    }
    if let Some(tail) = decoder.flush() {
        let _ = app.emit(&data_event, tail);
    }
    crate::history::stop(&id);
    let _ = app.emit(&format!("serial-exit-{id}"), serde_json::json!({ "error": error }));
}

#[allow(clippy::too_many_arguments)]
fn write_loop(
    app: tauri::AppHandle,
    id: String,
    mut port: Box<dyn SerialPort>,
    options: SerialOptions,
    commands: mpsc::Receiver<Command>,
    stop: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    data_event: String,
) {
    let fail = |e: String| {
        failure.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(e);
        stop.store(true, Ordering::SeqCst);
    };
    while !stop.load(Ordering::SeqCst) {
        let cmd = match commands.recv_timeout(READ_TIMEOUT) {
            Ok(cmd) => cmd,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match cmd {
            Command::Input(data) => {
                let bytes = options.encode(&data);
                if let Err(e) = port.write_all(&bytes).and_then(|_| port.flush()) {
                    fail(e.to_string());
                    break;
                }
                if options.local_echo {
                    let echo = String::from_utf8_lossy(&SerialOptions::echo(&data)).into_owned();
                    let _ = app.emit(&data_event, echo);
                }
            }
            Command::Break => {
                let held = port.set_break().and_then(|_| {
                    std::thread::sleep(BREAK_LENGTH);
                    port.clear_break()
                });
                if let Err(e) = held {
                    tracing::warn!("Serial {id}: break: {e}");
                }
            }
            Command::Close => break,
        }
    }
    stop.store(true, Ordering::SeqCst);
}

/// The ports the system knows about. A port that exists but is not listed
/// (some adapters, a pty) can still be typed into the editor by name.
pub fn list_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = serialport::available_ports().map_err(|e| format!("Could not list serial ports: {e}"))?;
    let mut infos: Vec<SerialPortInfo> = ports
        .into_iter()
        .map(|p| {
            let description = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    let name = [info.manufacturer.as_deref(), info.product.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" ");
                    let ids = format!("{:04x}:{:04x}", info.vid, info.pid);
                    if name.is_empty() { format!("USB {ids}") } else { format!("USB {name} ({ids})") }
                }
                serialport::SerialPortType::PciPort => "PCI".to_string(),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                serialport::SerialPortType::Unknown => String::new(),
            };
            SerialPortInfo { name: p.port_name, description }
        })
        .collect();
    infos.sort_by_key(|p| natural_key(&p.name));
    infos.dedup_by(|a, b| a.name == b.name);
    Ok(infos)
}

/// COM2 before COM10.
fn natural_key(name: &str) -> (String, u64) {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (stem, num) = name.split_at(name.len() - digits);
    (stem.to_string(), num.parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::natural_key;

    #[test]
    fn ports_sort_by_number() {
        let mut names = vec!["COM10", "COM2", "/dev/ttyUSB1", "/dev/ttyS0", "COM1"];
        names.sort_by_key(|n| natural_key(n));
        assert_eq!(names, vec!["/dev/ttyS0", "/dev/ttyUSB1", "COM1", "COM2", "COM10"]);
    }
}
