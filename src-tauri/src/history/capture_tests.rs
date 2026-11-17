use super::*;

fn cap(secrets: &[&str]) -> Capture {
    Capture::new(Scrubber::new(secrets.iter().map(|s| s.to_string()).collect(), &[]).0)
}

fn commands(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Command { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn output(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Output { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Types `line` with the server echoing each key, then Enter.
fn typed_echoed(c: &mut Capture, ev: &mut Vec<Event>, line: &str) {
    for ch in line.chars() {
        ev.extend(c.input(0, ch.to_string().as_bytes()));
        ev.extend(c.output(0, &ch.to_string()));
    }
    ev.extend(c.input(0, b"\r"));
}

#[test]
fn echoed_command_is_stored_without_prompt() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "Welcome\r\nradek@web:~$ ");
    typed_echoed(&mut c, &mut ev, "ls -la");
    ev.extend(c.output(10, "\r\ntotal 0\r\nradek@web:~$ "));
    ev.extend(c.finish(20));
    assert_eq!(commands(&ev), vec!["ls -la"]);
    let out = output(&ev);
    assert!(out.contains("Welcome") && out.contains("total 0"), "{out}");
}

#[test]
fn slow_echo_is_still_judged_on_the_finished_line() {
    // The user types fast; the echo arrives after Enter.
    let mut c = cap(&[]);
    let mut ev = c.output(0, "$ ");
    ev.extend(c.input(0, b"uptime"));
    ev.extend(c.input(0, b"\r"));
    ev.extend(c.output(100, "uptime\r\n 10:00 up 3 days\r\n$ "));
    assert_eq!(commands(&ev), vec!["uptime"]);
}

#[test]
fn hidden_input_is_never_stored() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "$ ");
    typed_echoed(&mut c, &mut ev, "sudo -i");
    ev.extend(c.output(0, "\r\n[sudo] password for radek: "));
    // Not echoed.
    ev.extend(c.input(0, b"Tr0ub4dor&3"));
    ev.extend(c.input(0, b"\r"));
    ev.extend(c.output(0, "\r\nroot@web:~# "));
    ev.extend(c.finish(1));
    assert_eq!(commands(&ev), vec!["sudo -i"]);
    assert!(!format!("{ev:?}").contains("Tr0ub4dor"));
}

#[test]
fn echoed_input_after_password_prompt_is_dropped() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "Password: ");
    typed_echoed(&mut c, &mut ev, "visible-secret");
    ev.extend(c.output(0, "\r\nok\r\n"));
    assert!(commands(&ev).is_empty());
}

#[test]
fn backspace_and_line_redraw() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "$ ");
    for ch in ["l", "x"] {
        ev.extend(c.input(0, ch.as_bytes()));
        ev.extend(c.output(0, ch));
    }
    // Backspace: the shell answers \b and erases to the end of the line.
    ev.extend(c.input(0, b"\x7f"));
    ev.extend(c.output(0, "\x08\x1b[K"));
    ev.extend(c.input(0, b"s"));
    ev.extend(c.output(0, "s"));
    ev.extend(c.input(0, b"\r"));
    ev.extend(c.output(0, "\r\n"));
    assert_eq!(commands(&ev), vec!["ls"]);
}

#[test]
fn history_recall_uses_the_screen() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "$ ");
    // Up arrow; the shell redraws the line with the old command.
    ev.extend(c.input(0, b"\x1b[A"));
    ev.extend(c.output(0, "\r$ git status\x1b[K"));
    ev.extend(c.input(0, b"\r"));
    ev.extend(c.output(0, "\r\nOn branch main\r\n"));
    assert_eq!(commands(&ev), vec!["git status"]);
}

#[test]
fn known_secret_in_command_and_output_is_masked() {
    let mut c = cap(&["S3cretPass!"]);
    let mut ev = c.output(0, "$ ");
    typed_echoed(&mut c, &mut ev, "echo S3cretPass!");
    ev.extend(c.output(0, "\r\nS3cretPass!\r\n$ "));
    ev.extend(c.finish(1));
    let all = format!("{ev:?}");
    assert!(!all.contains("S3cretPass!"), "{all}");
    assert!(ev.iter().any(|e| matches!(e, Event::Command { redacted: true, .. })));
}

#[test]
fn private_keys_are_left_out() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "-----BEGIN OPENSSH PRIVATE KEY-----\r\nb3BlbnNzaC1rZXktdjEA\r\n-----END OPENSSH PRIVATE KEY-----\r\nafter\r\n");
    ev.extend(c.finish(1));
    let out = output(&ev);
    assert!(!out.contains("b3BlbnNz"), "{out}");
    assert!(out.contains("after"));
}

#[test]
fn full_screen_programs_are_left_out() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "$ ");
    typed_echoed(&mut c, &mut ev, "vim notes");
    ev.extend(c.output(0, "\r\n\x1b[?1049hsecret file text\r\nmore\x1b[?1049l$ "));
    ev.extend(c.input(0, b":wq\r"));
    ev.extend(c.finish(1));
    assert_eq!(commands(&ev), vec!["vim notes"]);
    assert!(!output(&ev).contains("secret file text"));
}

#[test]
fn shell_integration_gives_command_cwd_and_exit() {
    let mut c = cap(&[]);
    let mut ev = c.output(0, "\x1b]7;file://web/home/radek%20x\x07\x1b]133;A\x07$ \x1b]133;B\x07");
    typed_echoed(&mut c, &mut ev, "false");
    ev.extend(c.output(0, "\x1b]633;E;false\x07\r\n\x1b]133;D;1\x07"));
    match ev.iter().find(|e| matches!(e, Event::Command { .. })) {
        Some(Event::Command { text, cwd, .. }) => {
            assert_eq!(text, "false");
            assert_eq!(cwd.as_deref(), Some("/home/radek x"));
        }
        _ => panic!("no command: {ev:?}"),
    }
    assert!(ev.contains(&Event::Exit { code: 1 }));
}

#[test]
fn blocks_are_cut_by_size_and_time() {
    let mut c = cap(&[]);
    let line = "x".repeat(100);
    let mut ev = Vec::new();
    for _ in 0..50 {
        ev.extend(c.output(0, &format!("{line}\r\n")));
    }
    assert!(ev.iter().any(|e| matches!(e, Event::Output { .. })));
    let mut c = cap(&[]);
    assert!(c.output(0, "a\r\n").is_empty());
    assert!(c.tick(1_000).is_empty());
    assert_eq!(output(&c.tick(BLOCK_MS + 1)), "a");
}
