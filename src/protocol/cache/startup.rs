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

        // 1. Carrega todas as guildas salvas no diretório guilds/
        let mut guilds = Vec::new();
        if let Ok(entries) = std::fs::read_dir(cache_dir.guilds_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(data) = std::fs::read_to_string(&path) {
                        if let Ok(g) = serde_json::from_str::<Guild>(&data) {
                            guilds.push(g);
                        }
                    }
                }
            }
        }

        // 2. Identifica o último servidor e canal ativos pelo manifesto
        let last_guild_id = manifest
            .as_ref()
            .and_then(|m| m.last_selected_guild_id.clone())
            .or_else(|| guilds.first().map(|g| g.id.clone()));

        let active_guild = last_guild_id
            .as_ref()
            .and_then(|gid| guilds.iter().find(|g| &g.id == gid).cloned())
            .or_else(|| guilds.first().cloned());

        let last_chan_id = manifest
            .as_ref()
            .and_then(|m| m.last_selected_channel_id.clone())
            .or_else(|| {
                active_guild
                    .as_ref()
                    .and_then(|g| g.channels.first().map(|c| c.id.clone()))
            });

        let active_channel = active_guild
            .as_ref()
            .and_then(|g| {
                last_chan_id
                    .as_ref()
                    .and_then(|cid| g.channels.iter().find(|c| &c.id == cid).cloned())
            })
            .or_else(|| active_guild.as_ref().and_then(|g| g.channels.first().cloned()));

        // 3. Carrega as mensagens salvas do canal ativo
        let active_messages = active_channel
            .as_ref()
            .and_then(|c| cache_dir.load_channel_messages(&c.id))
            .unwrap_or_default();

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
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_startup_test_{}", std::process::id()));
        let cache_dir = CacheDirectory::with_root(temp_dir);

        // Preenche cache simulando sessão anterior
        let user = User::new("user_me", "Smcodes");
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
        assert_eq!(hydrated.current_user.unwrap().username, "Smcodes");
        assert_eq!(hydrated.guilds.len(), 1);
        assert_eq!(hydrated.active_guild.unwrap().name, "Rust Brasil");
        assert_eq!(hydrated.active_channel.unwrap().name, "geral");
        assert_eq!(hydrated.active_messages.len(), 1);
        assert_eq!(hydrated.active_messages[0].content, "Mensagem em cache pronta!");

        Ok(())
    }
}
