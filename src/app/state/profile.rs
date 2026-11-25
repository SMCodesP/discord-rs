use super::*;
use gpui_kit::{Pixels, Point};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

static QUICK_SEND_IN_FLIGHT: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_QUICK_SEND_ID: AtomicU64 = AtomicU64::new(1);

/// Registers a send only when no newer request owns this user's quick-send slot.
/// The returned token prevents a late response from mutating a reopened popout.
fn begin_quick_send_in_flight(user_id: &str) -> Option<u64> {
    let mut in_flight = QUICK_SEND_IN_FLIGHT.lock().ok()?;
    if in_flight.contains_key(user_id) {
        return None;
    }
    let request_id = NEXT_QUICK_SEND_ID.fetch_add(1, Ordering::Relaxed);
    in_flight.insert(user_id.to_string(), request_id);
    Some(request_id)
}

fn finish_quick_send_in_flight(user_id: &str, request_id: u64) -> bool {
    let Ok(mut in_flight) = QUICK_SEND_IN_FLIGHT.lock() else {
        return false;
    };
    if in_flight.get(user_id).copied() != Some(request_id) {
        return false;
    }
    in_flight.remove(user_id);
    true
}

pub fn clear_quick_send_in_flight(user_id: &str) {
    if let Ok(mut in_flight) = QUICK_SEND_IN_FLIGHT.lock() {
        in_flight.remove(user_id);
    }
}

pub fn profile_quick_send_in_flight(user_id: &str) -> bool {
    QUICK_SEND_IN_FLIGHT
        .lock()
        .map(|in_flight| in_flight.contains_key(user_id))
        .unwrap_or(false)
}

#[allow(dead_code)]
pub fn clear_all_quick_send_in_flight() {
    if let Ok(mut in_flight) = QUICK_SEND_IN_FLIGHT.lock() {
        in_flight.clear();
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
                ActiveModal::Login | ActiveModal::LoginDiscord => {
                    crate::telemetry::ScreenKind::LoginModal
                }
                ActiveModal::AddFriendToGroup(_) | ActiveModal::CreateGroupDm => {
                    crate::telemetry::ScreenKind::FriendsHub
                }
                ActiveModal::EditGuildNickname { .. } | ActiveModal::LeaveGuild { .. } => {
                    crate::telemetry::ScreenKind::Guild {
                        id: self.selected_guild_id.clone().unwrap_or_default(),
                        name: self.get_selected_guild().map(|g| g.name.clone()).unwrap_or_default(),
                    }
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

        // Every replacement starts a new popout session. Invalidate both the
        // previous target and a stale token for a target reopened via A -> B -> A.
        if let Some(ref previous) = self.profile_popout {
            clear_quick_send_in_flight(&previous.user_id);
        }
        clear_quick_send_in_flight(&uid);

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
                match cm
                    .get_user_profile_with_guild(&target_uid, gid.as_deref())
                    .await
                {
                    Ok(profile) => {
                        cm.emit_event(
                            crate::protocol::transport::TransportEvent::UserProfileLoaded {
                                user_id: target_uid,
                                profile: Box::new(profile),
                            },
                        );
                    }
                    Err(err) => {
                        cm.emit_event(
                            crate::protocol::transport::TransportEvent::UserProfileError {
                                user_id: target_uid,
                                error: err.to_string(),
                            },
                        );
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
    pub fn set_profile_popout_data(
        &mut self,
        user_id: &str,
        data: crate::domain::profile::UserProfileData,
    ) {
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
    /// Identifies this send across close/reopen cycles of the same profile.
    pub request_id: u64,
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
    /// Opens the local DM slot used by quick-send. When identity data is not
    /// loaded, the slot stays nameless rather than fabricating a handle/topic.
    fn open_profile_quick_send_dm(&mut self, user_id: &str, username: &str) -> String {
        if !username.is_empty() {
            return self.open_dm_with_user(user_id, username);
        }
        if let Some(existing) = self.dm_channels.iter().find(|channel| {
            !channel.is_group_dm && channel.recipient_ids.iter().any(|id| id == user_id)
        }) {
            let id = existing.id.clone();
            self.select_channel(id.clone());
            return id;
        }

        let id = format!("dm-{user_id}");
        self.dm_channels.insert(
            0,
            ChannelItemState {
                id: id.clone(),
                name: String::new(),
                kind: ChannelKind::Text,
                topic: None,
                unread: false,
                mention_count: 0,
                icon_url: None,
                is_pinned: false,
                last_message_id: None,
                is_group_dm: false,
                recipient_ids: vec![user_id.to_string()],
                recipient_names: Vec::new(),
                user_limit: None,
                avatar_decoration_data: None,
                rate_limit_per_user: 0,
            },
        );
        self.select_channel(id.clone());
        id
    }

    /// Returns whether a quick message send is currently in-flight for the open popout
    pub fn is_profile_popout_quick_message_sending(&self) -> bool {
        self.profile_popout
            .as_ref()
            .map(|popout| profile_quick_send_in_flight(&popout.user_id))
            .unwrap_or(false)
    }

    /// First phase of popout quick direct message send.
    /// - Returns None if popout is absent, input is empty/whitespace, or already sending (blocks duplicate Enter).
    /// - If ClientManager is absent: records "Não conectado ao Discord." on popout.error, preserves field and returns None (ZERO MOCKS).
    /// - If ClientManager is present: registers in-flight send, resolves DM channel, and returns ProfileQuickSend.
    pub fn begin_profile_popout_quick_message(&mut self) -> Option<ProfileQuickSend> {
        let (user_id, username, content, request_id) = {
            let Some(ref mut popout) = self.profile_popout else {
                return None;
            };
            let content = popout.quick_message.trim().to_string();
            if content.is_empty() {
                return None;
            }
            let user_id = popout.user_id.clone();
            // The UI disables quick-send until a real username is loaded. Keep
            // the state transition usable for synthetic/test IDs without ever
            // turning a snowflake into a fabricated @handle; the transport
            // always addresses the real recipient by user_id.
            let username = popout
                .data
                .as_ref()
                .map(|data| data.user.username.trim().to_string())
                .filter(|username| !username.is_empty())
                .unwrap_or_default();

            if self.client_manager.is_none() {
                // ZERO MOCKS: Sem client_manager não inventa sucesso nem mensagem offline
                popout.error = Some("Não conectado ao Discord.".to_string());
                return None;
            }

            let Some(request_id) = begin_quick_send_in_flight(&user_id) else {
                // Blocks duplicate Enter while an existing request is in-flight.
                return None;
            };

            popout.error = None;
            (user_id, username, content, request_id)
        };

        let channel_id = self.open_profile_quick_send_dm(&user_id, &username);
        let cm = match self.client_manager.as_ref().cloned() {
            Some(cm) => cm,
            None => {
                finish_quick_send_in_flight(&user_id, request_id);
                return None;
            }
        };

        Some(ProfileQuickSend {
            client_manager: cm,
            channel_id,
            content,
            user_id,
            request_id,
        })
    }

    /// Second phase of popout quick direct message send.
    /// - Consumes result from ProfileQuickSend::execute.
    /// - On success: injects the API-confirmed message once, clears the draft and closes only the popout that owns this request.
    /// - On error: records the error only on the owning popout and preserves its draft.
    /// - A response from a closed/reopened profile can still confirm a real message, but cannot mutate the newer popout.
    pub fn finish_profile_popout_quick_message(
        &mut self,
        request: &ProfileQuickSend,
        result: Result<crate::domain::message::Message, String>,
    ) {
        let owns_current_popout = finish_quick_send_in_flight(&request.user_id, request.request_id)
            && self
                .profile_popout
                .as_ref()
                .map(|popout| popout.user_id == request.user_id)
                .unwrap_or(false);

        match result {
            Ok(msg) => {
                let target_cid = if msg.channel_id.is_empty() {
                    request.channel_id.clone()
                } else {
                    msg.channel_id.clone()
                };

                // The gateway may confirm the same message before this async
                // completion runs. Keep the real API snowflake as the single
                // source of truth and never inject a duplicate.
                if request.channel_id != target_cid {
                    if let Some(dm) = self
                        .dm_channels
                        .iter_mut()
                        .find(|channel| channel.id == request.channel_id)
                    {
                        dm.id = target_cid.clone();
                    }
                    if self.selected_channel_id.as_deref() == Some(&request.channel_id) {
                        self.selected_channel_id = Some(target_cid.clone());
                    }
                    if let Some(mut pending) = self.messages_by_channel.remove(&request.channel_id)
                    {
                        for message in &mut pending {
                            message.channel_id = target_cid.clone();
                        }
                        let target = self
                            .messages_by_channel
                            .entry(target_cid.clone())
                            .or_default();
                        for message in pending {
                            if !target.iter().any(|existing| existing.id == message.id) {
                                target.push(message);
                            }
                        }
                    }
                }

                let already_confirmed = !msg.id.is_empty()
                    && self
                        .messages_by_channel
                        .get(&target_cid)
                        .map(|messages| messages.iter().any(|message| message.id == msg.id))
                        .unwrap_or(false);
                if !already_confirmed {
                    self.inject_message(&target_cid, msg.clone());
                }

                if let Some(dm) = self
                    .dm_channels
                    .iter_mut()
                    .find(|channel| channel.id == target_cid || channel.id == request.channel_id)
                {
                    if !msg.id.is_empty() {
                        dm.last_message_id = Some(msg.id.clone());
                    }
                    self.sort_dm_channels();
                }

                if owns_current_popout {
                    if let Some(ref mut popout) = self.profile_popout {
                        popout.quick_message.clear();
                        popout.error = None;
                    }
                    self.close_profile_popout();
                }
            }
            Err(err_msg) => {
                if owns_current_popout {
                    if let Some(ref mut popout) = self.profile_popout {
                        popout.error = Some(err_msg);
                    }
                }
            }
        }
    }

    /// Builds fallback profile data from local friends, members, or current_user
    pub fn build_fallback_profile_data(
        &self,
        user_id: &str,
        _guild_id: Option<&str>,
    ) -> Option<crate::domain::profile::UserProfileData> {
        let to_domain_presence = |p: PresenceStatus| match p {
            PresenceStatus::Online => crate::domain::user::PresenceStatus::Online,
            PresenceStatus::Idle => crate::domain::user::PresenceStatus::Idle,
            PresenceStatus::DoNotDisturb => crate::domain::user::PresenceStatus::DoNotDisturb,
            PresenceStatus::Offline => crate::domain::user::PresenceStatus::Offline,
        };

        let uid = user_id.to_string();
        // Check current_user
        if self.current_user.id == uid || uid == "me" || uid == "@me" {
            let mut user =
                crate::domain::user::User::new(&self.current_user.id, &self.current_user.username);
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
            let recipient_id = dm
                .recipient_ids
                .first()
                .cloned()
                .unwrap_or_else(|| uid.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::{Author, Message};
    use crate::domain::profile::UserProfileData;
    use crate::domain::user::User;
    use crate::protocol::transport::TransportEvent;

    fn load_test_profile(state: &mut AppState, user_id: &str, username: &str, draft: &str) {
        state.set_profile_popout_data(
            user_id,
            UserProfileData {
                user: User::new(user_id, username),
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
                created_at: None,
                joined_server_at: None,
            },
        );
        state.profile_popout.as_mut().unwrap().quick_message = draft.to_string();
    }

    fn set_test_profile(state: &mut AppState, user_id: &str, username: &str, draft: &str) {
        state.open_user_profile(user_id, None);
        load_test_profile(state, user_id, username, draft);
    }

    #[::core::prelude::v1::test]
    fn test_late_quick_send_cannot_mutate_reopened_profile() {
        let mut state = AppState::new();
        let (client_manager, _events) = crate::protocol::ClientManager::new_mock();
        state.client_manager = Some(client_manager);

        set_test_profile(&mut state, "synth-quick-user", "synth_quick", "old draft");
        let old_request = state.begin_profile_popout_quick_message().unwrap();
        state.close_profile_popout();

        set_test_profile(&mut state, "synth-quick-user", "synth_quick", "new draft");
        let new_request = state.begin_profile_popout_quick_message().unwrap();
        assert_ne!(old_request.request_id, new_request.request_id);

        state.finish_profile_popout_quick_message(&old_request, Err("late failure".into()));
        let popout = state.profile_popout.as_ref().unwrap();
        assert_eq!(popout.quick_message, "new draft");
        assert_eq!(popout.error, None);
        assert!(state.is_profile_popout_quick_message_sending());

        state.finish_profile_popout_quick_message(&new_request, Err("current failure".into()));
        let popout = state.profile_popout.as_ref().unwrap();
        assert_eq!(popout.error.as_deref(), Some("current failure"));
        assert!(!state.is_profile_popout_quick_message_sending());
    }

    #[::core::prelude::v1::test]
    fn test_quick_send_a_b_a_switch_cannot_close_reopened_target() {
        let mut state = AppState::new();
        let (client_manager, _events) = crate::protocol::ClientManager::new_mock();
        state.client_manager = Some(client_manager);

        set_test_profile(&mut state, "synth-session-a", "synth_a", "first draft");
        let first = state.begin_profile_popout_quick_message().unwrap();
        state.open_user_profile("synth-session-b", None);
        state.open_user_profile("synth-session-a", None);
        load_test_profile(&mut state, "synth-session-a", "synth_a", "reopened draft");

        let confirmed = Message::new(
            "synth-session-confirmed",
            &first.channel_id,
            Author::new("synth-current-user", "synth_current"),
            "first draft",
        );
        state.finish_profile_popout_quick_message(&first, Ok(confirmed));
        let popout = state.profile_popout.as_ref().unwrap();
        assert_eq!(popout.user_id, "synth-session-a");
        assert_eq!(popout.quick_message, "reopened draft");
        assert_eq!(popout.error, None);
        assert_eq!(
            state
                .messages_by_channel
                .get(&first.channel_id)
                .unwrap()
                .iter()
                .filter(|message| message.id == "synth-session-confirmed")
                .count(),
            1
        );

        let second = state.begin_profile_popout_quick_message().unwrap();
        state.open_user_profile("synth-session-b", None);
        state.open_user_profile("synth-session-a", None);
        load_test_profile(&mut state, "synth-session-a", "synth_a", "newest draft");
        state.finish_profile_popout_quick_message(&second, Err("stale error".into()));
        let popout = state.profile_popout.as_ref().unwrap();
        assert_eq!(popout.user_id, "synth-session-a");
        assert_eq!(popout.quick_message, "newest draft");
        assert_eq!(popout.error, None);
    }

    #[::core::prelude::v1::test]
    fn test_quick_send_confirmation_is_injected_once_with_real_message_id() {
        let mut state = AppState::new();
        let (client_manager, _events) = crate::protocol::ClientManager::new_mock();
        state.client_manager = Some(client_manager);

        set_test_profile(
            &mut state,
            "synth-confirm-user",
            "synth_confirm",
            "confirmed once",
        );
        let request = state.begin_profile_popout_quick_message().unwrap();
        let message = Message::new(
            "synth-confirmed-message",
            &request.channel_id,
            Author::new("synth-current-user", "synth_current"),
            "confirmed once",
        );

        // Simulate the gateway/API event winning the race against completion.
        state.handle_transport_event(TransportEvent::MessageCreate(message.clone()));
        state.finish_profile_popout_quick_message(&request, Ok(message));

        let messages = state.messages_by_channel.get(&request.channel_id).unwrap();
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.id == "synth-confirmed-message")
                .count(),
            1
        );
        let dm = state
            .dm_channels
            .iter()
            .find(|channel| channel.id == request.channel_id)
            .unwrap();
        assert_eq!(
            dm.last_message_id.as_deref(),
            Some("synth-confirmed-message")
        );
        assert!(state.profile_popout.is_none());
    }

    #[::core::prelude::v1::test]
    fn test_quick_send_does_not_invent_username_before_profile_data() {
        let mut state = AppState::new();
        let (client_manager, _events) = crate::protocol::ClientManager::new_mock();
        state.client_manager = Some(client_manager);

        state.open_user_profile("synth-unloaded-user", None);
        state.profile_popout.as_mut().unwrap().quick_message = "draft".into();
        let request = state.begin_profile_popout_quick_message().unwrap();
        let dm = state
            .dm_channels
            .iter()
            .find(|channel| channel.id == request.channel_id)
            .unwrap();
        assert_eq!(dm.name, "");
        assert_eq!(dm.recipient_ids, vec!["synth-unloaded-user"]);
        assert!(state.messages_by_channel.is_empty());
        assert!(state.is_profile_popout_quick_message_sending());
        state.finish_profile_popout_quick_message(&request, Err("stopped before send".into()));
    }
}
