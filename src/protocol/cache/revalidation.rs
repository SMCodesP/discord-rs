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
use tokio::sync::{Mutex, RwLock};

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
    message_revisions: Arc<RwLock<HashMap<String, u64>>>,
    message_write_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    current_user: Arc<RwLock<Option<CacheEntry<User>>>>,
}

impl CacheRevalidator {
    pub fn new(cache_dir: CacheDirectory) -> Self {
        let current_user = cache_dir
            .load_current_user()
            .map(|user| CacheEntry::new(user, None, None));
        let mut guilds = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(cache_dir.guilds_dir()) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(data) = std::fs::read_to_string(entry.path()) {
                    if let Ok(guild) = serde_json::from_str::<Guild>(&data) {
                        guilds.insert(guild.id.clone(), CacheEntry::new(guild, None, None));
                    }
                }
            }
        }
        let mut channels = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(cache_dir.channels_dir()) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(data) = std::fs::read_to_string(entry.path()) {
                    if let Ok(channel) = serde_json::from_str::<Channel>(&data) {
                        channels.insert(channel.id.clone(), CacheEntry::new(channel, None, None));
                    }
                }
            }
        }

        Self {
            cache_dir,
            guilds: Arc::new(RwLock::new(guilds)),
            channels: Arc::new(RwLock::new(channels)),
            messages: Arc::new(RwLock::new(HashMap::new())),
            message_revisions: Arc::new(RwLock::new(HashMap::new())),
            message_write_locks: Arc::new(Mutex::new(HashMap::new())),
            current_user: Arc::new(RwLock::new(current_user)),
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
            if write.is_none() {
                if let Some(user) = self.cache_dir.load_current_user() {
                    *write = Some(CacheEntry::new(user, None, None));
                }
            }
            let entry = write
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("304 recebido sem entrada local de /users/@me"))?;
            entry.touch();
            return Ok(HttpCacheValidation::NotModified);
        }

        if status != 200 {
            anyhow::bail!("Resposta HTTP inválida para /users/@me: {status}");
        }

        if let Some(user) = user {
            let entry = CacheEntry::new(user.clone(), etag, last_modified);
            {
                let mut write = self.current_user.write().await;
                *write = Some(entry.clone());
            }
            self.cache_dir.save_current_user(&user)?;
            return Ok(HttpCacheValidation::Updated(entry));
        }

        anyhow::bail!("Invalid user response")
    }

    // --- Revalidação Cirúrgica Orientada a Eventos do Gateway (Zero Polling) ---

    fn ensure_message_channel_loaded(
        &self,
        channel_id: &str,
        messages: &mut HashMap<String, Vec<Message>>,
    ) {
        if !messages.contains_key(channel_id) {
            messages.insert(
                channel_id.to_string(),
                self.cache_dir
                    .load_channel_messages(channel_id)
                    .unwrap_or_default(),
            );
        }
    }

    /// Persiste apenas o snapshot mais recente de cada canal. Sem essa checagem,
    /// duas tasks write-behind podem concluir fora de ordem e regravar dados velhos.
    async fn persist_message_snapshot(
        &self,
        channel_id: String,
        messages: Vec<Message>,
        revision: u64,
    ) {
        let write_lock = {
            let mut locks = self.message_write_locks.lock().await;
            locks
                .entry(channel_id.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = write_lock.lock().await;

        let is_latest = {
            let revisions = self.message_revisions.read().await;
            revisions.get(&channel_id).copied() == Some(revision)
        };
        if is_latest {
            let _ = self.cache_dir.save_channel_messages(&channel_id, &messages);
        }
    }

    async fn next_message_revision(&self, channel_id: &str) -> u64 {
        let mut revisions = self.message_revisions.write().await;
        let revision = revisions.entry(channel_id.to_string()).or_default();
        *revision = revision.saturating_add(1);
        *revision
    }

    /// Aplica delta de MESSAGE_CREATE.
    pub async fn apply_message_create(&self, msg: Message) {
        let channel_id = msg.channel_id.clone();
        let (msgs_snapshot, revision) = {
            let mut msgs_map = self.messages.write().await;
            self.ensure_message_channel_loaded(&channel_id, &mut msgs_map);
            let channel_msgs = msgs_map.entry(channel_id.clone()).or_default();

            if let Some(existing) = channel_msgs.iter_mut().find(|m| m.id == msg.id) {
                *existing = msg;
            } else {
                channel_msgs.push(msg);
                if channel_msgs.len() > 200 {
                    channel_msgs.remove(0);
                }
            }

            let snapshot = channel_msgs.clone();
            let revision = self.next_message_revision(&channel_id).await;
            drop(msgs_map);
            (snapshot, revision)
        };

        let this = self.clone();
        tokio::spawn(async move {
            this.persist_message_snapshot(channel_id, msgs_snapshot, revision)
                .await;
        });
    }

    /// Aplica delta cirúrgico de MESSAGE_UPDATE (mantém autor e timestamp originais).
    pub async fn apply_message_update(&self, updated_msg: Message) {
        let channel_id = updated_msg.channel_id.clone();
        let (snapshot, revision) = {
            let mut msgs_map = self.messages.write().await;
            self.ensure_message_channel_loaded(&channel_id, &mut msgs_map);
            let channel_msgs = msgs_map
                .get_mut(&channel_id)
                .expect("canal deve existir após hidratação");
            let Some(existing) = channel_msgs.iter_mut().find(|m| m.id == updated_msg.id) else {
                return;
            };

            existing.content = updated_msg.content;
            existing.edited_timestamp = updated_msg.edited_timestamp;
            existing.embeds = updated_msg.embeds;
            existing.attachments = updated_msg.attachments;

            let snapshot = channel_msgs.clone();
            let revision = self.next_message_revision(&channel_id).await;
            drop(msgs_map);
            (snapshot, revision)
        };

        let this = self.clone();
        tokio::spawn(async move {
            this.persist_message_snapshot(channel_id, snapshot, revision)
                .await;
        });
    }

    /// Aplica delta cirúrgico de MESSAGE_DELETE.
    pub async fn apply_message_delete(&self, channel_id: &str, message_id: &str) {
        let (snapshot, revision) = {
            let mut msgs_map = self.messages.write().await;
            self.ensure_message_channel_loaded(channel_id, &mut msgs_map);
            let channel_msgs = msgs_map
                .get_mut(channel_id)
                .expect("canal deve existir após hidratação");
            channel_msgs.retain(|m| m.id != message_id);

            let snapshot = channel_msgs.clone();
            let revision = self.next_message_revision(channel_id).await;
            drop(msgs_map);
            (snapshot, revision)
        };
        let channel_id = channel_id.to_string();

        let this = self.clone();
        tokio::spawn(async move {
            this.persist_message_snapshot(channel_id, snapshot, revision)
                .await;
        });
    }

    /// Aplica delta de GUILD_UPDATE
    pub async fn apply_guild_update(&self, guild: Guild) {
        let mut guilds_map = self.guilds.write().await;
        let etag = guilds_map
            .get(&guild.id)
            .and_then(|entry| entry.etag.clone());
        let entry = CacheEntry::new(guild.clone(), etag, None);
        guilds_map.insert(guild.id.clone(), entry);
        drop(guilds_map);

        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            if let Err(error) = cache_dir.save_guild(&guild) {
                log::warn!("Falha ao persistir atualização de servidor no cache: {error:#}");
            }
        });
    }

    /// Aplica delta de CHANNEL_UPDATE
    pub async fn apply_channel_update(&self, channel: Channel) {
        let mut channels_map = self.channels.write().await;
        let etag = channels_map
            .get(&channel.id)
            .and_then(|entry| entry.etag.clone());
        let entry = CacheEntry::new(channel.clone(), etag, None);
        channels_map.insert(channel.id.clone(), entry);
        drop(channels_map);

        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            if let Err(error) = cache_dir.save_channel(&channel) {
                log::warn!("Falha ao persistir atualização de canal no cache: {error:#}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Author;

    #[tokio::test]
    async fn test_etag_304_and_update_lifecycle() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("discord_rs_etag_test_{}", std::process::id()));
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
    async fn test_304_without_local_entry_is_an_error() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_etag_missing_test_{}",
            std::process::id()
        ));
        let revalidator = CacheRevalidator::new(CacheDirectory::with_root(temp_dir));
        assert!(
            revalidator
                .process_user_me_response(304, None, None, None)
                .await
                .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_restart_hydrates_user_for_304_and_rejects_wrong_status() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_etag_restart_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let cache_dir = CacheDirectory::with_root(temp_dir);
        cache_dir.save_current_user(&User::new("user_123", "TestUser"))?;

        let revalidator = CacheRevalidator::new(cache_dir.clone());
        assert!(matches!(
            revalidator
                .process_user_me_response(304, None, None, None)
                .await?,
            HttpCacheValidation::NotModified
        ));
        for status in [401, 429, 500] {
            assert!(
                revalidator
                    .process_user_me_response(
                        status,
                        None,
                        None,
                        Some(User::new("user_123", "TestUser"))
                    )
                    .await
                    .is_err()
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_first_delta_hydrates_existing_history_from_disk() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_history_hydration_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let cache_dir = CacheDirectory::with_root(temp_dir);
        let history: Vec<Message> = (0..100)
            .map(|index| {
                Message::new(
                    format!("msg_{index}"),
                    "chan_1",
                    Author::new("author_1", "Author"),
                    format!("Mensagem {index}"),
                )
            })
            .collect();
        cache_dir.save_channel_messages("chan_1", &history)?;

        let revalidator = CacheRevalidator::new(cache_dir.clone());
        revalidator
            .apply_message_create(Message::new(
                "msg_new",
                "chan_1",
                Author::new("author_1", "Author"),
                "Nova",
            ))
            .await;

        for _ in 0..100 {
            if cache_dir
                .load_channel_messages("chan_1")
                .is_some_and(|messages| messages.len() == 101)
            {
                let messages = cache_dir.load_channel_messages("chan_1").unwrap();
                assert!(messages.iter().any(|message| message.id == "msg_0"));
                assert!(messages.iter().any(|message| message.id == "msg_new"));
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        anyhow::bail!("história existente não foi preservada");
    }

    #[tokio::test]
    async fn test_user_cache_persistence_error_is_observable() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_cache_write_error_test_{}",
            std::process::id()
        ));
        let file_path = temp_dir.with_extension("file");
        std::fs::write(&file_path, b"not-a-directory")?;
        let revalidator = CacheRevalidator::new(CacheDirectory::with_root(file_path.join("cache")));
        assert!(
            revalidator
                .process_user_me_response(200, None, None, Some(User::new("user_123", "TestUser")))
                .await
                .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_write_behind_never_regresses_to_an_older_snapshot() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_write_behind_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let cache_dir = CacheDirectory::with_root(temp_dir);
        let revalidator = CacheRevalidator::new(cache_dir.clone());
        let mut msg = Message::new(
            "msg_1",
            "chan_1",
            Author::new("author_1", "Author"),
            "Original",
        );

        revalidator.apply_message_create(msg.clone()).await;
        msg.content = "Edited".to_string();
        revalidator.apply_message_update(msg).await;
        revalidator.apply_message_delete("chan_1", "msg_1").await;

        for _ in 0..50 {
            if cache_dir
                .load_channel_messages("chan_1")
                .is_some_and(|messages| messages.is_empty())
            {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        anyhow::bail!("O snapshot mais recente não foi persistido");
    }

    #[tokio::test]
    async fn test_gateway_deltas_zero_polling() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("discord_rs_delta_test_{}", std::process::id()));
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
