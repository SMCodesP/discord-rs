use super::*;
use crate::protocol::TransportEvent;
use gpui::{App, AsyncApp, Entity};

/// Normalizes Discord activity timestamps (seconds or milliseconds) and
/// rejects missing, overflowing, zero-length, or reversed Spotify intervals.
fn spotify_timestamp_range(start: i64, end: i64) -> Option<(i64, i64)> {
    if start < 0 || end < 0 {
        return None;
    }
    let normalize = |timestamp: i64| {
        if timestamp < 100_000_000_000 {
            timestamp.checked_mul(1000)
        } else {
            Some(timestamp)
        }
    };
    let start_ms = normalize(start)?;
    let end_ms = normalize(end)?;
    if end_ms.checked_sub(start_ms)? <= 0 {
        return None;
    }
    Some((start_ms, end_ms))
}

impl ActiveNowItem {
    /// Valid, normalized Spotify interval used by both rendering and the 1 Hz
    /// progress timer. Invalid payloads never keep a timer alive.
    pub(crate) fn spotify_timestamp_range(&self) -> Option<(i64, i64)> {
        match (self.timestamps_start, self.timestamps_end) {
            (Some(start), Some(end)) => spotify_timestamp_range(start, end),
            _ => None,
        }
    }
}

impl AppState {
    /// Selects active tab in Friends Hub (Online, All, Pending, Blocked, AddFriend, MessageRequests)
    pub fn select_friends_tab(&mut self, tab: FriendsTab) {
        self.friends_tab = tab;
    }

    /// Selects subtab in Message Requests view (YouMayKnow or Spam)
    pub fn select_message_requests_subtab(&mut self, subtab: MessageRequestsSubTab) {
        self.message_requests_subtab = subtab;
    }

    /// Accepts an incoming message request, creating a DM channel and navigating to it
    pub fn accept_message_request(&mut self, request_id: &str) {
        if let Some(pos) = self
            .message_requests
            .iter()
            .position(|r| r.id == request_id)
        {
            let req = self.message_requests.remove(pos);
            self.open_dm_with_user(&req.user_id, &req.display_name);
        }
    }

    /// Ignores/dismisses an incoming message request
    pub fn ignore_message_request(&mut self, request_id: &str) {
        self.message_requests.retain(|r| r.id != request_id);
    }

    /// Clears all flagged spam message requests
    pub fn clear_spam_requests(&mut self) {
        self.message_requests.retain(|r| !r.is_spam);
    }

    /// Sends a friend request by username (and optional tag) via active ClientManager
    pub fn send_friend_request(&mut self, target: &str) {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            self.add_friend_feedback =
                Some((false, "Insira um nome de usuário válido.".to_string()));
            return;
        }

        let cm = match self.client_manager.as_ref() {
            Some(cm) => cm.clone(),
            None => {
                self.is_sending_friend_request = false;
                self.add_friend_feedback = Some((false, "Não conectado ao Discord.".to_string()));
                return;
            }
        };

        let (uname, disc) = match trimmed.split_once('#') {
            Some((u, d)) => (u.trim().to_string(), Some(d.trim().to_string())),
            None => (trimmed.to_string(), None),
        };

        self.is_sending_friend_request = true;
        self.add_friend_feedback = None;

        let u = uname;
        let d = disc;
        tokio::spawn(async move {
            match cm.send_friend_request(&u, d.as_deref()).await {
                Ok(()) => {
                    cm.emit_event(crate::protocol::TransportEvent::FriendRequestResult {
                        username: u.clone(),
                        success: true,
                        message: format!("Solicitação de amizade enviada para {}!", u),
                    });
                }
                Err(e) => {
                    cm.emit_event(crate::protocol::TransportEvent::FriendRequestResult {
                        username: u,
                        success: false,
                        message: e.to_string(),
                    });
                }
            }
        });
    }

    /// Accepts an incoming friend request (delegated to ClientManager)
    pub fn accept_friend_request(&mut self, user_id: &str) {
        let cm = match self.client_manager.as_ref() {
            Some(cm) => cm.clone(),
            None => {
                self.add_friend_feedback = Some((false, "Não conectado ao Discord.".to_string()));
                log::warn!(
                    "Não conectado ao Discord: impossível aceitar pedido de amizade para {}",
                    user_id
                );
                return;
            }
        };

        self.add_friend_feedback = None;
        self.pending_relationship_actions
            .insert(user_id.to_string());
        let uid = user_id.to_string();
        let accepted_relationship = self.friends.iter().find(|f| f.id == user_id).map(|f| {
            crate::protocol::transport::Relationship {
                id: f.id.clone(),
                username: f.username.clone(),
                avatar: f.avatar_url.clone(),
                global_name: Some(if f.display_name.is_empty() {
                    f.username.clone()
                } else {
                    f.display_name.clone()
                }),
                rel_type: crate::protocol::transport::Relationship::TYPE_FRIEND,
                avatar_decoration_data: f.avatar_decoration_data.clone(),
            }
        });
        tokio::spawn(async move {
            match cm.accept_friend_request(&uid).await {
                Ok(()) => {
                    // The REST success is authoritative. Emit the confirmed
                    // relationship immediately instead of depending solely on
                    // a gateway echo that may be delayed or lost.
                    if let Some(relationship) = accepted_relationship {
                        cm.emit_event(crate::protocol::TransportEvent::RelationshipAdd(
                            relationship,
                        ));
                    }
                }
                Err(e) => {
                    log::warn!("Falha ao aceitar pedido de amizade: {:?}", e);
                    cm.emit_event(crate::protocol::TransportEvent::RelationshipActionFailed {
                        id: uid,
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    /// Removes, declines, cancels or unblocks a relationship (delegated to ClientManager)
    pub fn remove_relationship(&mut self, user_id: &str) {
        let cm = match self.client_manager.as_ref() {
            Some(cm) => cm.clone(),
            None => {
                self.add_friend_feedback = Some((false, "Não conectado ao Discord.".to_string()));
                log::warn!(
                    "Não conectado ao Discord: impossível remover relacionamento para {}",
                    user_id
                );
                return;
            }
        };

        self.add_friend_feedback = None;
        self.pending_relationship_actions
            .insert(user_id.to_string());
        let uid = user_id.to_string();
        tokio::spawn(async move {
            match cm.remove_relationship(&uid).await {
                Ok(()) => {
                    // Confirmed API result: update immediately even if the
                    // gateway REMOVE echo is delayed or missing.
                    cm.emit_event(crate::protocol::TransportEvent::RelationshipRemove { id: uid });
                }
                Err(e) => {
                    log::warn!("Falha ao remover relacionamento: {:?}", e);
                    cm.emit_event(crate::protocol::TransportEvent::RelationshipActionFailed {
                        id: uid,
                        error: e.to_string(),
                    });
                }
            }
        });
    }

    /// Updates or removes an Active Now item when friend presence or activity changes
    pub fn update_active_now_from_presence(
        &mut self,
        user_id: &str,
        status: PresenceStatus,
        activity: Option<&crate::domain::Activity>,
    ) {
        self.active_now_items.retain(|a| a.user_id != user_id);
        if status == PresenceStatus::Offline {
            return;
        }
        let friend = match self
            .friends
            .iter()
            .find(|f| f.id == user_id && f.relationship_type == 1)
        {
            Some(f) => f.clone(),
            None => return,
        };
        if let Some(act) = activity {
            // Custom status is rendered in the DM/friend subtitle, not as an
            // Active Now activity card.
            if act.activity_type == crate::domain::user::ActivityType::Custom {
                return;
            }
            let is_spotify = act.activity_type == crate::domain::user::ActivityType::Listening
                && (act.name.eq_ignore_ascii_case("Spotify")
                    || act.sync_id.is_some()
                    || act
                        .assets
                        .as_ref()
                        .and_then(|a| a.large_image.as_deref())
                        .map_or(false, |image| image.starts_with("spotify:")));

            // Discord activity timestamps are milliseconds. Accept seconds for
            // older/synthetic payloads, but never let a future or overflowing
            // timestamp fabricate an elapsed duration.
            let (elapsed_short, elapsed_bottom) =
                if let Some(ts) = act.created_at.filter(|ts| *ts >= 0) {
                    let created_at_ms = if ts < 100_000_000_000 {
                        ts.checked_mul(1000)
                    } else {
                        Some(ts)
                    };
                    created_at_ms
                        .map(|created_at_ms| {
                            let now_ms = chrono::Utc::now().timestamp_millis();
                            if now_ms < created_at_ms {
                                return (None, None);
                            }
                            let diff = (now_ms - created_at_ms) / 1000;
                            let hours = diff / 3600;
                            let mins = (diff % 3600) / 60;
                            let secs = diff % 60;
                            let short = if hours > 0 {
                                format!("{}h {}m", hours, mins)
                            } else {
                                format!("{}m", mins.max(1))
                            };
                            let bottom = if hours > 0 {
                                format!("{:02}:{:02}:{:02} elapsed", hours, mins, secs)
                            } else {
                                format!("{:02}:{:02} elapsed", mins, secs)
                            };
                            (Some(short), Some(bottom))
                        })
                        .unwrap_or((None, None))
                } else {
                    (None, None)
                };

            let (kind, header_subtitle) = match act.activity_type {
                crate::domain::user::ActivityType::Playing => {
                    let sub = if let Some(ref dur) = elapsed_short {
                        format!("{} – {}", act.name, dur)
                    } else {
                        act.name.clone()
                    };
                    (ActiveNowKind::Game, sub)
                }
                crate::domain::user::ActivityType::Streaming => (
                    ActiveNowKind::RichPresence,
                    format!("Transmitindo {}", act.name),
                ),
                crate::domain::user::ActivityType::Listening if is_spotify => {
                    (ActiveNowKind::Spotify, "Spotify".to_string())
                }
                crate::domain::user::ActivityType::Listening => {
                    (ActiveNowKind::RichPresence, format!("Ouvindo {}", act.name))
                }
                crate::domain::user::ActivityType::Watching => (
                    ActiveNowKind::RichPresence,
                    format!("Assistindo {}", act.name),
                ),
                _ => (ActiveNowKind::RichPresence, act.name.clone()),
            };

            let elapsed_time_text = if is_spotify { None } else { elapsed_bottom };

            let large_image_url = act.large_image_url();
            let large_image_text = act.assets.as_ref().and_then(|a| a.large_text.clone());
            let small_image_url = act.small_image_url();
            let small_image_text = act.assets.as_ref().and_then(|a| a.small_text.clone());
            let timestamps_start = act.timestamps.as_ref().and_then(|t| t.start);
            let timestamps_end = act.timestamps.as_ref().and_then(|t| t.end);
            let application_id = act.application_id.clone();
            let spotify_album = if is_spotify {
                act.assets.as_ref().and_then(|a| a.large_text.clone())
            } else {
                None
            };

            let (spotify_progress_text, spotify_progress_pct) = if is_spotify {
                if let (Some(start), Some(end)) = (timestamps_start, timestamps_end) {
                    spotify_timestamp_range(start, end)
                        .map(|(start_ms, end_ms)| {
                            let now_ms = chrono::Utc::now().timestamp_millis();
                            let total_ms = end_ms - start_ms;
                            let elapsed_ms = now_ms.saturating_sub(start_ms).clamp(0, total_ms);
                            let ratio = (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0);
                            let cur_s = elapsed_ms / 1000;
                            let tot_s = total_ms / 1000;
                            (
                                Some((
                                    format!("{}:{:02}", cur_s / 60, cur_s % 60),
                                    format!("{}:{:02}", tot_s / 60, tot_s % 60),
                                )),
                                Some(ratio),
                            )
                        })
                        .unwrap_or((None, None))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            self.active_now_items.push(ActiveNowItem {
                id: format!("act-{}", user_id),
                user_id: user_id.to_string(),
                username: friend.username.clone(),
                display_name: friend.display_name.clone(),
                avatar_url: friend.avatar_url.clone(),
                avatar_text: friend.avatar_text.clone(),
                presence: status,
                kind,
                app_name: act.name.clone(),
                header_subtitle,
                app_icon_symbol: String::new(),
                details_title: act.details.clone().unwrap_or_else(|| act.name.clone()),
                details_state: act.state.clone(),
                elapsed_time_text,
                spotify_track: if is_spotify {
                    act.details.clone()
                } else {
                    None
                },
                spotify_artist: if is_spotify { act.state.clone() } else { None },
                spotify_album,
                spotify_progress_text,
                spotify_progress_pct,
                button_text: None,
                large_image_url,
                large_image_text,
                small_image_url,
                small_image_text,
                application_id,
                timestamps_start,
                timestamps_end,
                avatar_decoration_data: friend.avatar_decoration_data.clone(),
            });
        }
    }

    /// Returns true if the Friends Hub is currently visible (Home with no channel selected)
    /// and there is at least one Spotify activity with timestamps requiring a periodic clock update.
    pub fn has_visible_spotify_progress(&self) -> bool {
        self.selected_guild_id.is_none()
            && self.selected_channel_id.is_none()
            && self.active_now_items.iter().any(|item| {
                item.kind == ActiveNowKind::Spotify && item.spotify_timestamp_range().is_some()
            })
    }

    /// Updates dynamic Spotify progress for all active Spotify items with timestamps
    /// based on the current system clock. Returns true if any item was updated.
    pub fn tick_spotify_progress(&mut self) -> bool {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut updated = false;

        for item in &mut self.active_now_items {
            if item.kind == ActiveNowKind::Spotify {
                if let Some((start_ms, end_ms)) = item.spotify_timestamp_range() {
                    let total_ms = end_ms - start_ms;
                    let elapsed_ms = now_ms.saturating_sub(start_ms).clamp(0, total_ms);
                    let ratio = (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0);
                    let cur_s = elapsed_ms / 1000;
                    let tot_s = total_ms / 1000;
                    let text = Some((
                        format!("{}:{:02}", cur_s / 60, cur_s % 60),
                        format!("{}:{:02}", tot_s / 60, tot_s % 60),
                    ));
                    item.spotify_progress_text = text;
                    item.spotify_progress_pct = Some(ratio);
                    updated = true;
                }
            }
        }

        updated
    }
}

// =========================================================================
// Direct Messages & Group DM Subsystem
// =========================================================================

impl AppState {
    /// Starts a voice call with a friend, resolving the real DM channel first
    /// when the relationship does not already have one. No local/temporary
    /// channel ID is ever passed to the voice stack.
    pub fn call_friend(entity: Entity<Self>, user_id: String, cx: &mut App) {
        let app = entity.clone();
        entity.update(cx, |state, cx| {
            let uid = user_id.clone();
            let existing_dm = state.dm_channels.iter().find(|dm| {
                !dm.is_group_dm && dm.recipient_ids.iter().any(|id| id == &uid)
            });

            if let Some(dm) = existing_dm {
                if dm.id.parse::<u64>().is_ok() {
                    let already_active = state.active_voice_call.as_ref().map_or(false, |call| {
                        call.channel_id == dm.id
                            && !matches!(
                                call.connection_state,
                                crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }
                            )
                    });
                    if already_active {
                        return;
                    }

                    let channel_id = dm.id.clone();
                    let channel_name = dm.name.clone();
                    state.add_friend_feedback = None;
                    state.select_channel(channel_id.clone());
                    state.connect_voice_dm_with_ring(
                        &channel_id,
                        &channel_name,
                        Some(vec![uid]),
                    );
                    cx.notify();
                    return;
                }
            }

            let cm = match state.client_manager.clone() {
                Some(cm) => cm,
                None => {
                    state.add_friend_feedback =
                        Some((false, "Não conectado ao Discord.".to_string()));
                    cx.notify();
                    return;
                }
            };

            let app_weak = app.downgrade();
            let uid_async = user_id.clone();
            cx.spawn(move |_state, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    match cm.create_or_get_dm_channel(&uid_async).await {
                        Ok(channel) => {
                            let _ = cx.update(|cx| {
                                let _ = app_weak.update(cx, |state, cx| {
                                    state.handle_transport_event(
                                        TransportEvent::DmChannelCreated(channel),
                                    );
                                    let resolved = state.dm_channels.iter().find(|dm| {
                                        !dm.is_group_dm
                                            && dm.recipient_ids.iter().any(|id| id == &uid_async)
                                    });
                                    if let Some(dm) = resolved {
                                        let channel_id = dm.id.clone();
                                        let channel_name = dm.name.clone();
                                        state.add_friend_feedback = None;
                                        state.select_channel(channel_id.clone());
                                        state.connect_voice_dm_with_ring(
                                            &channel_id,
                                            &channel_name,
                                            Some(vec![uid_async.clone()]),
                                        );
                                    }
                                    cx.notify();
                                });
                            });
                        }
                        Err(error) => {
                            log::warn!("Falha ao resolver DM para chamada: {:?}", error);
                            let _ = cx.update(|cx| {
                                let _ = app_weak.update(cx, |state, cx| {
                                    state.add_friend_feedback = Some((
                                        false,
                                        format!("Não foi possível iniciar a chamada: {}", error),
                                    ));
                                    cx.notify();
                                });
                            });
                        }
                    }
                }
            })
            .detach();
        });
    }

    /// Opens or selects a 1-on-1 DM with a user
    pub fn open_dm_with_user(&mut self, user_id: &str, username: &str) -> String {
        if user_id.trim().is_empty() {
            self.add_friend_feedback = Some((
                false,
                "Não foi possível identificar o usuário desta conversa.".to_string(),
            ));
            return String::new();
        }

        if let Some(existing) = self.dm_channels.iter().find(|d| {
            !d.is_group_dm && (d.recipient_ids.contains(&user_id.to_string()) || d.name == username)
        }) {
            let id = existing.id.clone();
            // select_channel resolves temporary `dm-*` IDs through the real
            // ClientManager; do not issue a second duplicate request here.
            self.select_channel(id.clone());
            return id;
        }

        let new_id = format!("dm-{}", user_id);
        let item = ChannelItemState {
            id: new_id.clone(),
            name: username.to_string(),
            kind: ChannelKind::Text,
            topic: Some(format!("Mensagens Diretas com {}", username)),
            unread: false,
            mention_count: 0,
            last_read_message_id: None,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: false,
            recipient_ids: vec![user_id.to_string()],
            recipient_names: vec![username.to_string()],
            user_limit: None,
            avatar_decoration_data: None,
        };

        self.dm_channels.insert(0, item);
        self.sort_dm_channels();
        // select_channel owns temporary DM resolution and message loading.
        self.select_channel(new_id.clone());

        new_id
    }

    /// Creates a new Group DM channel with multiple participants
    pub fn create_group_dm(
        &mut self,
        recipient_ids: Vec<String>,
        recipient_names: Vec<String>,
        name: Option<String>,
    ) -> String {
        let gid = format!("dm-group-{}", chrono::Utc::now().timestamp_millis());
        let display_name = name.unwrap_or_else(|| {
            if recipient_names.is_empty() {
                "Grupo Vazio".to_string()
            } else {
                recipient_names.join(", ")
            }
        });

        let item = ChannelItemState {
            id: gid.clone(),
            name: display_name,
            kind: ChannelKind::Text,
            topic: Some("Conversa em Grupo".into()),
            unread: false,
            mention_count: 0,
            last_read_message_id: None,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: true,
            recipient_ids,
            recipient_names,
            user_limit: None,
            avatar_decoration_data: None,
        };

        self.dm_channels.insert(0, item);
        self.sort_dm_channels();
        self.select_channel(gid.clone());
        self.close_modal();
        gid
    }

    /// Adds a friend/recipient to an existing Group DM or escalates a 1-on-1 DM
    pub fn add_recipient_to_group(&mut self, channel_id: &str, user_id: &str, username: &str) {
        if let Some(ch) = self.dm_channels.iter_mut().find(|c| c.id == channel_id) {
            if !ch.recipient_ids.contains(&user_id.to_string()) {
                ch.recipient_ids.push(user_id.to_string());
                ch.recipient_names.push(username.to_string());
                ch.is_group_dm = true;
                if ch
                    .topic
                    .as_deref()
                    .map_or(true, |t| t.starts_with("Mensagens Diretas com"))
                {
                    ch.name = ch.recipient_names.join(", ");
                    ch.topic = Some("Conversa em Grupo".to_string());
                }
            }
        }
        self.close_modal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::user::Activity;

    fn friend(id: &str, username: &str, relationship_type: u32) -> FriendItemState {
        FriendItemState {
            id: id.into(),
            username: username.into(),
            display_name: username.into(),
            avatar_url: None,
            avatar_text: "F".into(),
            presence: PresenceStatus::Online,
            custom_status: None,
            relationship_type,
            avatar_decoration_data: None,
        }
    }

    #[test]
    fn spotify_range_rejects_invalid_intervals_and_normalizes_seconds() {
        assert_eq!(
            spotify_timestamp_range(1_700_000_000, 1_700_000_180),
            Some((1_700_000_000_000, 1_700_000_180_000))
        );
        assert_eq!(
            spotify_timestamp_range(1_700_000_000_000, 1_700_000_000_000),
            None
        );
        assert_eq!(
            spotify_timestamp_range(1_700_000_180_000, 1_700_000_000_000),
            None
        );
        assert_eq!(spotify_timestamp_range(i64::MAX, i64::MAX), None);
        assert_eq!(spotify_timestamp_range(-1, 1_700_000_180), None);
        assert_eq!(
            spotify_timestamp_range(i64::MAX / 2, i64::MAX / 2 + 1),
            None
        );
    }

    #[test]
    fn invalid_spotify_interval_does_not_keep_progress_timer_active() {
        let mut state = AppState::new();
        state.friends = vec![friend("1", "friend", 1)];
        state.active_now_items.clear();
        state.selected_guild_id = None;
        state.selected_channel_id = None;

        for (start, end) in [
            (1_700_000_000_000, 1_700_000_000_000),
            (1_700_000_180_000, 1_700_000_000_000),
        ] {
            let mut activity =
                Activity::new("Spotify", crate::domain::user::ActivityType::Listening);
            activity.timestamps = Some(crate::domain::user::ActivityTimestamps {
                start: Some(start),
                end: Some(end),
            });
            state.update_active_now_from_presence("1", PresenceStatus::Online, Some(&activity));
            assert_eq!(state.active_now_items.len(), 1);
            assert!(state.active_now_items[0].spotify_progress_text.is_none());
            assert!(state.active_now_items[0].spotify_progress_pct.is_none());
            assert!(!state.has_visible_spotify_progress());
            assert!(!state.tick_spotify_progress());
        }
    }

    #[test]
    fn friend_rows_use_only_relationships_and_deduplicate_by_id() {
        let mut state = AppState::new();
        state.friends = vec![friend("1", "same_name", 1), friend("2", "same_name", 1)];
        state.friends_tab = FriendsTab::All;
        state.auth_token = None;
        state.is_connected_to_gateway = false;
        state.dm_channels.push(ChannelItemState {
            id: "100000000000000001".into(),
            name: "DM sem relacionamento".into(),
            kind: ChannelKind::Text,
            topic: None,
            unread: false,
            mention_count: 0,
            last_read_message_id: None,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: false,
            recipient_ids: vec!["200000000000000002".into()],
            recipient_names: vec!["Contato".into()],
            user_limit: None,
            avatar_decoration_data: None,
        });

        let rows = crate::app::friends_hub::collect_friend_rows(&state, "");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user_id, "1");
        assert_eq!(rows[1].user_id, "2");
    }

    #[test]
    fn active_now_excludes_custom_status_and_non_spotify_listening() {
        let mut state = AppState::new();
        state.friends = vec![friend("1", "friend", 1)];
        state.active_now_items.clear();

        state.update_active_now_from_presence(
            "1",
            PresenceStatus::Online,
            Some(&Activity::custom("Trabalhando")),
        );
        assert!(state.active_now_items.is_empty());

        state.update_active_now_from_presence(
            "1",
            PresenceStatus::Online,
            Some(&Activity::new(
                "Outro player",
                crate::domain::user::ActivityType::Listening,
            )),
        );
        assert_eq!(state.active_now_items.len(), 1);
        assert_eq!(state.active_now_items[0].kind, ActiveNowKind::RichPresence);
    }

    #[test]
    fn friend_rows_include_emoji_only_custom_status_from_presence_cache() {
        let mut state = AppState::new();
        state.friends = vec![friend("1", "friend", 1)];
        state.friends_tab = FriendsTab::All;
        state.update_user_presence(
            "1".to_string(),
            crate::domain::user::PresenceStatus::Online,
            None,
            Some(crate::domain::user::CustomStatus::new(
                None,
                Some(crate::domain::user::CustomStatusEmoji::unicode("🌸")),
            )),
        );

        let rows = crate::app::friends_hub::collect_friend_rows(&state, "");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "🌸");
    }

    #[test]
    fn game_elapsed_time_normalizes_millisecond_created_at() {
        let mut state = AppState::new();
        state.friends = vec![friend("1", "friend", 1)];
        state.active_now_items.clear();
        let mut activity = Activity::playing("Game");
        activity.created_at = Some(chrono::Utc::now().timestamp_millis() - 120_000);

        state.update_active_now_from_presence("1", PresenceStatus::Online, Some(&activity));

        let elapsed = state.active_now_items[0]
            .elapsed_time_text
            .as_deref()
            .expect("elapsed time");
        assert!(elapsed.starts_with("02:"), "unexpected elapsed: {elapsed}");
    }

    #[test]
    fn opening_temporary_dm_keeps_pinned_dms_at_the_top() {
        let mut state = AppState::new();
        state.dm_channels.clear();
        state.pinned_dms.clear();
        state.client_manager = None;
        state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
            crate::domain::Channel::text("100000000000000001", "Fixa"),
        ]));
        state.pin_dm_channel("100000000000000001");

        let temporary_id = state.open_dm_with_user("200000000000000002", "Nova");
        assert_eq!(state.dm_channels[0].id, "100000000000000001");
        assert_eq!(state.dm_channels[1].id, temporary_id);
    }

    #[tokio::test]
    async fn accepted_request_emits_confirmed_relationship_without_gateway_echo() {
        let mut state = AppState::new();
        state.friends = vec![friend("target-user", "target", 3)];
        let (cm, mut rx) = crate::protocol::ClientManager::new_mock();
        state.client_manager = Some(cm);

        state.accept_friend_request("target-user");
        assert!(state.pending_relationship_actions.contains("target-user"));

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("confirmed relationship event")
            .expect("event channel open");
        match &event {
            TransportEvent::RelationshipAdd(relationship) => {
                assert_eq!(relationship.id, "target-user");
                assert_eq!(relationship.rel_type, 1);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        state.handle_transport_event(event);
        assert!(!state.pending_relationship_actions.contains("target-user"));
        assert_eq!(state.friends[0].relationship_type, 1);
    }
}
