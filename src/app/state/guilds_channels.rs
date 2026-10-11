use super::*;
use crate::protocol::TransportEvent;

impl AppState {
    /// Selects a guild (or None for DMs / Home)
    pub fn select_guild(&mut self, guild_id: Option<String>) {
        if let Some(ref tel) = self.telemetry {
            let guild_name = guild_id
                .as_ref()
                .and_then(|gid| self.guilds.iter().find(|g| &g.id == gid).map(|g| g.name.clone()))
                .unwrap_or_else(|| "Home / DMs".into());
            let mut meta = std::collections::HashMap::new();
            meta.insert("guild_id".into(), guild_id.clone().unwrap_or_else(|| "home".into()));
            meta.insert("guild_name".into(), guild_name.clone());

            let screen = if let Some(ref gid) = guild_id {
                crate::telemetry::ScreenKind::Guild {
                    id: gid.clone(),
                    name: guild_name.clone(),
                }
            } else {
                crate::telemetry::ScreenKind::FriendsHub
            };
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_guild_select(guild_id.clone().unwrap_or_else(|| "home".into()), guild_name, 0);
        }

        self.selected_guild_id = guild_id;
        if let Some(ref gid) = self.selected_guild_id {
            let gid_clone = gid.clone();
            let mut categories_empty = false;
            let mut first_text_channel_id = None;

            if let Some(guild) = self.guilds.iter().find(|g| &g.id == &gid_clone) {
                if guild.categories.is_empty() {
                    categories_empty = true;
                } else {
                    // Troca instantânea de servidor! Não faz requisição de rede se categories não estiver vazio
                    for cat in &guild.categories {
                        if let Some(ch) = cat.channels.iter().find(|c| c.kind == ChannelKind::Text) {
                            first_text_channel_id = Some(ch.id.clone());
                            break;
                        }
                    }
                    if first_text_channel_id.is_none() {
                        if let Some(ch) = guild.categories.first().and_then(|c| c.channels.first()) {
                            first_text_channel_id = Some(ch.id.clone());
                        }
                    }
                }
            }

            if let Some(cid) = first_text_channel_id {
                self.select_channel(cid);
            } else if categories_empty {
                self.selected_channel_id = None;
                // Carregamento sob demanda (lazy loading) dos canais da guild
                if let Some(ref cm) = self.client_manager {
                    let cm = cm.clone();
                    let gid_fetch = gid_clone;
                    tokio::spawn(async move {
                        match cm.list_channels_for_guild(&gid_fetch).await {
                            Ok(channels) => {
                                cm.emit_event(crate::protocol::TransportEvent::GuildChannelsUpdated {
                                    guild_id: gid_fetch,
                                    channels,
                                });
                            }
                            Err(e) => {
                                log::warn!("Falha ao carregar canais sob demanda para guild {}: {:?}", gid_fetch, e);
                            }
                        }
                    });
                }
            }
        } else {
            // Modo Direct Messages / Home
            self.sort_dm_channels();
            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                tokio::spawn(async move {
                    match cm.list_dm_channels().await {
                        Ok(channels) => {
                            cm.emit_event(crate::protocol::TransportEvent::DmChannelsUpdated(channels));
                        }
                        Err(e) => {
                            log::warn!("Falha ao carregar canais de DM sob demanda: {:?}", e);
                        }
                    }
                    match cm.list_relationships().await {
                        Ok(relationships) => {
                            cm.emit_event(crate::protocol::TransportEvent::RelationshipsUpdated(relationships));
                        }
                        Err(e) => {
                            log::warn!("Falha ao carregar relacionamentos/amigos sob demanda: {:?}", e);
                        }
                    }
                });
            }

            if let Some(first_dm) = self.dm_channels.first() {
                self.select_channel(first_dm.id.clone());
            } else {
                self.selected_channel_id = None;
            }
        }
    }

    /// Sorts DMs according to Discord rules:
    /// 1. Pinned DMs (is_pinned: true) at the absolute top, strictly adhering to the order in `pinned_dms`.
    /// 2. Channels with the most recent messages closest to the top (last_message_id descending Snowflake).
    pub fn sort_dm_channels(&mut self) {
        let pinned_order = &self.pinned_dms;
        self.dm_channels.sort_by(|a, b| {
            // 1. DMs fixadas no topo absoluto (is_pinned: true vem antes de false)
            match (a.is_pinned, b.is_pinned) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                (true, true) => {
                    // Ambas são fixadas: manter a ordem estrita do array pinned_dms
                    let pos_a = pinned_order.iter().position(|id| id == &a.id);
                    let pos_b = pinned_order.iter().position(|id| id == &b.id);
                    match (pos_a, pos_b) {
                        (Some(idx_a), Some(idx_b)) => {
                            if idx_a != idx_b {
                                return idx_a.cmp(&idx_b);
                            }
                        }
                        (Some(_), None) => return std::cmp::Ordering::Less,
                        (None, Some(_)) => return std::cmp::Ordering::Greater,
                        (None, None) => {}
                    }
                }
                (false, false) => {}
            }

            // 2. Mensagem mais recente mais perto do topo (decrescente por Snowflake)
            match (&a.last_message_id, &b.last_message_id) {
                (Some(id_a), Some(id_b)) => {
                    if let (Ok(na), Ok(nb)) = (id_a.parse::<u64>(), id_b.parse::<u64>()) {
                        nb.cmp(&na)
                    } else {
                        id_b.cmp(id_a)
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }

    /// Sets the disk persistence path for pinned DMs. When set, loads from disk if available.
    pub fn set_pinned_dms_path(&mut self, path: Option<std::path::PathBuf>) {
        self.pinned_dms_path = path;
        if let Some(ref p) = self.pinned_dms_path {
            self.pinned_dms = crate::protocol::pinned_dms::load_pinned_dms_from_path(p);
            for dm in &mut self.dm_channels {
                dm.is_pinned = self.pinned_dms.contains(&dm.id);
            }
            self.sort_dm_channels();
        }
    }

    /// Pins a DM channel: adds to pinned_dms, marks is_pinned = true, re-sorts and persists to disk if configured.
    pub fn pin_dm_channel(&mut self, channel_id: &str) {
        if !self.pinned_dms.contains(&channel_id.to_string()) {
            self.pinned_dms.push(channel_id.to_string());
        }
        for dm in &mut self.dm_channels {
            if dm.id == channel_id {
                dm.is_pinned = true;
            }
        }
        self.sort_dm_channels();
        if let Some(ref path) = self.pinned_dms_path {
            crate::protocol::pinned_dms::save_pinned_dms_to_path(path, &self.pinned_dms);
        }
    }

    /// Unpins a DM channel: removes from pinned_dms, sets is_pinned = false, re-sorts and persists to disk if configured.
    pub fn unpin_dm_channel(&mut self, channel_id: &str) {
        self.pinned_dms.retain(|id| id != channel_id);
        for dm in &mut self.dm_channels {
            if dm.id == channel_id {
                dm.is_pinned = false;
            }
        }
        self.sort_dm_channels();
        if let Some(ref path) = self.pinned_dms_path {
            crate::protocol::pinned_dms::save_pinned_dms_to_path(path, &self.pinned_dms);
        }
    }

    /// Toggles pin status for a DM channel.
    pub fn toggle_pin_dm_channel(&mut self, channel_id: &str) {
        let is_currently_pinned = self
            .dm_channels
            .iter()
            .find(|d| d.id == channel_id)
            .map(|d| d.is_pinned)
            .unwrap_or(false)
            || self.pinned_dms.contains(&channel_id.to_string());
        if is_currently_pinned {
            self.unpin_dm_channel(channel_id);
        } else {
            self.pin_dm_channel(channel_id);
        }
    }

    /// Selects a channel
    pub fn select_channel(&mut self, channel_id: String) {
        if let Some(ref tel) = self.telemetry {
            let (ch_name, ch_kind) = self
                .find_channel(&channel_id)
                .map(|(_, ch)| (ch.name.clone(), ch.kind))
                .unwrap_or_else(|| (channel_id.clone(), ChannelKind::Text));
            let kind_str = match ch_kind {
                ChannelKind::Voice => "voice",
                ChannelKind::Announcement => "announcement",
                ChannelKind::Text => "text",
            };
            let mut meta = std::collections::HashMap::new();
            meta.insert("channel_id".into(), channel_id.clone());
            meta.insert("channel_name".into(), ch_name.clone());
            meta.insert("kind".into(), kind_str.into());

            let screen = match ch_kind {
                ChannelKind::Voice => crate::telemetry::ScreenKind::VoiceChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                ChannelKind::Announcement => crate::telemetry::ScreenKind::AnnouncementChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                ChannelKind::Text => crate::telemetry::ScreenKind::TextChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
            };
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_channel_select(channel_id.clone(), ch_name, kind_str, 0);
        }

        let guild_name = self
            .get_selected_guild()
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Servidor".into());

        let voice_to_connect = if let Some((_, channel)) = self.find_channel(&channel_id) {
            if channel.kind == ChannelKind::Voice {
                Some((channel.id.clone(), channel.name.clone(), guild_name))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((cid, cname, gname)) = voice_to_connect {
            self.connect_voice(&cid, &cname, &gname);
        }
        self.selected_channel_id = Some(channel_id.clone());
        self.typing_users = self.get_active_typing_users(&channel_id);

        // Se houver ClientManager (seja Mock ou Discord API), resolver canal de DM temporário sob demanda
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let cid = channel_id.clone();
            if cid.starts_with("dm-") && !cid.starts_with("dm-group-") {
                let uid = cid.trim_start_matches("dm-").to_string();
                let temp_id = cid.clone();
                tokio::spawn(async move {
                    if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                        cm.emit_event(TransportEvent::DmChannelResolved {
                            temp_id,
                            channel: ch,
                        });
                    }
                });
            } else if self.is_connected_to_gateway {
                let has_messages = self.messages_by_channel.get(&cid).map_or(false, |msgs| !msgs.is_empty());
                if !has_messages {
                    tokio::spawn(async move {
                        if let Ok(mut msgs) = cm.get_messages(&cid, 50).await {
                            // Assegurar ordenação cronológica ascendente (mais antiga para mais nova)
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
    }

    /// Toggles collapse on a channel category
    pub fn toggle_category(&mut self, category_id: &str) {
        if self.collapsed_categories.contains(category_id) {
            self.collapsed_categories.remove(category_id);
        } else {
            self.collapsed_categories.insert(category_id.to_string());
        }
    }

    /// Checks if a category is collapsed
    pub fn is_category_collapsed(&self, category_id: &str) -> bool {
        self.collapsed_categories.contains(category_id)
    }

    /// Toggles expanded state of a guild folder in the guild rail
    pub fn toggle_folder(&mut self, folder_id: &str) {
        if self.expanded_folders.contains(folder_id) {
            self.expanded_folders.remove(folder_id);
        } else {
            self.expanded_folders.insert(folder_id.to_string());
        }
    }

    /// Checks if a guild folder is expanded
    pub fn is_folder_expanded(&self, folder_id: &str) -> bool {
        self.expanded_folders.contains(folder_id)
    }

    /// Toggles member sidebar visibility
    pub fn toggle_members(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_members", "Toggle Member Sidebar", 0);
        }
        self.show_members = !self.show_members;
    }
}

impl AppState {
    /// Gets the currently selected guild
    pub fn get_selected_guild(&self) -> Option<&GuildItemState> {
        let gid = self.selected_guild_id.as_ref()?;
        self.guilds.iter().find(|g| &g.id == gid)
    }

    /// Finds a channel by ID across guilds
    pub fn find_channel(&self, channel_id: &str) -> Option<(&GuildItemState, &ChannelItemState)> {
        for guild in &self.guilds {
            for cat in &guild.categories {
                for ch in &cat.channels {
                    if ch.id == channel_id {
                        return Some((guild, ch));
                    }
                }
            }
        }
        None
    }

    /// Gets the currently selected channel
    pub fn get_selected_channel(&self) -> Option<(&GuildItemState, &ChannelItemState)> {
        let cid = self.selected_channel_id.as_ref()?;
        self.find_channel(cid)
    }

    /// Gets the currently selected channel item (from either guilds or DM channels)
    pub fn get_active_channel_item(&self) -> Option<ChannelItemState> {
        let cid = self.selected_channel_id.as_ref()?;
        if let Some((_, ch)) = self.find_channel(cid) {
            Some(ch.clone())
        } else {
            self.dm_channels.iter().find(|d| &d.id == cid).cloned()
        }
    }
}
