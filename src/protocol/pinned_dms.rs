use std::path::PathBuf;

/// Returns the canonical path to the pinned DMs cache file:
/// `~/.config/discord-rs/cache/pinned_dms.json`
pub fn pinned_dms_cache_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("discord-rs")
        .join("cache")
        .join("pinned_dms.json")
}

/// Persists the list of pinned DM channel IDs to disk in JSON format.
pub fn save_pinned_dms_to_disk(pinned_dms: &[String]) {
    save_pinned_dms_to_path(&pinned_dms_cache_path(), pinned_dms);
}

/// Helper function to save pinned DMs to a specific file path (useful for testing).
pub fn save_pinned_dms_to_path(path: &std::path::Path, pinned_dms: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&pinned_dms) {
        let _ = std::fs::write(path, json);
    }
}

/// Loads cached pinned DM channel IDs from disk if available.
pub fn load_pinned_dms_from_disk() -> Vec<String> {
    load_pinned_dms_from_path(&pinned_dms_cache_path())
}

/// Helper function to load pinned DMs from a specific file path (supports both raw arrays and `{ "pinned_dms": [...] }`).
pub fn load_pinned_dms_from_path(path: &std::path::Path) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // Try parsing as simple Vec<String>
    if let Ok(ids) = serde_json::from_str::<Vec<String>>(&data) {
        return ids;
    }

    // Try parsing as JSON object with "pinned_dms" key
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        pinned_dms: Vec<String>,
    }
    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&data) {
        return wrapper.pinned_dms;
    }

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
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_test_pins_{}", std::process::id()));
        let file_path = temp_dir.join("pinned_dms.json");

        let initial = vec!["dm-101".to_string(), "dm-102".to_string(), "dm-103".to_string()];
        save_pinned_dms_to_path(&file_path, &initial);

        let loaded = load_pinned_dms_from_path(&file_path);
        assert_eq!(loaded, initial);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_wrapper_format() {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_test_pins_wrap_{}", std::process::id()));
        let file_path = temp_dir.join("pinned_dms.json");
        let _ = std::fs::create_dir_all(&temp_dir);

        let json = r#"{"pinned_dms": ["dm-vip-1", "dm-vip-2"]}"#;
        std::fs::write(&file_path, json).unwrap();

        let loaded = load_pinned_dms_from_path(&file_path);
        assert_eq!(loaded, vec!["dm-vip-1", "dm-vip-2"]);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
