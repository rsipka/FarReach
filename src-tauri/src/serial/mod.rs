//! Serial port sessions: a local COM / tty port opened with the session's
//! line settings, output streamed to the terminal as `serial-data-{id}`
//! events and the end of the connection as `serial-exit-{id}`.
//!
//! The settings ([`options`]) are part of a saved session and are read on
//! every platform; opening a port ([`port`]) is desktop only, since neither
//! phone build can reach a serial port.

pub mod options;
#[cfg(desktop)]
pub mod port;
