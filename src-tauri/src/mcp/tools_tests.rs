use super::*;
use crate::mcp::agents;
use crate::mcp::session::SessionKind;

fn world() -> (HashMap<String, SharedSession>, ClientState) {
    let mut m = HashMap::new();
    let mut s = SharedSession::new(
        "s1".into(),
        SessionKind::Ssh,
        "db.internal".into(),
        "root".into(),
    );
    s.push_output(b"root@db:~# ");
    m.insert("s1".to_string(), s);
    (m, ClientState::default())
}

fn write_req(cmd: &str) -> WriteRequest {
    WriteRequest {
        command: cmd.into(),
        rationale: "checking whether nginx is running".into(),
        references: vec!["man systemctl".into()],
        rollback: Some("none needed, read-only".into()),
    }
}

#[test]
fn architect_is_offered_no_write_tool() {
    let names: Vec<String> = definitions(&agents::find("architect").unwrap())
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["list_sessions", "describe_session", "read_output"]);
    assert!(!names.contains(&"send_input".to_string()));
}

#[test]
fn a_full_agent_is_offered_all_four() {
    assert_eq!(definitions(&agents::find("linux-administrator").unwrap()).len(), 4);
}

/// Every schema must be a closed object; a model that invents an argument
/// should be told, not silently ignored.
#[test]
fn every_tool_schema_is_closed_and_documented() {
    for agent in agents::built_in() {
        for tool in definitions(&agent) {
            assert_eq!(tool["inputSchema"]["additionalProperties"], json!(false));
            let desc = tool["description"].as_str().unwrap();
            assert!(desc.len() > 60, "thin description on {}", tool["name"]);
        }
    }
}

#[test]
fn an_empty_share_list_explains_itself_rather_than_erroring() {
    let v = list_sessions(&HashMap::new());
    assert_eq!(v["sessions"], json!([]));
    assert!(v["note"].as_str().unwrap().contains("not an error"));
}

#[test]
fn describe_reports_root_and_counts_as_a_read() {
    let (sessions, mut client) = world();
    let v = describe_session(&sessions, &mut client, "s1").unwrap();
    assert_eq!(v["isRoot"], json!(true));
    assert!(v["warning"].as_str().unwrap().contains("root"));
    assert!(client.has_described("s1"));
    // Describing shows the screen, so a following write must not be
    // refused as stale — that would burn a round trip for no reason.
    assert!(client.view_is_current("s1", sessions["s1"].seq));
}

#[test]
fn read_output_masks_and_says_that_it_did() {
    let mut sessions = HashMap::new();
    let mut s = SharedSession::new("s1".into(), SessionKind::Ssh, "h".into(), "u".into());
    s.push_output(b"PGPASSWORD=hunter2\n");
    sessions.insert("s1".to_string(), s);
    let mut client = ClientState::default();

    let v = read_output(&sessions, &mut client, "s1", None).unwrap();
    assert!(!v["output"].as_str().unwrap().contains("hunter2"));
    assert_eq!(v["redactedCount"], json!(1));
    assert!(v["note"].as_str().unwrap().contains("incomplete"));
}

#[test]
fn an_unknown_session_lists_what_is_actually_shared() {
    let (sessions, mut client) = world();
    let err = describe_session(&sessions, &mut client, "nope").unwrap_err();
    assert!(err.contains("s1"), "should name the real sessions: {err}");
}

#[test]
fn a_write_without_describe_is_refused() {
    let (sessions, mut client) = world();
    let agent = agents::find("linux-administrator").unwrap();
    let err = prepare_write(&agent, &sessions, &mut client, "s1", &write_req("uptime"))
        .unwrap_err();
    assert_eq!(err, Refusal::SessionNotDescribed);
}

#[test]
fn describe_then_write_succeeds() {
    let (sessions, mut client) = world();
    let agent = agents::find("linux-administrator").unwrap();
    describe_session(&sessions, &mut client, "s1").unwrap();
    prepare_write(&agent, &sessions, &mut client, "s1", &write_req("uptime"))
        .expect("describe should satisfy both preconditions");
}

/// The latency win: a stale-view refusal hands back the output, so the
/// model needs one more call, not two.
#[test]
fn a_stale_view_refusal_carries_the_missed_output() {
    let (mut sessions, mut client) = world();
    let agent = agents::find("linux-administrator").unwrap();
    describe_session(&sessions, &mut client, "s1").unwrap();

    sessions.get_mut("s1").unwrap().push_output(b"\nsomething happened\n");

    let err = prepare_write(&agent, &sessions, &mut client, "s1", &write_req("uptime"))
        .unwrap_err();
    assert_eq!(err, Refusal::StaleView);

    let payload = refusal_payload(&err, &sessions, &mut client, "s1");
    let text = payload["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("something happened"), "missed output not attached: {text}");
    assert!(text.contains("do not need to call read_output again"));

    // And the view is now current, so the retry goes straight through.
    prepare_write(&agent, &sessions, &mut client, "s1", &write_req("uptime"))
        .expect("the refusal should have brought the client up to date");
}

#[test]
fn a_password_prompt_blocks_writing() {
    let (mut sessions, mut client) = world();
    let agent = agents::find("linux-administrator").unwrap();
    describe_session(&sessions, &mut client, "s1").unwrap();
    sessions.get_mut("s1").unwrap().push_output(b"\n[sudo] password for alex: ");
    // Re-read so only the prompt, not staleness, can be the reason.
    read_output(&sessions, &mut client, "s1", None).unwrap();

    let err = prepare_write(&agent, &sessions, &mut client, "s1", &write_req("hunter2"))
        .unwrap_err();
    assert_eq!(err, Refusal::EchoDisabled);
}

#[test]
fn a_session_that_stopped_being_shared_is_a_refusal_not_an_error() {
    let (sessions, mut client) = world();
    let agent = agents::find("linux-administrator").unwrap();
    let err = prepare_write(&agent, &sessions, &mut client, "gone", &write_req("ls"))
        .unwrap_err();
    match err {
        Refusal::Denied { reason } => assert!(reason.contains("list_sessions")),
        other => panic!("expected a recoverable refusal, got {other:?}"),
    }
}

#[test]
fn prompts_expose_the_active_persona() {
    let agent = agents::find("devsecops-engineer").unwrap();
    let defs = prompt_definitions(&agent);
    assert_eq!(defs[0]["name"], json!("devsecops-engineer"));
    let content = prompt_content(&agent);
    let text = content["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("DevSecOps"));
}

#[test]
fn session_listing_is_stably_ordered() {
    let mut m = HashMap::new();
    for id in ["s3", "s1", "s2"] {
        m.insert(
            id.to_string(),
            SharedSession::new(id.into(), SessionKind::Local, "local".into(), "me".into()),
        );
    }
    let v = list_sessions(&m);
    let ids: Vec<&str> =
        v["sessions"].as_array().unwrap().iter().map(|s| s["sessionId"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["s1", "s2", "s3"]);
}
