use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

use super::*;
use crate::pwmanager::config::{OnePasswordAuth, OnePasswordConfig, OnePasswordSecrets};

const VAULT: &str = "abcdefghijklmnopqrstuvwxy0";
const ITEM: &str = "zyxwvutsrqponmlkjihgfedcb1";
const TOKEN: &str = "ops_eyJzaWduSW5BZGRyZXNzIjoibXkuMXBhc3N3b3JkLmNvbSJ9";

fn cfg(auth: OnePasswordAuth, account: &str) -> OnePasswordConfig {
    OnePasswordConfig { enabled: true, auth, account: account.into(), ..Default::default() }
}

fn secrets() -> OnePasswordSecrets {
    OnePasswordSecrets { service_account_token: Some(Zeroizing::new(TOKEN.into())) }
}

fn parent_env() -> Vec<(OsString, OsString)> {
    [
        ("PATH", "/usr/bin"),
        ("HOME", "/home/u"),
        ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ("OP_CONNECT_HOST", "https://evil.example.com"),
        ("OP_CONNECT_TOKEN", "x"),
        ("OP_SESSION_my", "session"),
        ("OP_SERVICE_ACCOUNT_TOKEN", "ops_inherited"),
        ("AWS_SECRET_ACCESS_KEY", "y"),
    ]
    .into_iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect()
}

fn build(c: &OnePasswordConfig, s: &OnePasswordSecrets, op: &Op) -> (Vec<String>, HashMap<String, Option<String>>) {
    let cmd = build_command_with_env(Path::new("/usr/bin/op"), c, s, op, parent_env()).unwrap();
    let std = cmd.as_std();
    let args = std.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let env = std
        .get_envs()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned())))
        .collect();
    (args, env)
}

fn set(env: &HashMap<String, Option<String>>, k: &str) -> Option<String> {
    env.get(k).cloned().flatten()
}

#[test]
fn entry_ids_are_vault_and_item_ids() {
    assert_eq!(validate_entry_id(&format!("{VAULT}/{ITEM}")).unwrap(), (VAULT.into(), ITEM.into()));
    for bad in ["", VAULT, "--debug/x", &format!("{VAULT}/{ITEM}/password"), &format!("{VAULT}/ITEMITEMITEMITEMITEMITEM12"), "../../etc/passwd"] {
        assert!(validate_entry_id(bad).is_err(), "accepted {bad}");
    }
}

#[test]
fn the_app_mode_passes_no_secret_and_the_account_when_set() {
    let (args, env) = build(&cfg(OnePasswordAuth::App, "my.1password.com"), &secrets(), &Op::List);
    assert_eq!(args[..4], ["item", "list", "--format", "json"]);
    assert!(args.contains(&"--account=my.1password.com".to_string()));
    assert!(args.contains(&"--no-color".to_string()));
    assert_eq!(set(&env, "OP_SERVICE_ACCOUNT_TOKEN"), None, "the stored token is not used in app mode");
    let (args, _) = build(&cfg(OnePasswordAuth::App, ""), &secrets(), &Op::List);
    assert!(!args.iter().any(|a| a.starts_with("--account")));
}

#[test]
fn a_service_account_token_goes_in_the_environment_only() {
    let (args, env) = build(&cfg(OnePasswordAuth::ServiceAccount, "ignored"), &secrets(), &Op::Read { vault: VAULT.into(), item: ITEM.into() });
    assert_eq!(args, ["read", "--no-newline", &format!("op://{VAULT}/{ITEM}/password"), "--no-color"]);
    assert_eq!(set(&env, "OP_SERVICE_ACCOUNT_TOKEN").as_deref(), Some(TOKEN));
    assert!(!args.iter().any(|a| a.contains(TOKEN)));
    let none = OnePasswordSecrets::default();
    assert!(build_command_with_env(Path::new("/usr/bin/op"), &cfg(OnePasswordAuth::ServiceAccount, ""), &none, &Op::List, parent_env()).is_err());
}

#[test]
fn only_the_allowlist_is_inherited() {
    let (_, env) = build(&cfg(OnePasswordAuth::App, ""), &secrets(), &Op::List);
    assert_eq!(set(&env, "PATH").as_deref(), Some("/usr/bin"));
    assert_eq!(set(&env, "XDG_RUNTIME_DIR").as_deref(), Some("/run/user/1000"));
    for k in ["OP_CONNECT_HOST", "OP_CONNECT_TOKEN", "OP_SESSION_my", "OP_SERVICE_ACCOUNT_TOKEN", "AWS_SECRET_ACCESS_KEY"] {
        assert_eq!(set(&env, k), None, "{k} got through");
    }
}

#[test]
fn the_version_call_signs_in_to_nothing() {
    let (args, env) = build(&cfg(OnePasswordAuth::ServiceAccount, "a"), &secrets(), &Op::Version);
    assert_eq!(args, ["--version"]);
    assert_eq!(set(&env, "OP_SERVICE_ACCOUNT_TOKEN"), None);
}

#[test]
fn the_list_keeps_password_items_and_metadata_only() {
    let json = format!(
        r#"[
        {{"id":"{ITEM}","title":"prod db","category":"LOGIN","vault":{{"id":"{VAULT}","name":"Private"}},
          "additional_information":"admin","urls":[{{"href":"https://other"}},{{"primary":true,"href":"ssh://db.example.com"}}]}},
        {{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaa","title":"A note","category":"SECURE_NOTE","vault":{{"id":"{VAULT}"}}}},
        {{"id":"bbbbbbbbbbbbbbbbbbbbbbbbbb","title":"Router","category":"PASSWORD","vault":{{"id":"{VAULT}"}},"additional_information":"not a user"}},
        {{"id":"--bad","title":"x","category":"LOGIN","vault":{{"id":"{VAULT}"}}}}
    ]"#
    );
    let list = parse_list(json.as_bytes()).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "prod db");
    assert_eq!(list[0].id, format!("{VAULT}/{ITEM}"));
    assert_eq!(list[0].username.as_deref(), Some("admin"));
    assert_eq!(list[0].uri.as_deref(), Some("ssh://db.example.com"));
    assert_eq!(list[1].name, "Router");
    assert_eq!(list[1].username, None, "only a login's extra line is its username");
    assert!(parse_list(b"not json").is_err());
}

#[test]
fn the_read_output_is_the_password() {
    assert_eq!(parse_read(b"s3cret").unwrap().as_str(), "s3cret");
    assert_eq!(parse_read(b"s3cret\n").unwrap().as_str(), "s3cret");
    assert_eq!(parse_read(b"s3cret\r\n").unwrap().as_str(), "s3cret");
    assert_eq!(parse_read(b" spaces kept ").unwrap().as_str(), " spaces kept ");
    assert!(parse_read(b"").is_err());
}

#[test]
fn versions_are_read() {
    assert_eq!(parse_version(b"2.30.3\n").as_deref(), Some("2.30.3"));
    assert_eq!(parse_version(b""), None);
}

#[test]
fn errors_are_explained() {
    let e = |s: &str| friendly_error(s.as_bytes());
    assert!(e("[ERROR] 2026/11/15 10:11:12 authorization prompt dismissed, please try again").contains("not approved"));
    assert!(e("[ERROR] 2026/11/15 10:11:12 \"zzz\" isn't an item in the \"Private\" vault").contains("not found"));
    assert!(e("[ERROR] 2026/11/15 10:11:12 no accounts configured for use with 1Password CLI").contains("Integrate with 1Password CLI"));
    let odd = e("[ERROR] 2026/11/15 10:11:12 something \u{1b}[31mnew\u{1b}[0m");
    assert_eq!(odd, "1Password: something new");
    assert_eq!(e(""), "1Password: the CLI failed without saying why.");
}

#[test]
fn configured_means_switched_on_with_what_it_signs_in_with() {
    let p = |c: OnePasswordConfig, s: OnePasswordSecrets| OnePassword::new(c, s, true).configured();
    assert!(p(cfg(OnePasswordAuth::App, ""), OnePasswordSecrets::default()));
    assert!(!p(OnePasswordConfig { enabled: false, ..cfg(OnePasswordAuth::App, "") }, OnePasswordSecrets::default()));
    assert!(p(cfg(OnePasswordAuth::ServiceAccount, ""), secrets()));
    assert!(!p(cfg(OnePasswordAuth::ServiceAccount, ""), OnePasswordSecrets::default()));
}

#[test]
fn only_the_password_field_is_read_for_now() {
    let p = OnePassword::new(cfg(OnePasswordAuth::App, ""), OnePasswordSecrets::default(), true);
    let id = format!("{VAULT}/{ITEM}");
    assert_eq!(p.check_entry(&id, None).unwrap(), id);
    assert!(p.check_entry(&id, Some("otp")).is_err());
}

/// Against a real 1Password, with the app integration on:
/// `REACH_OP_LIVE=1 cargo test --lib onepassword_live -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn onepassword_live() {
    if std::env::var("REACH_OP_LIVE").is_err() {
        return;
    }
    let p = OnePassword::new(cfg(OnePasswordAuth::App, &std::env::var("REACH_OP_ACCOUNT").unwrap_or_default()), OnePasswordSecrets::default(), true);
    let list = p.list(None).await.map_err(Failure::into_message).unwrap();
    println!("{} entries", list.len());
    if let Some(first) = list.first() {
        let secret = p.resolve(&first.id, None).await.map_err(Failure::into_message).unwrap();
        println!("read {}: {} characters", first.name, secret.len());
    }
}
