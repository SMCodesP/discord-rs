use std::collections::HashSet;
use std::sync::Mutex;
use gpui_kit::{Pixels, Point};
use super::*;

static QUICK_SEND_IN_FLIGHT: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn is_quick_send_in_flight(user_id: &str) -> bool {
    let mut lock = QUICK_SEND_IN_FLIGHT.lock().unwrap();
    let set = lock.get_or_insert_with(HashSet::new);
    if set.contains(user_id) {
        true
    } else {
        set.insert(user_id.to_string());
        false
    }
}

fn clear_quick_send_in_flight(user_id: &str) {
    if let Ok(mut lock) = QUICK_SEND_IN_FLIGHT.lock() {
        if let Some(ref mut set) = *lock {
            set.remove(user_id);
        }
    }
}

impl AppState {
    /// Opens a modal dialog
    pub fn open_modal(&mut self, modal: ActiveModal) {
        if let Some(ref tel) = self.telemetry {
            let screen = match modal {
                ActiveModal::UserSettings => crate::telemetry::ScreenKind::UserSettingsModal {
                    tab: self.user_settings_tab.title().into(),
                },
                ActiveModal::AddServer => crate::telemetry::ScreenKind::AddServerModal,
                ActiveModal::ExploreServers => crate::telemetry::ScreenKind::ExploreServersModal,
                ActiveModal::Login | ActiveModal::LoginDiscord => crate::telemetry::ScreenKind::LoginModal,
                ActiveModal::AddFriendToGroup(_) | ActiveModal::CreateGroupDm => {
                    crate::telemetry::ScreenKind::FriendsHub
                }
            };
            let mut meta = std::collections::HashMap::new();
            meta.insert("modal".into(), format!("{:?}", modal));
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_button_click("open_modal", format!("{:?}", modal), 0);
        }
        self.active_modal = Some(modal);
    }

    /// Closes the active modal
    pub fn close_modal(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("close_modal", "Fechar Modal", 0);
        }
        self.active_modal = None;
    }

    /// Opens the user profile popout for a user (with current guild context and optional screen anchor)
    pub fn open_user_profile(&mut self, user_id: &str, anchor: Option<Point<Pixels>>) {
        let current_guild = self.selected_guild_id.clone();
        self.open_user_profile_in_guild(user_id, current_guild, anchor);
    }

    /// Opens the user profile popout with specific guild context and optional screen anchor
    pub fn open_user_profile_in_guild(
        &mut self,
        user_id: &str,
        guild_id: Option<String>,
        anchor: Option<Point<Pixels>>,
    ) {
        let uid = user_id.to_string();

        // Guard against duplicate concurrent fetch if already loading this user
        if let Some(ref popout) = self.profile_popout {
            if popout.user_id == uid && popout.is_loading {
                self.profile_anchor = anchor;
                return;
            }
        }

        self.profile_anchor = anchor;
        self.profile_modal_open = false;
        self.profile_modal_tab = ProfileModalTab::AboutMe;

        let fallback = self.build_fallback_profile_data(&uid, guild_id.as_deref());

        self.profile_popout = Some(crate::domain::profile::ProfilePopoutState {
            user_id: uid.clone(),
            guild_id: guild_id.clone(),
            data: fallback,
            is_loading: true,
            error: None,
            active_tab: crate::domain::profile::ProfileTab::MutualGuilds,
            quick_message: String::new(),
        });

        // Trigger single asynchronous fetch via ClientManager if present
        if let Some(cm) = self.client_manager.clone() {
            let gid = guild_id.clone();
            let target_uid = uid.clone();
            tokio::spawn(async move {
                match cm.get_user_profile_with_guild(&target_uid, gid.as_deref()).await {
                    Ok(profile) => {
                        cm.emit_event(crate::protocol::transport::TransportEvent::UserProfileLoaded {
                            user_id: target_uid,
                            profile: Box::new(profile),
                        });
                    }
                    Err(err) => {
                        cm.emit_event(crate::protocol::transport::TransportEvent::UserProfileError {
                            user_id: target_uid,
                            error: err.to_string(),
                        });
                    }
                }
            });
        }
    }

    /// Closes the active user profile popout and resets anchor/modal state
    pub fn close_profile_popout(&mut self) {
        if let Some(ref popout) = self.profile_popout {
            clear_quick_send_in_flight(&popout.user_id);
        }
        self.profile_popout = None;
        self.profile_anchor = None;
        self.profile_modal_open = false;
    }

    /// Returns whether the full user profile modal dialog is open (requires popout to be open)
    pub fn is_profile_modal_open(&self) -> bool {
        self.profile_modal_open && self.profile_popout.is_some()
    }

    /// Opens the full user profile modal dialog (only succeeds if profile popout is currently open)
    pub fn open_user_profile_modal(&mut self) {
        if self.profile_popout.is_some() {
            self.profile_modal_open = true;
        }
    }

    /// Closes the full user profile modal dialog while keeping the popout open
    pub fn close_user_profile_modal(&mut self) {
        self.profile_modal_open = false;
    }

    /// Sets the active tab in the full user profile modal dialog
    pub fn set_profile_modal_tab(&mut self, tab: ProfileModalTab) {
        self.profile_modal_tab = tab;
    }

    /// Handles Escape key dispatch, closing one level at a time:
    /// Modal -> Popout -> other modals. Returns true if something was closed.
    pub fn handle_escape(&mut self) -> bool {
        if self.is_profile_modal_open() {
            self.close_user_profile_modal();
            true
        } else if self.profile_popout.is_some() {
            self.close_profile_popout();
            true
        } else if self.active_modal.is_some() {
            self.close_modal();
            true
        } else {
            false
        }
    }

    /// Updates the profile popout with loaded data
    pub fn set_profile_popout_data(&mut self, user_id: &str, data: crate::domain::profile::UserProfileData) {
        if let Some(ref mut popout) = self.profile_popout {
            if popout.user_id == user_id {
                popout.data = Some(data);
                popout.is_loading = false;
                popout.error = None;
            }
        }
    }

    /// Sets an error on the profile popout
    pub fn set_profile_popout_error(&mut self, user_id: &str, error: String) {
        if let Some(ref mut popout) = self.profile_popout {
            if popout.user_id == user_id {
                popout.is_loading = false;
                if popout.data.is_none() {
                    popout.error = Some(error);
                }
            }
        }
    }
}

/// Request bundle produced by `begin_profile_popout_quick_message`.
/// Encapsulates the target channel, content and client manager needed to execute the send
/// asynchronously outside `AppState` (e.g. via `cx.spawn`).
#[derive(Debug, Clone)]
pub struct ProfileQuickSend {
    pub client_manager: crate::protocol::ClientManager,
    pub channel_id: String,
    pub content: String,
    pub user_id: String,
}

impl ProfileQuickSend {
    /// Executes the message dispatch asynchronously via ClientManager
    pub async fn execute(&self) -> Result<crate::domain::message::Message, String> {
        self.client_manager
            .send_message(&self.channel_id, &self.content)
            .await
            .map_err(|e| e.to_string())
    }
}

impl AppState {
    /// Sets an error specifically on the active profile popout quick message
    pub fn set_profile_popout_quick_message_error(&mut self, error: String) {
        if let Some(ref mut popout) = self.profile_popout {
            popout.error = Some(error);
        }
    }

    /// Returns whether a quick message send is currently in-flight for the open popout
    pub fn is_profile_popout_quick_message_sending(&self) -> bool {
        if let Some(ref popout) = self.profile_popout {
            if let Ok(lock) = QUICK_SEND_IN_FLIGHT.lock() {
                if let Some(ref set) = *lock {
                    return set.contains(&popout.user_id);
                }
            }
        }
        false
    }

    /// First phase of popout quick direct message send.
    /// - Returns None if popout is absent, input is empty/whitespace, or already sending (blocks duplicate Enter).
    /// - If ClientManager is absent: records "Não conectado ao Discord." on popout.error, preserves field and returns None (ZERO MOCKS).
    /// - If ClientManager is present: registers in-flight send, resolves DM channel, and returns ProfileQuickSend.
    pub fn begin_profile_popout_quick_message(&mut self) -> Option<ProfileQuickSend> {
        let (user_id, username, content) = {
            let Some(ref mut popout) = self.profile_popout else {
                return None;
            };
            let content = popout.quick_message.trim().to_string();
            if content.is_empty() {
                return None;
            }
            let user_id = popout.user_id.clone();
            let username = popout
                .data
                .as_ref()
                .map(|d| d.user.username.clone())
                .unwrap_or_else(|| user_id.clone());

            if self.client_manager.is_none() {
                // ZERO MOCKS: Sem client_manager não inventa sucesso nem mensagem offline
                popout.error = Some("Não conectado ao Discord.".to_string());
                return None;
            }

            if is_quick_send_in_flight(&user_id) {
                // Blocks duplicate sends while an existing request for this user is in-flight
                return None;
            }

            popout.error = None;
            (user_id, username, content)
        };

        let channel_id = self.open_dm_with_user(&user_id, &username);
        let cm = match self.client_manager.as_ref().cloned() {
            Some(cm) => cm,
            None => {
                clear_quick_send_in_flight(&user_id);
                return None;
            }
        };

        Some(ProfileQuickSend {
            client_manager: cm,
            channel_id,
            content,
            user_id,
        })
    }

    /// Second phase of popout quick direct message send.
    /// - Consumes result from ProfileQuickSend::execute.
    /// - On success: injects confirmed message into chat, clears quick_message, resets error and closes popout.
    /// - On error: records error in popout.error, preserves quick_message, popout remains open.
    /// - Resets in-flight sending state in all cases.
    pub fn finish_profile_popout_quick_message(
        &mut self,
        request: &ProfileQuickSend,
        result: Result<crate::domain::message::Message, String>,
    ) {
        clear_quick_send_in_flight(&request.user_id);

        match result {
            Ok(msg) => {
                let target_cid = if !msg.channel_id.is_empty() {
                    &msg.channel_id
                } else {
                    &request.channel_id
                };
                self.inject_message(target_cid, msg.clone());
                if let Some(dm) = self
                    .dm_channels
                    .iter_mut()
                    .find(|c| c.id == request.channel_id || c.id == *target_cid)
                {
                    dm.last_message_id =
                        Some(format!("{}", chrono::Utc::now().timestamp_millis()));
                    self.sort_dm_channels();
                }
                if let Some(ref mut popout) = self.profile_popout {
                    if popout.user_id == request.user_id {
                        popout.quick_message.clear();
                        popout.error = None;
                    }
                }
                self.close_profile_popout();
            }
            Err(err_msg) => {
                if let Some(ref mut popout) = self.profile_popout {
                    if popout.user_id == request.user_id {
                        popout.error = Some(err_msg);
                    }
                }
            }
        }
    }

    /// Synchronous convenience entry point for popout quick message send (called by root_view.rs:261).
    /// Uses begin_profile_popout_quick_message to validate, block duplicate Enter and check connection.
    /// When ClientManager is absent, records "Não conectado ao Discord." without injecting fake messages.
    /// When ClientManager is present, dispatches asynchronously via ClientManager without optimistic injection.
    pub fn send_profile_popout_quick_message(&mut self) {
        if let Some(req) = self.begin_profile_popout_quick_message() {
            let cm = req.client_manager.clone();
            let target_cid = req.channel_id.clone();
            let text = req.content.clone();
            let uid = req.user_id.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.send_message(&target_cid, &text).await {
                    log::warn!("Falha ao enviar mensagem rápida no popout: {:?}", e);
                    cm.emit_event(crate::protocol::transport::TransportEvent::UserProfileError {
                        user_id: uid,
                        error: e.to_string(),
                    });
                }
            });
        }
    }

    /// Builds fallback profile data from local friends, members, or current_user
    pub fn build_fallback_profile_data(&self, user_id: &str, _guild_id: Option<&str>) -> Option<crate::domain::profile::UserProfileData> {
        let to_domain_presence = |p: PresenceStatus| match p {
            PresenceStatus::Online => crate::domain::user::PresenceStatus::Online,
            PresenceStatus::Idle => crate::domain::user::PresenceStatus::Idle,
            PresenceStatus::DoNotDisturb => crate::domain::user::PresenceStatus::DoNotDisturb,
            PresenceStatus::Offline => crate::domain::user::PresenceStatus::Offline,
        };

        let uid = user_id.to_string();
        // Check current_user
        if self.current_user.id == uid || uid == "me" || uid == "@me" {
            let mut user = crate::domain::user::User::new(&self.current_user.id, &self.current_user.username);
            user.global_name = Some(self.current_user.display_name.clone());
            user.avatar = self.current_user.avatar_url.clone();
            user.discriminator = self.current_user.discriminator.clone();
            user.presence = to_domain_presence(self.current_user.presence);
            user.avatar_decoration_data = self.current_user.avatar_decoration_data.clone();
            let created_at = crate::domain::profile::snowflake_to_datetime(&user.id);
            return Some(crate::domain::profile::UserProfileData {
                user,
                bio: None,
                pronouns: None,
                banner: None,
                banner_color: None,
                accent_color: None,
                profile_effect_id: None,
                badges: Vec::new(),
                connected_accounts: Vec::new(),
                mutual_guilds: Vec::new(),
                mutual_friends_count: 0,
                created_at,
                joined_server_at: None,
            });
        }

        // Check members
        if let Some(m) = self.members.iter().find(|m| m.id == uid) {
            let mut user = crate::domain::user::User::new(&m.id, &m.name);
            user.global_name = Some(m.name.clone());
            user.bot = m.is_bot;
            user.presence = to_domain_presence(m.presence);
            user.avatar_decoration_data = m.avatar_decoration_data.clone();
            let created_at = crate::domain::profile::snowflake_to_datetime(&user.id);
            return Some(crate::domain::profile::UserProfileData {
                user,
                bio: None,
                pronouns: None,
                banner: None,
                banner_color: None,
                accent_color: None,
                profile_effect_id: None,
                badges: Vec::new(),
                connected_accounts: Vec::new(),
                mutual_guilds: Vec::new(),
                mutual_friends_count: 0,
                created_at,
                joined_server_at: None,
            });
        }

        // Check friends
        if let Some(f) = self.friends.iter().find(|f| f.id == uid) {
            let mut user = crate::domain::user::User::new(&f.id, &f.username);
            user.global_name = Some(f.display_name.clone());
            user.avatar = f.avatar_url.clone();
            user.presence = to_domain_presence(f.presence);
            user.avatar_decoration_data = f.avatar_decoration_data.clone();
            let created_at = crate::domain::profile::snowflake_to_datetime(&user.id);
            return Some(crate::domain::profile::UserProfileData {
                user,
                bio: None,
                pronouns: None,
                banner: None,
                banner_color: None,
                accent_color: None,
                profile_effect_id: None,
                badges: Vec::new(),
                connected_accounts: Vec::new(),
                mutual_guilds: Vec::new(),
                mutual_friends_count: 0,
                created_at,
                joined_server_at: None,
            });
        }

        // Check DM channels
        if let Some(dm) = self.dm_channels.iter().find(|dm| {
            dm.id == uid
                || dm.recipient_ids.contains(&uid)
                || dm.id.trim_start_matches("dm-") == uid
        }) {
            let recipient_id = dm.recipient_ids.first().cloned().unwrap_or_else(|| uid.clone());
            let mut user = crate::domain::user::User::new(&recipient_id, &dm.name);
            user.global_name = Some(dm.name.clone());
            user.avatar = dm.icon_url.clone();
            user.avatar_decoration_data = dm.avatar_decoration_data.clone();
            let created_at = crate::domain::profile::snowflake_to_datetime(&user.id);
            return Some(crate::domain::profile::UserProfileData {
                user,
                bio: None,
                pronouns: None,
                banner: None,
                banner_color: None,
                accent_color: None,
                profile_effect_id: None,
                badges: Vec::new(),
                connected_accounts: Vec::new(),
                mutual_guilds: Vec::new(),
                mutual_friends_count: 0,
                created_at,
                joined_server_at: None,
            });
        }

        None
    }
}
