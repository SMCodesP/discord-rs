use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Returns the canonical path to the pinned DMs cache file:
/// `~/.config/discord-rs/cache/pinned_dms.json`
pub fn pinned_dms_cache_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("discord-rs")
        .join("cache")
        .join("pinned_dms.json")
}

/// Keeps the first occurrence of each non-empty channel ID in cache order.
fn normalize_pinned_dms(pinned_dms: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    pinned_dms
        .iter()
        .filter_map(|id| {
            let id = id.trim();
            if id.is_empty() {
                return None;
            }
            seen.insert(id.to_string()).then(|| id.to_string())
        })
        .collect()
}

/// Persists the list of pinned DM channel IDs to disk in JSON format.
pub fn save_pinned_dms_to_disk(pinned_dms: &[String]) {
    save_pinned_dms_to_path(&pinned_dms_cache_path(), pinned_dms);
}

/// Helper function to save pinned DMs to a specific file path (useful for testing).
/// Writes a normalized snapshot to a sibling temporary file and atomically
/// replaces the cache so an interrupted write cannot corrupt the pin order.
pub fn save_pinned_dms_to_path(path: &Path, pinned_dms: &[String]) {
    let normalized = normalize_pinned_dms(pinned_dms);
    let json = match serde_json::to_string_pretty(&normalized) {
        Ok(json) => json,
        Err(error) => {
            log::warn!("Falha ao serializar DMs fixadas: {:?}", error);
            return;
        }
    };

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                log::warn!(
                    "Falha ao criar diretório do cache de DMs fixadas: {:?}",
                    error
                );
                return;
            }
        }
    }

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "pinned_dms.json".to_string());
    let temp_path = path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        nonce
    ));

    if let Err(error) = std::fs::write(&temp_path, json) {
        let _ = std::fs::remove_file(&temp_path);
        log::warn!(
            "Falha ao gravar cache temporário de DMs fixadas: {:?}",
            error
        );
        return;
    }
    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        log::warn!("Falha ao substituir cache de DMs fixadas: {:?}", error);
    }
}

/// Loads cached pinned DM channel IDs from disk if available.
pub fn load_pinned_dms_from_disk() -> Vec<String> {
    load_pinned_dms_from_path(&pinned_dms_cache_path())
}

/// Helper function to load pinned DMs from a specific file path (supports both raw arrays and `{ "pinned_dms": [...] }`).
pub fn load_pinned_dms_from_path(path: &Path) -> Vec<String> {
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            log::warn!("Falha ao ler cache de DMs fixadas: {:?}", error);
            return Vec::new();
        }
    };

    // Try parsing as simple Vec<String>
    if let Ok(ids) = serde_json::from_str::<Vec<String>>(&data) {
        return normalize_pinned_dms(&ids);
    }

    // Try parsing as JSON object with "pinned_dms" key
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        pinned_dms: Vec<String>,
    }
    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&data) {
        return normalize_pinned_dms(&wrapper.pinned_dms);
    }

    log::warn!("Cache de DMs fixadas contém JSON inválido");
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_dms_cache_path_format() {
        let path = pinned_dms_cache_path();
        assert!(path.ends_with("discord-rs/cache/pinned_dms.json"));
    }

    #[test]
    fn test_save_and_load_pinned_dms() {
        let temp_dir =
            std::env::temp_dir().join(format!("discord_rs_test_pins_{}", std::process::id()));
        let file_path = temp_dir.join("pinned_dms.json");

        let initial = vec![
            "dm-101".to_string(),
            "dm-102".to_string(),
            "dm-103".to_string(),
        ];
        save_pinned_dms_to_path(&file_path, &initial);

        let loaded = load_pinned_dms_from_path(&file_path);
        assert_eq!(loaded, initial);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_wrapper_format() {
        let temp_dir =
            std::env::temp_dir().join(format!("discord_rs_test_pins_wrap_{}", std::process::id()));
        let file_path = temp_dir.join("pinned_dms.json");
        let _ = std::fs::create_dir_all(&temp_dir);

        let json = r#"{"pinned_dms": ["dm-vip-1", "dm-vip-2"]}"#;
        std::fs::write(&file_path, json).unwrap();

        let loaded = load_pinned_dms_from_path(&file_path);
        assert_eq!(loaded, vec!["dm-vip-1", "dm-vip-2"]);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cache_normalizes_ids_and_preserves_first_occurrence_order() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_test_pins_normalize_{}_{}",
            std::process::id(),
            nonce
        ));
        let file_path = temp_dir.join("pinned_dms.json");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(
            &file_path,
            r#"[" dm-b ", "", "dm-a", "dm-b", "   ", "dm-c"]"#,
        )
        .unwrap();

        assert_eq!(
            load_pinned_dms_from_path(&file_path),
            vec!["dm-b", "dm-a", "dm-c"]
        );

        save_pinned_dms_to_path(
            &file_path,
            &[
                " dm-c ".to_string(),
                "dm-b".to_string(),
                "dm-c".to_string(),
                String::new(),
            ],
        );
        let saved: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(saved, vec!["dm-c", "dm-b"]);
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
