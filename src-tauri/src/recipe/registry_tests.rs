use super::*;

#[test]
fn parses_a_registry_index() {
    let json = r#"[{
        "id": "harden-ssh",
        "name": "Harden SSH",
        "version": "1.0.0",
        "description": "Locks down sshd.",
        "author": "someone",
        "repo": "someone/reach-recipes",
        "tags": ["security"],
        "targets": ["debian"],
        "danger": "sensitive",
        "url": "https://example.com/harden-ssh.sh",
        "sha256": "abc123"
    }]"#;
    let entries: Vec<RecipeEntry> = serde_json::from_str(json).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "harden-ssh");
    assert_eq!(entries[0].tags, vec!["security"]);
}

#[test]
fn tolerates_a_minimal_entry() {
    // Older or hand-written registry files should not break the whole list.
    let json = r#"[{"id":"x","name":"X","url":"https://e/x.sh","sha256":"d"}]"#;
    let entries: Vec<RecipeEntry> = serde_json::from_str(json).unwrap();
    assert_eq!(entries[0].version, "");
    assert!(entries[0].tags.is_empty());
    assert_eq!(entries[0].danger, None);
}

#[test]
fn hex_is_lowercase_and_padded() {
    assert_eq!(hex_lower(&[0x0a, 0xff, 0x00]), "0aff00");
}

#[tokio::test]
async fn refuses_a_non_https_url() {
    let dir = std::env::temp_dir().join("reach-recipe-registry-http");
    let entry = RecipeEntry {
        id: "x".into(),
        name: "X".into(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
        repo: String::new(),
        tags: vec![],
        targets: vec![],
        danger: None,
        url: "http://example.com/x.sh".into(),
        sha256: "abc".into(),
    };
    assert!(install(&dir, &entry).await.unwrap_err().contains("https"));
}

#[tokio::test]
async fn refuses_an_entry_with_no_hash() {
    let dir = std::env::temp_dir().join("reach-recipe-registry-nohash");
    let entry = RecipeEntry {
        id: "x".into(),
        name: "X".into(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
        repo: String::new(),
        tags: vec![],
        targets: vec![],
        danger: None,
        url: "https://example.com/x.sh".into(),
        sha256: "   ".into(),
    };
    assert!(install(&dir, &entry).await.unwrap_err().contains("sha256"));
}

#[tokio::test]
async fn refuses_an_id_that_could_escape_the_directory() {
    let dir = std::env::temp_dir().join("reach-recipe-registry-escape");
    let entry = RecipeEntry {
        id: "../../evil".into(),
        name: "X".into(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
        repo: String::new(),
        tags: vec![],
        targets: vec![],
        danger: None,
        url: "https://example.com/x.sh".into(),
        sha256: "abc".into(),
    };
    assert!(install(&dir, &entry).await.unwrap_err().contains("Invalid recipe id"));
}
