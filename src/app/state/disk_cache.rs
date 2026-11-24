use super::*;
use serde::{Deserialize, Serialize};

/// Disk cache structure for persisting guilds and folder structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildsDiskCache {
    pub guilds: Vec<GuildItemState>,
    pub guild_folders: Vec<crate::protocol::GuildFolder>,
}

/// Path to the guilds cache file on disk (~/.config/discord-rs/guilds_cache.json)
pub fn guilds_cache_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("discord-rs")
        .join("guilds_cache.json")
}

/// Persists the list of guilds and guild folders to disk in JSON format.
///
/// A gravação usa arquivo temporário + `fsync` + `rename` atômico: uma queda no
/// meio da escrita nunca deixa o cache final truncado ou parcialmente escrito.
pub fn save_guilds_to_disk(guilds: &[GuildItemState], guild_folders: &[crate::protocol::GuildFolder]) {
    let _ = save_guilds_to_path(&guilds_cache_path(), guilds, guild_folders);
}

/// Grava o snapshot de guildas em um caminho explícito de forma atômica.
///
/// Útil para testes e para o flush de encerramento sem tocar no diretório de
/// configuração real do usuário.
pub fn save_guilds_to_path(
    path: &std::path::Path,
    guilds: &[GuildItemState],
    guild_folders: &[crate::protocol::GuildFolder],
) -> anyhow::Result<()> {
    let cache = GuildsDiskCache {
        guilds: guilds.to_vec(),
        guild_folders: guild_folders.to_vec(),
    };
    let json = serde_json::to_vec_pretty(&cache)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let directory = crate::protocol::cache::CacheDirectory::with_root(parent);
    directory.atomic_write_bytes(path, &json)
}

/// Loads cached guilds and guild folders from disk if available and non-empty
pub fn load_guilds_from_disk() -> Option<GuildsDiskCache> {
    load_guilds_from_path(&guilds_cache_path())
}

/// Carrega o snapshot de guildas de um caminho explícito.
pub fn load_guilds_from_path(path: &std::path::Path) -> Option<GuildsDiskCache> {
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(path).ok()?;
    if let Ok(cache) = serde_json::from_str::<GuildsDiskCache>(&data) {
        if !cache.guilds.is_empty() {
            return Some(cache);
        }
    }
    if let Ok(guilds) = serde_json::from_str::<Vec<GuildItemState>>(&data) {
        if !guilds.is_empty() {
            return Some(GuildsDiskCache {
                guilds,
                guild_folders: Vec::new(),
            });
        }
    }
    None
}


impl AppState {
    /// Persiste imediatamente os caches síncronos que precisam sobreviver ao
    /// encerramento: guildas/pastas e DMs fixadas.
    pub fn flush_caches(&self) {
        save_guilds_to_disk(&self.guilds, &self.guild_folders);
        if let Some(ref path) = self.pinned_dms_path {
            crate::protocol::pinned_dms::save_pinned_dms_to_path(path, &self.pinned_dms);
        }
    }

    /// Encerramento ordenado do estado da aplicação.
    ///
    /// O cache é descarregado antes de parar a rede para que o próximo start
    /// encontre o último snapshot íntegro. O `ClientManager` então encerra voz,
    /// áudio e gateway dentro do orçamento informado.
    pub async fn shutdown(&mut self, timeout: std::time::Duration) {
        self.flush_caches();
        if let Some(client_manager) = self.client_manager.clone() {
            let _ = client_manager.shutdown_with_timeout(timeout).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "discord_rs_guilds_cache_{}_{}_{}",
            name,
            std::process::id(),
            nonce
        ))
    }

    fn sample_guilds() -> Vec<GuildItemState> {
        vec![GuildItemState::new("guild-synthetic-1", "Synthetic Guild")]
    }

    #[test]
    fn save_and_load_round_trip_is_atomic_and_leaves_no_temp_file() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("guilds_cache.json");
        let guilds = sample_guilds();

        save_guilds_to_path(&path, &guilds, &[]).expect("atomic save");
        let loaded = load_guilds_from_path(&path).expect("cache must load");
        assert_eq!(loaded.guilds.len(), 1);
        assert_eq!(loaded.guilds[0].id, "guild-synthetic-1");

        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("cache dir must exist")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["guilds_cache.json".to_string()]);
        assert!(!entries.iter().any(|name| name.starts_with(".tmp_")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_missing_and_corrupt_cache_without_panicking() {
        let dir = temp_dir("corrupt");
        let path = dir.join("guilds_cache.json");
        assert!(load_guilds_from_path(&path).is_none());

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"{not valid json").unwrap();
        assert!(load_guilds_from_path(&path).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
