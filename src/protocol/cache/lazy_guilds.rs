//! Lazy Guilds & Opcode 14 (Guild Subscriptions)
//!
//! Garante que mensagens e membros nunca sejam puxados de todos os servidores simultaneamente.
//! O cliente assina sob demanda exclusivamente o servidor e canal em foco no viewport.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Faixa de índices de membros visíveis na sidebar de membros: `[start_index, end_index]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRange(pub usize, pub usize);

/// Payload do Opcode 14 (Guild Subscriptions) do Gateway Discord
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildSubscription {
    pub guild_id: String,
    pub typing: bool,
    pub threads: bool,
    pub activities: bool,
    pub members: Vec<String>,
    pub channels: HashMap<String, Vec<[usize; 2]>>,
}

/// Coordenador de Lazy Guilds gerenciando o estado de subscrições ativas
#[derive(Debug, Clone)]
pub struct LazyGuildCoordinator {
    active_guild_id: Arc<RwLock<Option<String>>>,
    active_channel_id: Arc<RwLock<Option<String>>>,
    channel_ranges: Arc<RwLock<HashMap<String, Vec<[usize; 2]>>>>,
}

impl Default for LazyGuildCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl LazyGuildCoordinator {
    pub fn new() -> Self {
        Self {
            active_guild_id: Arc::new(RwLock::new(None)),
            active_channel_id: Arc::new(RwLock::new(None)),
            channel_ranges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Retorna o ID da guilda atualmente ativa
    pub async fn active_guild_id(&self) -> Option<String> {
        self.active_guild_id.read().await.clone()
    }

    /// Retorna o ID do canal atualmente ativo
    pub async fn active_channel_id(&self) -> Option<String> {
        self.active_channel_id.read().await.clone()
    }

    /// Alterna o foco para um novo servidor e canal inicial, gerando o payload Op 14
    pub async fn focus_guild_and_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> serde_json::Value {
        {
            let mut g = self.active_guild_id.write().await;
            *g = Some(guild_id.to_string());
        }
        {
            let mut c = self.active_channel_id.write().await;
            *c = Some(channel_id.to_string());
        }

        // Padrão oficial: primeiros 100 membros no topo do viewport `[[0, 99]]`
        let default_range = vec![[0, 99]];
        {
            let mut ranges = self.channel_ranges.write().await;
            ranges.clear();
            ranges.insert(channel_id.to_string(), default_range.clone());
        }

        self.generate_opcode_14_payload(guild_id, channel_id, default_range)
    }

    /// Atualiza as faixas de membros visíveis (por exemplo durante scroll na sidebar)
    pub async fn update_viewport_ranges(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> serde_json::Value {
        {
            let mut map = self.channel_ranges.write().await;
            map.insert(channel_id.to_string(), ranges.clone());
        }

        self.generate_opcode_14_payload(guild_id, channel_id, ranges)
    }

    /// Constrói a mensagem WebSocket JSON Op 14 formatada exatamente conforme o protocolo Discord
    fn generate_opcode_14_payload(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> serde_json::Value {
        let mut channels_map = HashMap::new();
        channels_map.insert(channel_id.to_string(), ranges);

        json!({
            "op": 14,
            "d": {
                "guild_id": guild_id,
                "typing": true,
                "threads": true,
                "activities": true,
                "members": [],
                "channels": channels_map
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lazy_guild_opcode_14_generation() {
        let coordinator = LazyGuildCoordinator::new();

        let payload = coordinator
            .focus_guild_and_channel("guild_rust_brasil", "chan_geral")
            .await;

        assert_eq!(payload.get("op").and_then(|v| v.as_u64()), Some(14));

        let d = payload.get("d").expect("d field must exist");
        assert_eq!(
            d.get("guild_id").and_then(|v| v.as_str()),
            Some("guild_rust_brasil")
        );
        assert_eq!(d.get("typing").and_then(|v| v.as_bool()), Some(true));

        let channels = d.get("channels").expect("channels field must exist");
        let general_ranges = channels.get("chan_geral").expect("chan_geral ranges");
        let first_range = &general_ranges.as_array().unwrap()[0];
        assert_eq!(first_range[0].as_u64(), Some(0));
        assert_eq!(first_range[1].as_u64(), Some(99));
    }
}
