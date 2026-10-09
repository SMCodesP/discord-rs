//! Ciclo de Vida do Cache e Revalidação Delta-Driven
//!
//! Implementa:
//! 1. Suporte a cabeçalhos `ETag` e status `HTTP 304 Not Modified` para requisições REST sem re-transferência.
//! 2. Revalidação cirúrgica orientada a eventos do Gateway (Zero Polling):
//!    `MESSAGE_CREATE`, `MESSAGE_UPDATE`, `MESSAGE_DELETE`, `GUILD_UPDATE`, `CHANNEL_UPDATE`.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::domain::channel::Channel;
use crate::domain::guild::Guild;
use crate::domain::message::Message;
use crate::domain::user::User;
use crate::protocol::cache::directory::CacheDirectory;

/// Entrada de cache com metadados para requisições condicionais HTTP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub data: T,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub validated_at: DateTime<Utc>,
}

impl<T> CacheEntry<T> {
    pub fn new(data: T, etag: Option<String>, last_modified: Option<String>) -> Self {
        Self {
            data,
            etag,
            last_modified,
            validated_at: Utc::now(),
        }
    }

    /// Atualiza o timestamp de validação sem alterar o conteúdo (304 Not Modified)
    pub fn touch(&mut self) {
        self.validated_at = Utc::now();
    }
}

/// Resultado do processamento de uma resposta HTTP condicional
#[derive(Debug)]
pub enum HttpCacheValidation<T> {
    /// O servidor confirmou que os dados locais estão atualizados (HTTP 304). 0 bytes de dados transferidos!
    NotModified,
    /// O servidor retornou novos dados e novo ETag (HTTP 200).
    Updated(CacheEntry<T>),
}

/// Revalidador de cache centralizado mantendo camada em memória (L1) e sincronização em disco (L2)
#[derive(Clone)]
pub struct CacheRevalidator {
    cache_dir: CacheDirectory,
    guilds: Arc<RwLock<HashMap<String, CacheEntry<Guild>>>>,
    channels: Arc<RwLock<HashMap<String, CacheEntry<Channel>>>>,
    messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    current_user: Arc<RwLock<Option<CacheEntry<User>>>>,
}

impl CacheRevalidator {
    pub fn new(cache_dir: CacheDirectory) -> Self {
        Self {
            cache_dir,
            guilds: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            current_user: Arc::new(RwLock::new(None)),
        }
    }

    // --- Suporte a ETags & HTTP 304 ---

    /// Obtém o cabeçalho ETag salvo para uso em `If-None-Match`
    pub async fn get_guild_etag(&self, guild_id: &str) -> Option<String> {
        let read = self.guilds.read().await;
        read.get(guild_id).and_then(|entry| entry.etag.clone())
    }

    pub async fn get_user_me_etag(&self) -> Option<String> {
        let read = self.current_user.read().await;
        read.as_ref().and_then(|entry| entry.etag.clone())
    }

    /// Processa resposta REST para /users/@me
    pub async fn process_user_me_response(
        &self,
        status: u16,
        etag: Option<String>,
        last_modified: Option<String>,
        user: Option<User>,
    ) -> Result<HttpCacheValidation<User>> {
        if status == 304 {
            let mut write = self.current_user.write().await;
            if let Some(entry) = write.as_mut() {
                entry.touch();
            }
            return Ok(HttpCacheValidation::NotModified);
        }

        if let Some(user) = user {
            let entry = CacheEntry::new(user.clone(), etag, last_modified);
            {
                let mut write = self.current_user.write().await;
                *write = Some(entry.clone());
            }
            let _ = self.cache_dir.save_current_user(&user);
            return Ok(HttpCacheValidation::Updated(entry));
        }

        anyhow::bail!("Invalid user response")
    }

    // --- Revalidação Cirúrgica Orientada a Eventos do Gateway (Zero Polling) ---

    /// Aplica delta de MESSAGE_CREATE
    pub async fn apply_message_create(&self, msg: Message) {
        let channel_id = msg.channel_id.clone();
        let mut msgs_map = self.messages.write().await;
        let channel_msgs = msgs_map.entry(channel_id.clone()).or_default();

        // Evita duplicatas se o client já possuir a mensagem por optimistic sending
        if let Some(existing) = channel_msgs.iter_mut().find(|m| m.id == msg.id) {
            *existing = msg.clone();
        } else {
            channel_msgs.push(msg.clone());
            // Mantém buffer de no máximo 200 mensagens em memória por canal
            if channel_msgs.len() > 200 {
                channel_msgs.remove(0);
            }
        }

        let msgs_snapshot = channel_msgs.clone();
        drop(msgs_map);

        // Persiste assincronamente em disco
        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            let _ = cache_dir.save_channel_messages(&channel_id, &msgs_snapshot);
        });
    }

    /// Aplica delta cirúrgico de MESSAGE_UPDATE (mantém autor e timestamp originais)
    pub async fn apply_message_update(&self, updated_msg: Message) {
        let channel_id = updated_msg.channel_id.clone();
        let mut msgs_map = self.messages.write().await;
        if let Some(channel_msgs) = msgs_map.get_mut(&channel_id) {
            if let Some(existing) = channel_msgs.iter_mut().find(|m| m.id == updated_msg.id) {
                existing.content = updated_msg.content;
                existing.edited_timestamp = updated_msg.edited_timestamp;
                existing.embeds = updated_msg.embeds;
                existing.attachments = updated_msg.attachments;
            }

            let msgs_snapshot = channel_msgs.clone();
            drop(msgs_map);

            let cache_dir = self.cache_dir.clone();
            tokio::spawn(async move {
                let _ = cache_dir.save_channel_messages(&channel_id, &msgs_snapshot);
            });
        }
    }

    /// Aplica delta cirúrgico de MESSAGE_DELETE
    pub async fn apply_message_delete(&self, channel_id: &str, message_id: &str) {
        let mut msgs_map = self.messages.write().await;
        if let Some(channel_msgs) = msgs_map.get_mut(channel_id) {
            channel_msgs.retain(|m| m.id != message_id);

            let msgs_snapshot = channel_msgs.clone();
            let c_id = channel_id.to_string();
            drop(msgs_map);

            let cache_dir = self.cache_dir.clone();
            tokio::spawn(async move {
                let _ = cache_dir.save_channel_messages(&c_id, &msgs_snapshot);
            });
        }
    }

    /// Aplica delta de GUILD_UPDATE
    pub async fn apply_guild_update(&self, guild: Guild) {
        let mut guilds_map = self.guilds.write().await;
        let entry = CacheEntry::new(guild.clone(), None, None);
        guilds_map.insert(guild.id.clone(), entry);
        drop(guilds_map);

        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            let _ = cache_dir.save_guild(&guild);
        });
    }

    /// Aplica delta de CHANNEL_UPDATE
    pub async fn apply_channel_update(&self, channel: Channel) {
        let mut channels_map = self.channels.write().await;
        let entry = CacheEntry::new(channel.clone(), None, None);
        channels_map.insert(channel.id.clone(), entry);
        drop(channels_map);

        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            let _ = cache_dir.save_channel(&channel);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Author;

    #[tokio::test]
    async fn test_etag_304_and_update_lifecycle() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_etag_test_{}", std::process::id()));
        let cache_dir = CacheDirectory::with_root(temp_dir);
        let revalidator = CacheRevalidator::new(cache_dir);

        let user = User::new("user_123", "TestUser");

        // 1. Initial 200 OK with ETag
        let res = revalidator
            .process_user_me_response(200, Some("\"etag_initial\"".to_string()), None, Some(user))
            .await?;

        match res {
            HttpCacheValidation::Updated(entry) => {
                assert_eq!(entry.data.username, "TestUser");
                assert_eq!(entry.etag.as_deref(), Some("\"etag_initial\""));
            }
            _ => panic!("Expected Updated"),
        }

        // Check stored etag
        let etag = revalidator.get_user_me_etag().await;
        assert_eq!(etag.as_deref(), Some("\"etag_initial\""));

        // 2. Subsequent 304 Not Modified
        let res_304 = revalidator
            .process_user_me_response(304, None, None, None)
            .await?;

        match res_304 {
            HttpCacheValidation::NotModified => {}
            _ => panic!("Expected NotModified"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_gateway_deltas_zero_polling() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_delta_test_{}", std::process::id()));
        let cache_dir = CacheDirectory::with_root(temp_dir);
        let revalidator = CacheRevalidator::new(cache_dir);

        let mut msg = Message::new(
            "msg_1",
            "chan_1",
            Author::new("author_1", "Author"),
            "Original Content",
        );

        // 1. Apply CREATE
        revalidator.apply_message_create(msg.clone()).await;

        {
            let map = revalidator.messages.read().await;
            assert_eq!(map.get("chan_1").unwrap().len(), 1);
            assert_eq!(map.get("chan_1").unwrap()[0].content, "Original Content");
        }

        // 2. Apply UPDATE
        msg.content = "Edited Content".to_string();
        revalidator.apply_message_update(msg).await;

        {
            let map = revalidator.messages.read().await;
            assert_eq!(map.get("chan_1").unwrap()[0].content, "Edited Content");
        }

        // 3. Apply DELETE
        revalidator.apply_message_delete("chan_1", "msg_1").await;

        {
            let map = revalidator.messages.read().await;
            assert_eq!(map.get("chan_1").unwrap().len(), 0);
        }

        Ok(())
    }
}
