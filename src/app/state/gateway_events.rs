use super::*;
use crate::domain::channel::ChannelType;
use crate::domain::settings::ThemeChoice;
use crate::protocol::ConnectionMode;
use crate::domain::message::Reaction;
use crate::protocol::TransportEvent;
use std::collections::HashMap;

impl AppState {
    /// Enqueues a transport event for coalescing. Returns it immediately when it
    /// should not be batched.
    pub fn push_transport_event(
        &mut self,
        event: TransportEvent,
        now: std::time::Instant,
    ) -> Option<TransportEvent> {
        self.event_batcher.push(event, now)
    }

    /// Flushes coalesced transport events whose deadline has elapsed.
    pub fn flush_due_transport_events(&mut self, now: std::time::Instant) -> Vec<TransportEvent> {
        self.event_batcher.flush(now)
    }

    /// Next deadline for pending coalesced transport events.
    pub fn next_transport_batch_deadline(&self) -> Option<std::time::Instant> {
        self.event_batcher.next_deadline()
    }

    /// Applies a batch of transport events and preserves side effects that must
    /// happen at the UI dispatch boundary.
    fn event_affects_visible_ui(&self, event: &TransportEvent) -> bool {
        let selected_ch = self.selected_channel_id.as_deref();
        match event {
            TransportEvent::MessageCreate(msg) | TransportEvent::MessageUpdate(msg) => {
                selected_ch == Some(msg.channel_id.as_str())
                    || self.dm_channels.iter().any(|dm| dm.id == msg.channel_id)
            }
            TransportEvent::MessageDelete { channel_id, .. }
            | TransportEvent::TypingStart { channel_id, .. } => {
                selected_ch == Some(channel_id.as_str())
            }
            TransportEvent::ReactionAdd { channel_id, .. }
            | TransportEvent::ReactionRemove { channel_id, .. } => {
                selected_ch == Some(channel_id.as_str())
            }
            TransportEvent::PresenceUpdate { user_id, .. } => {
                self.selected_guild_id.is_none()
                    || self.friends.iter().any(|f| f.id == *user_id)
                    || self.members.iter().any(|m| m.id == *user_id)
            }
            _ => true,
        }
    }

    pub fn apply_transport_events(&mut self, events: Vec<TransportEvent>, cx: &mut gpui::Context<Self>) {
        let mut should_notify = false;
        let mut theme_updated = false;
        for event in events {
            if !should_notify && self.event_affects_visible_ui(&event) {
                should_notify = true;
            }
            if matches!(event, TransportEvent::AccountThemeUpdated { .. }) {
                theme_updated = true;
            }
            let deleted_call_channel = match &event {
                TransportEvent::CallDelete { channel_id } => Some(channel_id.clone()),
                _ => None,
            };
            self.handle_transport_event(event);
            if let Some(channel_id) = deleted_call_channel {
                if self
                    .active_voice_call
                    .as_ref()
                    .map(|call| call.channel_id.as_str())
                    == Some(channel_id.as_str())
                {
                    self.disconnect_voice();
                    should_notify = true;
                }
            }
        }
        if theme_updated {
            self.apply_active_theme(cx);
        }
        if should_notify {
            cx.notify();
        }
    }

    /// Dispatches a real-time event received from the Gateway / ClientManager
    pub fn handle_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::Ready {
                user,
                guilds,
                guild_folders,
                read_state,
            } => {
                let display_name = user.display_name().to_string();
                let avatar_text = user
                    .username
                    .chars()
                    .next()
                    .unwrap_or('U')
                    .to_uppercase()
                    .to_string();
                let avatar_url = user.avatar_url();
                let username = user.username;
                let ready_status =
                    crate::components::status_picker::UserStatusSelection::from_presence_status(
                        user.presence,
                    );
                let ready_custom_status = user
                    .custom_status
                    .as_ref()
                    .map(crate::components::status_picker::CustomStatusDraft::from_custom_status)
                    .unwrap_or_default();
                self.current_user.id = user.id;
                self.current_user.username = username.clone();
                self.current_user.display_name = display_name;
                self.current_user.discriminator = user.discriminator;
                self.current_user.avatar_text = avatar_text;
                self.current_user.avatar_url = avatar_url;
                self.current_user.avatar_decoration_data = user.avatar_decoration_data;
                self.current_user.custom_status = ready_custom_status.text.clone();
                self.current_status_selection = ready_status;
                self.current_custom_status = ready_custom_status;
                self.status_picker_status = ready_status;
                self.is_connected_to_gateway = true;
                self.is_gateway_reconnecting = false;
                self.reconnect_attempt = None;
                self.reconnect_delay_ms = None;
                self.reconnect_reason = None;
                self.is_connecting = false;
                self.connection_mode = ConnectionMode::LiveDiscord(username);
                self.connection_status = format!("Conectado como @{}", self.current_user.username);
                self.auth_error = None;
                self.token_input.clear();
                self.guild_folders = guild_folders;

                // Persiste apenas depois de uma confirmação real do Discord.
                if let Some(token) = self.auth_token.as_ref() {
                    let auth = if self.is_user_token {
                        crate::protocol::AuthToken::user(token.clone())
                    } else {
                        crate::protocol::AuthToken::bot(token.clone())
                    };
                    let storage = crate::protocol::TokenStorage::default();
                    let result = if self.save_token_to_disk {
                        storage.save(&auth)
                    } else {
                        storage.clear()
                    };
                    if let Err(error) = result {
                        log::warn!(
                            "Não foi possível atualizar as credenciais salvas: {:#}",
                            error
                        );
                    }
                }
                self.close_modal();

                // Limpar mensagens fictícias para exibir apenas dados reais carregados da API
                self.messages_by_channel.clear();
                self.history_pagination.clear();

                for g in &guilds {
                    crate::protocol::cache::lazy_guilds::cache_guild_roles(&g.id, g.roles.clone());
                }
                let mut new_guilds = Vec::new();
                for g in guilds {
                    let icon_url = g.icon_url();
                    let icon_hash = g.icon.clone();
                    let categories =
                        super::seed::organize_channels_into_categories(&g.id, g.channels);

                    let emojis = if !g.emojis.is_empty() {
                        let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                        g.emojis
                    } else if let Some(existing) = self
                        .guilds
                        .iter()
                        .find(|guild| guild.id == g.id)
                        .map(|guild| guild.emojis.clone())
                    {
                        if !existing.is_empty() {
                            existing
                        } else {
                            self.media_cache
                                .load_guild_emojis_from_disk(&g.id)
                                .unwrap_or_default()
                        }
                    } else {
                        self.media_cache
                            .load_guild_emojis_from_disk(&g.id)
                            .unwrap_or_default()
                    };

                    let stickers = if !g.stickers.is_empty() {
                        let _ = self
                            .media_cache
                            .save_guild_stickers_to_disk(&g.id, &g.stickers);
                        g.stickers
                    } else if let Some(existing) = self
                        .guilds
                        .iter()
                        .find(|guild| guild.id == g.id)
                        .map(|guild| guild.stickers.clone())
                    {
                        if !existing.is_empty() {
                            existing
                        } else {
                            self.media_cache
                                .load_guild_stickers_from_disk(&g.id)
                                .unwrap_or_default()
                        }
                    } else {
                        self.media_cache
                            .load_guild_stickers_from_disk(&g.id)
                            .unwrap_or_default()
                    };

                    new_guilds.push(GuildItemState {
                        id: g.id.clone(),
                        name: g.name.clone(),
                        acronym: g.name.chars().take(3).collect(),
                        icon_url,
                        icon_hash,
                        unread: false,
                        mention_count: 0,
                        categories,
                        emojis,
                        stickers,
                    });
                }

                // Ordenar com base na ordem das pastas de servidores
                if !self.guild_folders.is_empty() {
                    let mut order_map = HashMap::new();
                    let mut idx = 0;
                    for folder in &self.guild_folders {
                        for gid in &folder.guild_ids {
                            order_map.entry(gid.clone()).or_insert(idx);
                            idx += 1;
                        }
                    }
                    new_guilds.sort_by_key(|g| order_map.get(&g.id).copied().unwrap_or(usize::MAX));
                }

                self.guilds = new_guilds;
                self.sync_guild_media();
                save_guilds_to_disk(&self.guilds, &self.guild_folders);

                if let Some(read_state) = read_state {
                    self.read_state_version = read_state.version;
                    for entry in &read_state.entries {
                        let channel_id = entry.id.trim();
                        if channel_id.is_empty() { continue; }
                        let last_read = crate::protocol::read_states::normalize_snowflake_opt(entry.last_message_id.as_deref());
                        if let Some(ch) = self.channel_state_mut_by_id(channel_id) {
                            ch.last_read_message_id = last_read.clone();
                            ch.mention_count = entry.mention_count;
                        } else {
                            // Keep read state even if channel not known yet by creating no UI entry; handled on demand.
                        }
                        // Recompute unread after setting last_read/mentions if channel known.
                        let known = self.channel_state_ref_by_id(channel_id).is_some();
                        if known {
                            self.recompute_channel_unread_flags(channel_id);
                        }
                    }
                }

                if let Some(first) = self.guilds.first() {
                    self.select_guild(Some(first.id.clone()));
                } else {
                    self.selected_guild_id = None;
                    self.selected_channel_id = None;
                }
            }
            TransportEvent::MessageCreate(msg) => {
                self.clear_typing(&msg.channel_id, &msg.author.id);
                crate::chat::markdown::prewarm_message_markdown_cache(&msg.id, &msg.content);
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                let channel_id = msg.channel_id.clone();

                // 1. Identificar com precisão se a mensagem pertence a uma GUILDA/SERVIDOR.
                // Uma mensagem é estritamente de guilda se:
                // - msg.guild_id.is_some(), OU
                // - o canal pertencer a qualquer guilda conhecida em self.guilds.
                let is_guild_message = msg.guild_id.is_some()
                    || self.guilds.iter().any(|g| {
                        g.categories
                            .iter()
                            .any(|cat| cat.channels.iter().any(|c| c.id == channel_id))
                    });

                // Se NÃO for mensagem de guilda, verificar se havia mensagem pendente em canal temporário dm-*
                if !is_guild_message {
                    for (temp_cid, temp_msgs) in self.messages_by_channel.iter_mut() {
                        if temp_cid.starts_with("dm-") && temp_cid != &channel_id {
                            if let Some(pos) = temp_msgs.iter().position(|m| {
                                (m.id.starts_with("temp-") || m.is_pending)
                                    && (m.author.id == msg.author.id
                                        || m.author.username == msg.author.username
                                        || m.author.id == "user-me"
                                        || m.author.id == self.current_user.id)
                                    && m.content == msg.content
                            }) {
                                let removed = temp_msgs.remove(pos);
                                crate::app::state::messages::mark_pending_send_confirmed(
                                    &removed.id,
                                    &msg.id,
                                );
                                break;
                            }
                        }
                    }
                }

                let channel_msgs = self
                    .messages_by_channel
                    .entry(channel_id.clone())
                    .or_default();

                // Substituição otimista: se existir uma mensagem temporária pendente com mesmo conteúdo e autor
                if let Some(pos) = channel_msgs.iter().position(|m| {
                    (m.id.starts_with("temp-") || m.is_pending)
                        && (m.author.id == msg.author.id
                            || m.author.username == msg.author.username
                            || m.author.id == "user-me"
                            || m.author.id == self.current_user.id)
                        && m.content == msg.content
                }) {
                    let temp_id = channel_msgs[pos].id.clone();
                    crate::app::state::messages::mark_pending_send_confirmed(&temp_id, &msg.id);
                    channel_msgs[pos] = msg.clone();
                } else if !channel_msgs.iter().any(|m| m.id == msg.id) {
                    channel_msgs.push(msg.clone());
                }

                let is_selected = self.selected_channel_id.as_deref() == Some(&channel_id);
                let from_current_user = msg.author.id == self.current_user.id;
                let mention_for_me = if from_current_user {
                    0
                } else {
                    let mentions_me = msg.content.contains(&format!("<@{}>", self.current_user.id))
                        || msg.content.contains(&format!("<@!{}>", self.current_user.id))
                        || msg.content.contains("@everyone")
                        || msg.content.contains("@here");
                    if mentions_me { 1 } else { 0 }
                };

                if is_guild_message {
                    let guild_id_for_channel = self.guilds.iter().find(|g| {
                        g.categories.iter().any(|cat| cat.channels.iter().any(|c| c.id == channel_id))
                    }).map(|g| g.id.clone());
                    if let Some(ch) = self.channel_state_mut_by_id(&channel_id) {
                        if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), ch.last_message_id.as_deref()) {
                            ch.last_message_id = Some(msg.id.clone());
                        }
                        if from_current_user {
                            if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), ch.last_read_message_id.as_deref()) {
                                ch.last_read_message_id = Some(msg.id.clone());
                            }
                            ch.mention_count = 0;
                            ch.unread = false;
                        } else if is_selected {
                            // Visible channel: keep read marker at latest.
                            if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), ch.last_read_message_id.as_deref()) {
                                ch.last_read_message_id = Some(msg.id.clone());
                            }
                            ch.mention_count = 0;
                            ch.unread = false;
                        } else {
                            ch.mention_count = ch.mention_count.saturating_add(mention_for_me);
                            ch.unread = crate::protocol::is_message_unread(ch.last_message_id.as_deref(), ch.last_read_message_id.as_deref());
                        }
                    }
                    if let Some(gid) = guild_id_for_channel {
                        self.recompute_guild_unread(&gid);
                    }
                    if is_selected && !from_current_user {
                        self.mark_channel_read_up_to(&channel_id, &msg.id, false);
                    }
                    // MENSAGEM DE GUILDA NUNCA MODIFICA dm_channels NEM selected_channel_id DE DM!
                } else {
                    // Mensagem comprovadamente de DM privada (msg.guild_id.is_none() e canal não pertence a nenhuma guilda)
                    let mut dm_updated = false;

                    // 1. Tentar correspondência EXATA com o ID do canal de DM existente
                    if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == channel_id) {
                        if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), dm.last_message_id.as_deref()) {
                            dm.last_message_id = Some(msg.id.clone());
                        }
                        if from_current_user || self.selected_channel_id.as_deref() == Some(&channel_id) {
                            if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), dm.last_read_message_id.as_deref()) {
                                dm.last_read_message_id = Some(msg.id.clone());
                            }
                            dm.mention_count = 0;
                            dm.unread = false;
                        } else {
                            dm.mention_count = dm.mention_count.saturating_add(mention_for_me);
                            dm.unread = crate::protocol::is_message_unread(dm.last_message_id.as_deref(), dm.last_read_message_id.as_deref());
                        }
                        dm_updated = true;
                    } else if let Some(dm) = self.dm_channels.iter_mut().find(|c| {
                        // 2. Apenas se for canal temporário dm-* ainda não resolvido E o destinatário bater com o autor
                        c.id.starts_with("dm-") && c.recipient_ids.contains(&msg.author.id)
                    }) {
                        let temp_id = dm.id.clone();
                        dm.id = channel_id.clone();
                        if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), dm.last_message_id.as_deref()) {
                            dm.last_message_id = Some(msg.id.clone());
                        }
                        if from_current_user || self.selected_channel_id.as_deref() == Some(&channel_id) {
                            if crate::protocol::is_newer_snowflake(Some(msg.id.as_str()), dm.last_read_message_id.as_deref()) {
                                dm.last_read_message_id = Some(msg.id.clone());
                            }
                            dm.mention_count = 0;
                            dm.unread = false;
                        } else {
                            dm.mention_count = dm.mention_count.saturating_add(mention_for_me);
                            dm.unread = crate::protocol::is_message_unread(dm.last_message_id.as_deref(), dm.last_read_message_id.as_deref());
                        }
                        dm_updated = true;

                        // Se a seleção atual estiver no temporário correspondente, migra para o canal real da DM
                        if self.selected_channel_id.as_deref() == Some(&temp_id) {
                            self.selected_channel_id = Some(channel_id.clone());
                        }

                        // Migra mensagens pendentes do canal temporário para o canal real
                        if temp_id != channel_id {
                            if let Some(mut pending_msgs) =
                                self.messages_by_channel.remove(&temp_id)
                            {
                                for m in &mut pending_msgs {
                                    m.channel_id = channel_id.clone();
                                }
                                let target_msgs = self
                                    .messages_by_channel
                                    .entry(channel_id.clone())
                                    .or_default();
                                for p in pending_msgs {
                                    if !target_msgs.iter().any(|m| m.id == p.id) {
                                        target_msgs.push(p);
                                    }
                                }
                            }
                        }
                    }

                    if dm_updated {
                        self.sort_dm_channels();
                        if self.selected_channel_id.as_deref() == Some(&channel_id) && !from_current_user {
                            self.mark_channel_read_up_to(&channel_id, &msg.id, false);
                        }
                    }

                    // Se a seleção atual estiver em dm-{user_id} e esta DM real corresponde a esse mesmo user_id, migra a seleção
                    if let Some(ref sel) = self.selected_channel_id.clone() {
                        if sel.starts_with("dm-") && sel != &channel_id {
                            let uid = sel.trim_start_matches("dm-");
                            if uid == msg.author.id
                                && self.dm_channels.iter().any(|d| {
                                    d.id == channel_id && d.recipient_ids.contains(&uid.to_string())
                                })
                            {
                                self.selected_channel_id = Some(channel_id.clone());
                            }
                        }
                    }
                }
            }
            TransportEvent::ChannelMessagesLoaded {
                channel_id,
                messages,
            } => {
                self.set_channel_messages(&channel_id, messages);
            }
            TransportEvent::MessageUpdate(msg) => {
                crate::chat::markdown::prewarm_message_markdown_cache(&msg.id, &msg.content);
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                if let Some(msgs) = self.messages_by_channel.get_mut(&msg.channel_id) {
                    if let Some(existing) = msgs.iter_mut().find(|m| m.id == msg.id) {
                        *existing = msg;
                    }
                }
            }
            TransportEvent::MessageDelete {
                channel_id,
                message_id,
            } => {
                if let Some(msgs) = self.messages_by_channel.get_mut(&channel_id) {
                    msgs.retain(|m| m.id != message_id);
                }
            }
            TransportEvent::MessageDeleteFailed { .. } => {
                log::warn!("Falha ao deletar mensagem no canal.");
            }
            TransportEvent::MessageSendFailed {
                temp_id,
                channel_id,
                content,
                error,
            } => {
                self.handle_message_send_failure_event(&temp_id, &channel_id, &content, &error);
            }
            TransportEvent::GuildMemberListUpdate(update) => {
                if self.selected_guild_id.as_deref() == Some(&update.guild_id) {
                    let guild_roles = crate::protocol::cache::lazy_guilds::get_cached_guild_roles(
                        &update.guild_id,
                    );
                    crate::protocol::cache::lazy_guilds::apply_member_list_update(
                        &mut self.members,
                        &update,
                        &guild_roles,
                    );
                }
            }
            TransportEvent::GuildMembersChunk {
                guild_id,
                members,
                not_found,
            } => {
                for uid in &not_found {
                    crate::protocol::cache::lazy_guilds::mark_op8_requested(&guild_id, uid);
                }
                if self.selected_guild_id.as_deref() == Some(&guild_id) {
                    for m in &members {
                        if let Some(pos) = self.members.iter().position(|em| em.id == m.id) {
                            if m.role_color != 0 {
                                self.members[pos].role_color = m.role_color;
                            }
                        }
                    }
                }
            }
            TransportEvent::ReactionAdd {
                channel_id,
                message_id,
                user_id,
                emoji_name,
                emoji_id,
                emoji_animated,
            } => {
                let is_me = user_id == self.current_user.id;
                let rx = Reaction::from_parts(emoji_name, emoji_id, emoji_animated, 1, is_me);
                self.add_reaction_to_message(&channel_id, &message_id, rx);
            }
            TransportEvent::ReactionRemove {
                channel_id,
                message_id,
                user_id,
                emoji_name,
                emoji_id,
                ..
            } => {
                let is_me = user_id == self.current_user.id;
                let key = crate::domain::message::reaction_key(&emoji_name, emoji_id.as_deref());
                self.remove_reaction_from_message(&channel_id, &message_id, &key, is_me);
            }
            TransportEvent::ReactionReconcile {
                channel_id,
                message_id,
                reaction,
            } => {
                self.reconcile_reaction(&channel_id, &message_id, reaction);
            }
            TransportEvent::ReactionRollback {
                channel_id,
                message_id,
                reactions,
            } => {
                if let Some(messages) = self.messages_by_channel.get_mut(&channel_id) {
                    if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                        msg.reactions = reactions;
                    }
                }
            }
            TransportEvent::TypingStart {
                channel_id,
                user_id,
                ..
            } => {
                self.record_typing(&channel_id, &user_id, None);
            }
            TransportEvent::Connected => {
                self.is_connected_to_gateway = true;
                self.is_gateway_reconnecting = false;
                self.reconnect_attempt = None;
                self.reconnect_delay_ms = None;
                self.reconnect_reason = None;
                self.is_connecting = false;
                self.connection_status = "Conectado".to_string();
                self.auth_error = None;
                self.close_modal();
            }
            TransportEvent::Reconnecting { attempt, delay_ms, reason } => {
                let was_authenticated = self.auth_token.is_some() || !self.current_user.id.is_empty();
                self.is_connecting = false;
                self.reconnect_attempt = Some(attempt);
                self.reconnect_delay_ms = Some(delay_ms);
                self.reconnect_reason = Some(reason.clone());
                if was_authenticated {
                    self.is_connected_to_gateway = true;
                    self.is_gateway_reconnecting = true;
                    self.connection_mode = ConnectionMode::Reconnecting { attempt, delay_ms, reason: reason.clone() };
                    self.connection_status = format!("Reconectando (tentativa {})...", attempt);
                } else {
                    self.is_connected_to_gateway = false;
                    self.is_gateway_reconnecting = false;
                    self.connection_mode = ConnectionMode::Connecting;
                    self.connection_status = format!("Conectando (tentativa {})...", attempt);
                }
            }
            TransportEvent::Disconnected(reason) => {
                let was_authenticated = self.auth_token.is_some() || !self.current_user.id.is_empty();
                let is_auth_fatal = reason.contains("token inválido")
                    || reason.contains("Faça login novamente")
                    || reason.contains("4004")
                    || reason.contains("4010")
                    || reason.contains("4011")
                    || reason.contains("4012")
                    || reason.contains("4013")
                    || reason.contains("4014");
                let is_explicit = reason == "Disconnected by client"
                    || reason == "Desconectado"
                    || reason == "Mock provider disconnected"
                    || reason == "ProtocolBridge disconnected";
                self.is_connecting = false;
                self.is_gateway_reconnecting = false;
                self.reconnect_attempt = None;
                self.reconnect_delay_ms = None;
                self.reconnect_reason = if was_authenticated && !is_auth_fatal && !is_explicit {
                    Some(reason.clone())
                } else {
                    None
                };
                if was_authenticated && !is_auth_fatal && !is_explicit {
                    // Preserve authenticated shell/cache; show offline banner.
                    self.is_connected_to_gateway = true;
                    self.auth_error = Some(reason.clone());
                    self.connection_mode = ConnectionMode::Disconnected(reason.clone());
                    self.connection_status = format!("Desconectado: {}", reason);
                } else {
                    self.is_connected_to_gateway = false;
                    self.auth_error = Some(reason.clone());
                    self.connection_mode = ConnectionMode::Disconnected(reason.clone());
                    self.connection_status = format!("Desconectado: {}", reason);
                }
            }
            TransportEvent::VoiceStateUpdate(state) => {
                self.update_voice_state(state);
            }
            TransportEvent::GuildChannelsUpdated { guild_id, channels } => {
                let categories =
                    super::seed::organize_channels_into_categories(&guild_id, channels);

                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.categories = categories;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);

                if self.selected_guild_id.as_deref() == Some(&guild_id)
                    && self.selected_channel_id.is_none()
                {
                    if let Some(guild) = self.guilds.iter().find(|g| g.id == guild_id) {
                        for cat in &guild.categories {
                            if let Some(first_text) =
                                cat.channels.iter().find(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum | ChannelKind::Directory))
                            {
                                self.select_channel(first_text.id.clone());
                                break;
                            }
                        }
                    }
                }
            }
            TransportEvent::GuildCreate(g) => {
                crate::protocol::cache::lazy_guilds::cache_guild_roles(&g.id, g.roles.clone());
                let icon_url = g.icon_url();
                let icon_hash = g.icon.clone();
                let categories = super::seed::organize_channels_into_categories(&g.id, g.channels);

                let emojis = if !g.emojis.is_empty() {
                    let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                    g.emojis
                } else if let Some(existing) = self
                    .guilds
                    .iter()
                    .find(|guild| guild.id == g.id)
                    .map(|guild| guild.emojis.clone())
                {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache
                            .load_guild_emojis_from_disk(&g.id)
                            .unwrap_or_default()
                    }
                } else {
                    self.media_cache
                        .load_guild_emojis_from_disk(&g.id)
                        .unwrap_or_default()
                };

                let stickers = if !g.stickers.is_empty() {
                    let _ = self
                        .media_cache
                        .save_guild_stickers_to_disk(&g.id, &g.stickers);
                    g.stickers
                } else if let Some(existing) = self
                    .guilds
                    .iter()
                    .find(|guild| guild.id == g.id)
                    .map(|guild| guild.stickers.clone())
                {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache
                            .load_guild_stickers_from_disk(&g.id)
                            .unwrap_or_default()
                    }
                } else {
                    self.media_cache
                        .load_guild_stickers_from_disk(&g.id)
                        .unwrap_or_default()
                };

                let item = GuildItemState {
                    id: g.id.clone(),
                    name: g.name.clone(),
                    acronym: g.name.chars().take(3).collect(),
                    icon_url,
                    icon_hash,
                    unread: false,
                    mention_count: 0,
                    categories,
                    emojis,
                    stickers,
                };
                if let Some(pos) = self.guilds.iter().position(|guild| guild.id == g.id) {
                    self.guilds[pos] = item;
                } else {
                    self.guilds.push(item);
                }
                if !self.guild_folders.is_empty() {
                    let mut order_map = HashMap::new();
                    let mut idx = 0;
                    for folder in &self.guild_folders {
                        for gid in &folder.guild_ids {
                            order_map.entry(gid.clone()).or_insert(idx);
                            idx += 1;
                        }
                    }
                    self.guilds
                        .sort_by_key(|g| order_map.get(&g.id).copied().unwrap_or(usize::MAX));
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildEmojisUpdated { guild_id, emojis } => {
                let _ = self
                    .media_cache
                    .save_guild_emojis_to_disk(&guild_id, &emojis);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.emojis = emojis;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildStickersUpdated { guild_id, stickers } => {
                let _ = self
                    .media_cache
                    .save_guild_stickers_to_disk(&guild_id, &stickers);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.stickers = stickers;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildNicknameUpdated { guild_id, user_id, nick } => {
                if user_id == self.current_user.id {
                    if let Some(member) = self
                        .members
                        .iter_mut()
                        .find(|m| m.id == user_id && m.guild_id.as_deref() == Some(&guild_id))
                    {
                        member.nickname = nick.clone();
                    }
                    self.guild_nickname_draft = nick.unwrap_or_default();
                    self.pending_guild_action = None;
                    self.close_modal();
                }
            }
            TransportEvent::GuildLeft { guild_id } => {
                self.pending_guild_action = None;
                self.close_modal();
                self.apply_guild_left(&guild_id);
            }
            TransportEvent::GuildActionFailed { guild_id: _, action, error } => {
                self.pending_guild_action = None;
                self.auth_error = Some(format!("Ação de servidor falhou ({}): {}", action, error));
            }
            TransportEvent::DmChannelsUpdated(channels) => {
                let mut new_dms: Vec<ChannelItemState> = channels
                    .into_iter()
                    .map(|c| {
                        let is_group = c.channel_type == ChannelType::GroupDm;
                        let is_pinned = c.is_pinned || self.pinned_dms.contains(&c.id);
                        let avatar_decoration_data =
                            c.recipient_avatar_decorations.first().cloned().flatten();
                        ChannelItemState {
                            id: c.id,
                            name: c.name,
                            kind: ChannelKind::Text,
                            topic: c.topic,
                            unread: false,
                            mention_count: 0,
                            last_read_message_id: None,
                            icon_url: c.icon_url,
                            is_pinned,
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                            user_limit: None,
                            avatar_decoration_data,
                            ..Default::default()
                        }
                    })
                    .collect();

                // Preservar canais temporários legados de fixture sem
                // reutilizá-los como dados de produção. O filtro existente
                // continua isolado do fluxo de login real.
                for existing in &self.dm_channels {
                    if existing.id == "dm-ferris"
                        || existing.id == "dm-group-gpui"
                        || existing.id == "dm-ana"
                        || existing.recipient_ids.iter().any(|r| r == "user-ferris")
                        || existing.name.eq_ignore_ascii_case("ferris")
                    {
                        continue;
                    }

                    if existing.id.starts_with("dm-") {
                        let already_resolved = new_dms.iter().any(|d| {
                            existing
                                .recipient_ids
                                .iter()
                                .any(|uid| d.recipient_ids.contains(uid))
                        });
                        if !already_resolved && !new_dms.iter().any(|d| d.id == existing.id) {
                            new_dms.push(existing.clone());
                        }
                    }
                }

                self.dm_channels = new_dms;
                self.sort_dm_channels();

                if self.selected_guild_id.is_none() && self.selected_channel_id.is_none() {
                    if let Some(first_dm) = self.dm_channels.first() {
                        self.select_channel(first_dm.id.clone());
                    }
                }
            }
            TransportEvent::DmChannelCreated(c) => {
                let is_group = c.channel_type == ChannelType::GroupDm;
                let real_id = c.id.clone();
                let avatar_decoration_data =
                    c.recipient_avatar_decorations.first().cloned().flatten();
                let mut temp_id_to_migrate: Option<String> = None;
                if let Some(existing) = self.dm_channels.iter_mut().find(|d| {
                    d.id == real_id
                        || (d.id.starts_with("dm-")
                            && !d.is_group_dm
                            && !c.recipient_ids.is_empty()
                            && c.recipient_ids
                                .iter()
                                .any(|uid| d.recipient_ids.contains(uid)))
                }) {
                    if existing.id.starts_with("dm-") {
                        temp_id_to_migrate = Some(existing.id.clone());
                    }
                    existing.id = real_id.clone();
                    existing.name = c.name;
                    existing.icon_url = c.icon_url;
                    existing.recipient_ids = c.recipient_ids;
                    existing.recipient_names = c.recipient_names;
                    existing.is_group_dm = is_group;
                    existing.avatar_decoration_data = avatar_decoration_data;
                } else {
                    self.dm_channels.insert(
                        0,
                        ChannelItemState {
                            id: real_id.clone(),
                            name: c.name,
                            kind: ChannelKind::Text,
                            topic: c.topic,
                            unread: false,
                            mention_count: 0,
                            last_read_message_id: None,
                            icon_url: c.icon_url,
                            is_pinned: c.is_pinned || self.pinned_dms.contains(&real_id),
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                            user_limit: None,
                            avatar_decoration_data,
                            ..Default::default()
                        },
                    );
                }
                self.sort_dm_channels();

                if let Some(temp_id) = temp_id_to_migrate {
                    if self.selected_channel_id.as_deref() == Some(&temp_id) {
                        self.selected_channel_id = Some(real_id.clone());
                    }
                    if temp_id != real_id {
                        if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                            for msg in &mut pending_msgs {
                                msg.channel_id = real_id.clone();
                            }
                            let target_msgs =
                                self.messages_by_channel.entry(real_id.clone()).or_default();
                            for p_msg in pending_msgs {
                                if !target_msgs.iter().any(|m| m.id == p_msg.id) {
                                    target_msgs.push(p_msg);
                                }
                            }
                        }
                    }
                }
            }
            TransportEvent::DmChannelResolved { temp_id, channel } => {
                let real_id = channel.id.clone();
                let is_group = channel.channel_type == ChannelType::GroupDm;
                let avatar_decoration_data = channel
                    .recipient_avatar_decorations
                    .first()
                    .cloned()
                    .flatten();

                // 1. Atualizar ou inserir em self.dm_channels
                if let Some(dm) = self
                    .dm_channels
                    .iter_mut()
                    .find(|c| c.id == temp_id || c.id == real_id)
                {
                    dm.id = real_id.clone();
                    dm.name = channel.name.clone();
                    dm.icon_url = channel.icon_url.clone();
                    dm.is_group_dm = is_group;
                    dm.recipient_ids = channel.recipient_ids.clone();
                    dm.recipient_names = channel.recipient_names.clone();
                    dm.avatar_decoration_data = avatar_decoration_data.clone();
                } else {
                    self.dm_channels.insert(
                        0,
                        ChannelItemState {
                            id: real_id.clone(),
                            name: channel.name.clone(),
                            kind: ChannelKind::Text,
                            topic: channel.topic.clone(),
                            unread: false,
                            mention_count: 0,
                            last_read_message_id: None,
                            icon_url: channel.icon_url.clone(),
                            is_pinned: channel.is_pinned || self.pinned_dms.contains(&real_id),
                            last_message_id: channel.last_message_id.clone(),
                            is_group_dm: is_group,
                            recipient_ids: channel.recipient_ids.clone(),
                            recipient_names: channel.recipient_names.clone(),
                            user_limit: None,
                            avatar_decoration_data,
                            ..Default::default()
                        },
                    );
                }
                self.sort_dm_channels();

                // 2. Se o canal selecionado era o temporário, atualiza para o ID real
                if self.selected_channel_id.as_deref() == Some(&temp_id) {
                    self.selected_channel_id = Some(real_id.clone());
                }

                // 3. Migra mensagens pendentes/armazenadas do ID temporário para o real
                if temp_id != real_id {
                    if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                        for msg in &mut pending_msgs {
                            msg.channel_id = real_id.clone();
                        }
                        let target_msgs =
                            self.messages_by_channel.entry(real_id.clone()).or_default();
                        for p_msg in pending_msgs {
                            if !target_msgs.iter().any(|m| m.id == p_msg.id) {
                                target_msgs.push(p_msg);
                            }
                        }
                    }
                }

                // 4. Carrega histórico de mensagens reais da Discord API ou Mock se ainda não estiver presente
                if let Some(ref cm) = self.client_manager {
                    let has_messages = self
                        .messages_by_channel
                        .get(&real_id)
                        .map_or(false, |msgs| !msgs.is_empty());
                    if !has_messages || self.is_connected_to_gateway {
                        let cm = cm.clone();
                        let cid = real_id.clone();
                        tokio::spawn(async move {
                            if let Ok(mut msgs) = cm.get_messages(&cid, 50).await {
                                msgs.sort_by_key(|m| m.timestamp);
                                cm.emit_event(TransportEvent::ChannelMessagesLoaded {
                                    channel_id: cid,
                                    messages: msgs,
                                });
                            }
                        });
                    }
                }
            }
            TransportEvent::RelationshipsUpdated(relationships) => {
                self.friends = relationships
                    .into_iter()
                    .map(|r| {
                        let display_name = r.global_name.unwrap_or_else(|| r.username.clone());
                        let avatar_text = display_name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string();
                        let (presence, custom_status) = self
                            .presences_by_user
                            .get(&r.id)
                            .map(|p| {
                                (
                                    PresenceStatus::from(p.status),
                                    p.custom_status.as_ref().and_then(|cs| cs.text.clone()),
                                )
                            })
                            .unwrap_or((PresenceStatus::Offline, None));
                        FriendItemState {
                            id: r.id.clone(),
                            username: r.username,
                            display_name,
                            avatar_url: r.avatar.as_ref().map(|hash| {
                                if hash.starts_with("http://") || hash.starts_with("https://") {
                                    hash.clone()
                                } else {
                                    format!(
                                        "https://cdn.discordapp.com/avatars/{}/{}.png?size=128",
                                        r.id, hash
                                    )
                                }
                            }),
                            avatar_text,
                            presence,
                            custom_status,
                            relationship_type: r.rel_type,
                            avatar_decoration_data: r.avatar_decoration_data,
                        }
                    })
                    .collect();
            }
            TransportEvent::RelationshipAdd(r) => {
                self.pending_relationship_actions.remove(&r.id);
                let display_name = r.global_name.unwrap_or_else(|| r.username.clone());
                let avatar_text = display_name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let avatar_url = r.avatar.as_ref().map(|hash| {
                    if hash.starts_with("http://") || hash.starts_with("https://") {
                        hash.clone()
                    } else {
                        format!(
                            "https://cdn.discordapp.com/avatars/{}/{}.png?size=128",
                            r.id, hash
                        )
                    }
                });

                let (presence, custom_status) = self
                    .presences_by_user
                    .get(&r.id)
                    .map(|p| {
                        (
                            PresenceStatus::from(p.status),
                            p.custom_status.as_ref().and_then(|cs| cs.text.clone()),
                        )
                    })
                    .unwrap_or((PresenceStatus::Offline, None));

                if let Some(existing) = self.friends.iter_mut().find(|f| f.id == r.id) {
                    existing.username = r.username;
                    existing.display_name = display_name;
                    existing.avatar_url = avatar_url;
                    existing.avatar_text = avatar_text;
                    existing.presence = presence;
                    existing.custom_status = custom_status;
                    existing.relationship_type = r.rel_type;
                    existing.avatar_decoration_data = r.avatar_decoration_data.clone();
                } else {
                    self.friends.push(FriendItemState {
                        id: r.id.clone(),
                        username: r.username,
                        display_name,
                        avatar_url,
                        avatar_text,
                        presence,
                        custom_status,
                        relationship_type: r.rel_type,
                        avatar_decoration_data: r.avatar_decoration_data.clone(),
                    });
                }
                if let Some(active) = self.active_now_items.iter_mut().find(|a| a.user_id == r.id) {
                    active.avatar_decoration_data = r.avatar_decoration_data;
                }
            }
            TransportEvent::RelationshipRemove { id } => {
                self.pending_relationship_actions.remove(&id);
                self.friends.retain(|f| f.id != id);
                self.active_now_items.retain(|a| a.user_id != id);
            }
            TransportEvent::FriendRequestResult {
                username: _,
                success,
                message,
            } => {
                self.is_sending_friend_request = false;
                self.add_friend_feedback = Some((success, message));
                if success {
                    self.add_friend_input.clear();
                }
            }
            TransportEvent::RelationshipActionFailed { id, .. } => {
                self.pending_relationship_actions.remove(&id);
                log::warn!("Ação de relacionamento recusada pelo Discord.");
            }
            TransportEvent::PresenceUpdate {
                user_id,
                status,
                activity,
                custom_status,
            } => {
                self.update_user_presence(user_id.clone(), status, activity.clone(), custom_status);
                let st: PresenceStatus = status.into();
                self.update_active_now_from_presence(&user_id, st, activity.as_ref());
            }
            TransportEvent::UserStatusUpdateResult { request_id, error } => {
                match error {
                    Some(error) => self.finish_status_update(request_id, Err(error)),
                    None => self.finish_status_update(request_id, Ok(())),
                }
            }
            TransportEvent::PinnedDmsUpdated(pinned) => {
                self.pinned_dms = pinned;
                for dm in &mut self.dm_channels {
                    dm.is_pinned = self.pinned_dms.contains(&dm.id);
                }
                self.sort_dm_channels();
                if let Some(ref path) = self.pinned_dms_path {
                    crate::protocol::pinned_dms::save_pinned_dms_to_path(path, &self.pinned_dms);
                }
            }
            TransportEvent::AccountThemeUpdated {
                theme,
                client_theme_settings,
            } => {
                if theme != crate::protocol::user_settings_proto::Theme::Unset {
                    self.account_theme = Some(theme);
                    if let Some(choice) = ThemeChoice::from_account_theme(theme) {
                        self.theme_choice = choice;
                    }
                }
                if client_theme_settings.is_some() {
                    self.account_client_theme_settings = client_theme_settings;
                }
            }
            TransportEvent::UserProfileLoaded { user_id, profile } => {
                self.set_profile_popout_data(&user_id, *profile);
            }
            TransportEvent::UserProfileError { user_id, error } => {
                self.set_profile_popout_error(&user_id, error);
            }
            TransportEvent::CurrentUserUpdated(user) => {
                // Live USER_UPDATE dispatch: refresh only what the user payload
                // carries (identity, avatar, decoration). Presence, custom
                // status and mute/deafen state are not part of the payload and
                // stay untouched.
                let display_name = user.display_name().to_string();
                let avatar_text = user
                    .username
                    .chars()
                    .next()
                    .unwrap_or('U')
                    .to_uppercase()
                    .to_string();
                let avatar_url = user.avatar_url();
                self.current_user.id = user.id;
                self.current_user.username = user.username;
                self.current_user.display_name = display_name;
                self.current_user.discriminator = user.discriminator;
                self.current_user.avatar_text = avatar_text;
                self.current_user.avatar_url = avatar_url;
                self.current_user.avatar_decoration_data = user.avatar_decoration_data;
                if self.is_connected_to_gateway {
                    self.connection_status =
                        format!("Conectado como @{}", self.current_user.username);
                    if matches!(self.connection_mode, ConnectionMode::LiveDiscord(_)) {
                        self.connection_mode =
                            ConnectionMode::LiveDiscord(self.current_user.username.clone());
                    }
                }
            }
            _ => {}
        }
    }
}
