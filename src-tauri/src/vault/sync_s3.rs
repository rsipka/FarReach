//! An S3 bucket that holds the vault file: Amazon S3 or any S3-compatible
//! store (MinIO, Backblaze B2, Wasabi, Cloudflare R2, Ceph, Garage...).
//!
//! Requests are signed with AWS Signature Version 4, here, so no SDK is
//! needed. The object's ETag is its revision. A write names the ETag it
//! replaces (`If-Match`), or that there must be none yet (`If-None-Match`),
//! so two devices writing at once cannot both win: the second gets 412 and
//! merges again. A store that does not take conditional writes (501) gets a
//! read, a compare and a plain write instead.

use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

use crate::vault::error::VaultError;
use crate::vault::sync_store::FILE_NAME;

/// How long one request may take.
const STEP_TIMEOUT: Duration = Duration::from_secs(60);

/// The SHA-256 of nothing, the payload hash of a GET.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// How to reach a bucket.
#[derive(Clone)]
pub struct S3Target {
    /// `https://host[:port]`, or `http://` for a store on this computer.
    pub endpoint: String,
    pub bucket: String,
    /// The folder in the bucket, without leading or trailing `/`; may be empty.
    pub prefix: String,
    pub region: String,
    /// `bucket.host` rather than `host/bucket`.
    pub virtual_host: bool,
    pub access_key: String,
    pub secret_key: String,
}

/// What an S3 backend's token holds.
#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct S3Secret {
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
}

fn s3_error(msg: impl Into<String>) -> VaultError {
    VaultError::SyncError(msg.into())
}

impl S3Target {
    /// `s3://host[:port]/bucket[/folder...][?region=..&style=path|virtual]`,
    /// or `s3+http://` for a store on this computer, with a token of JSON
    /// `{access_key, secret_key}`.
    pub fn parse(url: &str, token: &str) -> Result<S3Target, VaultError> {
        let lower = url.to_ascii_lowercase();
        let (scheme, rest) = if lower.starts_with("s3+http://") {
            ("http", &url["s3+http://".len()..])
        } else if lower.starts_with("s3://") {
            ("https", &url["s3://".len()..])
        } else {
            return Err(s3_error("Not an S3 address"));
        };
        let parsed = url::Url::parse(&format!("{scheme}://{rest}")).map_err(|e| s3_error(format!("S3 address: {e}")))?;
        let host = parsed.host_str().filter(|h| !h.is_empty()).ok_or_else(|| s3_error("S3 address has no server"))?;
        let host = host.to_ascii_lowercase();
        let endpoint = match parsed.port() {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        };
        if scheme == "http" {
            // Keys and the vault file in the clear only on this computer.
            crate::vault::sync::check_sync_url(&endpoint)?;
        }
        let path: Vec<String> = parsed
            .path_segments()
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .map(|s| url::form_urlencoded::parse(format!("x={}", s.replace('+', "%2B")).as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default())
            .collect();
        let bucket = path.first().cloned().ok_or_else(|| s3_error("S3 address names no bucket"))?;
        if bucket.len() < 3 || !bucket.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c)) {
            return Err(s3_error("That S3 bucket name is not valid"));
        }
        let prefix = path[1..].join("/");
        if path[1..].iter().any(|s| s == "." || s == "..") {
            return Err(s3_error("That S3 folder is not valid"));
        }
        let query = |name: &str| parsed.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
        let region = query("region").unwrap_or_else(|| region_of(&host));
        // Amazon prefers bucket.host; most others take host/bucket. A bucket
        // with a dot breaks the certificate of bucket.host.
        let virtual_host = match query("style").as_deref() {
            Some("virtual") => true,
            Some("path") => false,
            Some(_) => return Err(s3_error("The S3 style is path or virtual")),
            None => host.ends_with(".amazonaws.com") && !bucket.contains('.'),
        };
        let secret: S3Secret = serde_json::from_str(token).map_err(|_| s3_error("S3 keys are missing or damaged"))?;
        if secret.access_key.trim().is_empty() || secret.secret_key.is_empty() {
            return Err(s3_error("The S3 access key and secret key are both needed"));
        }
        Ok(S3Target {
            endpoint,
            bucket,
            prefix,
            region,
            virtual_host,
            access_key: secret.access_key.trim().to_string(),
            secret_key: secret.secret_key,
        })
    }

    /// What names this bucket and folder, without a credential.
    pub fn identity(&self) -> String {
        format!("s3:{}/{}/{}", self.endpoint, self.bucket, self.prefix)
    }

    /// The object's key in the bucket.
    fn key(&self) -> String {
        if self.prefix.is_empty() {
            FILE_NAME.to_string()
        } else {
            format!("{}/{}", self.prefix, FILE_NAME)
        }
    }

    /// The object's URL, the Host it is signed for, and its canonical path.
    fn object(&self) -> (String, String, String) {
        let (scheme, authority) = self.endpoint.split_once("://").unwrap_or(("https", &self.endpoint));
        let key = encode_path(&self.key());
        if self.virtual_host {
            let host = format!("{}.{}", self.bucket, authority);
            let path = format!("/{key}");
            (format!("{scheme}://{host}{path}"), host, path)
        } else {
            let path = format!("/{}/{key}", encode_path(&self.bucket));
            (format!("{scheme}://{authority}{path}"), authority.to_string(), path)
        }
    }
}

/// The region a well-known endpoint names: `s3.<region>.amazonaws.com`,
/// `s3.<region>.backblazeb2.com`, `s3.<region>.wasabisys.com`; R2 is
/// `auto`; anything else (MinIO and most others) `us-east-1`.
pub fn region_of(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    if host.ends_with(".r2.cloudflarestorage.com") {
        return "auto".into();
    }
    if labels.len() >= 4 && labels[0] == "s3" && labels[1] != "dualstack" {
        return labels[1].to_string();
    }
    if labels.len() >= 4 && labels[0] == "s3" && labels[1] == "dualstack" {
        return labels[2].to_string();
    }
    if labels.len() >= 3 && labels[0].starts_with("s3-") {
        return labels[0]["s3-".len()..].to_string();
    }
    "us-east-1".into()
}

/// `%`-encoded as S3 wants a path: all but `A-Z a-z 0-9 - . _ ~` and `/`.
fn encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// The `Authorization` header for a request, by Signature Version 4.
/// `headers` are the lower-case names and values to sign, `host`,
/// `x-amz-content-sha256` and `x-amz-date` among them.
#[allow(clippy::too_many_arguments)]
pub fn authorization(
    access_key: &str,
    secret_key: &str,
    region: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    payload_sha256: &str,
    amz_date: &str,
) -> String {
    let mut headers: Vec<(&str, &str)> = headers.to_vec();
    headers.sort_by(|a, b| a.0.cmp(b.0));
    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{}\n", v.trim())).collect();
    let signed: Vec<&str> = headers.iter().map(|(k, _)| *k).collect();
    let signed = signed.join(";");
    let canonical = format!("{method}\n{path}\n\n{canonical_headers}\n{signed}\n{payload_sha256}");
    let date = &amz_date[..8];
    let scope = format!("{date}/{region}/s3/aws4_request");
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", hex(&Sha256::digest(canonical.as_bytes())));
    let mut key = hmac(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    for part in [region, "s3", "aws4_request"] {
        key = hmac(&key, part.as_bytes());
    }
    let signature = hex(&hmac(&key, to_sign.as_bytes()));
    format!("AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed}, Signature={signature}")
}

/// A bucket, open for reading and writing the vault file.
pub struct S3Store {
    target: S3Target,
    client: reqwest::Client,
}

/// What a write asks of the object already there.
enum Condition<'a> {
    None,
    Absent,
    Matches(&'a str),
}

impl S3Store {
    pub fn open(target: &S3Target) -> Result<S3Store, VaultError> {
        let client = crate::http::client_builder()
            .timeout(STEP_TIMEOUT)
            .build()
            .map_err(|e| s3_error(e.to_string()))?;
        Ok(S3Store { target: target.clone(), client })
    }

    async fn send(&self, method: reqwest::Method, body: Option<&[u8]>, condition: Condition<'_>) -> Result<reqwest::Response, VaultError> {
        let t = &self.target;
        let (url, host, path) = t.object();
        let payload = match body {
            Some(b) => hex(&Sha256::digest(b)),
            None => EMPTY_SHA256.to_string(),
        };
        let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let auth = authorization(
            &t.access_key,
            &t.secret_key,
            &t.region,
            method.as_str(),
            &path,
            &[("host", &host), ("x-amz-content-sha256", &payload), ("x-amz-date", &amz_date)],
            &payload,
            &amz_date,
        );
        let mut request = self
            .client
            .request(method, &url)
            .header("x-amz-content-sha256", &payload)
            .header("x-amz-date", &amz_date)
            .header("authorization", auth);
        request = match condition {
            Condition::None => request,
            Condition::Absent => request.header("if-none-match", "*"),
            Condition::Matches(etag) => request.header("if-match", etag),
        };
        if let Some(b) = body {
            request = request.header("content-type", "application/octet-stream").body(b.to_vec());
        }
        request.send().await.map_err(|e| s3_error(format!("Could not reach the S3 store: {e}")))
    }

    /// The vault file and its ETag, or `None` if there is none yet.
    pub async fn get(&mut self) -> Result<Option<(Vec<u8>, String)>, VaultError> {
        let response = self.send(reqwest::Method::GET, None, Condition::None).await?;
        let status = response.status();
        if status.is_success() {
            let etag = etag_of(&response)?;
            let data = response.bytes().await.map_err(|e| s3_error(format!("Could not read the vault file: {e}")))?;
            return Ok(Some((data.to_vec(), etag)));
        }
        let body = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::NOT_FOUND && error_code(&body).as_deref() != Some("NoSuchBucket") {
            return Ok(None);
        }
        Err(explain(status, &body, &self.target))
    }

    /// Write the vault file, provided it is still at `expected` (`None`:
    /// still absent). Returns false if another device wrote first.
    pub async fn put(&mut self, data: &[u8], expected: Option<&str>) -> Result<bool, VaultError> {
        let condition = match expected {
            Some(etag) => Condition::Matches(etag),
            None => Condition::Absent,
        };
        let response = self.send(reqwest::Method::PUT, Some(data), condition).await?;
        let status = response.status();
        if status.is_success() {
            return Ok(true);
        }
        let body = response.text().await.unwrap_or_default();
        // Another device wrote first (409: a write raced this one).
        if status == reqwest::StatusCode::PRECONDITION_FAILED || error_code(&body).as_deref() == Some("ConditionalRequestConflict") {
            return Ok(false);
        }
        if status == reqwest::StatusCode::NOT_IMPLEMENTED {
            // No conditional writes here: check, then write.
            tracing::debug!("S3 store takes no conditional writes; checking before writing");
            let current = self.get().await?.map(|(_, etag)| etag);
            if current.as_deref() != expected {
                return Ok(false);
            }
            let response = self.send(reqwest::Method::PUT, Some(data), Condition::None).await?;
            let status = response.status();
            if status.is_success() {
                return Ok(true);
            }
            let body = response.text().await.unwrap_or_default();
            return Err(explain(status, &body, &self.target));
        }
        Err(explain(status, &body, &self.target))
    }
}

fn etag_of(response: &reqwest::Response) -> Result<String, VaultError> {
    response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
        .ok_or_else(|| s3_error("The S3 store gave no ETag for the vault file"))
}

/// The `<Code>` of an S3 error answer.
fn error_code(body: &str) -> Option<String> {
    let start = body.find("<Code>")? + "<Code>".len();
    let end = body[start..].find("</Code>")? + start;
    Some(body[start..end].to_string())
}

fn explain(status: reqwest::StatusCode, body: &str, target: &S3Target) -> VaultError {
    let code = error_code(body).unwrap_or_default();
    let msg = match code.as_str() {
        "NoSuchBucket" => format!("The bucket {} does not exist on {}", target.bucket, target.endpoint),
        "InvalidAccessKeyId" => "The S3 store does not know that access key".to_string(),
        "SignatureDoesNotMatch" => "The S3 secret key is wrong".to_string(),
        "AccessDenied" => format!("The S3 keys may not read and write {} in {}", target.key(), target.bucket),
        "AuthorizationHeaderMalformed" | "PermanentRedirect" | "IllegalLocationConstraintException" => {
            format!("The bucket is in another region than {}: set its region", target.region)
        }
        "RequestTimeTooSkewed" => "This computer's clock is too far off for the S3 store".to_string(),
        "" => format!("The S3 store answered {status}"),
        other => format!("The S3 store answered {status} ({other})"),
    };
    s3_error(msg)
}

#[cfg(test)]
#[path = "sync_s3_tests.rs"]
mod tests;
