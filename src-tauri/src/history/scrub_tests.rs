use super::*;

fn plain() -> Scrubber {
    Scrubber::new(Vec::new(), &[]).0
}

#[test]
fn known_secret_is_masked_anywhere() {
    let (s, _) = Scrubber::new(vec!["hunter22secret".into(), "abc".into()], &[]);
    let (out, hit) = s.scrub("echo hunter22secret | sudo -S true");
    assert_eq!(out, format!("echo {MASK} | sudo -S true"));
    assert!(hit);
    // Too short to look for.
    assert_eq!(s.scrub("abc def").0, "abc def");
}

#[test]
fn command_line_options() {
    let s = plain();
    assert_eq!(s.scrub("mysql -u root -pS3cr3t db").0, format!("mysql -u root -p{MASK} db"));
    assert_eq!(s.scrub("tool --password=xyz --token abc").0, format!("tool --password={MASK} --token {MASK}"));
    assert_eq!(s.scrub("curl -u admin:pa55 https://x").0, format!("curl -u admin:{MASK} https://x"));
    assert_eq!(s.scrub("sshpass -p 'pw' ssh host").0, format!("sshpass -p {MASK} ssh host"));
    // -p elsewhere is a flag, not a password.
    assert_eq!(s.scrub("mkdir -p /tmp/x && ls -pa").0, "mkdir -p /tmp/x && ls -pa");
}

#[test]
fn urls_headers_variables_tokens() {
    let s = plain();
    assert_eq!(s.scrub("git clone https://bob:t0k3n@git.example.com/r").0, format!("git clone https://bob:{MASK}@git.example.com/r"));
    assert_eq!(s.scrub("curl -H 'Authorization: Bearer abc.def'").0, format!("curl -H 'Authorization: Bearer {MASK}"));
    assert_eq!(s.scrub("export GITHUB_TOKEN=ghx").0, format!("export GITHUB_TOKEN={MASK}"));
    assert_eq!(s.scrub("DB_PASSWORD: \"x y\"").0, format!("DB_PASSWORD: {MASK}"));
    assert!(s.scrub("key AKIAABCDEFGHIJKLMNOP here").1);
    assert!(s.scrub("ghp_abcdefghijklmnopqrstuvwxyz0123456789").1);
    assert!(s.scrub("eyJhbGciOiJIUzI1.eyJzdWIiOiIxMjM0.SflKxwRJSMeKKF2QT4").1);
    assert_eq!(s.scrub("ls -la /etc"), ("ls -la /etc".to_string(), false));
}

#[test]
fn user_patterns() {
    let (s, bad) = Scrubber::new(Vec::new(), &["CORP-[0-9]{6}".into(), "(".into()]);
    assert_eq!(bad, vec!["(".to_string()]);
    assert_eq!(s.scrub("ticket CORP-123456").0, format!("ticket {MASK}"));
}

#[test]
fn secret_prompts() {
    for p in ["[sudo] password for radek: ", "Password:", "Enter passphrase for key '/home/x/.ssh/id': ", "Verification code: ", "PIN? "] {
        assert!(is_secret_prompt(p), "{p}");
    }
    for p in ["radek@host:~$ ", "PS C:\\> ", "mysql> "] {
        assert!(!is_secret_prompt(p), "{p}");
    }
}
