//! The history file: `history.db` in the app data folder.
//!
//! One SQLite file through libSQL, the engine the vaults use (a second
//! bundled SQLite would collide with it at link time). FTS5 must index
//! plain text, so the vault's per-field sealing cannot apply here: the file
//! is encrypted page by page instead (libSQL's `encryption` feature, SQLite3
//! Multiple Ciphers), with a key kept as a secret in the settings vault.
//!
//! - `runs`: one row per connection of a tab.
//! - `commands`: one row per command run, with the folder and exit code when
//!   the shell sends them.
//! - `output`: blocks of printable output, deflate-compressed.
//! - `commands_fts`, `output_fts`: contentless FTS5 indexes with the trigram
//!   tokenizer, so a part of a word or a path matches; their rowid is the
//!   row's id.

use std::io::{Read, Write};
use std::path::Path;

use libsql::Connection;
use serde::{Deserialize, Serialize};

use crate::history::capture::Event;

/// What a run is.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMeta {
    /// The saved session's id; none for a quick connect or a local shell.
    #[serde(default)]
    pub session_id: Option<String>,
    /// What the tab was called.
    #[serde(default)]
    pub name: String,
    /// ssh, telnet, local, wsl, serial.
    pub protocol: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub user: String,
}

/// How long history is kept, from Settings.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Retention {
    pub output_days: u32,
    pub command_days: u32,
    pub max_mb: u32,
}

impl Default for Retention {
    fn default() -> Self {
        Retention { output_days: 90, command_days: 365, max_mb: 2048 }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    All,
    Commands,
    Output,
}

/// A search, from the panel.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub from: Option<i64>,
    #[serde(default)]
    pub to: Option<i64>,
    #[serde(default)]
    pub limit: Option<u32>,
}

/// One result.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// "command" or "output".
    pub kind: String,
    pub id: i64,
    pub run: i64,
    pub at: i64,
    /// The command, or the lines of output around the match.
    pub text: String,
    pub cwd: Option<String>,
    pub exit: Option<i64>,
    pub redacted: bool,
    pub session_id: Option<String>,
    pub name: String,
    pub host: String,
    pub protocol: String,
}

/// What to forget.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "what", rename_all = "camelCase")]
pub enum Forget {
    Command { id: i64 },
    Output { id: i64 },
    Run { id: i64 },
    Host { host: String },
    Session { session_id: String },
    All,
}

/// Counts for Settings.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub runs: i64,
    pub commands: i64,
    pub outputs: i64,
    pub bytes: i64,
}

const MAX_HITS: u32 = 500;
const SNIPPET_CHARS: usize = 240;
/// Output blocks unpacked to look for text too short for the index.
const SHORT_SCAN_BLOCKS: i64 = 2000;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY,
    session_id TEXT,
    name TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL,
    host TEXT NOT NULL DEFAULT '',
    user TEXT NOT NULL DEFAULT '',
    device TEXT NOT NULL DEFAULT '',
    started INTEGER NOT NULL,
    ended INTEGER
);
CREATE INDEX IF NOT EXISTS runs_session ON runs(session_id);
CREATE INDEX IF NOT EXISTS runs_host ON runs(host);
CREATE TABLE IF NOT EXISTS commands (
    id INTEGER PRIMARY KEY,
    run INTEGER NOT NULL,
    at INTEGER NOT NULL,
    text TEXT NOT NULL,
    cwd TEXT,
    exit INTEGER,
    redacted INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS commands_run ON commands(run);
CREATE INDEX IF NOT EXISTS commands_at ON commands(at);
CREATE TABLE IF NOT EXISTS output (
    id INTEGER PRIMARY KEY,
    run INTEGER NOT NULL,
    first INTEGER NOT NULL,
    last INTEGER NOT NULL,
    body BLOB NOT NULL,
    redacted INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS output_run ON output(run);
CREATE INDEX IF NOT EXISTS output_last ON output(last);
CREATE VIRTUAL TABLE IF NOT EXISTS commands_fts USING fts5(text, content='', contentless_delete=1, tokenize='trigram');
CREATE VIRTUAL TABLE IF NOT EXISTS output_fts USING fts5(text, content='', contentless_delete=1, tokenize='trigram');
";

fn err(e: impl std::fmt::Display) -> String {
    format!("history: {e}")
}

fn deflate(text: &str) -> Vec<u8> {
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = enc.write_all(text.as_bytes());
    enc.finish().unwrap_or_default()
}

fn inflate(body: &[u8]) -> String {
    let mut out = String::new();
    let _ = flate2::read::DeflateDecoder::new(body).read_to_string(&mut out);
    out
}

/// `text` as one FTS5 phrase: matched as typed, never read as a query.
fn phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The lines of `body` around the first match of `needle`.
pub fn snippet(body: &str, needle: &str) -> String {
    if needle.is_empty() {
        // The block's first lines, cut at a line end where one fits.
        let head: String = body.chars().take(SNIPPET_CHARS).collect();
        let cut = if head.len() < body.len() { head.rfind('\n').unwrap_or(head.len()) } else { head.len() };
        return head[..cut].trim().to_string();
    }
    let lower = body.to_lowercase();
    let at = if needle.is_empty() { None } else { lower.find(&needle.to_lowercase()) };
    // to_lowercase can change byte lengths; map back by characters.
    let at = at.map(|b| lower[..b].chars().count()).unwrap_or(0);
    let chars: Vec<char> = body.chars().collect();
    let at = at.min(chars.len());
    let mut start = at.saturating_sub(SNIPPET_CHARS / 3);
    let mut end = (at + SNIPPET_CHARS).min(chars.len());
    // Whole lines where they fit.
    if let Some(nl) = chars[start..at].iter().rposition(|c| *c == '\n') {
        start += nl + 1;
    }
    if let Some(nl) = chars[at..end].iter().skip(needle.chars().count()).position(|c| *c == '\n') {
        end = at + needle.chars().count() + nl;
    }
    chars[start..end].iter().collect::<String>().trim().to_string()
}

pub struct Store {
    _db: libsql::Database,
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the file with `key`. A wrong key fails here.
    pub async fn open(path: &Path, key: [u8; 32]) -> Result<Store, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let db = build(path, key).await?;
        let conn = db.connect().map_err(err)?;
        // A wrong key shows as "file is not a database" on the first read.
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .await
            .map_err(err)?;
        conn.execute_batch(SCHEMA).await.map_err(err)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(Store { _db: db, conn })
    }

    pub async fn new_run(&self, meta: &RunMeta, device: &str, started: i64) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO runs (session_id, name, protocol, host, user, device, started) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                libsql::params![meta.session_id.clone(), meta.name.clone(), meta.protocol.clone(), meta.host.clone(), meta.user.clone(), device, started],
            )
            .await
            .map_err(err)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn end_run(&self, run: i64, ended: i64) -> Result<(), String> {
        self.conn.execute("UPDATE runs SET ended = ?1 WHERE id = ?2", libsql::params![ended, run]).await.map_err(err)?;
        Ok(())
    }

    pub async fn add(&self, run: i64, event: &Event) -> Result<(), String> {
        match event {
            Event::Command { at, text, cwd, redacted } => {
                let tx = self.conn.transaction().await.map_err(err)?;
                tx.execute(
                    "INSERT INTO commands (run, at, text, cwd, redacted) VALUES (?1, ?2, ?3, ?4, ?5)",
                    libsql::params![run, *at, text.clone(), cwd.clone(), *redacted as i64],
                )
                .await
                .map_err(err)?;
                let id = tx.last_insert_rowid();
                tx.execute("INSERT INTO commands_fts (rowid, text) VALUES (?1, ?2)", libsql::params![id, text.clone()]).await.map_err(err)?;
                tx.commit().await.map_err(err)?;
            }
            Event::Exit { code } => {
                self.conn
                    .execute(
                        "UPDATE commands SET exit = ?1 WHERE id = (SELECT MAX(id) FROM commands WHERE run = ?2) AND exit IS NULL",
                        libsql::params![*code, run],
                    )
                    .await
                    .map_err(err)?;
            }
            Event::Output { first, last, text, redacted } => {
                let tx = self.conn.transaction().await.map_err(err)?;
                tx.execute(
                    "INSERT INTO output (run, first, last, body, redacted) VALUES (?1, ?2, ?3, ?4, ?5)",
                    libsql::params![run, *first, *last, deflate(text), *redacted as i64],
                )
                .await
                .map_err(err)?;
                let id = tx.last_insert_rowid();
                tx.execute("INSERT INTO output_fts (rowid, text) VALUES (?1, ?2)", libsql::params![id, text.clone()]).await.map_err(err)?;
                tx.commit().await.map_err(err)?;
            }
        }
        Ok(())
    }

    pub async fn search(&self, q: &Query) -> Result<Vec<Hit>, String> {
        let text = q.text.trim();
        let limit = q.limit.unwrap_or(200).min(MAX_HITS) as i64;
        let mut hits = Vec::new();
        // Trigrams need three characters; shorter text is looked for by a
        // plain scan.
        let short = text.chars().count() < 3;
        if q.kind != Kind::Output {
            let (filter, m) = if text.is_empty() {
                ("1 = 1", String::new())
            } else if short {
                ("c.text LIKE '%' || ?1 || '%' ESCAPE '\\'", text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
            } else {
                ("c.id IN (SELECT rowid FROM commands_fts WHERE commands_fts MATCH ?1)", phrase(text))
            };
            let sql = format!(
                "SELECT c.id, c.run, c.at, c.text, c.cwd, c.exit, c.redacted, r.session_id, r.name, r.host, r.protocol
                 FROM commands c JOIN runs r ON r.id = c.run
                 WHERE {filter} AND (?2 IS NULL OR r.session_id = ?2) AND (?3 IS NULL OR r.host = ?3)
                   AND (?4 IS NULL OR c.at >= ?4) AND (?5 IS NULL OR c.at <= ?5)
                 ORDER BY c.at DESC, c.id DESC LIMIT ?6"
            );
            let mut rows = self
                .conn
                .query(&sql, libsql::params![m, q.session_id.clone(), q.host.clone(), q.from, q.to, limit])
                .await
                .map_err(err)?;
            while let Some(row) = rows.next().await.map_err(err)? {
                hits.push(Hit {
                    kind: "command".into(),
                    id: row.get(0).map_err(err)?,
                    run: row.get(1).map_err(err)?,
                    at: row.get(2).map_err(err)?,
                    text: row.get(3).map_err(err)?,
                    cwd: row.get(4).map_err(err)?,
                    exit: row.get(5).map_err(err)?,
                    redacted: row.get::<i64>(6).map_err(err)? != 0,
                    session_id: row.get(7).map_err(err)?,
                    name: row.get(8).map_err(err)?,
                    host: row.get(9).map_err(err)?,
                    protocol: row.get(10).map_err(err)?,
                });
            }
        }
        if q.kind != Kind::Commands {
            // Empty text lists the newest blocks; text too short for trigrams
            // is looked for in the newest blocks, unpacked here; longer text
            // goes through the index.
            let (filter, m, take) = if text.is_empty() {
                ("1 = 1", String::new(), limit)
            } else if short {
                ("1 = 1", String::new(), SHORT_SCAN_BLOCKS)
            } else {
                ("o.id IN (SELECT rowid FROM output_fts WHERE output_fts MATCH ?1)", phrase(text), limit)
            };
            let sql = format!(
                "SELECT o.id, o.run, o.first, o.body, o.redacted, r.session_id, r.name, r.host, r.protocol
                 FROM output o JOIN runs r ON r.id = o.run
                 WHERE {filter} AND (?2 IS NULL OR r.session_id = ?2) AND (?3 IS NULL OR r.host = ?3)
                   AND (?4 IS NULL OR o.last >= ?4) AND (?5 IS NULL OR o.first <= ?5)
                 ORDER BY o.first DESC, o.id DESC LIMIT ?6"
            );
            let mut rows = self
                .conn
                .query(&sql, libsql::params![m, q.session_id.clone(), q.host.clone(), q.from, q.to, take])
                .await
                .map_err(err)?;
            let lower = text.to_lowercase();
            let mut found = 0;
            while let Some(row) = rows.next().await.map_err(err)? {
                if found >= limit {
                    break;
                }
                let body = inflate(&row.get::<Vec<u8>>(3).map_err(err)?);
                if short && !text.is_empty() && !body.to_lowercase().contains(&lower) {
                    continue;
                }
                found += 1;
                hits.push(Hit {
                    kind: "output".into(),
                    id: row.get(0).map_err(err)?,
                    run: row.get(1).map_err(err)?,
                    at: row.get(2).map_err(err)?,
                    text: snippet(&body, text),
                    cwd: None,
                    exit: None,
                    redacted: row.get::<i64>(4).map_err(err)? != 0,
                    session_id: row.get(5).map_err(err)?,
                    name: row.get(6).map_err(err)?,
                    host: row.get(7).map_err(err)?,
                    protocol: row.get(8).map_err(err)?,
                });
            }
        }
        hits.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
        hits.truncate(limit as usize);
        Ok(hits)
    }

    /// A run's commands and output in order, as read-only text.
    pub async fn transcript(&self, run: i64) -> Result<Vec<Hit>, String> {
        let mut items = Vec::new();
        let mut meta = self
            .conn
            .query("SELECT session_id, name, host, protocol FROM runs WHERE id = ?1", [run])
            .await
            .map_err(err)?;
        let Some(m) = meta.next().await.map_err(err)? else { return Ok(items) };
        let (session_id, name, host, protocol): (Option<String>, String, String, String) =
            (m.get(0).map_err(err)?, m.get(1).map_err(err)?, m.get(2).map_err(err)?, m.get(3).map_err(err)?);
        let base = |kind: &str, id: i64, at: i64, text: String, cwd: Option<String>, exit: Option<i64>, redacted: bool| Hit {
            kind: kind.into(),
            id,
            run,
            at,
            text,
            cwd,
            exit,
            redacted,
            session_id: session_id.clone(),
            name: name.clone(),
            host: host.clone(),
            protocol: protocol.clone(),
        };
        let mut rows = self
            .conn
            .query("SELECT id, at, text, cwd, exit, redacted FROM commands WHERE run = ?1", [run])
            .await
            .map_err(err)?;
        while let Some(r) = rows.next().await.map_err(err)? {
            items.push(base("command", r.get(0).map_err(err)?, r.get(1).map_err(err)?, r.get(2).map_err(err)?, r.get(3).map_err(err)?, r.get(4).map_err(err)?, r.get::<i64>(5).map_err(err)? != 0));
        }
        let mut rows = self.conn.query("SELECT id, first, body, redacted FROM output WHERE run = ?1", [run]).await.map_err(err)?;
        while let Some(r) = rows.next().await.map_err(err)? {
            let body: Vec<u8> = r.get(2).map_err(err)?;
            items.push(base("output", r.get(0).map_err(err)?, r.get(1).map_err(err)?, inflate(&body), None, None, r.get::<i64>(3).map_err(err)? != 0));
        }
        // A command comes before the output it printed, which starts at the
        // same moment or later; by id within the same kind.
        items.sort_by(|a, b| a.at.cmp(&b.at).then((a.kind == "output").cmp(&(b.kind == "output"))).then(a.id.cmp(&b.id)));
        Ok(items)
    }

    pub async fn forget(&self, what: &Forget) -> Result<(), String> {
        let (runs_where, param): (&str, libsql::Value) = match what {
            Forget::Command { id } => {
                let tx = self.conn.transaction().await.map_err(err)?;
                tx.execute("DELETE FROM commands_fts WHERE rowid = ?1", [*id]).await.map_err(err)?;
                tx.execute("DELETE FROM commands WHERE id = ?1", [*id]).await.map_err(err)?;
                tx.commit().await.map_err(err)?;
                return Ok(());
            }
            Forget::Output { id } => {
                let tx = self.conn.transaction().await.map_err(err)?;
                tx.execute("DELETE FROM output_fts WHERE rowid = ?1", [*id]).await.map_err(err)?;
                tx.execute("DELETE FROM output WHERE id = ?1", [*id]).await.map_err(err)?;
                tx.commit().await.map_err(err)?;
                return Ok(());
            }
            Forget::Run { id } => ("id = ?1", (*id).into()),
            Forget::Host { host } => ("host = ?1", host.clone().into()),
            Forget::Session { session_id } => ("session_id = ?1", session_id.clone().into()),
            Forget::All => ("?1 IS NULL", libsql::Value::Null),
        };
        let tx = self.conn.transaction().await.map_err(err)?;
        let runs = format!("SELECT id FROM runs WHERE {runs_where}");
        for sql in [
            format!("DELETE FROM commands_fts WHERE rowid IN (SELECT id FROM commands WHERE run IN ({runs}))"),
            format!("DELETE FROM commands WHERE run IN ({runs})"),
            format!("DELETE FROM output_fts WHERE rowid IN (SELECT id FROM output WHERE run IN ({runs}))"),
            format!("DELETE FROM output WHERE run IN ({runs})"),
            format!("DELETE FROM runs WHERE {runs_where}"),
        ] {
            tx.execute(&sql, [param.clone()]).await.map_err(err)?;
        }
        tx.commit().await.map_err(err)?;
        let _ = self.conn.execute_batch("PRAGMA incremental_vacuum;").await;
        Ok(())
    }

    /// Applies `keep`: old output and commands go, then the oldest output
    /// until the file fits.
    pub async fn prune(&self, now: i64, keep: Retention) -> Result<(), String> {
        const DAY: i64 = 86_400_000;
        let tx = self.conn.transaction().await.map_err(err)?;
        if keep.output_days > 0 {
            let before = now - keep.output_days as i64 * DAY;
            tx.execute("DELETE FROM output_fts WHERE rowid IN (SELECT id FROM output WHERE last < ?1)", [before]).await.map_err(err)?;
            tx.execute("DELETE FROM output WHERE last < ?1", [before]).await.map_err(err)?;
        }
        if keep.command_days > 0 {
            let before = now - keep.command_days as i64 * DAY;
            tx.execute("DELETE FROM commands_fts WHERE rowid IN (SELECT id FROM commands WHERE at < ?1)", [before]).await.map_err(err)?;
            tx.execute("DELETE FROM commands WHERE at < ?1", [before]).await.map_err(err)?;
        }
        tx.commit().await.map_err(err)?;
        if keep.max_mb > 0 {
            let cap = keep.max_mb as i64 * 1024 * 1024;
            for _ in 0..10_000 {
                if self.size().await? <= cap {
                    break;
                }
                let tx = self.conn.transaction().await.map_err(err)?;
                let oldest = "SELECT id FROM output ORDER BY last LIMIT 200";
                tx.execute(&format!("DELETE FROM output_fts WHERE rowid IN ({oldest})"), ()).await.map_err(err)?;
                let n = tx.execute(&format!("DELETE FROM output WHERE id IN ({oldest})"), ()).await.map_err(err)?;
                tx.commit().await.map_err(err)?;
                if n == 0 {
                    break;
                }
            }
        }
        self.conn
            .execute(
                "DELETE FROM runs WHERE ended IS NOT NULL AND NOT EXISTS (SELECT 1 FROM commands WHERE run = runs.id) AND NOT EXISTS (SELECT 1 FROM output WHERE run = runs.id)",
                (),
            )
            .await
            .map_err(err)?;
        let _ = self.conn.execute_batch("INSERT INTO commands_fts(commands_fts) VALUES('optimize'); INSERT INTO output_fts(output_fts) VALUES('optimize'); PRAGMA incremental_vacuum;").await;
        Ok(())
    }

    /// The space the data takes: stored text and its index, estimated as
    /// twice the compressed output plus the commands (the trigram index is
    /// about the size of the text it indexes).
    async fn size(&self) -> Result<i64, String> {
        let mut rows = self
            .conn
            .query("SELECT (SELECT COALESCE(SUM(length(body)), 0) * 4 FROM output) + (SELECT COALESCE(SUM(length(text)), 0) * 2 FROM commands)", ())
            .await
            .map_err(err)?;
        match rows.next().await.map_err(err)? {
            Some(r) => r.get(0).map_err(err),
            None => Ok(0),
        }
    }

    pub async fn stats(&self) -> Result<Stats, String> {
        let mut rows = self
            .conn
            .query("SELECT (SELECT COUNT(*) FROM runs), (SELECT COUNT(*) FROM commands), (SELECT COUNT(*) FROM output)", ())
            .await
            .map_err(err)?;
        let r = rows.next().await.map_err(err)?.ok_or_else(|| err("no row"))?;
        Ok(Stats { runs: r.get(0).map_err(err)?, commands: r.get(1).map_err(err)?, outputs: r.get(2).map_err(err)?, bytes: self.size().await? })
    }
}

#[cfg(desktop)]
async fn build(path: &Path, key: [u8; 32]) -> Result<libsql::Database, String> {
    let cfg = libsql::EncryptionConfig::new(libsql::Cipher::Aes256Cbc, bytes::Bytes::copy_from_slice(&key));
    libsql::Builder::new_local(path).encryption_config(cfg).build().await.map_err(err)
}

/// Page encryption needs SQLite3 Multiple Ciphers, which is built for the
/// desktop only for now; a phone keeps no history rather than an
/// unencrypted one.
#[cfg(not(desktop))]
async fn build(_path: &Path, _key: [u8; 32]) -> Result<libsql::Database, String> {
    Err("history: not available on this device yet".into())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
