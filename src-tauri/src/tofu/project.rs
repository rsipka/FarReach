use std::path::Path;

/// Scaffold a new project directory with a starter main.tf.
pub fn scaffold_project(path: &str, name: &str) -> Result<(), String> {
    let dir = Path::new(path);
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;

    let main_tf = dir.join("main.tf");
    if !main_tf.exists() {
        let content = format!(
            r#"# OpenTofu project: {}
#
# Add your provider and resource configurations below.

terraform {{
  required_version = ">= 1.6.0"
}}
"#,
            name
        );
        std::fs::write(&main_tf, content).map_err(|e| format!("Failed to write main.tf: {}", e))?;
    }

    Ok(())
}
