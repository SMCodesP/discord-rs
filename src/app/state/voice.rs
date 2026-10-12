use super::*;
use crate::domain::voice::VoiceState;
use crate::protocol::voice_session::VoiceSessionState;

impl AppState {
    /// Connects to a voice call in a guild channel
    pub fn connect_voice(&mut self, channel_id: &str, channel_name: &str, guild_name: &str) {
        let gid_opt = self.selected_guild_id.as_deref().and_then(|g| g.parse::<u64>().ok());
        self.connect_voice_internal(channel_id, channel_name, guild_name, gid_opt, None);
    }

    /// Connects to a voice call in a DM channel with explicit guild_id = None
    pub fn connect_voice_dm(&mut self, channel_id: &str, channel_name: &str) {
        self.connect_voice_internal(channel_id, channel_name, "Mensagem Direta", None, None);
    }

    /// Connects to a voice call in a DM channel with explicit guild_id = None and optional ring_recipients
    pub fn connect_voice_dm_with_ring(
        &mut self,
        channel_id: &str,
        channel_name: &str,
        ring_recipients: Option<Vec<String>>,
    ) {
        self.connect_voice_internal(channel_id, channel_name, "Mensagem Direta", None, ring_recipients);
    }

    fn connect_voice_internal(
        &mut self,
        channel_id: &str,
        channel_name: &str,
        guild_name: &str,
        explicit_guild_id: Option<u64>,
        ring_recipients: Option<Vec<String>>,
    ) {
        let mut participants = Vec::new();

        // 1. Participantes reais vindos de voice_states_by_channel
        if let Some(states) = self.voice_states_by_channel.get(channel_id) {
            for vs in states {
                let (name, avatar_url, avatar_decoration_data) = self.resolve_participant_info(
                    &vs.user_id,
                    vs.user_name.as_deref(),
                    vs.avatar_url.as_deref(),
                    vs.avatar_decoration_data.as_ref(),
                    channel_id,
                );
                let avatar_text = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
                participants.push(VoiceParticipantState {
                    id: vs.user_id.clone(),
                    name,
                    avatar_text,
                    avatar_url,
                    is_speaking: false,
                    is_muted: vs.self_mute || vs.mute,
                    is_deafened: vs.self_deaf || vs.deaf,
                    is_camera_on: vs.self_video,
                    is_screen_sharing: false,
                    avatar_decoration_data,
                });
            }
        }

        // 2. Se o usuário atual ainda não constar na lista do canal, inclui-o inicialmente
        if !participants.iter().any(|p| p.id == self.current_user.id) {
            participants.push(VoiceParticipantState {
                id: self.current_user.id.clone(),
                name: self.current_user.display_name.clone(),
                avatar_text: self.current_user.avatar_text.clone(),
                avatar_url: self.current_user.avatar_url.clone(),
                is_speaking: false,
                is_muted: self.current_user.is_muted,
                is_deafened: self.current_user.is_deafened,
                is_camera_on: false,
                is_screen_sharing: false,
                avatar_decoration_data: self.current_user.avatar_decoration_data.clone(),
            });
        }

        // Validação estrita de Snowflake numérico (Zero IDs inventados / fallback)
        let channel_id_u64 = match channel_id.parse::<u64>() {
            Ok(id) => id,
            Err(e) => {
                let err_msg = format!("ID do canal inválido: deve ser numérico (Snowflake): {}", e);
                log::error!("[AppState] {}", err_msg);
                self.active_voice_call = Some(ActiveVoiceCall {
                    channel_id: channel_id.to_string(),
                    channel_name: channel_name.to_string(),
                    guild_name: guild_name.to_string(),
                    ping_ms: None,
                    is_muted: self.current_user.is_muted,
                    is_deafened: self.current_user.is_deafened,
                    is_screen_sharing: false,
                    participants,
                    connection_state: VoiceSessionState::Disconnected {
                        reason: err_msg.clone(),
                    },
                    fatal_error: Some(err_msg),
                    is_camera_on: false,
                    focus_user_id: None,
                    selected_input_device: None,
                    selected_output_device: None,
                    selected_camera_device: None,
                    voice_session: None,
                });
                return;
            }
        };

        let user_id_u64 = match self.current_user.id.parse::<u64>() {
            Ok(id) => id,
            Err(e) => {
                let err_msg = format!("ID do usuário inválido: deve ser numérico (Snowflake): {}", e);
                log::error!("[AppState] {}", err_msg);
                self.active_voice_call = Some(ActiveVoiceCall {
                    channel_id: channel_id.to_string(),
                    channel_name: channel_name.to_string(),
                    guild_name: guild_name.to_string(),
                    ping_ms: None,
                    is_muted: self.current_user.is_muted,
                    is_deafened: self.current_user.is_deafened,
                    is_screen_sharing: false,
                    participants,
                    connection_state: VoiceSessionState::Disconnected {
                        reason: err_msg.clone(),
                    },
                    fatal_error: Some(err_msg),
                    is_camera_on: false,
                    focus_user_id: None,
                    selected_input_device: None,
                    selected_output_device: None,
                    selected_camera_device: None,
                    voice_session: None,
                });
                return;
            }
        };

        // 3. Cria ActiveVoiceCall com ping_ms: None (ZERO MOCKS: latência só com medida real)
        self.active_voice_call = Some(ActiveVoiceCall {
            channel_id: channel_id.to_string(),
            channel_name: channel_name.to_string(),
            guild_name: guild_name.to_string(),
            ping_ms: None,
            is_muted: self.current_user.is_muted,
            is_deafened: self.current_user.is_deafened,
            is_screen_sharing: false,
            participants,
            connection_state: VoiceSessionState::Connecting,
            fatal_error: None,
            is_camera_on: false,
            focus_user_id: None,
            selected_input_device: None,
            selected_output_device: None,
            selected_camera_device: None,
            voice_session: None,
        });

        // 4. Inicia a VoiceSession de verdade através do client_manager (se conectado)
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let self_mute = self.current_user.is_muted;
            let self_deaf = self.current_user.is_deafened;
            let ring_recips = ring_recipients;

            tokio::spawn(async move {
                if let Err(e) = cm
                    .start_voice_session(
                        explicit_guild_id,
                        channel_id_u64,
                        user_id_u64,
                        self_mute,
                        self_deaf,
                        ring_recips,
                    )
                    .await
                {
                    log::error!("[AppState] Falha ao iniciar VoiceSession: {:?}", e);
                }
            });
        }
    }

    /// Disconnects from current voice call
    pub fn disconnect_voice(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("disconnect_voice", "Desconectar Voz", 0);
        }
        self.active_voice_call = None;
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                let _ = cm.disconnect_voice_session().await;
            });
        }
    }

    /// Updates voice session connection state and real RTC ping
    pub fn update_voice_session_state(
        &mut self,
        session_state: VoiceSessionState,
        ping_ms: Option<u32>,
    ) {
        if let Some(ref mut call) = self.active_voice_call {
            if let VoiceSessionState::Disconnected { ref reason } = session_state {
                if reason != "Sessão inicializada" && reason != "Desconectado pelo usuário" {
                    call.fatal_error = Some(reason.clone());
                }
            }
            call.connection_state = session_state;
            call.ping_ms = ping_ms;
        }
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
                *existing = state.clone();
            } else {
                entry.push(state.clone());
            }
        }

        // Sincroniza participantes da chamada de voz ativa se corresponder ao canal
        if let Some(ref call) = self.active_voice_call {
            let is_same_channel = state.channel_id.as_deref() == Some(&call.channel_id);
            let is_user_left = state.channel_id.is_none() && user_id != self.current_user.id;
            let active_channel_id = call.channel_id.clone();

            if is_same_channel {
                let (name, avatar_url, avatar_dec) = self.resolve_participant_info(
                    &user_id,
                    state.user_name.as_deref(),
                    state.avatar_url.as_deref(),
                    state.avatar_decoration_data.as_ref(),
                    &active_channel_id,
                );

                if let Some(ref mut call) = self.active_voice_call {
                    if let Some(p) = call.participants.iter_mut().find(|p| p.id == user_id) {
                        if !name.is_empty() {
                            p.name = name;
                            p.avatar_text = p.name.chars().next().map(|c| c.to_string()).unwrap_or_default();
                        }
                        if avatar_url.is_some() {
                            p.avatar_url = avatar_url;
                        }
                        if avatar_dec.is_some() {
                            p.avatar_decoration_data = avatar_dec;
                        }
                        p.is_muted = state.self_mute || state.mute;
                        p.is_deafened = state.self_deaf || state.deaf;
                        p.is_camera_on = state.self_video;
                    } else {
                        let avatar_text = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
                        call.participants.push(VoiceParticipantState {
                            id: user_id.clone(),
                            name,
                            avatar_text,
                            avatar_url,
                            is_speaking: false,
                            is_muted: state.self_mute || state.mute,
                            is_deafened: state.self_deaf || state.deaf,
                            is_camera_on: state.self_video,
                            is_screen_sharing: false,
                            avatar_decoration_data: avatar_dec,
                        });
                    }
                }
            } else if is_user_left {
                if let Some(ref mut call) = self.active_voice_call {
                    call.participants.retain(|p| p.id != user_id);
                }
            }
        }
    }

    /// Sets speaking state for a participant in the active voice call
    pub fn set_participant_speaking(&mut self, user_id: &str, is_speaking: bool) {
        if let Some(ref mut call) = self.active_voice_call {
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == user_id) {
                p.is_speaking = is_speaking;
            }
        }
    }

    /// Sets the active VoiceSession on the active voice call
    pub fn set_voice_session(&mut self, session: std::sync::Arc<crate::protocol::voice_session::VoiceSession>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.voice_session = Some(session);
        }
    }

    /// Records a fatal error on the active call and marks connection state as Disconnected
    pub fn set_call_fatal_error(&mut self, error_msg: String) {
        if let Some(ref mut call) = self.active_voice_call {
            call.fatal_error = Some(error_msg.clone());
            call.connection_state = VoiceSessionState::Disconnected { reason: error_msg };
        }
    }

    /// Sets or clears focus on a specific participant (modo foco para compartilhamento de tela)
    pub fn set_focus_user(&mut self, user_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.focus_user_id = user_id;
        }
    }

    /// Sets selected audio input device (microfone) and dynamically switches hardware stream
    pub fn set_input_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_input_device = device_id.clone();
            if let Some(ref session) = call.voice_session {
                let session = session.clone();
                let dev_id = device_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = session.switch_input_device(dev_id).await {
                        log::error!("[AppState] Falha ao trocar dispositivo de entrada: {:?}", e);
                    }
                });
            }
        }
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let dev_id = device_id.clone();
            tokio::spawn(async move {
                if let Some(session) = cm.active_voice_session().await {
                    let _ = session.switch_input_device(dev_id).await;
                }
            });
        }
    }

    /// Sets selected audio output device (alto-falante/fone) and dynamically switches hardware stream
    pub fn set_output_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_output_device = device_id.clone();
            if let Some(ref session) = call.voice_session {
                let session = session.clone();
                let dev_id = device_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = session.switch_output_device(dev_id).await {
                        log::error!("[AppState] Falha ao trocar dispositivo de saída: {:?}", e);
                    }
                });
            }
        }
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let dev_id = device_id.clone();
            tokio::spawn(async move {
                if let Some(session) = cm.active_voice_session().await {
                    let _ = session.switch_output_device(dev_id).await;
                }
            });
        }
    }

    /// Toggles local camera broadcasting state
    pub fn toggle_camera(&mut self) {
        // Transmissão de vídeo ainda não suportada (PATCH PENDENTE); não altera a flag para não fingir estado.
        log::warn!("[AppState] toggle_camera acionado, mas transmissão de vídeo não está suportada nesta versão.");
    }

    /// Toggles local screen sharing state
    pub fn toggle_screen_share(&mut self) {
        // Compartilhamento de tela ainda não suportado (PATCH PENDENTE); não altera a flag para não fingir estado.
        log::warn!("[AppState] toggle_screen_share acionado, mas compartilhamento de tela não está suportado nesta versão.");
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
        let is_muted = self.current_user.is_muted;
        if let Some(ref mut call) = self.active_voice_call {
            call.is_muted = is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_muted = is_muted;
            }
            if let Some(ref session) = call.voice_session {
                let session = session.clone();
                tokio::spawn(async move {
                    let _ = session.set_muted(is_muted).await;
                });
            }
        }
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                if let Some(session) = cm.active_voice_session().await {
                    let _ = session.set_muted(is_muted).await;
                }
            });
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
        let is_deafened = self.current_user.is_deafened;
        let is_muted = self.current_user.is_muted;
        if let Some(ref mut call) = self.active_voice_call {
            call.is_deafened = is_deafened;
            call.is_muted = is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_deafened = is_deafened;
                p.is_muted = is_muted;
            }
            if let Some(ref session) = call.voice_session {
                let session = session.clone();
                tokio::spawn(async move {
                    let _ = session.set_deafened(is_deafened).await;
                });
            }
        }
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                if let Some(session) = cm.active_voice_session().await {
                    let _ = session.set_deafened(is_deafened).await;
                }
            });
        }
    }

    /// Resolve o nome, avatar_url e decoração de um participante de chamada.
    /// Em DMs, consulta `dm_channels`, `friends`, `members`, etc.
    /// Se não houver nenhum dado real, retorna nome vazio "" (Zero mocks: sem palavra inventada "Usuário").
    pub fn resolve_participant_info(
        &self,
        user_id: &str,
        fallback_name: Option<&str>,
        fallback_avatar: Option<&str>,
        fallback_decoration: Option<&crate::domain::user::AvatarDecorationData>,
        channel_id: &str,
    ) -> (String, Option<String>, Option<crate::domain::user::AvatarDecorationData>) {
        // 1. Se for o usuário atual
        if user_id == self.current_user.id {
            return (
                self.current_user.display_name.clone(),
                self.current_user.avatar_url.clone(),
                None,
            );
        }

        // 2. Se o fallback de nome fornecido for válido e diferente de "Usuário"
        let mut resolved_name = fallback_name
            .filter(|n| !n.trim().is_empty() && *n != "Usuário")
            .map(|s| s.to_string());
        let mut resolved_avatar = fallback_avatar
            .filter(|a| !a.trim().is_empty())
            .map(|s| s.to_string());
        let mut resolved_dec = fallback_decoration.cloned();

        // 3. Procura no canal de DM específico (se for DM)
        if let Some(dm) = self.dm_channels.iter().find(|d| d.id == channel_id) {
            if let Some(idx) = dm.recipient_ids.iter().position(|r| r == user_id) {
                if resolved_name.is_none() {
                    if let Some(name) = dm.recipient_names.get(idx) {
                        resolved_name = Some(name.clone());
                    } else if !dm.name.is_empty() && !dm.is_group_dm {
                        resolved_name = Some(dm.name.clone());
                    }
                }
                if resolved_avatar.is_none() {
                    resolved_avatar = dm.icon_url.clone();
                }
                if resolved_dec.is_none() {
                    resolved_dec = dm.avatar_decoration_data.clone();
                }
            } else if !dm.is_group_dm && resolved_name.is_none() && !dm.name.is_empty() {
                // Em DM individual direta, o nome e ícone do canal correspondem ao outro participante
                resolved_name = Some(dm.name.clone());
                if resolved_avatar.is_none() {
                    resolved_avatar = dm.icon_url.clone();
                }
                if resolved_dec.is_none() {
                    resolved_dec = dm.avatar_decoration_data.clone();
                }
            }
        }

        // 4. Procura em qualquer outro canal de DM
        if resolved_name.is_none() || resolved_avatar.is_none() {
            if let Some(dm) = self.dm_channels.iter().find(|d| d.recipient_ids.contains(&user_id.to_string())) {
                if resolved_name.is_none() {
                    if let Some(idx) = dm.recipient_ids.iter().position(|r| r == user_id) {
                        if let Some(name) = dm.recipient_names.get(idx) {
                            resolved_name = Some(name.clone());
                        } else if !dm.name.is_empty() && !dm.is_group_dm {
                            resolved_name = Some(dm.name.clone());
                        }
                    }
                }
                if resolved_avatar.is_none() {
                    resolved_avatar = dm.icon_url.clone();
                }
                if resolved_dec.is_none() {
                    resolved_dec = dm.avatar_decoration_data.clone();
                }
            }
        }

        // 5. Procura na lista de amigos (`friends`)
        if let Some(friend) = self.friends.iter().find(|f| f.id == user_id) {
            if resolved_name.is_none() {
                let name = if !friend.display_name.is_empty() {
                    friend.display_name.clone()
                } else {
                    friend.username.clone()
                };
                resolved_name = Some(name);
            }
            if resolved_avatar.is_none() {
                resolved_avatar = friend.avatar_url.clone();
            }
            if resolved_dec.is_none() {
                resolved_dec = friend.avatar_decoration_data.clone();
            }
        }

        // 6. Procura nos membros (`members`)
        if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
            if resolved_name.is_none() {
                resolved_name = Some(m.name.clone());
            }
            if resolved_dec.is_none() {
                resolved_dec = m.avatar_decoration_data.clone();
            }
        }

        let name = resolved_name.unwrap_or_default();
        (name, resolved_avatar, resolved_dec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::models::{ChannelItemState, ChannelKind, FriendItemState, PresenceStatus};

    #[test]
    fn test_resolve_participant_info_from_dm_channel() {
        let mut state = AppState::default();
        let dm = ChannelItemState {
            id: "dm_123".into(),
            name: "amigo_legal".into(),
            kind: ChannelKind::Text,
            topic: None,
            unread: false,
            mention_count: 0,
            icon_url: Some("https://cdn.discordapp.com/avatars/u1/hash.png".into()),
            is_pinned: false,
            last_message_id: None,
            is_group_dm: false,
            recipient_ids: vec!["u1".into()],
            recipient_names: vec!["Amigo Legal".into()],
            user_limit: None,
            avatar_decoration_data: None,
        };
        state.dm_channels.push(dm);

        let (name, avatar, dec) = state.resolve_participant_info("u1", None, None, None, "dm_123");
        assert_eq!(name, "Amigo Legal");
        assert_eq!(avatar, Some("https://cdn.discordapp.com/avatars/u1/hash.png".into()));
        assert!(dec.is_none());
    }

    #[test]
    fn test_resolve_participant_info_from_friends() {
        let mut state = AppState::default();
        let friend = FriendItemState {
            id: "u2".into(),
            username: "friend_user".into(),
            display_name: "Melhor Amigo".into(),
            avatar_url: Some("https://cdn.discordapp.com/avatars/u2/avatar.png".into()),
            avatar_text: "M".into(),
            presence: PresenceStatus::Online,
            custom_status: None,
            relationship_type: 1,
            avatar_decoration_data: None,
        };
        state.friends.push(friend);

        let (name, avatar, _) = state.resolve_participant_info("u2", None, None, None, "other_channel");
        assert_eq!(name, "Melhor Amigo");
        assert_eq!(avatar, Some("https://cdn.discordapp.com/avatars/u2/avatar.png".into()));
    }

    #[test]
    fn test_resolve_participant_info_empty_when_no_data_never_mock_usuario() {
        let state = AppState::default();
        let (name, avatar, dec) = state.resolve_participant_info("unknown_u999", None, None, None, "unknown_channel");
        // Zero mocks: sem dado real, retorna string vazia, NUNCA "Usuário"
        assert_eq!(name, "");
        assert_ne!(name, "Usuário");
        assert!(avatar.is_none());
        assert!(dec.is_none());
    }
}

