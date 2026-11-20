//! WebView2: certificates and HTTP sign-in for web windows.
//!
//! WebView2 reports a certificate it does not trust with
//! `ServerCertificateErrorDetected`. If the session trusts its fingerprint,
//! the answer is `AlwaysAllow`, which WebView2 remembers for that browser
//! process only: each session has its own (its own data folder), so this
//! reaches no other session and not the app. Otherwise the load is
//! cancelled and the user asked; trusting reloads the page.

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_10, ICoreWebView2_14, COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW,
    COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL,
};
use webview2_com::{take_pwstr, BasicAuthenticationRequestedEventHandler, ServerCertificateErrorDetectedEventHandler};
use windows::core::{Interface, HSTRING, PWSTR};

/// The DER bytes inside a PEM certificate.
fn pem_to_der(pem: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let body: String = pem
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD.decode(body).ok()
}

fn host_of(uri: &str) -> String {
    url::Url::parse(uri).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
}

pub(super) fn install(win: &WebviewWindow, label: String) {
    let result = win.with_webview(move |pv| unsafe {
        let Ok(core) = pv.controller().CoreWebView2() else { return };
        if let Ok(core14) = core.cast::<ICoreWebView2_14>() {
            let l = label.clone();
            let handler = ServerCertificateErrorDetectedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = PWSTR::null();
                args.RequestUri(&mut uri)?;
                let uri = take_pwstr(uri);
                let cert = args.ServerCertificate()?;
                let mut pem = PWSTR::null();
                cert.ToPemEncoding(&mut pem)?;
                let pem = take_pwstr(pem);
                let allow = pem_to_der(&pem)
                    .map(|der| super::window::certificate_error(&l, &host_of(&uri), &super::fingerprint(&der)))
                    .unwrap_or(false);
                args.SetAction(if allow {
                    COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW
                } else {
                    COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL
                })?;
                Ok(())
            }));
            let mut token = 0i64;
            if let Err(e) = core14.add_ServerCertificateErrorDetected(&handler, &mut token) {
                tracing::warn!("Web session: could not watch certificates: {e}");
            }
        } else {
            // WebView2 older than 1.0.1245: its default (refuse) stands.
            tracing::warn!("Web session: this WebView2 cannot report certificate errors; untrusted certificates are refused");
        }
        if let Ok(core10) = core.cast::<ICoreWebView2_10>() {
            let l = label;
            let handler = BasicAuthenticationRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_pwstr(uri);
                if let Some((user, password)) = super::window::http_login(&l, &host_of(&uri)) {
                    let response = args.Response()?;
                    response.SetUserName(&HSTRING::from(user))?;
                    response.SetPassword(&HSTRING::from(password))?;
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core10.add_BasicAuthenticationRequested(&handler, &mut token);
        }
    });
    if let Err(e) = result {
        tracing::warn!("Web session: could not watch certificates: {e}");
    }
}

/// The user trusted the certificate: load the page again (the handler
/// above now allows it).
pub(super) fn retry(win: &WebviewWindow, _label: String) {
    let _ = win.with_webview(|pv| unsafe {
        if let Ok(core) = pv.controller().CoreWebView2() {
            let _ = core.Reload();
        }
    });
}

pub(super) fn forget(_label: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pem_reads_back_to_der() {
        let pem = "-----BEGIN CERTIFICATE-----\r\nAAEC\r\nAw==\r\n-----END CERTIFICATE-----\r\n";
        assert_eq!(pem_to_der(pem), Some(vec![0, 1, 2, 3]));
    }
}
