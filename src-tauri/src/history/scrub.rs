//! Taking secrets out of a line before it is stored.
//!
//! Two kinds of removal, applied to every command and every output line:
//!
//! - **Known secrets.** At connect the app holds the session's password, key
//!   passphrase and linked credential. Any exact occurrence of one is
//!   replaced, which catches a password pasted into a command line, where
//!   no pattern could tell it apart from other text.
//! - **Patterns.** Command-line options, URLs, headers and variables that
//!   name a password or token, private keys, and token formats with a fixed
//!   shape (AWS, GitHub, Slack, JWT). The user can add their own.
//!
//! What is removed becomes [`MASK`], and the caller marks the row, so a
//! result shows that something was taken out.

use std::sync::OnceLock;

use regex::Regex;

/// What a removed secret is replaced with.
pub const MASK: &str = "••••";

/// A known secret shorter than this is not looked for: a two-letter
/// password would mask every word that contains it.
pub const MIN_SECRET_LEN: usize = 6;

/// A pattern and which of its groups is the secret (0: the whole match).
struct Rule {
    re: Regex,
    group: usize,
}

fn builtin() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let rule = |re: &str, group: usize| Rule { re: Regex::new(re).expect("built-in pattern"), group };
        vec![
            // --password=x, --token x, -password x (the long options of most CLIs).
            rule(r"(?i)(?:^|\s)--?(?:password|passwd|pass|pwd|token|secret|api-?key|access-?key|client-?secret)(?:=|\s+)(\S+)", 1),
            // mysql -pSECRET (the password glued to -p), only for the MySQL tools,
            // where -p means password; elsewhere -p is mostly a harmless flag.
            rule(r"(?i)\b(?:mysql|mariadb|mysqldump|mysqladmin|mysqlimport)\b.*?\s-p([^\s-]\S*)", 1),
            // sshpass -p SECRET
            rule(r"\bsshpass\s+-p\s*(\S+)", 1),
            // curl -u user:SECRET, --user user:SECRET
            rule(r"(?:^|\s)(?:-u|--user)\s+[^:\s]+:(\S+)", 1),
            // scheme://user:SECRET@host
            rule(r"(?i)\b[a-z][a-z0-9+.-]*://[^/\s:@]+:([^@\s/]+)@", 1),
            // Authorization: Bearer SECRET (also Basic and Token)
            rule(r"(?i)\bauthorization:\s*(?:(?:bearer|basic|token)\s+)?(\S+)", 1),
            // PASSWORD=x, export GITHUB_TOKEN="x", api_key: x
            rule(
                r#"(?i)\b[A-Z0-9_]*(?:PASSWORD|PASSWD|TOKEN|SECRET|API_?KEY|ACCESS_?KEY|PRIVATE_?KEY)[A-Z0-9_]*\s*[=:]\s*("[^"]*"|'[^']*'|\S+)"#,
                1,
            ),
            // AWS access key ids.
            rule(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", 0),
            // GitHub tokens.
            rule(r"\bgh[pousr]_[A-Za-z0-9]{30,}\b", 0),
            rule(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b", 0),
            // Slack tokens.
            rule(r"\bxox[abposr]-[A-Za-z0-9-]{10,}", 0),
            // JSON Web Tokens.
            rule(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", 0),
        ]
    })
}

/// Removes secrets from lines of one session.
#[derive(Default)]
pub struct Scrubber {
    /// Longest first, so a secret that contains another is masked whole.
    secrets: Vec<String>,
    /// The user's own patterns; the whole match is the secret.
    extra: Vec<Regex>,
}

impl Scrubber {
    /// `secrets`: what this session is known to use. `patterns`: the user's
    /// own regexes; one that does not compile is skipped, and returned.
    pub fn new(secrets: Vec<String>, patterns: &[String]) -> (Scrubber, Vec<String>) {
        let mut secrets: Vec<String> = secrets.into_iter().filter(|s| s.chars().count() >= MIN_SECRET_LEN).collect();
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        secrets.dedup();
        let mut extra = Vec::new();
        let mut bad = Vec::new();
        for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            match Regex::new(p) {
                Ok(re) => extra.push(re),
                Err(_) => bad.push(p.to_string()),
            }
        }
        (Scrubber { secrets, extra }, bad)
    }

    /// `line` with every secret masked, and whether anything was.
    pub fn scrub(&self, line: &str) -> (String, bool) {
        let mut out = line.to_string();
        let mut hit = false;
        for s in &self.secrets {
            if out.contains(s.as_str()) {
                out = out.replace(s.as_str(), MASK);
                hit = true;
            }
        }
        for rule in builtin() {
            let (next, changed) = mask(&rule.re, rule.group, &out);
            if changed {
                out = next;
                hit = true;
            }
        }
        for re in &self.extra {
            let (next, changed) = mask(re, 0, &out);
            if changed {
                out = next;
                hit = true;
            }
        }
        (out, hit)
    }
}

/// `text` with group `group` of every match of `re` replaced by [`MASK`].
fn mask(re: &Regex, group: usize, text: &str) -> (String, bool) {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let Some(m) = caps.get(group) else { continue };
        if m.as_str() == MASK || m.is_empty() {
            continue;
        }
        out.push_str(&text[last..m.start()]);
        out.push_str(MASK);
        last = m.end();
    }
    if last == 0 {
        return (text.to_string(), false);
    }
    out.push_str(&text[last..]);
    (out, true)
}

/// Whether a prompt asks for something secret: input typed after it is
/// never stored, even when the terminal echoed it (as `*` or in clear).
pub fn is_secret_prompt(prompt: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(pass(word|phrase|code)?|\bpin\b|token|secret|\botp\b|one-time|verification code|2fa|mfa)[^\n]*[:?>]\s*$")
            .expect("built-in pattern")
    });
    re.is_match(prompt.trim_end_matches(['\r', '\n']))
}

#[cfg(test)]
#[path = "scrub_tests.rs"]
mod tests;
