//! Reach: RDSTLS ([MS-RDPBCGR] 5.4.5.3), the authentication a client performs inside TLS after a
//! Server Redirection PDU whose password is encrypted for the target (`LB_PASSWORD_IS_PK_ENCRYPTED`).
//!
//! The server sends its capabilities, the client answers with the redirection GUID, user name,
//! domain and the password blob exactly as the redirection carried them, and the server replies
//! with a result code. GNOME Remote Desktop's Remote Login hands every client over this way.
//! The byte layout follows FreeRDP's `rdstls.c`, which is what that server is built on.
//!
//! [MS-RDPBCGR]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpbcgr/

use core::fmt;

use ironrdp_core::{DecodeResult, WriteBuf};
use ironrdp_pdu::PduHint;

use crate::{ConnectorResult, reason_err};

const VERSION_1: u16 = 0x0001;
const TYPE_CAPABILITIES: u16 = 0x0001;
const TYPE_AUTHREQ: u16 = 0x0002;
const TYPE_AUTHRSP: u16 = 0x0004;
const DATA_CAPABILITIES: u16 = 0x0001;
const DATA_PASSWORD_CREDS: u16 = 0x0001;
const DATA_RESULT_CODE: u16 = 0x0001;

/// What the redirecting server gave the client to present to the target.
#[derive(Clone)]
pub struct RdstlsCredentials {
    /// `LB_REDIRECTION_GUID`, as received.
    pub redirection_guid: Vec<u8>,
    pub username: String,
    pub domain: Option<String>,
    /// `LB_PASSWORD`, as received (encrypted for the target).
    pub password: Vec<u8>,
}

impl fmt::Debug for RdstlsCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdstlsCredentials")
            .field("redirection_guid", &self.redirection_guid.len())
            .field("username", &self.username)
            .field("domain", &self.domain)
            .field("password", &self.password.len())
            .finish()
    }
}

/// A PDU of a fixed size.
#[derive(Debug)]
pub(crate) struct FixedSizeHint(pub(crate) usize);

pub(crate) const CAPABILITIES_HINT: FixedSizeHint = FixedSizeHint(8);
pub(crate) const RESPONSE_HINT: FixedSizeHint = FixedSizeHint(10);

impl PduHint for FixedSizeHint {
    fn find_size(&self, bytes: &[u8]) -> DecodeResult<Option<(bool, usize)>> {
        Ok((bytes.len() >= self.0).then_some((true, self.0)))
    }
}

fn u16_at(input: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*input.get(at)?, *input.get(at + 1)?]))
}

/// Checks the server's RDSTLS Capabilities PDU.
pub(crate) fn read_capabilities(input: &[u8]) -> ConnectorResult<()> {
    let (Some(pdu_type), Some(data_type), Some(versions)) = (u16_at(input, 2), u16_at(input, 4), u16_at(input, 6))
    else {
        return Err(reason_err!("RDSTLS", "capabilities PDU too short"));
    };
    if pdu_type != TYPE_CAPABILITIES || data_type != DATA_CAPABILITIES {
        return Err(reason_err!(
            "RDSTLS",
            "expected capabilities, got PDU type {pdu_type:#x} data type {data_type:#x}"
        ));
    }
    if versions & VERSION_1 == 0 {
        return Err(reason_err!("RDSTLS", "server does not support RDSTLS version 1 ({versions:#x})"));
    }
    Ok(())
}

/// Encodes the Authentication Request PDU with password credentials.
pub(crate) fn write_auth_request(credentials: &RdstlsCredentials, output: &mut WriteBuf) -> ConnectorResult<usize> {
    let mut pdu = Vec::new();
    pdu.extend_from_slice(&VERSION_1.to_le_bytes());
    pdu.extend_from_slice(&TYPE_AUTHREQ.to_le_bytes());
    pdu.extend_from_slice(&DATA_PASSWORD_CREDS.to_le_bytes());
    write_data(&mut pdu, &credentials.redirection_guid)?;
    write_string(&mut pdu, Some(credentials.username.as_str()))?;
    write_string(&mut pdu, credentials.domain.as_deref().filter(|d| !d.is_empty()))?;
    write_data(&mut pdu, &credentials.password)?;
    output.write_slice(&pdu);
    Ok(pdu.len())
}

fn write_data(pdu: &mut Vec<u8>, data: &[u8]) -> ConnectorResult<()> {
    let len = u16::try_from(data.len()).map_err(|_| reason_err!("RDSTLS", "field too long"))?;
    pdu.extend_from_slice(&len.to_le_bytes());
    pdu.extend_from_slice(data);
    Ok(())
}

/// UTF-16LE with its null terminator, the length in bytes first. An absent string is a lone
/// terminator, as FreeRDP writes it.
fn write_string(pdu: &mut Vec<u8>, value: Option<&str>) -> ConnectorResult<()> {
    let mut bytes: Vec<u8> = value
        .unwrap_or("")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    bytes.extend_from_slice(&[0, 0]);
    write_data(pdu, &bytes)
}

/// Reads the Authentication Response PDU and returns its result code (0 is success).
pub(crate) fn read_auth_response(input: &[u8]) -> ConnectorResult<u32> {
    let (Some(pdu_type), Some(data_type)) = (u16_at(input, 2), u16_at(input, 4)) else {
        return Err(reason_err!("RDSTLS", "authentication response too short"));
    };
    if pdu_type != TYPE_AUTHRSP || data_type != DATA_RESULT_CODE {
        return Err(reason_err!(
            "RDSTLS",
            "expected authentication response, got PDU type {pdu_type:#x} data type {data_type:#x}"
        ));
    }
    let code = input
        .get(6..10)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| reason_err!("RDSTLS", "authentication response too short"))?;
    Ok(code)
}

/// A readable name for an RDSTLS result code.
pub(crate) fn result_name(code: u32) -> &'static str {
    match code {
        0x0000_0005 => "access denied",
        0x0000_052e => "logon failure",
        0x0000_0530 => "invalid logon hours",
        0x0000_0532 => "password expired",
        0x0000_0533 => "account disabled",
        0x0000_0773 => "password must change",
        0x0000_0775 => "account locked out",
        _ => "unknown error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_request_matches_freerdp_layout() {
        let credentials = RdstlsCredentials {
            redirection_guid: vec![0x41, 0x00, 0x00, 0x00],
            username: "ab".to_owned(),
            domain: None,
            password: vec![1, 2, 3],
        };
        let mut out = WriteBuf::new();
        let n = write_auth_request(&credentials, &mut out).unwrap();
        assert_eq!(
            out.filled(),
            &[
                1, 0, 2, 0, 1, 0, // version, type, data type
                4, 0, 0x41, 0, 0, 0, // GUID
                6, 0, b'a', 0, b'b', 0, 0, 0, // user name
                2, 0, 0, 0, // no domain
                3, 0, 1, 2, 3, // password
            ][..]
        );
        assert_eq!(n, out.filled_len());
    }

    #[test]
    fn reads_capabilities_and_response() {
        read_capabilities(&[1, 0, 1, 0, 1, 0, 1, 0]).unwrap();
        assert!(read_capabilities(&[1, 0, 4, 0, 1, 0, 1, 0]).is_err());
        assert_eq!(read_auth_response(&[1, 0, 4, 0, 1, 0, 0, 0, 0, 0]).unwrap(), 0);
        assert_eq!(read_auth_response(&[1, 0, 4, 0, 1, 0, 0x2e, 5, 0, 0]).unwrap(), 0x52e);
    }
}
