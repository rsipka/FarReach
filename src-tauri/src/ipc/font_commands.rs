//! The fonts installed on this computer, for the terminal font picker.
//!
//! Based on ReachQ's `list_system_fonts` (Copyright (c) 2026 CPTProgrammer,
//! MIT): scans the system font folders with fontdb. Changed here: the scan
//! runs off the main thread, each family says whether it is monospaced, and
//! hidden system families are left out.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FontFamily {
    pub name: String,
    /// True when any face of the family is marked fixed pitch.
    pub monospace: bool,
}

/// The font families installed on this computer, sorted by name. The phones
/// have no font folders to read, so they get an empty list and the picker
/// keeps its built-in names.
#[tauri::command]
pub async fn fonts_list_installed() -> Result<Vec<FontFamily>, String> {
    #[cfg(desktop)]
    {
        tokio::task::spawn_blocking(scan)
            .await
            .map_err(|e| format!("Font scan failed: {e}"))
    }
    #[cfg(mobile)]
    {
        Ok(Vec::new())
    }
}

#[cfg(desktop)]
fn scan() -> Vec<FontFamily> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let families = group(db.faces().filter_map(|face| {
        // The first name is the English one, which is what CSS matches.
        let (name, _) = face.families.first()?;
        Some((name.clone(), face.monospaced))
    }));
    tracing::debug!("Found {} installed font families", families.len());
    families
}

/// One entry per family from one (name, monospaced) pair per face.
fn group(faces: impl Iterator<Item = (String, bool)>) -> Vec<FontFamily> {
    let mut by_name: std::collections::BTreeMap<String, FontFamily> = Default::default();
    for (name, monospace) in faces {
        let name = name.trim();
        // A leading dot marks a family the OS keeps for itself (macOS
        // ".SF NS Mono"); a page cannot ask for it by name.
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        by_name
            .entry(name.to_lowercase())
            .and_modify(|f| f.monospace |= monospace)
            .or_insert_with(|| FontFamily { name: name.to_string(), monospace });
    }
    by_name.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fam(name: &str, monospace: bool) -> FontFamily {
        FontFamily { name: name.into(), monospace }
    }

    #[test]
    fn groups_faces_into_sorted_families() {
        let faces = [
            ("Noto Sans", false),
            ("JetBrains Mono", true),
            ("JetBrains Mono", true),
            ("jetbrains mono", true),
            ("Fira Code", false),
            ("Fira Code", true),
            (".SF NS Mono", true),
            ("  ", false),
            ("", true),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_string(), m));
        assert_eq!(
            group(faces),
            vec![fam("Fira Code", true), fam("JetBrains Mono", true), fam("Noto Sans", false)]
        );
    }

    #[cfg(desktop)]
    #[test]
    fn scan_does_not_panic() {
        // Whatever this machine has installed, even nothing.
        let _ = scan();
    }
}
