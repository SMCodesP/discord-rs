use super::*;
use crate::domain::voice::VoiceState;

impl AppState {
    /// Connects to a voice call
    pub fn connect_voice(&mut self, channel_id: &str, channel_name: &str, guild_name: &str) {
        self.active_voice_call = Some(ActiveVoiceCall {
            channel_id: channel_id.to_string(),
            channel_name: channel_name.to_string(),
            guild_name: guild_name.to_string(),
            ping_ms: 18,
            is_muted: self.current_user.is_muted,
            is_deafened: self.current_user.is_deafened,
            is_screen_sharing: false,
            participants: vec![
                VoiceParticipantState {
                    id: self.current_user.id.clone(),
                    name: self.current_user.display_name.clone(),
                    avatar_text: self.current_user.avatar_text.clone(),
                    is_speaking: false,
                    is_muted: self.current_user.is_muted,
                    is_deafened: self.current_user.is_deafened,
                },
            ],
        });
    }

    /// Disconnects from current voice call
    pub fn disconnect_voice(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("disconnect_voice", "Desconectar Voz", 0);
        }
        self.active_voice_call = None;
    }

    /// Updates or removes a voice state across all channels
    pub fn update_voice_state(&mut self, mut state: VoiceState) {
        let user_id = state.user_id.clone();
        // Resolução de nome de exibição e avatar a partir dos caches locais se não vierem no voice state
        if state.user_name.is_none() {
            if user_id == self.current_user.id {
                state.user_name = Some(self.current_user.display_name.clone());
                if state.avatar_url.is_none() {
                    state.avatar_url = self.current_user.avatar_url.clone();
                }
            } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
                state.user_name = Some(m.name.clone());
            } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
                state.user_name = Some(f.display_name.clone());
                if state.avatar_url.is_none() {
                    state.avatar_url = f.avatar_url.clone();
                }
            }
        }

        // Remove the user from any channel they were previously in
        for states in self.voice_states_by_channel.values_mut() {
            states.retain(|s| s.user_id != user_id);
        }
        // If channel_id == Some(cid), add or update in cid
        if let Some(ref cid) = state.channel_id {
            let entry = self.voice_states_by_channel.entry(cid.clone()).or_default();
            if let Some(existing) = entry.iter_mut().find(|s| s.user_id == user_id) {
                *existing = state;
            } else {
                entry.push(state);
            }
        }
    }

    /// Returns the slice of active voice states connected to a specific channel
    pub fn voice_states_for_channel(&self, channel_id: &str) -> &[VoiceState] {
        self.voice_states_by_channel
            .get(channel_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Populates multiple voice states from a guild initial snapshot
    pub fn populate_guild_voice_states(&mut self, states: Vec<VoiceState>) {
        for state in states {
            self.update_voice_state(state);
        }
    }

    /// Toggles self mute
    pub fn toggle_mute(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_mute", "Toggle Mute", 0);
        }
        self.current_user.is_muted = !self.current_user.is_muted;
        if let Some(ref mut call) = self.active_voice_call {
            call.is_muted = self.current_user.is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_muted = self.current_user.is_muted;
            }
        }
    }

    /// Toggles self deafen
    pub fn toggle_deafen(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_deafen", "Toggle Deafen", 0);
        }
        self.current_user.is_deafened = !self.current_user.is_deafened;
        if self.current_user.is_deafened {
            self.current_user.is_muted = true;
        }
        if let Some(ref mut call) = self.active_voice_call {
            call.is_deafened = self.current_user.is_deafened;
            call.is_muted = self.current_user.is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_deafened = self.current_user.is_deafened;
                p.is_muted = self.current_user.is_muted;
            }
        }
    }
}
