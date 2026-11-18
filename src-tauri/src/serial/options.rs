//! A serial session's line settings and what the keyboard sends. Pure, so
//! it is read on every platform and tested without a port.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    #[default]
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowControl {
    #[default]
    None,
    /// XON/XOFF.
    Software,
    /// RTS/CTS.
    Hardware,
}

/// What the Enter key sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialNewline {
    /// CR, what a terminal sends and what consoles expect (PuTTY's default).
    #[default]
    Cr,
    Lf,
    Crlf,
}

/// What the Backspace key sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialBackspace {
    /// DEL (0x7f), as the terminal sends it.
    #[default]
    Del,
    /// Ctrl+H (0x08).
    CtrlH,
}

fn default_baud() -> u32 {
    9600
}

fn default_data_bits() -> u8 {
    8
}

fn default_stop_bits() -> u8 {
    1
}

/// A serial session's settings, stored with the session. The port itself
/// is the session's `host` (`COM3`, `/dev/ttyUSB0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialOptions {
    #[serde(default = "default_baud")]
    pub baud_rate: u32,
    /// 5 to 8.
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default)]
    pub parity: Parity,
    /// 1 or 2.
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    #[serde(default)]
    pub flow_control: FlowControl,
    #[serde(default)]
    pub newline: SerialNewline,
    #[serde(default)]
    pub backspace: SerialBackspace,
    /// Show what is typed: for devices that do not echo it back.
    #[serde(default)]
    pub local_echo: bool,
}

impl Default for SerialOptions {
    fn default() -> Self {
        Self {
            baud_rate: default_baud(),
            data_bits: default_data_bits(),
            parity: Parity::None,
            stop_bits: default_stop_bits(),
            flow_control: FlowControl::None,
            newline: SerialNewline::Cr,
            backspace: SerialBackspace::Del,
            local_echo: false,
        }
    }
}

impl SerialOptions {
    /// Settings a port can be opened with, or why not.
    pub fn check(&self) -> Result<(), String> {
        if self.baud_rate == 0 || self.baud_rate > 12_000_000 {
            return Err(format!("Unsupported speed: {} baud", self.baud_rate));
        }
        if !(5..=8).contains(&self.data_bits) {
            return Err(format!("Unsupported data bits: {}", self.data_bits));
        }
        if !(1..=2).contains(&self.stop_bits) {
            return Err(format!("Unsupported stop bits: {}", self.stop_bits));
        }
        Ok(())
    }

    /// What goes out on the line for keys typed in the terminal.
    pub fn encode(&self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len() + 2);
        for &b in input {
            match b {
                b'\r' => match self.newline {
                    SerialNewline::Cr => out.push(b'\r'),
                    SerialNewline::Lf => out.push(b'\n'),
                    SerialNewline::Crlf => out.extend_from_slice(b"\r\n"),
                },
                0x7f if self.backspace == SerialBackspace::CtrlH => out.push(0x08),
                _ => out.push(b),
            }
        }
        out
    }

    /// What the terminal shows for keys typed, with local echo on: Enter
    /// moves to a new line and Backspace erases the character before.
    pub fn echo(input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len() + 2);
        for &b in input {
            match b {
                b'\r' => out.extend_from_slice(b"\r\n"),
                0x7f | 0x08 => out.extend_from_slice(b"\x08 \x08"),
                _ => out.push(b),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_9600_8n1() {
        let o: SerialOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(o, SerialOptions::default());
        assert_eq!((o.baud_rate, o.data_bits, o.parity, o.stop_bits), (9600, 8, Parity::None, 1));
        assert_eq!(o.flow_control, FlowControl::None);
    }

    #[test]
    fn stored_as_snake_case_lowercase() {
        let o = SerialOptions {
            baud_rate: 115200,
            parity: Parity::Even,
            flow_control: FlowControl::Hardware,
            backspace: SerialBackspace::CtrlH,
            ..Default::default()
        };
        let json = serde_json::to_value(o).unwrap();
        assert_eq!(json["baud_rate"], 115200);
        assert_eq!(json["parity"], "even");
        assert_eq!(json["flow_control"], "hardware");
        assert_eq!(json["backspace"], "ctrlh");
        assert_eq!(serde_json::from_value::<SerialOptions>(json).unwrap(), o);
    }

    #[test]
    fn enter_and_backspace() {
        let mut o = SerialOptions::default();
        assert_eq!(o.encode(b"ab\r\x7f"), b"ab\r\x7f");
        o.newline = SerialNewline::Crlf;
        o.backspace = SerialBackspace::CtrlH;
        assert_eq!(o.encode(b"a\r\x7f"), b"a\r\n\x08");
        o.newline = SerialNewline::Lf;
        assert_eq!(o.encode(b"\r"), b"\n");
    }

    #[test]
    fn echo_shows_typing() {
        assert_eq!(SerialOptions::echo(b"ok\r"), b"ok\r\n");
        assert_eq!(SerialOptions::echo(b"\x7f"), b"\x08 \x08");
    }

    #[test]
    fn check_rejects_odd_settings() {
        assert!(SerialOptions::default().check().is_ok());
        assert!(SerialOptions { data_bits: 9, ..Default::default() }.check().is_err());
        assert!(SerialOptions { stop_bits: 3, ..Default::default() }.check().is_err());
        assert!(SerialOptions { baud_rate: 0, ..Default::default() }.check().is_err());
    }
}
