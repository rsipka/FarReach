use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::*;

const SPEC: CliSpec = CliSpec {
    name: "Tool",
    program: "tool",
    file_names: &["tool", "tool.exe"],
    install_hint: "Install tool.",
    inherited_env: &["PATH", "HOME"],
    removed_env: &["TOOL_DEBUG"],
};

#[test]
fn the_binary_must_have_the_tools_own_name() {
    let base = if cfg!(windows) { "C:\\bin\\" } else { "/usr/bin/" };
    assert!(check_cli_path_shape(&SPEC, Path::new(&format!("{base}tool"))).is_ok());
    assert!(check_cli_path_shape(&SPEC, Path::new(&format!("{base}TOOL.EXE"))).is_ok());
    let err = check_cli_path_shape(&SPEC, Path::new(&format!("{base}tool.cmd"))).unwrap_err();
    assert!(err.starts_with("Tool: "), "{err}");
    assert!(err.contains("a file named tool or tool.exe"), "{err}");
    assert!(check_cli_path_shape(&SPEC, Path::new("tool")).is_err());
    assert!(check_cli_path_shape(&SPEC, Path::new("\\\\server\\share\\tool")).is_err());
}

#[test]
fn network_paths_are_recognised() {
    assert!(is_network_path("\\\\server\\share\\x"));
    assert!(is_network_path("//server/share/x"));
    assert!(is_network_path("  \\\\?\\UNC\\server\\x"));
    assert!(!is_network_path("/usr/bin/x"));
    assert!(!is_network_path("C:\\x"));
}

#[test]
fn the_child_starts_from_the_allowlist_only() {
    let mut cmd = tokio::process::Command::new("tool");
    let parent: Vec<(OsString, OsString)> = [("PATH", "/bin"), ("HOME", "/h"), ("SECRET", "x"), ("TOOL_DEBUG", "1")]
        .into_iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect();
    set_base_env(&SPEC, &mut cmd, parent);
    let envs: Vec<(String, Option<String>)> = cmd
        .as_std()
        .get_envs()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned())))
        .collect();
    assert!(envs.contains(&("PATH".into(), Some("/bin".into()))));
    assert!(envs.contains(&("HOME".into(), Some("/h".into()))));
    assert!(!envs.iter().any(|(k, v)| k == "SECRET" && v.is_some()));
    assert!(!envs.iter().any(|(k, v)| k == "TOOL_DEBUG" && v.is_some()));
    assert!(is_inherited_env(&SPEC, OsStr::new("PATH")));
    assert!(!is_inherited_env(&SPEC, OsStr::new("SECRET")));
}
