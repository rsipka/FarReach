//! A session's terminal settings for SSH: the terminal type announced to
//! the server, what Backspace sends, and the character encoding the server
//! speaks (uniTerm's and PuTTY's Terminal Type, Backspace Key and Remote
//! character set).
//!
//! The terminal in the app always speaks UTF-8 and sends DEL for
//! Backspace; this turns that into what the server expects on the way out,
//! and its output back into UTF-8 on the way in.

use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};

/// The terminal type when the session sets none, matching xterm.js.
pub const DEFAULT_TERM: &str = "xterm-256color";

/// What the Backspace key sends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backspace {
    /// DEL (0x7f, ASCII 127), what xterm and OpenSSH send.
    #[default]
    Del,
    /// Ctrl+H (0x08), what some old devices and Windows consoles expect.
    CtrlH,
}

/// The settings as stored on the session. Every field unset is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOptions {
    /// `TERM` announced in the pty request (xterm-256color, vt100, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backspace: Option<Backspace>,
    /// A WHATWG encoding label (ISO-8859-2, windows-1250, ...); unset or
    /// unknown: UTF-8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

impl TerminalOptions {
    /// Drops what is empty, the defaults, a terminal type no server could
    /// take, and an encoding this build does not know. `None` when nothing
    /// is left.
    pub fn tidy(self) -> Option<Self> {
        let term = self.term.map(|t| t.trim().to_string()).filter(|t| valid_term(t) && t != DEFAULT_TERM);
        let backspace = self.backspace.filter(|b| *b != Backspace::Del);
        let encoding = self
            .encoding
            .and_then(|e| Encoding::for_label(e.trim().as_bytes()))
            .filter(|e| *e != UTF_8 && e.output_encoding() == *e)
            .map(|e| e.name().to_string());
        let t = Self { term, backspace, encoding };
        (t != Self::default()).then_some(t)
    }

    /// The terminal type for the pty request.
    pub fn term(&self) -> &str {
        self.term.as_deref().map(str::trim).filter(|t| valid_term(t)).unwrap_or(DEFAULT_TERM)
    }

    fn encoding(&self) -> &'static Encoding {
        self.encoding.as_deref().and_then(|e| Encoding::for_label(e.trim().as_bytes())).filter(|e| e.output_encoding() == *e).unwrap_or(UTF_8)
    }

    /// The decoder for the server's output.
    pub fn decoder(&self) -> OutputDecoder {
        match self.encoding() {
            e if e == UTF_8 => OutputDecoder::Utf8(crate::text::Utf8Stream::new()),
            e => OutputDecoder::Other(e.new_decoder_without_bom_handling()),
        }
    }

    /// The encoder for keystrokes and pastes.
    pub fn encoder(&self) -> InputEncoder {
        let e = self.encoding();
        InputEncoder {
            backspace: self.backspace.unwrap_or_default(),
            encoder: (e != UTF_8).then(|| e.new_encoder()),
            pending: crate::text::Utf8Stream::new(),
        }
    }
}

/// Printable ASCII without spaces, as terminfo names are.
fn valid_term(t: &str) -> bool {
    !t.is_empty() && t.len() <= 64 && t.bytes().all(|b| b.is_ascii_graphic())
}

/// The server's output as text.
pub enum OutputDecoder {
    Utf8(crate::text::Utf8Stream),
    Other(encoding_rs::Decoder),
}

impl OutputDecoder {
    /// Decode `chunk`, carrying an unfinished character to the next call.
    pub fn push(&mut self, chunk: &[u8]) -> String {
        match self {
            Self::Utf8(s) => s.push(chunk),
            Self::Other(d) => {
                let mut out = String::with_capacity(d.max_utf8_buffer_length(chunk.len()).unwrap_or(chunk.len() * 3));
                let _ = d.decode_to_string(chunk, &mut out, false);
                out
            }
        }
    }

    /// What is held when the stream ends mid-character, decoded lossily.
    pub fn flush(&mut self) -> Option<String> {
        match self {
            Self::Utf8(s) => s.flush(),
            Self::Other(d) => {
                let mut out = String::with_capacity(16);
                let _ = d.decode_to_string(b"", &mut out, true);
                (!out.is_empty()).then_some(out)
            }
        }
    }
}

/// What the terminal sends, as the server expects it.
pub struct InputEncoder {
    backspace: Backspace,
    encoder: Option<encoding_rs::Encoder>,
    /// UTF-8 split across two sends, held until it is whole.
    pending: crate::text::Utf8Stream,
}

impl InputEncoder {
    pub fn encode(&mut self, data: Vec<u8>) -> Vec<u8> {
        let data = match self.backspace {
            Backspace::CtrlH => data.into_iter().map(|b| if b == 0x7f { 0x08 } else { b }).collect(),
            Backspace::Del => data,
        };
        let Some(enc) = self.encoder.as_mut() else {
            return data;
        };
        let text = self.pending.push(&data);
        let mut out = Vec::with_capacity(enc.max_buffer_length_from_utf8_if_no_unmappables(text.len()).unwrap_or(text.len() * 2));
        // A character the server's encoding lacks goes as an HTML entity by
        // encoding_rs' rule; a "?" is what other terminals send.
        let mut rest = text.as_str();
        loop {
            let (res, read) = enc.encode_from_utf8_to_vec_without_replacement(rest, &mut out, false);
            rest = &rest[read..];
            match res {
                encoding_rs::EncoderResult::InputEmpty => break,
                encoding_rs::EncoderResult::OutputFull => out.reserve(rest.len() * 2 + 16),
                encoding_rs::EncoderResult::Unmappable(_) => out.push(b'?'),
            }
        }
        out
    }
}

/// The encodings the session editor offers (sessions.ts
/// `TERMINAL_ENCODINGS`), each of which must work both ways.
#[cfg(test)]
const ENCODINGS: &[&str] = &[
    "UTF-8", "ISO-8859-2", "ISO-8859-7", "ISO-8859-15", "windows-1250", "windows-1251", "windows-1252", "windows-1253", "KOI8-R", "KOI8-U", "IBM866",
    "Shift_JIS", "EUC-JP", "EUC-KR", "GBK", "gb18030", "Big5",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(term: Option<&str>, backspace: Option<Backspace>, encoding: Option<&str>) -> TerminalOptions {
        TerminalOptions { term: term.map(Into::into), backspace, encoding: encoding.map(Into::into) }
    }

    #[test]
    fn defaults_tidy_away() {
        assert_eq!(opts(Some(" xterm-256color "), Some(Backspace::Del), Some("utf8")).tidy(), None);
        assert_eq!(opts(Some("vt 100"), None, Some("nonsense")).tidy(), None);
        let t = opts(Some(" vt100 "), Some(Backspace::CtrlH), Some("latin2")).tidy().unwrap();
        assert_eq!(t, opts(Some("vt100"), Some(Backspace::CtrlH), Some("ISO-8859-2")));
        // UTF-16 decodes but cannot be sent: refused.
        assert_eq!(opts(None, None, Some("utf-16le")).tidy(), None);
    }

    #[test]
    fn term_falls_back_to_xterm() {
        assert_eq!(TerminalOptions::default().term(), DEFAULT_TERM);
        assert_eq!(opts(Some("vt220"), None, None).term(), "vt220");
        assert_eq!(opts(Some("bad\nterm"), None, None).term(), DEFAULT_TERM);
    }

    #[test]
    fn backspace_ctrl_h() {
        let mut e = opts(None, Some(Backspace::CtrlH), None).encoder();
        assert_eq!(e.encode(b"ab\x7f".to_vec()), b"ab\x08");
        let mut e = TerminalOptions::default().encoder();
        assert_eq!(e.encode(b"\x7f".to_vec()), b"\x7f");
    }

    #[test]
    fn latin2_both_ways() {
        let o = opts(None, None, Some("ISO-8859-2"));
        // "žluť" in ISO-8859-2.
        let wire = [0xbe, b'l', b'u', 0xbb];
        let mut d = o.decoder();
        assert_eq!(d.push(&wire[..2]) + &d.push(&wire[2..]), "žluť");
        let mut e = o.encoder();
        // A character split across two sends still arrives whole.
        let bytes = "žluť".as_bytes();
        let mut sent = e.encode(bytes[..1].to_vec());
        sent.extend(e.encode(bytes[1..].to_vec()));
        assert_eq!(sent, wire);
        // Unmappable: a question mark.
        assert_eq!(e.encode("€x".as_bytes().to_vec()), b"?x");
    }

    #[test]
    fn every_offered_encoding_works_both_ways() {
        for e in ENCODINGS {
            let t = opts(None, None, Some(e)).tidy();
            if *e == "UTF-8" {
                assert_eq!(t, None);
            } else {
                assert_eq!(t.and_then(|t| t.encoding).as_deref(), Some(*e), "{e}");
            }
        }
    }

    #[test]
    fn utf8_passes_through() {
        let o = TerminalOptions::default();
        let mut e = o.encoder();
        assert_eq!(e.encode("ž".as_bytes().to_vec()), "ž".as_bytes());
        let mut d = o.decoder();
        assert_eq!(d.push("ž".as_bytes()), "ž");
    }
}
