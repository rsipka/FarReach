//! Tool definitions and the pure part of their dispatch.
//!
//! Everything here is a function over state — no sockets, no Tauri handle, no
//! terminal writes. The server owns the IO: it calls [`prepare_write`], puts
//! the result in front of the user, and only then touches the session. Keeping
//! the gate separate from the act is what makes the guard testable without a
//! running app, and it means the confirmation cannot be accidentally bypassed
//! by a future caller that forgot a step — there is no path from a tool call to
//! a terminal write that does not return through the server.
//!
//! The `description` on each tool is not documentation. It is the only text a
//! model reliably reads before choosing what to call, so each one states the
//! precondition that will otherwise fail the call.

use serde_json::{json, Value};

use crate::mcp::agents::{Agent, ToolName};
use crate::mcp::guard::{self, Approved, Refusal, WriteRequest};
use crate::mcp::redact;
use crate::mcp::session::{ClientState, SharedSession};

use std::collections::HashMap;

/// How much scrollback `read_output` returns by default.
const DEFAULT_LINES: usize = 120;
const MAX_LINES: usize = 500;

/// The tool list for an agent. Only what it was granted — a model cannot
/// misuse a capability it was never offered, and cannot argue about one it
/// cannot see.
pub fn definitions(agent: &Agent) -> Vec<Value> {
    let mut out = Vec::new();

    if agent.grants(ToolName::ListSessions) {
        out.push(json!({
            "name": "list_sessions",
            "description":
                "List the terminal sessions the user has explicitly shared with you. \
                 Returns an empty list when nothing is shared — that is normal and not an \
                 error; ask the user to share a session in Reach. Start here.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }));
    }

    if agent.grants(ToolName::DescribeSession) {
        out.push(json!({
            "name": "describe_session",
            "description":
                "Everything about one session in a single call: host, user, whether that user \
                 is root, shell, and the current state of the screen. You MUST call this \
                 before send_input — that is enforced, not advisory. Call it once per session.",
            "inputSchema": {
                "type": "object",
                "properties": { "sessionId": { "type": "string" } },
                "required": ["sessionId"],
                "additionalProperties": false
            }
        }));
    }

    if agent.grants(ToolName::ReadOutput) {
        out.push(json!({
            "name": "read_output",
            "description":
                "Read recent output from a session. Call this IMMEDIATELY before every \
                 send_input: writing against a screen you have not just read is refused. \
                 Returns the sequence number your next write must match, so a read and a \
                 write are a complete pair with no third call.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string" },
                    "lines": {
                        "type": "integer",
                        "description": "Lines from the end of the scrollback. Default 120.",
                        "minimum": 1,
                        "maximum": MAX_LINES
                    }
                },
                "required": ["sessionId"],
                "additionalProperties": false
            }
        }));
    }

    if agent.grants(ToolName::SendInput) {
        out.push(json!({
            "name": "send_input",
            "description":
                "Type into a session. A human sees your rationale and approves or rejects \
                 before anything reaches the shell — write the rationale for them, not for a \
                 log. This runs on a real machine and there is no undo. Preconditions, all \
                 enforced: describe_session called, read_output called since the last output, \
                 no password prompt waiting, no credential in the command itself.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string" },
                    "command": {
                        "type": "string",
                        "description": "Exactly what to type. A newline is added; do not include one."
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Why, in plain language. A human reads this and decides."
                    },
                    "references": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "What you are basing this on: a man page, vendor docs, a runbook. Required for anything that changes system state."
                    },
                    "rollback": {
                        "type": "string",
                        "description": "How to undo it. Required for destructive commands. If you cannot state it, do not run the command."
                    }
                },
                "required": ["sessionId", "command", "rationale"],
                "additionalProperties": false
            }
        }));
    }

    out
}

/// The prompts an agent contributes. One per agent, so a client can load the
/// persona for whichever the user selected.
pub fn prompt_definitions(agent: &Agent) -> Vec<Value> {
    vec![json!({
        "name": agent.id,
        "title": agent.name,
        "description": agent.description,
        "arguments": []
    })]
}

pub fn prompt_content(agent: &Agent) -> Value {
    json!({
        "description": agent.description,
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": agent.persona }
        }]
    })
}

/// `list_sessions`.
pub fn list_sessions(sessions: &HashMap<String, SharedSession>) -> Value {
    let mut items: Vec<Value> = sessions
        .values()
        .map(|s| {
            json!({
                "sessionId": s.id,
                "kind": s.kind,
                "host": s.host,
                "username": s.username
            })
        })
        .collect();
    // Stable order so a model re-reading the list does not see it shuffle.
    items.sort_by(|a, b| a["sessionId"].as_str().cmp(&b["sessionId"].as_str()));

    if items.is_empty() {
        return json!({
            "sessions": [],
            "note": "The user has not shared any session. This is not an error. Ask them to \
                     enable sharing on a terminal tab in Reach."
        });
    }
    json!({ "sessions": items })
}

/// `describe_session`. Answers in one call so nothing needs a follow-up, and
/// records that the precondition is met.
pub fn describe_session(
    sessions: &HashMap<String, SharedSession>,
    client: &mut ClientState,
    session_id: &str,
) -> Result<Value, String> {
    let Some(s) = sessions.get(session_id) else {
        return Err(unknown_session(sessions, session_id));
    };

    client.mark_described(session_id);
    // Describing shows the screen, so it also counts as a read. Otherwise a
    // model that calls describe then send is refused for a reason it cannot
    // see, and burns a round trip discovering it.
    client.mark_read(session_id, s.seq);

    let (screen, masked) = redact::redact(&s.tail(40));

    Ok(json!({
        "sessionId": s.id,
        "kind": s.kind,
        "host": s.host,
        "username": s.username,
        "isRoot": s.username == "root",
        "shell": s.shell,
        "sequence": s.seq,
        "awaitingSecretPrompt": s.awaiting_secret(),
        "recentScreen": screen,
        "redactedCount": masked,
        "warning": if s.username == "root" {
            "You are root on this machine. Every command runs unrestricted."
        } else {
            "Commands may still escalate via sudo. Check before assuming they cannot."
        }
    }))
}

/// `read_output`.
pub fn read_output(
    sessions: &HashMap<String, SharedSession>,
    client: &mut ClientState,
    session_id: &str,
    lines: Option<usize>,
) -> Result<Value, String> {
    let Some(s) = sessions.get(session_id) else {
        return Err(unknown_session(sessions, session_id));
    };

    let want = lines.unwrap_or(DEFAULT_LINES).clamp(1, MAX_LINES);
    let (text, masked) = redact::redact(&s.tail(want));
    client.mark_read(session_id, s.seq);

    Ok(json!({
        "sessionId": s.id,
        "sequence": s.seq,
        "output": text,
        "redactedCount": masked,
        "awaitingSecretPrompt": s.awaiting_secret(),
        "note": if masked > 0 {
            Some(format!(
                "{masked} value(s) were masked by Reach before you saw this. The text is \
                 incomplete — do not infer their contents."
            ))
        } else {
            None
        }
    }))
}

/// The write gate. Returns what the confirm dialog needs, or a refusal.
///
/// This does **not** write. The server confirms with the user first; there is
/// no path from here to the terminal.
pub fn prepare_write(
    agent: &Agent,
    sessions: &HashMap<String, SharedSession>,
    client: &mut ClientState,
    session_id: &str,
    req: &WriteRequest,
) -> Result<Approved, Refusal> {
    let Some(s) = sessions.get(session_id) else {
        // An unknown session is a refusal, not an error: the model should list
        // sessions again rather than abort.
        return Err(Refusal::Denied {
            reason: format!(
                "No shared session `{session_id}`. Call list_sessions — the user may have \
                 stopped sharing it."
            ),
        });
    };

    let facts = guard::SessionFacts {
        described: client.has_described(session_id),
        view_current: client.view_is_current(session_id, s.seq),
        echo_off: s.awaiting_secret(),
        // Wired when skills land; until then nothing is outstanding.
        unread_skills: Vec::new(),
    };

    guard::check_write(agent, &facts, req, &mut client.rate)
}

/// Turn a stale-view refusal into a reply that already contains the fix.
///
/// The single largest latency saving in the protocol. Without it the common
/// failure is: write refused, read, write again — three round trips for one
/// action. With the missed output attached, the model can compose the next
/// write immediately.
pub fn refusal_payload(
    refusal: &Refusal,
    sessions: &HashMap<String, SharedSession>,
    client: &mut ClientState,
    session_id: &str,
) -> Value {
    let mut text = refusal.message();

    if matches!(refusal, Refusal::StaleView) {
        if let Some(s) = sessions.get(session_id) {
            let (out, masked) = redact::redact(&s.tail(DEFAULT_LINES));
            // Attaching the output *is* the read, so record it. The model is
            // then one call from succeeding instead of two.
            client.mark_read(session_id, s.seq);
            text.push_str(&format!(
                "\n\nHere is the output you had not read, so you do not need to call \
                 read_output again. Sequence is now {}.{}\n\n---\n{}",
                s.seq,
                if masked > 0 {
                    format!(" {masked} value(s) were masked.")
                } else {
                    String::new()
                },
                out
            ));
        }
    }

    crate::mcp::protocol::tool_refusal(text)
}

fn unknown_session(sessions: &HashMap<String, SharedSession>, id: &str) -> String {
    let known: Vec<&str> = sessions.keys().map(String::as_str).collect();
    if known.is_empty() {
        format!("No session `{id}`, and nothing is shared. Ask the user to share a terminal tab.")
    } else {
        format!("No session `{id}`. Shared sessions are: {}.", known.join(", "))
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
