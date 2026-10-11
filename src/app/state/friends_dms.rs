use super::*;
use crate::protocol::TransportEvent;

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
        if let Some(pos) = self.message_requests.iter().position(|r| r.id == request_id) {
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
            self.add_friend_feedback = Some((false, "Insira um nome de usuário válido.".to_string()));
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
                log::warn!("Não conectado ao Discord: impossível aceitar pedido de amizade para {}", user_id);
                return;
            }
        };

        self.pending_relationship_actions.insert(user_id.to_string());
        let uid = user_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = cm.accept_friend_request(&uid).await {
                log::warn!("Falha ao aceitar pedido de amizade: {:?}", e);
                cm.emit_event(crate::protocol::TransportEvent::RelationshipActionFailed {
                    id: uid,
                    error: e.to_string(),
                });
            }
        });
    }

    /// Removes, declines, cancels or unblocks a relationship (delegated to ClientManager)
    pub fn remove_relationship(&mut self, user_id: &str) {
        let cm = match self.client_manager.as_ref() {
            Some(cm) => cm.clone(),
            None => {
                log::warn!("Não conectado ao Discord: impossível remover relacionamento para {}", user_id);
                return;
            }
        };

        self.pending_relationship_actions.insert(user_id.to_string());
        let uid = user_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = cm.remove_relationship(&uid).await {
                log::warn!("Falha ao remover relacionamento: {:?}", e);
                cm.emit_event(crate::protocol::TransportEvent::RelationshipActionFailed {
                    id: uid,
                    error: e.to_string(),
                });
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
        let friend = match self.friends.iter().find(|f| f.id == user_id && f.relationship_type == 1) {
            Some(f) => f.clone(),
            None => return,
        };
        if let Some(act) = activity {
            let is_spotify = act.activity_type == crate::domain::user::ActivityType::Listening;

            // Compute real elapsed time from activity created_at timestamp if present
            let (elapsed_short, elapsed_bottom) = if let Some(ts) = act.created_at {
                let now_ts = chrono::Utc::now().timestamp();
                let diff = (now_ts - ts).max(0);
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
                crate::domain::user::ActivityType::Streaming => {
                    (ActiveNowKind::RichPresence, format!("Transmitindo {}", act.name))
                }
                crate::domain::user::ActivityType::Listening => {
                    (ActiveNowKind::Spotify, "Spotify".to_string())
                }
                crate::domain::user::ActivityType::Watching => {
                    (ActiveNowKind::RichPresence, format!("Assistindo {}", act.name))
                }
                _ => (ActiveNowKind::RichPresence, act.name.clone()),
            };

            let elapsed_time_text = if is_spotify { None } else { elapsed_bottom };

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
                spotify_track: if is_spotify { act.details.clone() } else { None },
                spotify_artist: if is_spotify { act.state.clone() } else { None },
                spotify_album: None,
                spotify_progress_text: None,
                spotify_progress_pct: None,
                button_text: None,
                avatar_decoration_data: friend.avatar_decoration_data.clone(),
            });
        }
    }
}

    // =========================================================================
    // Direct Messages & Group DM Subsystem
    // =========================================================================

impl AppState {
    /// Opens or selects a 1-on-1 DM with a user
    pub fn open_dm_with_user(&mut self, user_id: &str, username: &str) -> String {
        if let Some(existing) = self.dm_channels.iter().find(|d| {
            !d.is_group_dm && (d.recipient_ids.contains(&user_id.to_string()) || d.name == username)
        }) {
            let id = existing.id.clone();
            self.select_channel(id.clone());
            if id.starts_with("dm-") {
                if let Some(ref cm) = self.client_manager {
                    let cm = cm.clone();
                    let uid = user_id.to_string();
                    let temp_id = id.clone();
                    tokio::spawn(async move {
                        if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                            cm.emit_event(TransportEvent::DmChannelResolved {
                                temp_id,
                                channel: ch,
                            });
                        }
                    });
                }
            }
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
        self.select_channel(new_id.clone());

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let uid = user_id.to_string();
            let temp_id = new_id.clone();
            tokio::spawn(async move {
                if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                    cm.emit_event(TransportEvent::DmChannelResolved {
                        temp_id,
                        channel: ch,
                    });
                }
            });
        }

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
                if ch.topic.as_deref().map_or(true, |t| t.starts_with("Mensagens Diretas com")) {
                    ch.name = ch.recipient_names.join(", ");
                    ch.topic = Some("Conversa em Grupo".to_string());
                }
            }
        }
        self.close_modal();
    }
}
