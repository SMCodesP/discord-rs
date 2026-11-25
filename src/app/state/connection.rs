use super::*;
use crate::protocol::AuthToken;
use crate::protocol::ConnectionMode;
use std::collections::HashMap;

impl AppState {
    /// Authenticates with a Discord token and updates connection status
    pub fn authenticate(&mut self, token: String, is_user: bool) {
        let trimmed = token.trim().to_string();
        if trimmed.is_empty() {
            self.auth_error = Some("O token não pode estar vazio.".to_string());
            self.is_connecting = false;
            return;
        }

        self.auth_token = Some(trimmed.clone());
        self.token_input = trimmed.clone();
        self.is_user_token = is_user;
        self.auth_error = None;

        self.is_connecting = true;
        self.is_connected_to_gateway = false;
        self.connection_status = "Conectando ao Gateway v10...".to_string();

        // O token só é persistido depois de READY, em gateway_events. Assim,
        // credenciais inválidas/expiradas/2FA nunca substituem a conta salva.
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let auth = if is_user {
                AuthToken::user(trimmed)
            } else {
                AuthToken::bot(trimmed)
            };
            tokio::spawn(async move {
                if let Err(e) = cm.connect_with_token(auth).await {
                    log::error!("Falha ao conectar via ClientManager: {:#}", e);
                }
            });
        } else {
            self.is_connecting = false;
            self.auth_error = Some("Gerenciador de conexão não inicializado.".to_string());
            self.connection_status = "Desconectado".to_string();
        }
    }

    /// Disconnects from live gateway
    pub fn disconnect_gateway(&mut self) {
        self.is_connected_to_gateway = false;
        self.auth_token = None;
        self.connection_mode = ConnectionMode::Disconnected("Desconectado".to_string());
        self.connection_status = "Desconectado".to_string();
        self.messages_by_channel = HashMap::new();
        self.dm_channels = Vec::new();

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                let _ = cm.disconnect().await;
            });
        }
    }
}

impl AppState {
    /// Opens the user-panel status picker with the last confirmed state.
    pub fn open_status_picker(&mut self) {
        if self.current_user.id.is_empty() {
            return;
        }
        self.status_picker_open = true;
        self.status_picker_status = self.current_status_selection;
        self.status_picker_custom_status = self.current_custom_status.clone();
        self.status_update_error = None;
    }

    /// Closes the status picker without changing the confirmed status.
    pub fn close_status_picker(&mut self) {
        self.status_picker_open = false;
    }

    pub fn toggle_status_picker(&mut self) {
        if self.status_picker_open {
            self.close_status_picker();
        } else {
            self.open_status_picker();
        }
    }

    pub fn set_status_picker_status(
        &mut self,
        status: crate::components::status_picker::UserStatusSelection,
    ) {
        self.status_picker_status = status;
    }

    pub fn set_status_picker_text(&mut self, text: impl Into<String>) {
        self.status_picker_custom_status.set_text(text);
    }

    pub fn set_status_picker_emoji(
        &mut self,
        emoji: Option<crate::domain::user::CustomStatusEmoji>,
    ) {
        self.status_picker_custom_status.set_emoji(emoji);
    }

    pub fn set_status_picker_expiration(
        &mut self,
        expiration: crate::components::status_picker::CustomStatusExpiration,
    ) {
        self.status_picker_custom_status.expiration = expiration;
    }

    pub fn clear_status_picker_custom_status(&mut self) {
        self.status_picker_custom_status =
            crate::components::status_picker::CustomStatusDraft::default();
    }

    /// Applies one of the four status choices right away, keeping the custom
    /// status draft currently in the picker.
    pub fn apply_status_selection(
        &mut self,
        status: crate::components::status_picker::UserStatusSelection,
    ) {
        let draft = self.status_picker_custom_status.clone();
        self.request_status_update(status, draft);
    }

    /// Saves the current picker draft (status + custom status).
    pub fn save_status_picker(&mut self) {
        let status = self.status_picker_status;
        let draft = self.status_picker_custom_status.clone();
        self.request_status_update(status, draft);
    }

    /// Clears the custom status and applies it with the current status choice.
    pub fn clear_custom_status_and_save(&mut self) {
        let draft = crate::components::status_picker::CustomStatusDraft::default();
        let status = self.status_picker_status;
        self.status_picker_custom_status = draft.clone();
        self.request_status_update(status, draft);
    }

    /// Optimistically applies a status/custom-status change and dispatches the
    /// Gateway Opcode 3 + REST settings request. `finish_status_update` promotes
    /// or rolls it back when the transport answers.
    pub fn request_status_update(
        &mut self,
        target_status: crate::components::status_picker::UserStatusSelection,
        target_custom_status: crate::components::status_picker::CustomStatusDraft,
    ) {
        let request_id = self.status_update_request_seq.wrapping_add(1);
        self.status_update_request_seq = request_id;

        let pending = PendingStatusUpdate {
            request_id,
            previous_status: self.current_status_selection,
            previous_custom_status: self.current_custom_status.clone(),
            target_status,
            target_custom_status: target_custom_status.clone(),
        };
        self.pending_status_update = Some(pending);
        self.status_update_error = None;
        self.status_picker_status = target_status;
        self.status_picker_custom_status = target_custom_status.clone();

        // Optimistic: show the requested state immediately, but keep the pending
        // marker so the UI does not treat it as confirmed before the echo/API.
        self.current_status_selection = target_status;
        self.current_custom_status = target_custom_status.clone();
        self.current_user.presence = target_status.to_presence_status().into();
        self.current_user.custom_status = target_custom_status.text.clone();

        let Some(cm) = self.client_manager.clone() else {
            self.finish_status_update(
                request_id,
                Err("Não conectado ao Discord.".to_string()),
            );
            return;
        };

        let now = chrono::Utc::now();
        let gateway_payload = crate::components::status_picker::build_gateway_presence_update_payload(
            target_status,
            &target_custom_status,
            now,
        );
        let settings_payload = crate::components::status_picker::build_settings_custom_status_payload(
            &target_custom_status,
            now,
        );

        tokio::spawn(async move {
            let error = cm
                .update_user_status(&gateway_payload, &settings_payload)
                .await
                .err()
                .map(|error| error.to_string());
            cm.emit_event(crate::protocol::TransportEvent::UserStatusUpdateResult {
                request_id,
                error,
            });
        });
    }

    /// Promotes a successful status update or restores the previous snapshot on
    /// failure. Responses from superseded requests are ignored.
    pub fn finish_status_update(&mut self, request_id: u64, result: Result<(), String>) {
        let Some(pending) = self.pending_status_update.clone() else {
            return;
        };
        if pending.request_id != request_id {
            return;
        }
        self.pending_status_update = None;

        let (status, custom_status) = match result {
            Ok(()) => (pending.target_status, pending.target_custom_status),
            Err(error) => {
                self.status_picker_status = pending.previous_status;
                self.status_picker_custom_status = pending.previous_custom_status.clone();
                self.status_update_error = Some(error);
                (pending.previous_status, pending.previous_custom_status)
            }
        };

        self.current_status_selection = status;
        self.current_custom_status = custom_status.clone();
        self.current_user.presence = status.to_presence_status().into();
        self.current_user.custom_status = custom_status.text;
    }
}

impl AppState {
    /// Returns the cached presence for a given user ID
    pub fn get_user_presence(&self, user_id: &str) -> Option<&crate::domain::user::Presence> {
        self.presences_by_user.get(user_id)
    }

    /// Updates or inserts a user's real-time presence, propagating to friends and members
    pub fn update_user_presence(
        &mut self,
        user_id: String,
        status: crate::domain::user::PresenceStatus,
        activity: Option<crate::domain::user::Activity>,
        custom_status: Option<crate::domain::user::CustomStatus>,
    ) {
        let entry = self
            .presences_by_user
            .entry(user_id.clone())
            .or_insert_with(|| crate::domain::user::Presence::new(&user_id, status));
        entry.status = status;
        entry.activity = activity.clone();
        entry.custom_status = custom_status.clone();

        let app_status: PresenceStatus = status.into();

        // 1. Atualizar amigo na lista de amigos se existir
        if let Some(friend) = self.friends.iter_mut().find(|f| f.id == user_id) {
            friend.presence = app_status;
            friend.custom_status = custom_status.as_ref().and_then(|cs| cs.text.clone());
        }

        // 2. Se for o usuário autenticado, atualizar perfil atual. Enquanto uma
        // mudança está em voo, o eco do Gateway não deve sobrescrever o estado
        // otimista; `finish_status_update` decide entre confirmar ou reverter.
        if self.current_user.id == user_id {
            self.current_user.presence = app_status;
            self.current_user.custom_status = custom_status.as_ref().and_then(|cs| cs.text.clone());
            if self.pending_status_update.is_none() {
                self.current_status_selection =
                    crate::components::status_picker::UserStatusSelection::from_presence_status(
                        status,
                    );
                self.current_custom_status = custom_status
                    .as_ref()
                    .map(crate::components::status_picker::CustomStatusDraft::from_custom_status)
                    .unwrap_or_default();
                self.status_picker_status = self.current_status_selection;
            }
        }

        // 3. Atualizar membros na lista de membros se presente
        if let Some(member) = self.members.iter_mut().find(|m| m.id == user_id) {
            member.presence = app_status;
            member.activity = activity.as_ref().map(|act| act.name.clone());
        }
    }
}
