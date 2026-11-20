//! Spotting the start of a ZMODEM session in terminal output.
//!
//! Both programs open with a hex header: `**` ZDLE `B`, then the frame type
//! as two hex digits. `sz` (the remote sends) starts with ZRQINIT, `00`;
//! `rz` (the remote receives) starts with ZRINIT, `01`. lrzsz writes `rz\r`
//! or "rz waiting to receive." just before it, which is shown as usual.

/// Which way the files go, from this side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// The remote ran `rz`: files go from here to there.
    Upload,
    /// The remote ran `sz`: files come here.
    Download,
}

/// What [`Detector::scan`] found in a chunk.
#[derive(Debug, PartialEq, Eq)]
pub struct Scan {
    /// Bytes for the terminal: everything before the header.
    pub display: Vec<u8>,
    /// A session starts: its direction and the header bytes onward.
    pub start: Option<(Direction, Vec<u8>)>,
}

/// `*` ZDLE `B` `0`, then `0` (ZRQINIT) or `1` (ZRINIT).
const MARK: &[u8] = b"*\x18B0";

/// Watches output as it streams by. A header split across two reads is
/// still found: the bytes that may begin one (from the ZDLE on) are held
/// back until the next read says what they are. A lone `*` is never held,
/// so what is typed at a prompt shows at once.
#[derive(Debug, Default)]
pub struct Detector {
    held: Vec<u8>,
    /// How many of the held bytes were shown already (the `*` below).
    held_shown: usize,
    /// The last byte shown was a `*`, which may be the header's own.
    star: bool,
}

impl Detector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&mut self, chunk: &[u8]) -> Scan {
        let mut buf = std::mem::take(&mut self.held);
        let mut skip = std::mem::take(&mut self.held_shown);
        // Put back the `*` already shown, so a header split right after it
        // matches like any other; it is not shown twice.
        if self.star && buf.is_empty() && chunk.first() == Some(&0x18) {
            buf.push(b'*');
            skip = 1;
        }
        buf.extend_from_slice(chunk);

        if let Some(i) = find_header(&buf) {
            let direction = if buf[i + MARK.len()] == b'0' { Direction::Download } else { Direction::Upload };
            self.star = false;
            return Scan { display: buf[skip.min(i)..i].to_vec(), start: Some((direction, buf[i..].to_vec())) };
        }

        // Hold a tail that could still become a header: `*` ZDLE onward.
        let hold_from = (2..=MARK.len())
            .rev()
            .filter(|&n| n <= buf.len())
            .map(|n| buf.len() - n)
            .find(|&at| MARK.starts_with(&buf[at..]));
        let shown_end = hold_from.unwrap_or(buf.len());
        let display = buf[skip.min(shown_end)..shown_end].to_vec();
        if let Some(at) = hold_from {
            self.held_shown = skip.saturating_sub(at);
            self.held = buf[at..].to_vec();
            self.star = false;
        } else if let Some(&last) = buf.last() {
            self.star = last == b'*';
        }
        Scan { display, start: None }
    }

    /// Whatever is held back and not shown yet, for when the stream ends.
    pub fn flush(&mut self) -> Vec<u8> {
        self.star = false;
        let held = std::mem::take(&mut self.held);
        held[std::mem::take(&mut self.held_shown).min(held.len())..].to_vec()
    }
}

/// Where a full header start (`*` ZDLE `B0` then `0` or `1`) begins.
fn find_header(buf: &[u8]) -> Option<usize> {
    buf.windows(MARK.len() + 1)
        .position(|w| w.starts_with(MARK) && matches!(w[MARK.len()], b'0' | b'1'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SZ: &[u8] = b"rz\r**\x18B00000000000000\r\x8a\x11";
    const RZ: &[u8] = b"rz waiting to receive.**\x18B0100000023be50\r\x8a\x11";

    #[test]
    fn plain_output_passes() {
        let mut d = Detector::new();
        let s = d.scan(b"hello * world\r\n");
        assert_eq!(s.display, b"hello * world\r\n");
        assert!(s.start.is_none());
    }

    #[test]
    fn finds_sz() {
        let mut d = Detector::new();
        let s = d.scan(SZ);
        assert_eq!(s.display, b"rz\r*");
        let (dir, rest) = s.start.unwrap();
        assert_eq!(dir, Direction::Download);
        assert!(rest.starts_with(b"*\x18B00"));
    }

    #[test]
    fn finds_rz() {
        let mut d = Detector::new();
        let s = d.scan(RZ);
        assert_eq!(s.display, b"rz waiting to receive.*");
        assert_eq!(s.start.unwrap().0, Direction::Upload);
    }

    #[test]
    fn finds_a_header_split_anywhere() {
        for cut in 1..SZ.len() {
            let mut d = Detector::new();
            let a = d.scan(&SZ[..cut]);
            let mut shown = a.display;
            // Once a header is found the tap stops scanning.
            let start = match a.start {
                Some(s) => Some(s),
                None => {
                    let b = d.scan(&SZ[cut..]);
                    shown.extend(b.display);
                    b.start
                }
            };
            assert_eq!(start.map(|s| s.0), Some(Direction::Download), "cut at {cut}");
            // The header's own `*` may show too, when a read ends on it.
            while shown.len() > 3 && shown.ends_with(b"*") {
                shown.pop();
            }
            assert_eq!(shown, b"rz\r", "cut at {cut}");
        }
    }

    #[test]
    fn a_star_typed_shows_at_once() {
        let mut d = Detector::new();
        assert_eq!(d.scan(b"*").display, b"*");
        assert_eq!(d.scan(b"*").display, b"*");
        assert_eq!(d.scan(b"x").display, b"x");
    }

    #[test]
    fn a_held_tail_that_is_not_a_header_comes_out() {
        let mut d = Detector::new();
        assert_eq!(d.scan(b"ab*\x18").display, b"ab");
        assert_eq!(d.scan(b"Q").display, b"*\x18Q");
    }

    #[test]
    fn a_header_split_after_its_star_and_zdle() {
        let mut d = Detector::new();
        assert_eq!(d.scan(b"rz\r**").display, b"rz\r**");
        assert_eq!(d.scan(b"\x18").display, b"");
        let s = d.scan(b"B00000000000000\r\x8a\x11");
        assert!(s.display.is_empty());
        assert_eq!(s.start.unwrap().0, Direction::Download);
    }

    #[test]
    fn flush_gives_back_only_what_was_not_shown() {
        let mut d = Detector::new();
        assert_eq!(d.scan(b"a*").display, b"a*");
        assert_eq!(d.scan(b"\x18B").display, b"");
        assert_eq!(d.flush(), b"\x18B");
    }

    #[test]
    fn other_frame_types_are_not_a_start() {
        let mut d = Detector::new();
        let s = d.scan(b"**\x18B0800000000022d\r\n");
        assert!(s.start.is_none());
    }
}
