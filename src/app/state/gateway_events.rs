use super::*;
use std::collections::HashMap;
use crate::domain::channel::ChannelType;
use crate::protocol::ConnectionMode;
use crate::protocol::TransportEvent;

impl AppState {
    /// Dispatches a real-time event received from the Gateway / ClientManager
    pub fn handle_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::Ready { user, guilds, guild_folders } => {
                let display_name = user.display_name().to_string();
                let avatar_text = user.username.chars().next().unwrap_or('U').to_uppercase().to_string();
                let avatar_url = user.avatar_url();
                let username = user.username;
                self.current_user.id = user.id;
                self.current_user.username = username.clone();
                self.current_user.display_name = display_name;
                self.current_user.avatar_text = avatar_text;
                self.current_user.avatar_url = avatar_url;
                self.current_user.avatar_decoration_data = user.avatar_decoration_data;
                self.is_connected_to_gateway = true;
                self.is_connecting = false;
                self.connection_mode = ConnectionMode::LiveDiscord(username);
                self.connection_status = format!("Conectado como @{}", self.current_user.username);
                self.auth_error = None;
                if !guild_folders.is_empty() {
                    self.guild_folders = guild_folders;
                }
                self.close_modal();

                // Limpar mensagens fictícias para exibir apenas dados reais carregados da API
                self.messages_by_channel.clear();

                if !guilds.is_empty() {
                    let mut new_guilds = Vec::new();
                    for g in guilds {
                        let icon_url = g.icon_url();
                        let icon_hash = g.icon.clone();
                        let categories = super::seed::organize_channels_into_categories(&g.id, g.channels);

                        let emojis = if !g.emojis.is_empty() {
                            let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                            g.emojis
                        } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.emojis.clone()) {
                            if !existing.is_empty() {
                                existing
                            } else {
                                self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                            }
                        } else {
                            self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                        };

                        let stickers = if !g.stickers.is_empty() {
                            let _ = self.media_cache.save_guild_stickers_to_disk(&g.id, &g.stickers);
                            g.stickers
                        } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.stickers.clone()) {
                            if !existing.is_empty() {
                                existing
                            } else {
                                self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                            }
                        } else {
                            self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
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
                        new_guilds.sort_by_key(|g| {
                            order_map.get(&g.id).copied().unwrap_or(usize::MAX)
                        });
                    }

                    self.guilds = new_guilds;
                    self.sync_guild_media();
                    save_guilds_to_disk(&self.guilds, &self.guild_folders);
                    if let Some(first) = self.guilds.first() {
                        self.select_guild(Some(first.id.clone()));
                    }
                }
            }
            TransportEvent::MessageCreate(msg) => {
                self.clear_typing(&msg.channel_id, &msg.author.id);
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                let channel_id = msg.channel_id.clone();

                // 1. Identificar com precisão se a mensagem pertence a uma GUILDA/SERVIDOR.
                // Uma mensagem é estritamente de guilda se:
                // - msg.guild_id.is_some(), OU
                // - o canal pertencer a qualquer guilda conhecida em self.guilds.
                let is_guild_message = msg.guild_id.is_some()
                    || self.guilds.iter().any(|g| {
                        g.categories.iter().any(|cat| cat.channels.iter().any(|c| c.id == channel_id))
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
                                temp_msgs.remove(pos);
                                break;
                            }
                        }
                    }
                }

                let channel_msgs = self.messages_by_channel.entry(channel_id.clone()).or_default();
                
                // Substituição otimista: se existir uma mensagem temporária pendente com mesmo conteúdo e autor
                if let Some(pos) = channel_msgs.iter().position(|m| {
                    (m.id.starts_with("temp-") || m.is_pending)
                        && (m.author.id == msg.author.id
                            || m.author.username == msg.author.username
                            || m.author.id == "user-me"
                            || m.author.id == self.current_user.id)
                        && m.content == msg.content
                }) {
                    channel_msgs[pos] = msg.clone();
                } else if !channel_msgs.iter().any(|m| m.id == msg.id) {
                    channel_msgs.push(msg.clone());
                }

                if is_guild_message {
                    // Atualiza o last_message_id do canal da guilda, se existir
                    for guild in &mut self.guilds {
                        for cat in &mut guild.categories {
                            for ch in &mut cat.channels {
                                if ch.id == channel_id {
                                    ch.last_message_id = Some(msg.id.clone());
                                }
                            }
                        }
                    }
                    // MENSAGEM DE GUILDA NUNCA MODIFICA dm_channels NEM selected_channel_id DE DM!
                } else {
                    // Mensagem comprovadamente de DM privada (msg.guild_id.is_none() e canal não pertence a nenhuma guilda)
                    let mut dm_updated = false;

                    // 1. Tentar correspondência EXATA com o ID do canal de DM existente
                    if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == channel_id) {
                        dm.last_message_id = Some(msg.id.clone());
                        dm_updated = true;
                    } else if let Some(dm) = self.dm_channels.iter_mut().find(|c| {
                        // 2. Apenas se for canal temporário dm-* ainda não resolvido E o destinatário bater com o autor
                        c.id.starts_with("dm-") && c.recipient_ids.contains(&msg.author.id)
                    }) {
                        let temp_id = dm.id.clone();
                        dm.id = channel_id.clone();
                        dm.last_message_id = Some(msg.id.clone());
                        dm_updated = true;

                        // Se a seleção atual estiver no temporário correspondente, migra para o canal real da DM
                        if self.selected_channel_id.as_deref() == Some(&temp_id) {
                            self.selected_channel_id = Some(channel_id.clone());
                        }

                        // Migra mensagens pendentes do canal temporário para o canal real
                        if temp_id != channel_id {
                            if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                                for m in &mut pending_msgs {
                                    m.channel_id = channel_id.clone();
                                }
                                let target_msgs = self.messages_by_channel.entry(channel_id.clone()).or_default();
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
                    }

                    // Se a seleção atual estiver em dm-{user_id} e esta DM real corresponde a esse mesmo user_id, migra a seleção
                    if let Some(ref sel) = self.selected_channel_id.clone() {
                        if sel.starts_with("dm-") && sel != &channel_id {
                            let uid = sel.trim_start_matches("dm-");
                            if uid == msg.author.id
                                && self.dm_channels.iter().any(|d| d.id == channel_id && d.recipient_ids.contains(&uid.to_string()))
                            {
                                self.selected_channel_id = Some(channel_id.clone());
                            }
                        }
                    }
                }
            }
            TransportEvent::ChannelMessagesLoaded { channel_id, messages } => {
                self.set_channel_messages(&channel_id, messages);
            }
            TransportEvent::MessageUpdate(msg) => {
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                if let Some(msgs) = self.messages_by_channel.get_mut(&msg.channel_id) {
                    if let Some(existing) = msgs.iter_mut().find(|m| m.id == msg.id) {
                        *existing = msg;
                    }
                }
            }
            TransportEvent::MessageDelete { channel_id, message_id } => {
                if let Some(msgs) = self.messages_by_channel.get_mut(&channel_id) {
                    msgs.retain(|m| m.id != message_id);
                }
            }
            TransportEvent::ReactionAdd { channel_id, message_id, emoji, .. } => {
                self.add_reaction_to_message(&channel_id, &message_id, &emoji);
            }
            TransportEvent::TypingStart { channel_id, user_id, .. } => {
                self.record_typing(&channel_id, &user_id, None);
            }
            TransportEvent::Connected => {
                self.is_connected_to_gateway = true;
                self.is_connecting = false;
                self.connection_status = "Conectado".to_string();
                self.auth_error = None;
                self.close_modal();
            }
            TransportEvent::Disconnected(reason) => {
                self.is_connected_to_gateway = false;
                self.is_connecting = false;
                self.auth_error = Some(reason.clone());
                self.connection_status = format!("Desconectado: {}", reason);
            }
            TransportEvent::VoiceStateUpdate(state) => {
                self.update_voice_state(state);
            }
            TransportEvent::GuildChannelsUpdated { guild_id, channels } => {
                let categories = super::seed::organize_channels_into_categories(&guild_id, channels);

                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.categories = categories;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);

                if self.selected_guild_id.as_deref() == Some(&guild_id) && self.selected_channel_id.is_none() {
                    if let Some(guild) = self.guilds.iter().find(|g| g.id == guild_id) {
                        for cat in &guild.categories {
                            if let Some(first_text) = cat.channels.iter().find(|c| c.kind == ChannelKind::Text) {
                                self.select_channel(first_text.id.clone());
                                break;
                            }
                        }
                    }
                }
            }
            TransportEvent::GuildCreate(g) => {
                let icon_url = g.icon_url();
                let icon_hash = g.icon.clone();
                let categories = super::seed::organize_channels_into_categories(&g.id, g.channels);

                let emojis = if !g.emojis.is_empty() {
                    let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                    g.emojis
                } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.emojis.clone()) {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                    }
                } else {
                    self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                };

                let stickers = if !g.stickers.is_empty() {
                    let _ = self.media_cache.save_guild_stickers_to_disk(&g.id, &g.stickers);
                    g.stickers
                } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.stickers.clone()) {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                    }
                } else {
                    self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
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
                    self.guilds.sort_by_key(|g| {
                        order_map.get(&g.id).copied().unwrap_or(usize::MAX)
                    });
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildEmojisUpdated { guild_id, emojis } => {
                let _ = self.media_cache.save_guild_emojis_to_disk(&guild_id, &emojis);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.emojis = emojis;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildStickersUpdated { guild_id, stickers } => {
                let _ = self.media_cache.save_guild_stickers_to_disk(&guild_id, &stickers);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.stickers = stickers;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::DmChannelsUpdated(channels) => {
                let mut new_dms: Vec<ChannelItemState> = channels
                    .into_iter()
                    .map(|c| {
                        let is_group = c.channel_type == ChannelType::GroupDm;
                        let is_pinned = c.is_pinned || self.pinned_dms.contains(&c.id);
                        let avatar_decoration_data = c.recipient_avatar_decorations.first().cloned().flatten();
                        ChannelItemState {
                            id: c.id,
                            name: c.name,
                            kind: ChannelKind::Text,
                            topic: c.topic,
                            unread: false,
                            mention_count: 0,
                            icon_url: c.icon_url,
                            is_pinned,
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                            user_limit: None,
                            avatar_decoration_data,
                        }
                    })
                    .collect();

                // Preservar canais de DM temporários ainda não resolvidos (ex: dm-<user_id> abertos dinamicamente)
                for existing in &self.dm_channels {
                    // Ignorar canais mock / fixos de demonstração antigos
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
                            existing.recipient_ids.iter().any(|uid| d.recipient_ids.contains(uid))
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
                let avatar_decoration_data = c.recipient_avatar_decorations.first().cloned().flatten();
                let mut temp_id_to_migrate: Option<String> = None;
                if let Some(existing) = self.dm_channels.iter_mut().find(|d| {
                    d.id == real_id
                        || (d.id.starts_with("dm-")
                            && !d.is_group_dm
                            && !c.recipient_ids.is_empty()
                            && c.recipient_ids.iter().any(|uid| d.recipient_ids.contains(uid)))
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
                            icon_url: c.icon_url,
                            is_pinned: c.is_pinned || self.pinned_dms.contains(&real_id),
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                            user_limit: None,
                            avatar_decoration_data,
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
                            let target_msgs = self.messages_by_channel.entry(real_id.clone()).or_default();
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
                let avatar_decoration_data = channel.recipient_avatar_decorations.first().cloned().flatten();

                // 1. Atualizar ou inserir em self.dm_channels
                if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == temp_id || c.id == real_id) {
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
                            icon_url: channel.icon_url.clone(),
                            is_pinned: channel.is_pinned || self.pinned_dms.contains(&real_id),
                            last_message_id: channel.last_message_id.clone(),
                            is_group_dm: is_group,
                            recipient_ids: channel.recipient_ids.clone(),
                            recipient_names: channel.recipient_names.clone(),
                            user_limit: None,
                            avatar_decoration_data,
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
                        let target_msgs = self.messages_by_channel.entry(real_id.clone()).or_default();
                        for p_msg in pending_msgs {
                            if !target_msgs.iter().any(|m| m.id == p_msg.id) {
                                target_msgs.push(p_msg);
                            }
                        }
                    }
                }

                // 4. Carrega histórico de mensagens reais da Discord API ou Mock se ainda não estiver presente
                if let Some(ref cm) = self.client_manager {
                    let has_messages = self.messages_by_channel.get(&real_id).map_or(false, |msgs| !msgs.is_empty());
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
                                    format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=96", r.id, hash)
                                }
                            }),
                            avatar_text,
                            presence,
                            custom_status,
                            relationship_type: r.rel_type,
                            // Relationship (src/protocol/transport.rs, fora do escopo desta
                            // tarefa) ainda não transporta avatar_decoration_data.
                            avatar_decoration_data: None,
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
                        format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=96", r.id, hash)
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
                } else {
                    self.friends.push(FriendItemState {
                        id: r.id,
                        username: r.username,
                        display_name,
                        avatar_url,
                        avatar_text,
                        presence,
                        custom_status,
                        relationship_type: r.rel_type,
                        // Ver nota em RelationshipsUpdated: Relationship ainda não
                        // transporta avatar_decoration_data.
                        avatar_decoration_data: None,
                    });
                }
            }
            TransportEvent::RelationshipRemove { id } => {
                self.pending_relationship_actions.remove(&id);
                self.friends.retain(|f| f.id != id);
                self.active_now_items.retain(|a| a.user_id != id);
            }
            TransportEvent::FriendRequestResult { username: _, success, message } => {
                self.is_sending_friend_request = false;
                self.add_friend_feedback = Some((success, message));
                if success {
                    self.add_friend_input.clear();
                }
            }
            TransportEvent::RelationshipActionFailed { id, error } => {
                self.pending_relationship_actions.remove(&id);
                log::warn!("Relationship action failed for user {}: {}", id, error);
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
            TransportEvent::UserProfileLoaded { user_id, profile } => {
                self.set_profile_popout_data(&user_id, *profile);
            }
            TransportEvent::UserProfileError { user_id, error } => {
                self.set_profile_popout_error(&user_id, error);
            }
            _ => {}
        }
    }
}
