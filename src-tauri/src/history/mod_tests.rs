use super::*;

#[test]
fn hex_round_trip() {
    let k = [0u8, 1, 0xab, 0xff];
    assert_eq!(decode_hex(&encode_hex(&k)).unwrap(), k);
    assert!(decode_hex("abc").is_none());
    assert!(decode_hex("zz").is_none());
}

#[test]
fn commands_level_drops_output_and_stop_ends_the_tap() {
    let cfg = TapConfig {
        meta: RunMeta { protocol: "local".into(), ..Default::default() },
        level: Some(Level::Commands),
        secrets: vec![],
        patterns: vec![],
        log: None,
    };
    let st = start("test-stream", cfg).unwrap();
    assert_eq!(st.recording, Some(Level::Commands));
    assert!(status("test-stream").is_some());
    assert!(pause("test-stream", true).unwrap().paused);
    stop("test-stream");
    assert!(status("test-stream").is_none());
}
