use super::*;

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

    /// Opens the user profile popout for a user (with current guild context)
    pub fn open_user_profile(&mut self, user_id: &str) {
        let current_guild = self.selected_guild_id.clone();
        self.open_user_profile_in_guild(user_id, current_guild);
    }

    /// Opens the user profile popout with specific guild context
    pub fn open_user_profile_in_guild(&mut self, user_id: &str, guild_id: Option<String>) {
        let uid = user_id.to_string();

        // Guard against duplicate concurrent fetch if already loading this user
        if let Some(ref popout) = self.profile_popout {
            if popout.user_id == uid && popout.is_loading {
                return;
            }
        }

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

    /// Closes the active user profile popout
    pub fn close_profile_popout(&mut self) {
        self.profile_popout = None;
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

    /// Sends a quick message from the user profile popout input and closes the popout
    pub fn send_profile_popout_quick_message(&mut self) {
        let (user_id, username, content) = {
            let Some(ref popout) = self.profile_popout else {
                return;
            };
            let content = popout.quick_message.trim().to_string();
            if content.is_empty() {
                return;
            }
            let user_id = popout.user_id.clone();
            let username = popout
                .data
                .as_ref()
                .map(|d| d.user.username.clone())
                .unwrap_or_else(|| user_id.clone());
            (user_id, username, content)
        };

        let channel_id = self.open_dm_with_user(&user_id, &username);
        self.send_message(&channel_id, &content);
        self.close_profile_popout();
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
