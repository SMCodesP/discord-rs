use super::*;
use crate::domain::permissions::{
    self as permission, GuildPermissionContext, RolePermissions,
};
use crate::protocol::TransportEvent;

impl AppState {
    /// Recomputes the current member's effective permissions for every channel
    /// in `guild`, including category overwrites and communication timeouts.
    pub fn update_guild_permissions(&mut self, guild: &crate::domain::Guild) {
        let roles: Vec<RolePermissions> = guild
            .roles
            .iter()
            .map(|role| {
                RolePermissions::new(
                    role.id.clone(),
                    permission::parse_permissions(&role.permissions),
                )
            })
            .collect();
        let timeout_active = permission::is_timeout_active(
            guild.current_user_timeout_until.as_deref(),
            chrono::Utc::now(),
        );
        let context = GuildPermissionContext::new(
            guild.id.clone(),
            guild.owner_id.clone(),
            self.current_user.id.clone(),
            guild.current_user_roles.clone(),
            roles,
            timeout_active,
        );

        self.remove_guild_channel_permissions(&guild.id, &guild.channels);
        if context.known {
            self.insert_channel_permissions(&context, &guild.channels);
            self.guild_permission_contexts
                .insert(guild.id.clone(), context);
        } else {
            // A snapshot without roles (bot REST fallback) has no reliable
            // baseline. Do not hide channels or block actions in that state.
            self.guild_permission_contexts.remove(&guild.id);
        }
    }

    /// Applies fresh channel overwrites using the cached guild permission
    /// context (for lazy channel loads without a full guild payload).
    pub fn update_channel_permissions(&mut self, guild_id: &str, channels: &[crate::domain::Channel]) {
        let Some(context) = self.guild_permission_contexts.get(guild_id).cloned() else {
            return;
        };
        if !context.known {
            return;
        }
        self.remove_guild_channel_permissions(guild_id, channels);
        self.insert_channel_permissions(&context, channels);
    }

    /// Stores effective permissions for a synthetic channel (tests/UI).
    pub fn set_channel_permissions(
        &mut self,
        channel_id: impl Into<String>,
        permissions: u64,
    ) {
        self.channel_permissions.insert(channel_id.into(), permissions);
    }

    pub fn effective_channel_permissions(&self, channel_id: &str) -> Option<u64> {
        self.channel_permissions.get(channel_id).copied()
    }

    /// Unknown channels (DMs and not-yet-loaded guilds) remain visible so a
    /// transient snapshot cannot blank the sidebar.
    pub fn can_view_channel(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_view_channel)
            .unwrap_or(true)
    }

    pub fn can_send_messages(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_send_messages)
            .unwrap_or(true)
    }

    pub fn can_add_reactions(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_add_reactions)
            .unwrap_or(true)
    }

    pub fn can_attach_files(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_attach_files)
            .unwrap_or(true)
    }

    pub fn can_connect_to_channel(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_connect)
            .unwrap_or(true)
    }

    pub fn can_speak_in_channel(&self, channel_id: &str) -> bool {
        self.effective_channel_permissions(channel_id)
            .map(permission::can_speak)
            .unwrap_or(true)
    }

    fn insert_channel_permissions(
        &mut self,
        context: &GuildPermissionContext,
        channels: &[crate::domain::Channel],
    ) {
        for channel in channels {
            let parent_overwrites = channel
                .category_id
                .as_deref()
                .and_then(|category_id| {
                    channels
                        .iter()
                        .find(|candidate| candidate.id == category_id)
                })
                .map(|parent| parent.permission_overwrites.as_slice())
                .unwrap_or(&[]);
            let permissions = context.channel_permissions(
                &channel.permission_overwrites,
                parent_overwrites,
            );
            self.channel_permissions
                .insert(channel.id.clone(), permissions);
        }
    }

    fn remove_guild_channel_permissions(
        &mut self,
        guild_id: &str,
        channels: &[crate::domain::Channel],
    ) {
        let mut ids: std::collections::HashSet<String> =
            channels.iter().map(|channel| channel.id.clone()).collect();
        if let Some(guild) = self.guilds.iter().find(|guild| guild.id == guild_id) {
            for category in &guild.categories {
                for channel in &category.channels {
                    ids.insert(channel.id.clone());
                }
            }
        }
        self.channel_permissions
            .retain(|channel_id, _| !ids.contains(channel_id));
    }
}

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

        if self.selected_guild_id != guild_id {
            self.members.clear();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let coordinator = crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared();
                if let Some(ref cid) = self.selected_channel_id {
                    let cid_clone = cid.clone();
                    handle.spawn(async move {
                        coordinator.reset_channel_ranges(&cid_clone).await;
                    });
                }
            }
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
                        if let Some(ch) = cat.channels.iter().find(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum | ChannelKind::Directory)) {
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
            // Modo Direct Messages / Home: aproveita DMs e relacionamentos já entregues pelo
            // Gateway READY sem disparar chamadas REST redundantes a cada clique em Home.
            self.sort_dm_channels();
            let need_dms = self.dm_channels.is_empty();
            let need_rels = self.friends.is_empty();
            if need_dms || need_rels {
                if let Some(ref cm) = self.client_manager {
                    let cm = cm.clone();
                    tokio::spawn(async move {
                        if need_dms {
                            match cm.list_dm_channels().await {
                                Ok(channels) => {
                                    cm.emit_event(crate::protocol::TransportEvent::DmChannelsUpdated(channels));
                                }
                                Err(e) => {
                                    log::warn!("Falha ao carregar canais de DM sob demanda: {:?}", e);
                                }
                            }
                        }
                        if need_rels {
                            match cm.list_relationships().await {
                                Ok(relationships) => {
                                    cm.emit_event(crate::protocol::TransportEvent::RelationshipsUpdated(relationships));
                                }
                                Err(e) => {
                                    log::warn!("Falha ao carregar relacionamentos/amigos sob demanda: {:?}", e);
                                }
                            }
                        }
                    });
                }
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
                ChannelKind::Forum => "forum",
                ChannelKind::PublicThread => "public_thread",
                ChannelKind::PrivateThread => "private_thread",
                ChannelKind::AnnouncementThread => "announcement_thread",
                ChannelKind::StageVoice => "stage_voice",
                ChannelKind::Directory => "directory",
                ChannelKind::Text => "text",
            };
            let mut meta = std::collections::HashMap::new();
            meta.insert("channel_id".into(), channel_id.clone());
            meta.insert("channel_name".into(), ch_name.clone());
            meta.insert("kind".into(), kind_str.into());

            let screen = match ch_kind {
                ChannelKind::Voice | ChannelKind::StageVoice => crate::telemetry::ScreenKind::VoiceChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                ChannelKind::Announcement => crate::telemetry::ScreenKind::AnnouncementChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                _ => crate::telemetry::ScreenKind::TextChannel {
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
            if channel.kind.is_voice_like() {
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
        if self.selected_channel_id.as_deref() != Some(&channel_id) {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let coordinator = crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared();
                let cid_clone = channel_id.clone();
                handle.spawn(async move {
                    coordinator.reset_channel_ranges(&cid_clone).await;
                });
            }
        }
        let previous_channel = self.selected_channel_id.clone();
        self.selected_channel_id = Some(channel_id.clone());
        self.typing_users = self.get_active_typing_users(&channel_id);

        if previous_channel.as_deref() != Some(channel_id.as_str()) {
            self.prepare_unread_divider_for_channel(&channel_id);
        }
        self.maybe_ack_visible_channel_read();

        // Se houver ClientManager (seja Mock ou Discord API), resolver canal de DM temporário sob demanda
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let cid = channel_id.clone();
            if cid.starts_with("dm-") && !cid.starts_with("dm-group-") {
                let uid = cid.trim_start_matches("dm-").to_string();
                let temp_id = cid.clone();
                let cm_dm = cm.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        if let Ok(ch) = cm_dm.create_or_get_dm_channel(&uid).await {
                            cm_dm.emit_event(TransportEvent::DmChannelResolved {
                                temp_id,
                                channel: ch,
                            });
                        }
                    });
                }
            } else if self.is_connected_to_gateway {
                let has_messages = self.messages_by_channel.get(&cid).map_or(false, |msgs| !msgs.is_empty());
                if !has_messages {
                    let cm_msg = cm.clone();
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        handle.spawn(async move {
                        if let Ok(mut msgs) = cm_msg.get_messages(&cid, 50).await {
                            // Assegurar ordenação cronológica ascendente (mais antiga para mais nova)
                            msgs.sort_by_key(|m| m.timestamp);
                            cm_msg.emit_event(TransportEvent::ChannelMessagesLoaded {
                                channel_id: cid,
                                messages: msgs,
                            });
                        }
                        });
                    }
                }
            }

            // Assina membros da guilda sob demanda (Opcode 14 no Gateway do Discord)
            if let Some(ref gid) = self.selected_guild_id {
                let gid_sub = gid.clone();
                let cid_sub = channel_id.clone();
                let cm_sub = cm.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        if let Err(e) = cm_sub.subscribe_guild_members(&gid_sub, &cid_sub).await {
                            log::warn!("Falha ao assinar membros da guilda {} no canal {}: {:?}", gid_sub, cid_sub, e);
                        }
                    });
                }
            }
        }
    }

    /// Atualiza as faixas de membros visíveis (Opcode 14 ao rolar a lista de membros além dos 100 primeiros)
    pub fn update_member_viewport(&mut self, ranges: Vec<[usize; 2]>) {
        if let (Some(gid), Some(cid)) = (&self.selected_guild_id, &self.selected_channel_id) {
            let gid = gid.clone();
            let cid = cid.clone();
            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        if let Err(e) = cm.update_guild_member_viewport(&gid, &cid, ranges).await {
                            log::warn!("Falha ao enviar faixas do viewport para guild {} canal {}: {:?}", gid, cid, e);
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

    pub(crate) fn channel_state_mut_by_id(&mut self, channel_id: &str) -> Option<&mut ChannelItemState> {
        if let Some(dm) = self.dm_channels.iter_mut().find(|dm| dm.id == channel_id) {
            return Some(dm);
        }
        for guild in &mut self.guilds {
            for category in &mut guild.categories {
                for channel in &mut category.channels {
                    if channel.id == channel_id {
                        return Some(channel);
                    }
                }
            }
        }
        None
    }

    pub(crate) fn channel_state_ref_by_id(&self, channel_id: &str) -> Option<&ChannelItemState> {
        if let Some(dm) = self.dm_channels.iter().find(|dm| dm.id == channel_id) {
            return Some(dm);
        }
        for guild in &self.guilds {
            for category in &guild.categories {
                for channel in &category.channels {
                    if channel.id == channel_id {
                        return Some(channel);
                    }
                }
            }
        }
        None
    }

    pub(crate) fn recompute_guild_unread(&mut self, guild_id: &str) {
        if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
            guild.mention_count = guild
                .categories
                .iter()
                .flat_map(|c| c.channels.iter())
                .map(|c| c.mention_count)
                .sum();
            guild.unread = guild
                .categories
                .iter()
                .flat_map(|c| c.channels.iter())
                .any(|c| c.unread);
        }
    }

    pub(crate) fn recompute_channel_unread_flags(&mut self, channel_id: &str) {
        let Some(channel) = self.channel_state_mut_by_id(channel_id) else {
            return;
        };
        let last_message_id = channel.last_message_id.clone();
        let last_read_message_id = channel.last_read_message_id.clone();
        let unread = crate::protocol::is_message_unread(
            last_message_id.as_deref(),
            last_read_message_id.as_deref(),
        );
        channel.unread = unread;
        if !unread {
            channel.mention_count = 0;
        }
        let guild_id = self
            .guilds
            .iter()
            .find(|g| {
                g.categories
                    .iter()
                    .any(|cat| cat.channels.iter().any(|ch| ch.id == channel_id))
            })
            .map(|g| g.id.clone());
        if let Some(guild_id) = guild_id {
            self.recompute_guild_unread(&guild_id);
        }
    }

    /// Returns the first unread message id for the currently selected channel, if any.
    pub fn first_unread_message_id_for_channel(&self, channel_id: &str) -> Option<String> {
        let channel = self.channel_state_ref_by_id(channel_id)?;
        let last_read = channel.last_read_message_id.as_deref();
        let last_message = channel.last_message_id.as_deref();
        if !crate::protocol::is_message_unread(last_message, last_read) {
            return None;
        }
        let msgs = self.messages_by_channel.get(channel_id)?;
        if let Some(last_read) = last_read {
            msgs.iter()
                .find(|m| crate::protocol::is_newer_snowflake(Some(m.id.as_str()), Some(last_read)))
                .map(|m| m.id.clone())
        } else {
            msgs.first().map(|m| m.id.clone())
        }
    }

    pub fn unread_divider_message_id(&self, channel_id: &str) -> Option<&str> {
        if self.unread_divider_channel_id.as_deref() == Some(channel_id) {
            self.unread_divider_message_id.as_deref()
        } else {
            None
        }
    }

    fn prepare_unread_divider_for_channel(&mut self, channel_id: &str) {
        self.unread_divider_channel_id = Some(channel_id.to_string());
        self.unread_divider_message_id = self.first_unread_message_id_for_channel(channel_id);
    }

    pub fn clear_unread_divider_for_channel(&mut self, channel_id: &str) {
        if self.unread_divider_channel_id.as_deref() == Some(channel_id) {
            self.unread_divider_channel_id = None;
            self.unread_divider_message_id = None;
        }
    }

    /// Marks a channel read up to its latest known message and schedules ACK.
    pub fn mark_channel_read(&mut self, channel_id: &str) -> bool {
        let Some(last_message_id) = self
            .channel_state_ref_by_id(channel_id)
            .and_then(|c| c.last_message_id.clone())
        else {
            return false;
        };
        self.mark_channel_read_up_to(channel_id, &last_message_id, true)
    }

    pub fn mark_channel_read_up_to(
        &mut self,
        channel_id: &str,
        message_id: &str,
        manual: bool,
    ) -> bool {
        let Some(msg_id) = crate::protocol::normalize_snowflake(message_id) else {
            return false;
        };
        let Some(channel) = self.channel_state_mut_by_id(channel_id) else {
            return false;
        };

        let changed = !crate::protocol::is_message_unread(
            Some(msg_id.as_str()),
            channel.last_read_message_id.as_deref(),
        ) == false
            || channel.mention_count > 0;
        if crate::protocol::is_newer_snowflake(
            Some(msg_id.as_str()),
            channel.last_read_message_id.as_deref(),
        ) || channel.last_read_message_id.is_none()
        {
            channel.last_read_message_id = Some(msg_id.clone());
        }
        let caught_up = !crate::protocol::is_message_unread(
            channel.last_message_id.as_deref(),
            channel.last_read_message_id.as_deref(),
        );
        if caught_up {
            channel.mention_count = 0;
            channel.unread = false;
            self.clear_unread_divider_for_channel(channel_id);
        }
        self.recompute_channel_unread_flags(channel_id);

        self.read_ack_debouncer.schedule(
            channel_id.to_string(),
            &msg_id,
            self.channel_state_ref_by_id(channel_id)
                .map(|c| c.mention_count)
                .unwrap_or(0),
            manual,
            std::time::Instant::now(),
        );
        self.flush_due_read_acks();
        changed || caught_up
    }

    /// Called when the user is at bottom of selected channel; ACKs latest message.
    pub fn maybe_ack_visible_channel_read(&mut self) {
        let Some(channel_id) = self.selected_channel_id.clone() else {
            return;
        };
        let Some(last_message_id) = self
            .channel_state_ref_by_id(&channel_id)
            .and_then(|c| c.last_message_id.clone())
        else {
            return;
        };
        let should_ack = self
            .channel_state_ref_by_id(&channel_id)
            .map(|c| {
                crate::protocol::is_message_unread(
                    Some(last_message_id.as_str()),
                    c.last_read_message_id.as_deref(),
                ) || c.mention_count > 0
            })
            .unwrap_or(false);
        if should_ack {
            self.mark_channel_read_up_to(&channel_id, &last_message_id, false);
        } else {
            self.flush_due_read_acks();
        }
    }

    pub fn flush_due_read_acks(&mut self) {
        let Some(cm) = self.client_manager.clone() else {
            return;
        };
        for ack in self.read_ack_debouncer.drain_due(std::time::Instant::now()) {
            let cm = cm.clone();
            tokio::spawn(async move {
                let token = ack.payload.token.as_deref();
                let _ = cm.ack_message(&ack.channel_id, &ack.message_id, token).await;
            });
        }
    }

    /// Opens the nickname editor for a guild, prefilled with current member nick.
    pub fn open_edit_guild_nickname(&mut self, guild_id: String) {
        let current_nick = self
            .members
            .iter()
            .find(|m| m.id == self.current_user.id)
            .and_then(|m| m.nickname.clone())
            .unwrap_or_default();
        self.guild_nickname_draft = current_nick;
        self.open_modal(ActiveModal::EditGuildNickname { guild_id });
    }

    /// Submits guild nickname change. UI updates only after 2xx response.
    pub fn submit_guild_nickname(&mut self, guild_id: String, nick: String) {
        if self.pending_guild_action.as_deref() == Some("nickname") {
            return;
        }
        let Some(cm) = self.client_manager.clone() else {
            self.auth_error = Some("Cliente não conectado.".to_string());
            return;
        };
        let nick_payload = {
            let trimmed = nick.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        };
        self.pending_guild_action = Some("nickname".to_string());
        let cm_clone = cm.clone();
        let gid = guild_id.clone();
        let current_user_id = self.current_user.id.clone();
        tokio::spawn(async move {
            let result = cm_clone
                .update_my_guild_nickname(&gid, nick_payload.as_deref())
                .await;
            match result {
                Ok(new_nick) => {
                    cm_clone.emit_event(TransportEvent::GuildNicknameUpdated {
                        guild_id: gid,
                        user_id: current_user_id,
                        nick: new_nick,
                    });
                }
                Err(e) => {
                    cm_clone.emit_event(TransportEvent::GuildActionFailed {
                        guild_id: gid,
                        action: "nickname".to_string(),
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    /// Confirms leaving a guild. Guild removed only after 2xx response.
    pub fn confirm_leave_guild(&mut self, guild_id: String) {
        if self.pending_guild_action.as_deref() == Some("leave") {
            return;
        }
        let Some(cm) = self.client_manager.clone() else {
            self.auth_error = Some("Cliente não conectado.".to_string());
            return;
        };
        self.pending_guild_action = Some("leave".to_string());
        let cm_clone = cm.clone();
        let gid = guild_id.clone();
        tokio::spawn(async move {
            match cm_clone.leave_guild(&gid).await {
                Ok(()) => {
                    cm_clone.emit_event(TransportEvent::GuildLeft { guild_id: gid });
                }
                Err(e) => {
                    cm_clone.emit_event(TransportEvent::GuildActionFailed {
                        guild_id: gid,
                        action: "leave".to_string(),
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    /// Removes guild locally and selects next destination. Called only after confirmed leave.
    pub fn apply_guild_left(&mut self, guild_id: &str) {
        let was_selected = self.selected_guild_id.as_deref() == Some(guild_id);
        let removed_index = self.guilds.iter().position(|g| g.id == guild_id);
        self.guilds.retain(|g| g.id != guild_id);
        self.guild_folders.iter_mut().for_each(|f| f.guild_ids.retain(|id| id != guild_id));
        self.guild_folders.retain(|f| !f.guild_ids.is_empty() || f.id.is_some());

        // Clean caches/state tied to guild
        self.members.retain(|m| m.guild_id.as_deref() != Some(guild_id));
        self.voice_states_by_channel.retain(|_, states| {
            states.retain(|s| s.guild_id.as_deref() != Some(guild_id));
            !states.is_empty()
        });

        save_guilds_to_disk(&self.guilds, &self.guild_folders);

        if was_selected {
            let next_guild = removed_index
                .and_then(|idx| self.guilds.get(idx).or_else(|| self.guilds.get(idx.saturating_sub(1))))
                .map(|g| g.id.clone());
            if let Some(next) = next_guild {
                self.select_guild(Some(next));
            } else {
                self.select_guild(None);
            }
        }
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
