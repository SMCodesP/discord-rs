use super::*;
use crate::domain::message::Message;
use crate::domain::message::Reaction;
use crate::protocol::TransportEvent;

impl AppState {
    /// Gets message history for the currently selected channel without copying
    pub fn get_active_messages(&self) -> &[Message] {
        if let Some(ref cid) = self.selected_channel_id {
            self.messages_by_channel.get(cid).map(|v| v.as_slice()).unwrap_or(&[])
        } else {
            &[]
        }
    }

    /// Injects or appends a message to a channel's history
    pub fn inject_message(&mut self, channel_id: &str, message: Message) {
        crate::chat::markdown::prewarm_markdown_cache(&message.content);
        self.messages_by_channel
            .entry(channel_id.to_string())
            .or_default()
            .push(message);
    }

    /// Adds an emoji reaction to a message in state
    pub fn add_reaction_to_message(&mut self, channel_id: &str, message_id: &str, emoji: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                if let Some(rx) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                    rx.count += 1;
                    rx.me = true;
                } else {
                    msg.reactions.push(Reaction {
                        emoji: emoji.to_string(),
                        count: 1,
                        me: true,
                    });
                }
            }
        }
    }

    /// Deletes a message by ID from the specified channel
    pub fn delete_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            messages.retain(|m| m.id != message_id);
        }
    }

    /// Toggles the pinned status of a message by ID
    pub fn pin_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.pinned = !msg.pinned;
            }
        }
    }

    /// Sets / synchronizes real messages received from the API/Gateway for a channel
    pub fn set_channel_messages(&mut self, channel_id: &str, mut messages: Vec<Message>) {
        for msg in &messages {
            crate::chat::markdown::prewarm_markdown_cache(&msg.content);
        }
        // Preserva quaisquer mensagens otimistas/pendentes ainda não confirmadas no histórico carregado
        if let Some(existing) = self.messages_by_channel.get(channel_id) {
            for m in existing {
                if (m.is_pending || m.id.starts_with("temp-"))
                    && !messages.iter().any(|new_m| {
                        new_m.id == m.id
                            || (new_m.content == m.content
                                && (new_m.author.id == m.author.id
                                    || new_m.author.username == m.author.username
                                    || m.author.id == "user-me"
                                    || m.author.id == self.current_user.id))
                    })
                {
                    messages.push(m.clone());
                }
            }
        }
        self.messages_by_channel.insert(channel_id.to_string(), messages);
    }
}

impl AppState {
    /// Sends a chat message to the currently selected channel via ClientManager
    pub fn send_chat_message(&mut self, content: &str) {
        if let Some(cid) = self.selected_channel_id.clone() {
            self.send_message(&cid, content);
        }
    }

    /// Sends a message to a specific channel via ClientManager with optimistic UI update
    pub fn send_message(&mut self, channel_id: &str, content: &str) {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        let cid = channel_id.to_string();
        if let Some(ref tel) = self.telemetry {
            tel.track_message_send(&cid, trimmed.len(), true, 0);
            tel.track_button_click("send_message", "Enviar Mensagem", 0);
        }
        // Optimistically insert message into UI with temporary ID temp-<timestamp> and pending flag
        let new_id = format!("temp-{}", chrono::Utc::now().timestamp_millis());
        let new_msg = Message::new(
            new_id,
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: None,
                bot: false,
            },
            trimmed,
        )
        .with_pending(true);

        self.inject_message(&cid, new_msg);
        if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == cid) {
            dm.last_message_id = Some(format!("{}", chrono::Utc::now().timestamp_millis()));
            self.sort_dm_channels();
        }
        self.replying_to_message = None;
        self.active_draft = String::new();

        // Dispatch via ClientManager if present
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let target_cid = cid;
            let text = trimmed.to_string();
            tokio::spawn(async move {
                if let Err(e) = cm.send_message(&target_cid, &text).await {
                    log::warn!("Falha ao enviar mensagem via ClientManager: {:?}", e);
                }
            });
        }
    }

    /// Sends a sticker message to a specific channel immediately with authentic sticker rendering
    pub fn send_sticker(&mut self, channel_id: &str, sticker: &crate::protocol::media_cache::DiscordSticker) {
        let cid = channel_id.to_string();
        let new_id = format!("temp-{}", chrono::Utc::now().timestamp_millis());

        let mut new_msg = Message::new(
            new_id,
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: None,
                bot: false,
            },
            "",
        )
        .with_pending(true);

        new_msg.sticker_items.push(crate::domain::message::MessageStickerItem::new(
            sticker.id.clone(),
            sticker.name.clone(),
            sticker.format_type as u8,
        ));

        self.inject_message(&cid, new_msg);
        if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == cid) {
            dm.last_message_id = Some(format!("{}", chrono::Utc::now().timestamp_millis()));
            self.sort_dm_channels();
        }
        self.replying_to_message = None;

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let target_cid = cid;
            let sticker_id = sticker.id.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.send_sticker(&target_cid, &sticker_id).await {
                    log::warn!("Falha ao enviar figurinha via ClientManager: {:?}", e);
                }
            });
        }
    }

    /// Records emoji usage in local media cache
    pub fn record_emoji_usage(&self, emoji: &str) {
        let mgr = self.media_cache.clone();
        let em = emoji.to_string();
        tokio::spawn(async move {
            mgr.record_emoji_use(&em, &em, false, None).await;
        });
    }

    /// Returns custom emojis for the currently selected guild, or across all guilds
    pub fn get_current_custom_emojis(&self) -> Vec<crate::protocol::media_cache::DiscordEmoji> {
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                if !guild.emojis.is_empty() {
                    return guild.emojis.clone();
                }
            }
        }
        let mut all = Vec::new();
        for g in &self.guilds {
            all.extend(g.emojis.clone());
        }
        all
    }

    /// Returns custom stickers and favorites for the currently selected guild
    pub fn get_current_stickers(&self) -> Vec<crate::protocol::media_cache::DiscordSticker> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. Figurinhas favoritas persistidas
        for s in self.media_cache.get_favorite_stickers_sync() {
            if seen.insert(s.id.clone()) {
                result.push(s);
            }
        }

        // 2. Figurinhas do servidor selecionado (ou todos)
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                for s in &guild.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        } else {
            for g in &self.guilds {
                for s in &g.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        }
        result
    }

    /// Carrega emojis e figurinhas customizados de cada guilda a partir do cache em disco imediatamente
    /// e busca da API em segundo plano caso algum servidor ainda não os possua.
    pub fn sync_guild_media(&mut self) {
        let mut missing_emojis = Vec::new();
        let mut missing_stickers = Vec::new();

        // 1. Fast-path: carrega do cache particionado em disco
        for guild in &mut self.guilds {
            if guild.emojis.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_emojis_from_disk(&guild.id) {
                    guild.emojis = cached;
                }
            }
            if guild.stickers.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_stickers_from_disk(&guild.id) {
                    guild.stickers = cached;
                }
            }

            if guild.emojis.is_empty() {
                missing_emojis.push(guild.id.clone());
            }
            if guild.stickers.is_empty() {
                missing_stickers.push(guild.id.clone());
            }
        }

        // 2. Background-path: busca da API se houver ClientManager
        if let Some(ref cm) = self.client_manager {
            if !missing_emojis.is_empty() || !missing_stickers.is_empty() {
                let cm = cm.clone();
                tokio::spawn(async move {
                    for gid in missing_emojis {
                        if let Ok(emojis) = cm.get_guild_emojis(&gid).await {
                            if !emojis.is_empty() {
                                cm.emit_event(TransportEvent::GuildEmojisUpdated {
                                    guild_id: gid,
                                    emojis,
                                });
                            }
                        }
                    }
                    for gid in missing_stickers {
                        if let Ok(stickers) = cm.get_guild_stickers(&gid).await {
                            if !stickers.is_empty() {
                                cm.emit_event(TransportEvent::GuildStickersUpdated {
                                    guild_id: gid,
                                    stickers,
                                });
                            }
                        }
                    }
                });
            }
        }
    }

    /// Adds a reaction and dispatches it via ClientManager
    pub fn add_reaction(&mut self, message_id: &str, emoji: &str) {
        if let Some(ref tel) = self.telemetry {
            tel.track_reaction(emoji, message_id, true, 0);
        }
        let channel_id_opt = self.selected_channel_id.clone();
        if let Some(cid) = channel_id_opt {
            self.add_reaction_to_message(&cid, message_id, emoji);

            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                let cid = cid.clone();
                let mid = message_id.to_string();
                let em = emoji.to_string();
                tokio::spawn(async move {
                    if let Err(e) = cm.add_reaction(&cid, &mid, &em).await {
                        log::warn!("Falha ao adicionar reação via ClientManager: {:?}", e);
                    }
                });
            }
        }
    }

    // =========================================================================
    // Real-Time Coalesced Typing Tracker Subsystem
    // =========================================================================

    /// Records a typing event with 9-10 second coalescing window
    pub fn record_typing(&mut self, channel_id: &str, user_id: &str, username: Option<&str>) {
        let resolved_name = if let Some(un) = username {
            un.to_string()
        } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
            m.name.clone()
        } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
            f.display_name.clone()
        } else if let Some(dm) = self.dm_channels.iter().find(|d| d.id == user_id || d.name.to_lowercase() == user_id.to_lowercase()) {
            dm.name.clone()
        } else {
            user_id.to_string()
        };

        let channel_map = self.typing_tracker.entry(channel_id.to_string()).or_default();
        channel_map.insert(user_id.to_string(), (resolved_name, std::time::Instant::now()));

        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Clears typing status for a user in a channel (e.g. upon MessageCreate)
    pub fn clear_typing(&mut self, channel_id: &str, user_id: &str) {
        if let Some(channel_map) = self.typing_tracker.get_mut(channel_id) {
            channel_map.remove(user_id);
        }
        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Retrieves active typers in a channel whose events arrived within the 9-second TTL window
    pub fn get_active_typing_users(&self, channel_id: &str) -> Vec<String> {
        let now = std::time::Instant::now();
        if let Some(channel_map) = self.typing_tracker.get(channel_id) {
            let mut users: Vec<String> = channel_map
                .values()
                .filter(|(_, ts)| now.duration_since(*ts).as_secs() <= 9)
                .map(|(name, _)| name.clone())
                .collect();
            users.sort();
            users.dedup();
            users
        } else {
            Vec::new()
        }
    }

    /// Formats active typing users into localized natural language string
    pub fn get_active_typing_text(&self, channel_id: &str) -> Option<String> {
        let users = self.get_active_typing_users(channel_id);
        match users.len() {
            0 => None,
            1 => Some(format!("{} está digitando...", users[0])),
            2 => Some(format!("{} e {} estão digitando...", users[0], users[1])),
            3 => Some(format!("{}, {} e {} estão digitando...", users[0], users[1], users[2])),
            _ => Some("Várias pessoas estão digitando...".to_string()),
        }
    }
}
