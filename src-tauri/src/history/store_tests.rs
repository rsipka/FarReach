use super::*;
#[cfg(desktop)]
use crate::history::capture::Event;

#[cfg(desktop)]
fn tempdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("reach-history-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(desktop)]
async fn open(dir: &std::path::Path) -> Store {
    Store::open(&dir.join("h.db"), [9u8; 32]).await.unwrap()
}

#[cfg(desktop)]
fn meta(host: &str) -> RunMeta {
    RunMeta { session_id: Some("s1".into()), name: "web".into(), protocol: "ssh".into(), host: host.into(), user: "radek".into() }
}

#[cfg(desktop)]
fn cmd(at: i64, text: &str) -> Event {
    Event::Command { at, text: text.into(), cwd: None, redacted: false }
}

// The file needs page encryption, built for the desktop only.
#[cfg(desktop)]
#[tokio::test]
async fn search_commands_and_output_then_forget() {
    let dir = tempdir("t1");
    let s = open(dir.as_path()).await;
    let run = s.new_run(&meta("web1"), "dev", 1).await.unwrap();
    s.add(run, &cmd(10, "systemctl restart nginx")).await.unwrap();
    s.add(run, &Event::Exit { code: 0 }).await.unwrap();
    s.add(run, &Event::Output { first: 11, last: 12, text: "line one\nJob for nginx.service failed\nline three".into(), redacted: false }).await.unwrap();
    s.add(run, &cmd(20, "ls")).await.unwrap();

    let hits = s.search(&Query { text: "nginx".into(), ..Default::default() }).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].kind, "output");
    assert!(hits[0].text.contains("Job for nginx.service failed"));
    assert_eq!(hits[1].text, "systemctl restart nginx");
    assert_eq!(hits[1].exit, Some(0));

    // Short text: commands and output, by a scan.
    let hits = s.search(&Query { text: "ls".into(), ..Default::default() }).await.unwrap();
    assert_eq!(hits.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(), vec!["command"]);
    let hits = s.search(&Query { text: "Jo".into(), ..Default::default() }).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].text.starts_with("Job for"));
    // Empty text: the newest of a session, output included.
    let all = s.search(&Query { session_id: Some("s1".into()), ..Default::default() }).await.unwrap();
    assert_eq!(all.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(), vec!["command", "output", "command"]);
    assert!(all[1].text.starts_with("line one\nJob for"));
    let hits = s.search(&Query { session_id: Some("s1".into()), kind: Kind::Commands, ..Default::default() }).await.unwrap();
    assert_eq!(hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["ls", "systemctl restart nginx"]);
    // Quotes in the query are text, not FTS syntax.
    assert!(s.search(&Query { text: "\"nginx OR".into(), ..Default::default() }).await.is_ok());

    let t = s.transcript(run).await.unwrap();
    assert_eq!(t.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(), vec!["command", "output", "command"]);

    let id = hits[1].id;
    s.forget(&Forget::Command { id }).await.unwrap();
    let hits = s.search(&Query { text: "restart".into(), ..Default::default() }).await.unwrap();
    assert!(hits.is_empty());
    s.forget(&Forget::Host { host: "web1".into() }).await.unwrap();
    assert_eq!(s.stats().await.unwrap().runs, 0);
    assert!(s.search(&Query { text: "nginx".into(), ..Default::default() }).await.unwrap().is_empty());
}

// The file needs page encryption, built for the desktop only.
#[cfg(desktop)]
#[tokio::test]
async fn file_is_encrypted_and_needs_its_key() {
    let dir = tempdir("t2");
    {
        let s = open(dir.as_path()).await;
        let run = s.new_run(&meta("h"), "dev", 1).await.unwrap();
        s.add(run, &cmd(1, "echo plaintext-marker")).await.unwrap();
    }
    for f in std::fs::read_dir(dir.as_path()).unwrap() {
        let bytes = std::fs::read(f.unwrap().path()).unwrap();
        assert!(!bytes.windows(16).any(|w| w == b"plaintext-marker"));
    }
    assert!(Store::open(&dir.as_path().join("h.db"), [1u8; 32]).await.is_err());
    let s = open(dir.as_path()).await;
    assert_eq!(s.search(&Query { text: "marker".into(), ..Default::default() }).await.unwrap().len(), 1);
}

// The file needs page encryption, built for the desktop only.
#[cfg(desktop)]
#[tokio::test]
async fn prune_drops_old_rows() {
    let dir = tempdir("t3");
    let s = open(dir.as_path()).await;
    let day = 86_400_000;
    let run = s.new_run(&meta("h"), "dev", 0).await.unwrap();
    s.add(run, &cmd(0, "old command")).await.unwrap();
    s.add(run, &Event::Output { first: 0, last: 0, text: "old output".into(), redacted: false }).await.unwrap();
    s.add(run, &cmd(200 * day, "new command")).await.unwrap();
    s.prune(200 * day, Retention { output_days: 90, command_days: 365, max_mb: 2048 }).await.unwrap();
    let st = s.stats().await.unwrap();
    assert_eq!((st.commands, st.outputs), (2, 0));
    s.prune(400 * day, Retention { output_days: 90, command_days: 365, max_mb: 2048 }).await.unwrap();
    assert_eq!(s.stats().await.unwrap().commands, 1);
}

#[test]
fn snippet_keeps_lines_around_match() {
    let body = "a\nb\nthe needle line\nc\nd";
    assert_eq!(snippet(body, "NEEDLE"), "the needle line");
    let long = format!("{}needle", "x".repeat(500));
    assert!(snippet(&long, "needle").ends_with("needle"));
}
