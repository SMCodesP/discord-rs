use super::*;
use crate::domain::voice::VoiceState;
use crate::protocol::voice_session::{VoiceControlState, VoiceSessionState};

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
        // Evita reiniciar a sessão em clique duplo ou ao reselecionar o canal.
        if self.active_voice_call.as_ref().is_some_and(|call| {
            call.channel_id == channel_id
                && !matches!(
                    call.connection_state,
                    VoiceSessionState::Disconnected { .. }
                )
        }) {
            return;
        }

        // Sincroniza a máquina de estados local com o perfil real antes de abrir
        // a chamada; o estado de mute anterior ao deafen é preservado a partir daqui.
        self.voice_control =
            VoiceControlState::new(self.current_user.is_muted, self.current_user.is_deafened);

        let mut participants = Vec::new();

        // 1. Participantes reais vindos de voice_states_by_channel
        if let Some(states) = self.voice_states_by_channel.get(channel_id) {
            for vs in states {
                participants.push(self.participant_from_voice_state(vs, channel_id));
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

    /// Constrói um participante somente a partir de estado de voz real.
    fn participant_from_voice_state(
        &self,
        state: &VoiceState,
        channel_id: &str,
    ) -> VoiceParticipantState {
        let (name, avatar_url, avatar_decoration_data) = self.resolve_participant_info(
            &state.user_id,
            state.user_name.as_deref(),
            state.avatar_url.as_deref(),
            state.avatar_decoration_data.as_ref(),
            channel_id,
        );
        let avatar_text = name.chars().next().map(|c| c.to_string()).unwrap_or_default();

        VoiceParticipantState {
            id: state.user_id.clone(),
            name,
            avatar_text,
            avatar_url,
            is_speaking: state.is_speaking,
            is_muted: state.is_muted(),
            is_deafened: state.is_deafened(),
            is_camera_on: state.self_video,
            is_screen_sharing: state.self_stream,
            avatar_decoration_data,
        }
    }

    /// Disconnects from current voice call
    pub fn disconnect_voice(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("disconnect_voice", "Desconectar Voz", 0);
        }
        let direct_session = self
            .active_voice_call
            .as_ref()
            .and_then(|call| call.voice_session.clone());
        self.active_voice_call = None;
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.disconnect_voice_session().await {
                    log::error!("[AppState] Falha ao desconectar VoiceSession: {:?}", e);
                }
            });
        } else if let Some(session) = direct_session {
            tokio::spawn(async move {
                if let Err(e) = session.disconnect().await {
                    log::error!("[AppState] Falha ao desconectar VoiceSession local: {:?}", e);
                }
            });
        }
    }

    /// Updates voice session connection state and real RTC ping
    pub fn update_voice_session_state(
        &mut self,
        session_state: VoiceSessionState,
        ping_ms: Option<u32>,
    ) {
        // A sessão é a autoridade do controle; reconcilia o espelho local após
        // rollback por falha de sinalização ou mudança vinda do Gateway.
        if let Some(session) = self.voice_session_for_control() {
            let control = session.control_state();
            self.apply_local_voice_control(control);
        }

        if let Some(ref mut call) = self.active_voice_call {
            let previous_state = call.connection_state.clone();
            match &session_state {
                VoiceSessionState::Disconnected { reason }
                    if reason != "Sessão inicializada"
                        && reason != "Desconectado pelo usuário" =>
                {
                    call.fatal_error = Some(reason.clone());
                }
                VoiceSessionState::Disconnected { .. } => {
                    call.fatal_error = None;
                }
                VoiceSessionState::Connecting
                | VoiceSessionState::Connected
                | VoiceSessionState::Reconnecting => {
                    call.fatal_error = None;
                }
            }

            // Uma reconexão só volta a exibir RTT depois de um novo ACK real.
            // O primeiro Connected ainda pode carregar uma medida já recebida.
            call.ping_ms = match (&previous_state, &session_state) {
                (VoiceSessionState::Reconnecting, VoiceSessionState::Connected) => None,
                (_, VoiceSessionState::Connected) => ping_ms,
                _ => None,
            };
            call.connection_state = session_state;
        }
    }

    /// Updates or removes a voice state across all channels
    pub fn update_voice_state(&mut self, mut state: VoiceState) {
        let user_id = state.user_id.clone();

        // Resolução de nome de exibição e avatar a partir dos caches locais.
        if state.user_name.is_none() {
            if user_id == self.current_user.id {
                state.user_name = Some(self.current_user.display_name.clone());
            } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
                state.user_name = Some(m.name.clone());
            } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
                state.user_name = Some(if f.display_name.is_empty() {
                    f.username.clone()
                } else {
                    f.display_name.clone()
                });
            }
        }
        if state.avatar_url.is_none() {
            if user_id == self.current_user.id {
                state.avatar_url = self.current_user.avatar_url.clone();
            } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
                state.avatar_url = f.avatar_url.clone();
            }
        }
        if state.avatar_decoration_data.is_none() {
            if user_id == self.current_user.id {
                state.avatar_decoration_data = self.current_user.avatar_decoration_data.clone();
            } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
                state.avatar_decoration_data = m.avatar_decoration_data.clone();
            } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
                state.avatar_decoration_data = f.avatar_decoration_data.clone();
            }
        }

        // VOICE_STATE_UPDATE não carrega Opcode 5. Preserva o indicador real
        // já recebido pelo Voice Gateway ao atualizar mute/câmera/stream.
        let previous_speaking = self
            .voice_states_by_channel
            .values()
            .flatten()
            .find(|existing| {
                existing.user_id == user_id
                    && existing.channel_id.as_deref() == state.channel_id.as_deref()
            })
            .map(|existing| existing.is_speaking)
            .unwrap_or(false);
        let participant_speaking = self
            .active_voice_call
            .as_ref()
            .filter(|call| state.channel_id.as_deref() == Some(call.channel_id.as_str()))
            .and_then(|call| call.participants.iter().find(|participant| participant.id == user_id))
            .map(|participant| participant.is_speaking)
            .unwrap_or(false);
        if !state.is_speaking {
            state.is_speaking = previous_speaking || participant_speaking;
        }

        // Remove o usuário de qualquer canal anterior antes de inseri-lo.
        for states in self.voice_states_by_channel.values_mut() {
            states.retain(|existing| existing.user_id != user_id);
        }
        if let Some(ref cid) = state.channel_id {
            let entry = self.voice_states_by_channel.entry(cid.clone()).or_default();
            if let Some(existing) = entry.iter_mut().find(|existing| existing.user_id == user_id) {
                *existing = state.clone();
            } else {
                entry.push(state.clone());
            }
        }

        let active_channel_id = self
            .active_voice_call
            .as_ref()
            .map(|call| call.channel_id.clone());
        let is_active_channel = state.channel_id.is_some()
            && active_channel_id.as_deref() == state.channel_id.as_deref();
        let resolved_participant = is_active_channel
            .then(|| {
                self.resolve_participant_info(
                    &user_id,
                    state.user_name.as_deref(),
                    state.avatar_url.as_deref(),
                    state.avatar_decoration_data.as_ref(),
                    state.channel_id.as_deref().unwrap_or_default(),
                )
            });

        // O eco do Gateway é a fonte da verdade para mute/deafen locais.
        if is_active_channel && user_id == self.current_user.id {
            let echoed_muted = state.is_muted();
            let echoed_deafened = state.is_deafened();
            self.current_user.is_muted = echoed_muted;
            self.current_user.is_deafened = echoed_deafened;
            if let Some(session) = self.voice_session_for_control() {
                session.observe_gateway_echo(echoed_muted, echoed_deafened);
                self.voice_control = session.control_state();
            } else {
                self.voice_control = VoiceControlState::new(echoed_muted, echoed_deafened);
            }
        }

        if let Some(ref mut call) = self.active_voice_call {
            if is_active_channel {
                if user_id == self.current_user.id {
                    call.is_muted = state.is_muted();
                    call.is_deafened = state.is_deafened();
                }

                if state.self_stream && call.focus_user_id.is_none() {
                    call.focus_user_id = Some(user_id.clone());
                } else if !state.self_stream
                    && call.focus_user_id.as_deref() == Some(user_id.as_str())
                {
                    call.focus_user_id = None;
                }

                if let Some(p) = call.participants.iter_mut().find(|p| p.id == user_id) {
                    if let Some((name, avatar_url, avatar_dec)) = resolved_participant {
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
                    }
                    p.is_speaking = state.is_speaking;
                    p.is_muted = state.is_muted();
                    p.is_deafened = state.is_deafened();
                    p.is_camera_on = state.self_video;
                    p.is_screen_sharing = state.self_stream;
                } else if let Some((name, avatar_url, avatar_decoration_data)) = resolved_participant {
                    let avatar_text = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
                    call.participants.push(VoiceParticipantState {
                        id: user_id.clone(),
                        name,
                        avatar_text,
                        avatar_url,
                        is_speaking: state.is_speaking,
                        is_muted: state.is_muted(),
                        is_deafened: state.is_deafened(),
                        is_camera_on: state.self_video,
                        is_screen_sharing: state.self_stream,
                        avatar_decoration_data,
                    });
                }
            } else if user_id != self.current_user.id {
                // Também remove quem mudou para outro canal, não só quem saiu.
                call.participants.retain(|p| p.id != user_id);
                if call.focus_user_id.as_deref() == Some(user_id.as_str()) {
                    call.focus_user_id = None;
                }
            }
        }
    }

    /// Sets speaking state for a participant in the active voice call
    pub fn set_participant_speaking(&mut self, user_id: &str, is_speaking: bool) {
        let channel_id = self
            .active_voice_call
            .as_ref()
            .map(|call| call.channel_id.clone());
        if let Some(channel_id) = channel_id {
            if let Some(states) = self.voice_states_by_channel.get_mut(&channel_id) {
                if let Some(state) = states.iter_mut().find(|state| state.user_id == user_id) {
                    state.is_speaking = is_speaking;
                }
            }
        }
        if let Some(ref mut call) = self.active_voice_call {
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == user_id) {
                p.is_speaking = is_speaking;
            } else if is_speaking {
                // Opcode 5 pode chegar antes do VOICE_STATE_UPDATE. Guarda o
                // participante real pelo ID até nome/avatar chegarem.
                call.participants.push(VoiceParticipantState {
                    id: user_id.to_string(),
                    name: String::new(),
                    avatar_text: String::new(),
                    avatar_url: None,
                    is_speaking: true,
                    is_muted: false,
                    is_deafened: false,
                    is_camera_on: false,
                    is_screen_sharing: false,
                    avatar_decoration_data: None,
                });
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

    /// Uma única sessão deve receber cada comando de controle. O `RootView`
    /// anexa a sessão ativa antes de o painel aceitar interação do usuário.
    fn voice_session_for_control(
        &self,
    ) -> Option<std::sync::Arc<crate::protocol::voice_session::VoiceSession>> {
        self.active_voice_call
            .as_ref()
            .and_then(|call| call.voice_session.clone())
    }

    /// Espelha o estado canônico de mute/deafen no perfil e na chamada ativa.
    fn apply_local_voice_control(&mut self, control: VoiceControlState) {
        self.voice_control = control;
        self.current_user.is_muted = control.muted;
        self.current_user.is_deafened = control.deafened;

        let current_user_id = self.current_user.id.clone();
        if let Some(ref mut call) = self.active_voice_call {
            call.is_muted = control.muted;
            call.is_deafened = control.deafened;
            if let Some(participant) = call
                .participants
                .iter_mut()
                .find(|participant| participant.id == current_user_id)
            {
                participant.is_muted = control.muted;
                participant.is_deafened = control.deafened;
            }
        }
    }

    /// Sets selected audio input device (microfone) and dynamically switches hardware stream
    pub fn set_input_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_input_device = device_id.clone();
        }
        if let Some(session) = self.voice_session_for_control() {
            tokio::spawn(async move {
                if let Err(e) = session.switch_input_device(device_id).await {
                    let warning = format!("Falha ao trocar microfone: {e:#}");
                    log::error!("[AppState] {}", warning);
                    session.set_call_warning(Some(warning));
                }
            });
        }
    }

    /// Sets selected audio output device (alto-falante/fone) and dynamically switches hardware stream
    pub fn set_output_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_output_device = device_id.clone();
        }
        if let Some(session) = self.voice_session_for_control() {
            tokio::spawn(async move {
                if let Err(e) = session.switch_output_device(device_id).await {
                    let warning = format!("Falha ao trocar saída de áudio: {e:#}");
                    log::error!("[AppState] {}", warning);
                    session.set_call_warning(Some(warning));
                }
            });
        }
    }

    /// Guarda a câmera escolhida sem afirmar que há transmissão de vídeo.
    pub fn set_camera_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_camera_device = device_id;
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

    /// Toggles self mute. Enquanto ensurdecido, o comando é recusado para não
    /// reativar o microfone (invariante deafen ⇒ mute).
    pub fn toggle_mute(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_mute", "Toggle Mute", 0);
        }

        if let Some(session) = self.voice_session_for_control() {
            let (desired, generation) = session.plan_toggle_mute();
            self.apply_local_voice_control(desired);
            if let Some(generation) = generation {
                tokio::spawn(async move {
                    if let Err(e) = session.apply_control(desired, generation).await {
                        let warning =
                            format!("Falha ao sincronizar microfone com o Discord: {e:#}");
                        log::error!("[AppState] {}", warning);
                        session.set_call_warning(Some(warning));
                    }
                });
            }
            return;
        }

        let mut control = self.voice_control;
        control.toggle_mute();
        self.apply_local_voice_control(control);
    }

    /// Toggles self deafen. Ensurdecer força o mute; desensurdecer restaura o
    /// mute que existia antes do deafen.
    pub fn toggle_deafen(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_deafen", "Toggle Deafen", 0);
        }

        if let Some(session) = self.voice_session_for_control() {
            let (desired, generation) = session.plan_toggle_deafen();
            self.apply_local_voice_control(desired);
            if let Some(generation) = generation {
                tokio::spawn(async move {
                    if let Err(e) = session.apply_control(desired, generation).await {
                        let warning =
                            format!("Falha ao sincronizar ensurdecimento com o Discord: {e:#}");
                        log::error!("[AppState] {}", warning);
                        session.set_call_warning(Some(warning));
                    }
                });
            }
            return;
        }

        let mut control = self.voice_control;
        control.toggle_deafen();
        self.apply_local_voice_control(control);
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
                self.current_user.avatar_decoration_data.clone(),
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
                if resolved_avatar.is_none() && !dm.is_group_dm {
                    resolved_avatar = dm.icon_url.clone();
                }
                if resolved_dec.is_none() && idx == 0 {
                    resolved_dec = dm.avatar_decoration_data.clone();
                }
            } else if !dm.is_group_dm
                && dm.recipient_ids.is_empty()
                && resolved_name.is_none()
                && !dm.name.is_empty()
            {
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
                let recipient_idx = dm.recipient_ids.iter().position(|r| r == user_id);
                if resolved_name.is_none() {
                    if let Some(idx) = recipient_idx {
                        if let Some(name) = dm.recipient_names.get(idx) {
                            resolved_name = Some(name.clone());
                        } else if !dm.name.is_empty() && !dm.is_group_dm {
                            resolved_name = Some(dm.name.clone());
                        }
                    }
                }
                if resolved_avatar.is_none() && !dm.is_group_dm {
                    resolved_avatar = dm.icon_url.clone();
                }
                if resolved_dec.is_none() && recipient_idx == Some(0) {
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
            rate_limit_per_user: 0,
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
            rate_limit_per_user: 0,
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

    #[test]
    fn test_voice_state_move_to_another_channel_removes_participant() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        state.update_voice_state(
            VoiceState::new("100000000000000002", "session-2")
                .with_channel_id("200000000000000001")
                .with_user_name("Participante sintético"),
        );
        assert!(state.active_voice_call.as_ref().unwrap().participants.iter().any(|p| {
            p.id == "100000000000000002"
        }));

        state.update_voice_state(
            VoiceState::new("100000000000000002", "session-2")
                .with_channel_id("200000000000000002")
                .with_user_name("Participante sintético"),
        );
        assert!(!state.active_voice_call.as_ref().unwrap().participants.iter().any(|p| {
            p.id == "100000000000000002"
        }));
    }

    #[test]
    fn test_speaking_before_voice_state_is_not_lost() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        state.set_participant_speaking("100000000000000002", true);
        state.update_voice_state(
            VoiceState::new("100000000000000002", "session-2")
                .with_channel_id("200000000000000001")
                .with_user_name("Participante sintético"),
        );

        let call = state.active_voice_call.as_ref().unwrap();
        let participant = call.participants.iter().find(|p| p.id == "100000000000000002").unwrap();
        assert!(participant.is_speaking);
    }

    #[test]
    fn test_voice_state_stream_and_mute_are_reflected() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        let mut remote = VoiceState::new("100000000000000002", "session-2")
            .with_channel_id("200000000000000001")
            .with_user_name("Participante sintético")
            .with_self_stream(true)
            .with_mute(true);
        remote.self_video = true;
        state.update_voice_state(remote);

        let call = state.active_voice_call.as_ref().unwrap();
        let participant = call.participants.iter().find(|p| p.id == "100000000000000002").unwrap();
        assert!(participant.is_screen_sharing);
        assert!(participant.is_camera_on);
        assert!(participant.is_muted);
        assert_eq!(call.focus_user_id.as_deref(), Some("100000000000000002"));
    }

    #[test]
    fn test_reconnect_requires_new_heartbeat_ack_and_clears_fatal_error() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        state.update_voice_session_state(VoiceSessionState::Connected, Some(35));
        assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, Some(35));
        state.update_voice_session_state(VoiceSessionState::Reconnecting, Some(35));
        assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, None);
        state.update_voice_session_state(VoiceSessionState::Connected, Some(35));
        assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, None);
        state.update_voice_session_state(VoiceSessionState::Connected, Some(42));
        assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, Some(42));

        state.set_call_fatal_error("erro sintético".into());
        state.update_voice_session_state(VoiceSessionState::Connected, Some(42));
        assert_eq!(state.active_voice_call.as_ref().unwrap().fatal_error, None);
    }

    #[test]
    fn test_current_user_avatar_decoration_is_resolved() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.current_user.avatar_decoration_data =
            Some(crate::domain::user::AvatarDecorationData::new("synthetic_decoration"));

        let (_, _, decoration) = state.resolve_participant_info(
            &state.current_user.id,
            None,
            None,
            None,
            "200000000000000001",
        );
        assert_eq!(decoration, state.current_user.avatar_decoration_data);
    }

    #[test]
    fn test_gateway_echo_syncs_current_user_mute_and_deafen() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        let mut own_state = VoiceState::new("100000000000000001", "session-local")
            .with_channel_id("200000000000000001")
            .with_user_name("Usuário sintético");
        own_state.self_mute = true;
        own_state.self_deaf = true;
        state.update_voice_state(own_state);

        assert!(state.current_user.is_muted);
        assert!(state.current_user.is_deafened);
        let call = state.active_voice_call.as_ref().unwrap();
        assert!(call.is_muted);
        assert!(call.is_deafened);
        let participant = call
            .participants
            .iter()
            .find(|participant| participant.id == state.current_user.id)
            .unwrap();
        assert!(participant.is_muted);
        assert!(participant.is_deafened);
    }

    #[test]
    fn test_camera_selection_does_not_fake_broadcast_state() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");

        state.set_camera_device(Some("/dev/video-synthetic".into()));
        assert_eq!(
            state.active_voice_call.as_ref().unwrap().selected_camera_device.as_deref(),
            Some("/dev/video-synthetic")
        );
        assert!(!state.active_voice_call.as_ref().unwrap().is_camera_on);
        state.toggle_camera();
        assert!(!state.active_voice_call.as_ref().unwrap().is_camera_on);
    }
}

