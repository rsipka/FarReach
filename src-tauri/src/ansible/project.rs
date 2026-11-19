use std::path::Path;

/// Scaffold a new project directory with starter files.
pub fn scaffold_project(path: &str, name: &str) -> Result<(), String> {
    let dir = Path::new(path);
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;

    let site_yml = dir.join("site.yml");
    if !site_yml.exists() {
        let content = format!(
            r#"---
# Ansible project: {}
# Main playbook

- name: Configure all hosts
  hosts: all
  become: true
  tasks:
    - name: Ping all hosts
      ansible.builtin.ping:
"#,
            name
        );
        std::fs::write(&site_yml, content)
            .map_err(|e| format!("Failed to write site.yml: {}", e))?;
    }

    let inventory = dir.join("inventory.ini");
    if !inventory.exists() {
        let content = r#"# Ansible inventory file
# Add your hosts and groups below

[all]
# host1 ansible_host=192.168.1.10 ansible_user=admin
"#;
        std::fs::write(&inventory, content)
            .map_err(|e| format!("Failed to write inventory.ini: {}", e))?;
    }

    Ok(())
}
