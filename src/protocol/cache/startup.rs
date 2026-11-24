//! Inicialização Instantânea (0ms Blank Screen)
//!
//! Lê o snapshot completo do cache local no momento da inicialização do processo GPUI,
//! permitindo a renderização do primeiro frame da interface com dados locais salvos
//! antes mesmo de iniciar ou completar a conexão com o Gateway do Discord.

use crate::domain::channel::Channel;
use crate::domain::guild::Guild;
use crate::domain::message::Message;
use crate::domain::user::User;
use crate::protocol::cache::directory::{CacheDirectory, CacheManifest};

/// Estado inicial hidratado diretamente do disco em < 5ms
#[derive(Debug, Clone, Default)]
pub struct HydratedInitialState {
    pub manifest: Option<CacheManifest>,
    pub current_user: Option<User>,
    pub guilds: Vec<Guild>,
    pub active_guild: Option<Guild>,
    pub active_channel: Option<Channel>,
    pub active_messages: Vec<Message>,
    pub is_from_disk_cache: bool,
}

/// Orquestrador de Inicialização Instantânea
pub struct InstantStartupOrchestrator;

impl InstantStartupOrchestrator {
    /// Executa a leitura imediata do disco antes de qualquer I/O de rede
    pub fn hydrate_from_disk(cache_dir: &CacheDirectory) -> HydratedInitialState {
        let manifest = cache_dir.load_manifest();
        let current_user = cache_dir.load_current_user();

        // Não renderiza cache de outra conta nem formato de manifesto futuro.
        if let Some(cached_manifest) = manifest.as_ref() {
            let wrong_account = cached_manifest.active_user_id.is_some()
                && current_user
                    .as_ref()
                    .map(|user| &cached_manifest.active_user_id != &Some(user.id.clone()))
                    .unwrap_or(true);
            if cached_manifest.version != 1 || wrong_account {
                log::warn!("Cache de inicialização ignorado: versão ou conta incompatível.");
                return HydratedInitialState {
                    manifest,
                    current_user: None,
                    ..HydratedInitialState::default()
                };
            }
        }

        // Ordenação determinística; o manifesto apenas escolhe a guilda ativa.
        let mut guild_files: Vec<_> = match std::fs::read_dir(cache_dir.guilds_dir()) {
            Ok(entries) => entries.flatten().map(|entry| entry.path()).collect(),
            Err(_) => Vec::new(),
        };
        guild_files.sort();
        let mut guilds = Vec::new();
        let mut invalid_files = 0usize;
        for path in guild_files {
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path)
                .ok()
                .and_then(|data| serde_json::from_str::<Guild>(&data).ok())
            {
                Some(guild) => guilds.push(guild),
                None => invalid_files += 1,
            }
        }
        if invalid_files > 0 {
            log::warn!(
                "Cache de inicialização: {invalid_files} arquivos de servidor inválidos ignorados."
            );
        }

        let last_guild_id = manifest
            .as_ref()
            .and_then(|manifest| manifest.last_selected_guild_id.clone())
            .or_else(|| guilds.first().map(|guild| guild.id.clone()));
        if let Some(active_id) = last_guild_id.as_deref() {
            guilds.sort_by_key(|guild| if guild.id == active_id { 0 } else { 1 });
        }

        let active_guild = last_guild_id
            .as_ref()
            .and_then(|guild_id| guilds.iter().find(|guild| &guild.id == guild_id).cloned())
            .or_else(|| guilds.first().cloned());

        let last_channel_id = manifest
            .as_ref()
            .and_then(|manifest| manifest.last_selected_channel_id.clone())
            .or_else(|| {
                active_guild
                    .as_ref()
                    .and_then(|guild| guild.channels.first().map(|channel| channel.id.clone()))
            });

        let active_channel = active_guild
            .as_ref()
            .and_then(|guild| {
                last_channel_id.as_ref().and_then(|channel_id| {
                    guild
                        .channels
                        .iter()
                        .find(|channel| &channel.id == channel_id)
                        .cloned()
                })
            })
            .or_else(|| {
                active_guild
                    .as_ref()
                    .and_then(|guild| guild.channels.first().cloned())
            });

        let active_messages = active_channel
            .as_ref()
            .and_then(|channel| cache_dir.load_channel_messages(&channel.id))
            .unwrap_or_default()
            .into_iter()
            .filter(|message| message.channel_id == active_channel.as_ref().unwrap().id)
            .collect();

        let has_cached_data = !guilds.is_empty() || current_user.is_some();

        HydratedInitialState {
            manifest,
            current_user,
            guilds,
            active_guild,
            active_channel,
            active_messages,
            is_from_disk_cache: has_cached_data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::channel::ChannelType;
    use crate::domain::message::Author;

    #[test]
    fn test_instant_startup_hydration() -> anyhow::Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("discord_rs_startup_test_{}", std::process::id()));
        let cache_dir = CacheDirectory::with_root(temp_dir);

        // Preenche cache simulando sessão anterior
        let user = User::new("user_me", "usuario_a");
        cache_dir.save_current_user(&user)?;

        let mut chan = Channel::new("chan_10", "geral", ChannelType::Text);
        chan.guild_id = Some("guild_1".to_string());

        let guild = Guild::new("guild_1", "Rust Brasil", "owner_1").with_channels(vec![chan]);
        cache_dir.save_guild(&guild)?;

        let msg = Message::new(
            "msg_1",
            "chan_10",
            Author::new("author_1", "Amigo"),
            "Mensagem em cache pronta!",
        );
        cache_dir.save_channel_messages("chan_10", &[msg])?;

        let manifest = CacheManifest {
            version: 1,
            last_selected_guild_id: Some("guild_1".to_string()),
            last_selected_channel_id: Some("chan_10".to_string()),
            active_user_id: Some("user_me".to_string()),
            updated_at: chrono::Utc::now(),
        };
        cache_dir.save_manifest(&manifest)?;

        // Bootstrapping instantâneo: 0ms
        let hydrated = InstantStartupOrchestrator::hydrate_from_disk(&cache_dir);

        assert!(hydrated.is_from_disk_cache);
        assert_eq!(hydrated.current_user.unwrap().username, "usuario_a");
        assert_eq!(hydrated.guilds.len(), 1);
        assert_eq!(hydrated.active_guild.unwrap().name, "Rust Brasil");
        assert_eq!(hydrated.active_channel.unwrap().name, "geral");
        assert_eq!(hydrated.active_messages.len(), 1);
        assert_eq!(
            hydrated.active_messages[0].content,
            "Mensagem em cache pronta!"
        );

        Ok(())
    }

    #[test]
    fn test_startup_rejects_other_account_and_future_version() -> anyhow::Result<()> {
        for (version, active_user) in [(1, "user_b"), (999, "user_a")] {
            let temp_dir = std::env::temp_dir().join(format!(
                "discord_rs_startup_reject_{version}_{}_{}",
                active_user,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&temp_dir);
            let cache_dir = CacheDirectory::with_root(temp_dir);
            cache_dir.save_current_user(&User::new("user_a", "usuario_a"))?;
            cache_dir.save_guild(&Guild::new("guild_1", "Servidor", "owner_1"))?;
            cache_dir.save_manifest(&CacheManifest {
                version,
                last_selected_guild_id: Some("guild_1".into()),
                last_selected_channel_id: None,
                active_user_id: Some(active_user.into()),
                updated_at: chrono::Utc::now(),
            })?;

            let hydrated = InstantStartupOrchestrator::hydrate_from_disk(&cache_dir);
            assert!(!hydrated.is_from_disk_cache);
            assert!(hydrated.current_user.is_none());
            assert!(hydrated.guilds.is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_startup_filters_messages_from_another_channel() -> anyhow::Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_startup_channel_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let cache_dir = CacheDirectory::with_root(temp_dir);
        cache_dir.save_current_user(&User::new("user_a", "usuario_a"))?;
        let channel = Channel::new("chan_a", "canal", ChannelType::Text);
        let guild = Guild::new("guild_1", "Servidor", "owner_1").with_channels(vec![channel]);
        cache_dir.save_guild(&guild)?;
        cache_dir.save_channel_messages(
            "chan_a",
            &[
                Message::new(
                    "msg_a",
                    "chan_a",
                    Author::new("author_1", "Autor"),
                    "válida",
                ),
                Message::new(
                    "msg_b",
                    "chan_b",
                    Author::new("author_1", "Autor"),
                    "inválida",
                ),
            ],
        )?;
        cache_dir.save_manifest(&CacheManifest {
            version: 1,
            last_selected_guild_id: Some("guild_1".into()),
            last_selected_channel_id: Some("chan_a".into()),
            active_user_id: Some("user_a".into()),
            updated_at: chrono::Utc::now(),
        })?;

        let hydrated = InstantStartupOrchestrator::hydrate_from_disk(&cache_dir);
        assert_eq!(hydrated.active_messages.len(), 1);
        assert_eq!(hydrated.active_messages[0].id, "msg_a");
        Ok(())
    }
}
