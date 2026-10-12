use std::collections::HashMap;
use super::*;
use crate::protocol::AuthToken;
use crate::protocol::ConnectionMode;
use crate::protocol::TokenStorage;

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

        if self.save_token_to_disk {
            let auth = if is_user {
                AuthToken::user(trimmed.clone())
            } else {
                AuthToken::bot(trimmed.clone())
            };
            let storage = TokenStorage::default();
            let _ = storage.save(&auth);
        }

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let auth = if is_user {
                AuthToken::user(trimmed)
            } else {
                AuthToken::bot(trimmed)
            };
            tokio::spawn(async move {
                if let Err(e) = cm.connect_with_token(auth).await {
                    log::error!("Falha ao conectar via ClientManager: {:?}", e);
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

        // 2. Se for o usuário autenticado, atualizar perfil atual
        if self.current_user.id == user_id {
            self.current_user.presence = app_status;
            self.current_user.custom_status = custom_status.as_ref().and_then(|cs| cs.text.clone());
        }

        // 3. Atualizar membros na lista de membros se presente
        if let Some(member) = self.members.iter_mut().find(|m| m.id == user_id) {
            member.presence = app_status;
            member.activity = activity.as_ref().map(|act| act.name.clone());
        }
    }
}
