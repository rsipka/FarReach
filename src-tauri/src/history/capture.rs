//! Turning one terminal's traffic into history: the commands run and the
//! output printed.
//!
//! Raw keystrokes are never stored. Output goes through the same VT parser
//! as the file log (`vte`) into a one-line screen that applies carriage
//! returns, backspaces and line erases the way a terminal does, so a line
//! reads as it looked. Typed keys are only used to find where a command
//! starts and whether it was echoed:
//!
//! - When a key is typed on a fresh line, the text left of the cursor is the
//!   prompt.
//! - At Enter the line is judged once the terminal moves to the next line
//!   (by then the echo has arrived, however slow the link). What follows the
//!   prompt on that line is the command.
//! - A line whose typed keys never came back is hidden input (a password at
//!   a `sudo` or `ssh` prompt) and is dropped. So is anything typed after a
//!   prompt that asks for a password, a PIN or a code, echoed or not.
//! - Shell integration marks (OSC 133, OSC 633, OSC 7), when the shell sends
//!   them, give the exact command, its folder and its exit code.
//! - Full-screen programs (the alternate screen: vim, less, top) are left
//!   out entirely: their keys are not commands and their screens not a log.

use crate::history::scrub::{is_secret_prompt, Scrubber, MASK};

/// A block of output is cut once it holds this much text.
pub const BLOCK_BYTES: usize = 4096;
/// A block is cut after this long even if small, so a crash loses little.
pub const BLOCK_MS: i64 = 30_000;
/// An Enter not followed by a new line within this long is let go: nothing
/// is stored for it.
pub const ENTER_MS: i64 = 5_000;
/// A screen line longer than this is cut (a binary dump with no newline).
const MAX_LINE: usize = 16 * 1024;

/// What the capture hands on to be stored.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Command { at: i64, text: String, cwd: Option<String>, redacted: bool },
    /// The exit code of the last command, from shell integration.
    Exit { code: i64 },
    Output { first: i64, last: i64, text: String, redacted: bool },
}

/// The keys typed on the current line.
#[derive(Default)]
struct Typed {
    /// Printable characters, after backspaces.
    text: String,
    /// The screen text left of the cursor when the first key came, in
    /// characters; `None` until a key is typed.
    prompt: Option<String>,
    /// Keys that change the line in ways the text above cannot follow
    /// (arrows, Tab, history recall): the echo cannot be compared.
    edited: bool,
}

/// An Enter waiting for its line to end on screen.
struct Pending {
    at: i64,
    typed: Typed,
}

#[derive(Default)]
struct Screen {
    line: Vec<char>,
    cursor: usize,
}

impl Screen {
    fn put(&mut self, c: char) {
        if self.cursor < self.line.len() {
            self.line[self.cursor] = c;
        } else if self.line.len() < MAX_LINE {
            while self.line.len() < self.cursor {
                self.line.push(' ');
            }
            self.line.push(c);
        }
        self.cursor += 1;
    }

    fn text(&self) -> String {
        self.line.iter().collect::<String>()
    }

    fn left_of_cursor(&self) -> String {
        self.line[..self.cursor.min(self.line.len())].iter().collect()
    }
}

/// The parser's side: what the output does to the screen.
#[derive(Default)]
struct Perf {
    screen: Screen,
    /// Lines finished since the last call, in order.
    done: Vec<String>,
    alt: bool,
    /// Entered or left the alternate screen during the last call.
    alt_changed: bool,
    /// From OSC 633;E: the command line the shell is about to run.
    osc_command: Option<String>,
    /// From OSC 133;B: where the prompt ends, in characters.
    osc_prompt_end: Option<usize>,
    /// From OSC 7 or OSC 633;P;Cwd=: the shell's folder.
    cwd: Option<String>,
    /// From OSC 133;D;code.
    exits: Vec<i64>,
}

fn param(params: &vte::Params, i: usize, default: usize) -> usize {
    params
        .iter()
        .nth(i)
        .and_then(|p| p.first().copied())
        .filter(|v| *v != 0)
        .map(|v| v as usize)
        .unwrap_or(default)
}

/// OSC 633's escaping: `\\` is a backslash, `\xAB` a byte.
fn unescape_633(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            if b[i + 1] == b'\\' {
                out.push(b'\\');
                i += 2;
                continue;
            }
            if b[i + 1] == b'x' && i + 3 < b.len() {
                if let Ok(v) = u8::from_str_radix(&s[i + 2..i + 4], 16) {
                    out.push(v);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl vte::Perform for Perf {
    fn print(&mut self, c: char) {
        if !self.alt {
            self.screen.put(c);
        }
    }

    fn execute(&mut self, byte: u8) {
        if self.alt {
            return;
        }
        match byte {
            b'\r' => self.screen.cursor = 0,
            b'\n' => {
                let line = std::mem::take(&mut self.screen);
                self.done.push(line.text().trim_end().to_string());
            }
            0x08 => self.screen.cursor = self.screen.cursor.saturating_sub(1),
            b'\t' => {
                let next = (self.screen.cursor / 8 + 1) * 8;
                while self.screen.cursor < next {
                    self.screen.put(' ');
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, intermediates: &[u8], _ignore: bool, action: char) {
        if intermediates == b"?" {
            if matches!(action, 'h' | 'l') && params.iter().any(|p| matches!(p.first(), Some(1049 | 1047 | 47))) {
                let on = action == 'h';
                if on != self.alt {
                    self.alt = on;
                    self.alt_changed = true;
                    if on {
                        self.screen = Screen::default();
                    }
                }
            }
            return;
        }
        if self.alt || !intermediates.is_empty() {
            return;
        }
        let s = &mut self.screen;
        match action {
            'K' => match param(params, 0, 0) {
                0 => s.line.truncate(s.cursor),
                1 => {
                    for c in s.line.iter_mut().take(s.cursor + 1) {
                        *c = ' ';
                    }
                }
                _ => s.line.clear(),
            },
            'C' => s.cursor += param(params, 0, 1),
            'D' => s.cursor = s.cursor.saturating_sub(param(params, 0, 1)),
            'G' => s.cursor = param(params, 0, 1) - 1,
            'P' => {
                let n = param(params, 0, 1);
                let end = (s.cursor + n).min(s.line.len());
                if s.cursor < end {
                    s.line.drain(s.cursor..end);
                }
            }
            '@' => {
                let n = param(params, 0, 1);
                if s.cursor <= s.line.len() {
                    for _ in 0..n {
                        s.line.insert(s.cursor, ' ');
                    }
                }
            }
            // Clearing the screen or moving to another row: the line on
            // screen is no longer one line of output.
            'J' if param(params, 0, 0) >= 2 => *s = Screen::default(),
            'H' | 'f' => s.cursor = param(params, 1, 1) - 1,
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let get = |i: usize| params.get(i).map(|p| String::from_utf8_lossy(p).into_owned());
        match params.first().copied() {
            Some(b"133") => match get(1).as_deref() {
                Some("A") => self.osc_prompt_end = None,
                Some("B") => self.osc_prompt_end = Some(self.screen.cursor),
                Some("D") => {
                    if let Some(code) = get(2).and_then(|c| c.parse().ok()) {
                        self.exits.push(code);
                    }
                }
                _ => {}
            },
            Some(b"633") => match get(1).as_deref() {
                Some("A") => self.osc_prompt_end = None,
                Some("B") => self.osc_prompt_end = Some(self.screen.cursor),
                Some("D") => {
                    if let Some(code) = get(2).and_then(|c| c.parse().ok()) {
                        self.exits.push(code);
                    }
                }
                // The command line may contain `;`, which split it.
                Some("E") => {
                    let parts: Vec<String> = params[2..].iter().map(|p| String::from_utf8_lossy(p).into_owned()).collect();
                    let cmd = parts.first().map(|c| unescape_633(c)).unwrap_or_default();
                    self.osc_command = Some(cmd);
                }
                Some("P") => {
                    if let Some(cwd) = get(2).and_then(|p| p.strip_prefix("Cwd=").map(unescape_633)) {
                        self.cwd = Some(cwd);
                    }
                }
                _ => {}
            },
            Some(b"7") => {
                // file://host/path, percent-encoded.
                if let Some(url) = get(1) {
                    if let Some(rest) = url.strip_prefix("file://") {
                        let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
                        self.cwd = Some(percent_decode(path));
                    }
                }
            }
            _ => {}
        }
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One terminal's capture.
pub struct Capture {
    parser: vte::Parser,
    perf: Perf,
    scrub: Scrubber,
    typed: Typed,
    pending: Option<Pending>,
    /// Inside an escape sequence the user's keys sent (an arrow key).
    in_key_escape: bool,
    /// Inside a bracketed paste: keys are typed text, not editing.
    in_paste: bool,
    block: String,
    block_first: i64,
    block_last: i64,
    block_redacted: bool,
    /// Between a private key's BEGIN and END lines.
    in_private_key: bool,
}

impl Capture {
    pub fn new(scrub: Scrubber) -> Capture {
        Capture {
            parser: vte::Parser::new(),
            perf: Perf::default(),
            scrub,
            typed: Typed::default(),
            pending: None,
            in_key_escape: false,
            in_paste: false,
            block: String::new(),
            block_first: 0,
            block_last: 0,
            block_redacted: false,
            in_private_key: false,
        }
    }

    /// Output, as it is about to be shown.
    pub fn output(&mut self, now: i64, text: &str) -> Vec<Event> {
        let mut events = Vec::new();
        self.parser.advance(&mut self.perf, text.as_bytes());
        let lines = std::mem::take(&mut self.perf.done);
        for line in lines {
            if let Some(p) = self.pending.take() {
                self.judge(&mut events, p, &line);
            } else if !self.typed.text.is_empty() || self.typed.edited {
                // A line ended without Enter (the output wrapped, or the
                // program printed while the user typed): start over.
                self.typed = Typed::default();
            }
            self.add_line(&mut events, now, &line);
        }
        for code in self.perf.exits.drain(..) {
            events.push(Event::Exit { code });
        }
        // Lines finished before a full-screen program started are judged
        // above; keys typed for the program are not a command.
        if self.perf.alt_changed {
            self.perf.alt_changed = false;
            self.typed = Typed::default();
            self.pending = None;
        }
        events
    }

    /// Keys the user sent to the terminal. Nothing is stored from them
    /// directly; a command is stored once its line ends on screen.
    pub fn input(&mut self, now: i64, data: &[u8]) -> Vec<Event> {
        let events = Vec::new();
        if self.perf.alt {
            return events;
        }
        let text = String::from_utf8_lossy(data);
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if self.in_key_escape {
                // CSI/SS3 parameters, up to the final byte.
                if c == '[' || c == 'O' || c.is_ascii_digit() || c == ';' || c == '?' {
                    continue;
                }
                self.in_key_escape = false;
                continue;
            }
            match c {
                '\x1b' => {
                    let rest: String = chars.clone().take(5).collect();
                    if rest.starts_with("[200~") {
                        self.in_paste = true;
                        for _ in 0..5 {
                            chars.next();
                        }
                    } else if rest.starts_with("[201~") {
                        self.in_paste = false;
                        for _ in 0..5 {
                            chars.next();
                        }
                    } else {
                        self.mark_prompt();
                        self.typed.edited = true;
                        self.in_key_escape = true;
                    }
                }
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        // A \r\n pair is one Enter.
                        chars.next();
                    }
                    let typed = std::mem::take(&mut self.typed);
                    if typed.prompt.is_some() || typed.edited || self.perf.osc_command.is_some() {
                        self.pending = Some(Pending { at: now, typed });
                    }
                }
                '\x7f' | '\x08' => {
                    self.typed.text.pop();
                }
                // Ctrl+C, Ctrl+U, Ctrl+D: the line is thrown away.
                '\x03' | '\x15' | '\x04' => self.typed = Typed::default(),
                '\t' => {
                    self.mark_prompt();
                    self.typed.edited = true;
                }
                c if c.is_control() => {}
                c => {
                    self.mark_prompt();
                    self.typed.text.push(c);
                }
            }
        }
        events
    }

    /// The first key of a line: what is left of the cursor is the prompt.
    fn mark_prompt(&mut self) {
        if self.typed.prompt.is_none() {
            self.typed.prompt = Some(match self.perf.osc_prompt_end {
                Some(end) => self.perf.screen.line[..end.min(self.perf.screen.line.len())].iter().collect(),
                None => self.perf.screen.left_of_cursor(),
            });
        }
    }

    /// Time passing: cuts an old block, lets go of an Enter that never
    /// reached a new line.
    pub fn tick(&mut self, now: i64) -> Vec<Event> {
        let mut events = Vec::new();
        if self.pending.as_ref().is_some_and(|p| now - p.at > ENTER_MS) {
            self.pending = None;
        }
        if !self.block.is_empty() && now - self.block_first > BLOCK_MS {
            self.flush(&mut events);
        }
        events
    }

    /// The terminal closed: what is left goes out.
    pub fn finish(&mut self, now: i64) -> Vec<Event> {
        let mut events = Vec::new();
        let rest = self.perf.screen.text();
        let rest = rest.trim_end();
        if !rest.is_empty() {
            let rest = rest.to_string();
            self.add_line(&mut events, now, &rest);
        }
        self.flush(&mut events);
        events
    }

    /// Decides what the Enter in `p` ran, now that its line is `line`.
    fn judge(&mut self, events: &mut Vec<Event>, p: Pending, line: &str) {
        let osc = self.perf.osc_command.take();
        let prompt = p.typed.prompt.clone().unwrap_or_default();
        if is_secret_prompt(&prompt) {
            return;
        }
        let cmd = match osc {
            Some(cmd) => cmd,
            None => {
                let start = prompt.chars().count();
                let after: String = line.chars().skip(start).collect();
                let after = after.trim().to_string();
                let typed = p.typed.text.trim();
                // Not echoed: hidden input.
                if !p.typed.edited && !typed.is_empty() && !after.contains(typed) {
                    return;
                }
                after
            }
        };
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }
        // Output before the command belongs before it.
        self.flush(events);
        let (text, redacted) = self.scrub.scrub(cmd);
        events.push(Event::Command { at: p.at, text, cwd: self.perf.cwd.clone(), redacted });
    }

    fn add_line(&mut self, events: &mut Vec<Event>, now: i64, line: &str) {
        let (line, mut redacted) = if self.in_private_key {
            if line.contains("-----END") && line.contains("PRIVATE KEY-----") {
                self.in_private_key = false;
            }
            return;
        } else if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
            self.in_private_key = !line.contains("-----END");
            (format!("-----BEGIN PRIVATE KEY----- {MASK}"), true)
        } else {
            self.scrub.scrub(line)
        };
        if self.block.is_empty() {
            self.block_first = now;
        } else {
            self.block.push('\n');
        }
        redacted |= self.block_redacted;
        self.block.push_str(&line);
        self.block_last = now;
        self.block_redacted = redacted;
        if self.block.len() >= BLOCK_BYTES {
            self.flush(events);
        }
    }

    fn flush(&mut self, events: &mut Vec<Event>) {
        if self.block.trim().is_empty() {
            self.block.clear();
            self.block_redacted = false;
            return;
        }
        events.push(Event::Output {
            first: self.block_first,
            last: self.block_last,
            text: std::mem::take(&mut self.block),
            redacted: std::mem::take(&mut self.block_redacted),
        });
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
