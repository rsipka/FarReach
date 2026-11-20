//! Which certificates a web session accepts.
//!
//! A certificate the system trusts is accepted as any browser would; this
//! is only asked about the others (a device's self-signed certificate, an
//! expired one, one for another name). Such a certificate is accepted when
//! the session trusts its SHA-256 fingerprint, and otherwise the page is not
//! loaded and the app asks, showing the fingerprint, and says so when the
//! session trusted a different one before (the device's certificate was
//! replaced, or someone is in between). Trust is by fingerprint, not by
//! host: whoever presents that exact certificate holds its private key.
//!
//! The engines differ in how they let the app decide (see `platform_*`),
//! but each asks this module, and none is told to ignore errors in general.

use serde::Serialize;

/// What to do with a certificate the system does not trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The session trusts it: load the page.
    Allow,
    /// Not trusted: do not load, ask the user. `changed`: the session
    /// trusted other certificates before, and this is none of them.
    Ask { changed: bool },
}

pub fn decide(trusted: &[String], had_any: bool, fingerprint: &str) -> Decision {
    if trusted.iter().any(|t| t.eq_ignore_ascii_case(fingerprint)) {
        Decision::Allow
    } else {
        Decision::Ask { changed: had_any }
    }
}

/// Sent to the main window (`web-cert-ask`) to ask about a certificate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertAsk {
    /// The web window that met it.
    pub label: String,
    pub session_id: String,
    pub session_name: String,
    /// The server that presented it.
    pub host: String,
    pub fingerprint: String,
    pub changed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trusted_fingerprint_loads() {
        let t = vec!["AB:CD".to_string()];
        assert_eq!(decide(&t, true, "ab:cd"), Decision::Allow);
    }

    #[test]
    fn an_unknown_one_asks_and_says_if_it_changed() {
        assert_eq!(decide(&[], false, "AB:CD"), Decision::Ask { changed: false });
        assert_eq!(decide(&["11:22".to_string()], true, "AB:CD"), Decision::Ask { changed: true });
    }
}
