use super::*;
use crate::domain::voice::VoiceState;
use crate::protocol::voice_session::VoiceSessionState;

impl AppState {
    /// Connects to a voice call in a guild channel
    pub fn connect_voice(&mut self, channel_id: &str, channel_name: &str, guild_name: &str) {
        let gid_opt = self.selected_guild_id.as_deref().and_then(|g| g.parse::<u64>().ok());
        self.connect_voice_internal(channel_id, channel_name, guild_name, gid_opt);
    }

    /// Connects to a voice call in a DM channel with explicit guild_id = None
    pub fn connect_voice_dm(&mut self, channel_id: &str, channel_name: &str) {
        self.connect_voice_internal(channel_id, channel_name, "Mensagem Direta", None);
    }

    fn connect_voice_internal(
        &mut self,
        channel_id: &str,
        channel_name: &str,
        guild_name: &str,
        explicit_guild_id: Option<u64>,
    ) {
        let mut participants = Vec::new();

        // 1. Participantes reais vindos de voice_states_by_channel
        if let Some(states) = self.voice_states_by_channel.get(channel_id) {
            for vs in states {
                let name = vs.user_name.clone().unwrap_or_else(|| {
                    if vs.user_id == self.current_user.id {
                        self.current_user.display_name.clone()
                    } else if let Some(m) = self.members.iter().find(|m| m.id == vs.user_id) {
                        m.name.clone()
                    } else if let Some(f) = self.friends.iter().find(|f| f.id == vs.user_id) {
                        f.display_name.clone()
                    } else {
                        "Usuário".into()
                    }
                });
                let avatar_text = name.chars().next().unwrap_or('?').to_string();
                participants.push(VoiceParticipantState {
                    id: vs.user_id.clone(),
                    name,
                    avatar_text,
                    avatar_url: vs.avatar_url.clone(),
                    is_speaking: false,
                    is_muted: vs.self_mute || vs.mute,
                    is_deafened: vs.self_deaf || vs.deaf,
                    is_camera_on: vs.self_video,
                    is_screen_sharing: false,
                    avatar_decoration_data: vs.avatar_decoration_data.clone(),
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

            tokio::spawn(async move {
                if let Err(e) = cm
                    .start_voice_session(
                        explicit_guild_id,
                        channel_id_u64,
                        user_id_u64,
                        self_mute,
                        self_deaf,
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
        if let Some(ref mut call) = self.active_voice_call {
            if let Some(ref cid) = state.channel_id {
                if cid == &call.channel_id {
                    if let Some(p) = call.participants.iter_mut().find(|p| p.id == user_id) {
                        if let Some(ref un) = state.user_name {
                            p.name = un.clone();
                        }
                        if let Some(ref au) = state.avatar_url {
                            p.avatar_url = Some(au.clone());
                        }
                        p.is_muted = state.self_mute || state.mute;
                        p.is_deafened = state.self_deaf || state.deaf;
                        p.is_camera_on = state.self_video;
                    } else {
                        let name = state.user_name.clone().unwrap_or_else(|| "Usuário".into());
                        let avatar_text = name.chars().next().unwrap_or('?').to_string();
                        call.participants.push(VoiceParticipantState {
                            id: user_id.clone(),
                            name,
                            avatar_text,
                            avatar_url: state.avatar_url.clone(),
                            is_speaking: false,
                            is_muted: state.self_mute || state.mute,
                            is_deafened: state.self_deaf || state.deaf,
                            is_camera_on: state.self_video,
                            is_screen_sharing: false,
                            avatar_decoration_data: state.avatar_decoration_data.clone(),
                        });
                    }
                }
            } else if user_id != self.current_user.id {
                call.participants.retain(|p| p.id != user_id);
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
}
