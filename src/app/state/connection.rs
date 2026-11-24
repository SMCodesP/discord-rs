use super::*;
use crate::protocol::AuthToken;
use crate::protocol::ConnectionMode;
use std::collections::HashMap;

/// Whether raw bot-token login is explicitly enabled for development.
///
/// The official login path is OAuth2 Authorization Code + PKCE. Bot tokens are
/// only accepted when `DISCORD_RS_DEV_BOT_TOKEN=1` (or `true`) is set, so a
/// production build cannot silently fall back to a bot/self-bot credential.
pub fn bot_token_login_enabled() -> bool {
    std::env::var("DISCORD_RS_DEV_BOT_TOKEN")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

impl AppState {
    /// Authenticates with a Discord token and updates connection status.
    ///
    /// Bot tokens require explicit development mode. The credential is not
    /// written to disk here: `gateway_events` persists it only after a real
    /// READY from the Gateway.
    pub fn authenticate(&mut self, token: String, is_user: bool) {
        let trimmed = token.trim().to_string();
        if trimmed.is_empty() {
            self.auth_error = Some("O token não pode estar vazio.".to_string());
            self.is_connecting = false;
            return;
        }

        if !is_user && !bot_token_login_enabled() {
            self.is_connecting = false;
            self.is_connected_to_gateway = false;
            self.auth_error = Some(
                "Login por token de bot está disponível apenas em modo explícito de desenvolvimento (DISCORD_RS_DEV_BOT_TOKEN=1). Use o login OAuth2."
                    .to_string(),
            );
            self.connection_status = "Desconectado".to_string();
            return;
        }

        self.auth_token = Some(trimmed.clone());
        self.auth_token_type = Some(if is_user {
            crate::protocol::TokenType::User
        } else {
            crate::protocol::TokenType::Bot
        });
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

    /// Authenticates with a successfully exchanged OAuth2 access token.
    ///
    /// Persistence remains deferred to the Gateway READY handler, which only
    /// writes a valid credential after Discord confirms it.
    pub fn authenticate_oauth(&mut self, access_token: String) {
        let trimmed = access_token.trim().to_string();
        if trimmed.is_empty() {
            self.auth_error = Some("O token OAuth2 não pode estar vazio.".to_string());
            self.is_connecting = false;
            return;
        }

        self.auth_token = Some(trimmed.clone());
        self.auth_token_type = Some(crate::protocol::TokenType::OAuth);
        self.token_input = trimmed.clone();
        self.is_user_token = true;
        self.auth_error = None;
        self.is_connecting = true;
        self.is_connected_to_gateway = false;
        self.connection_status = "Conectando ao Gateway v10...".to_string();

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let auth = AuthToken::oauth(trimmed);
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

    /// Performs a real logout: disconnects the Gateway and removes the saved
    /// credential from disk. The UI only leaves the connected state after the
    /// local session is actually torn down.
    pub fn logout(&mut self) {
        self.disconnect_gateway();

        self.auth_token = None;
        self.auth_token_type = None;
        self.token_input.clear();
        self.auth_error = None;
        self.is_connecting = false;
        self.is_connected_to_gateway = false;
        self.connection_mode = ConnectionMode::Disconnected("Desconectado".to_string());
        self.connection_status = "Desconectado".to_string();
        self.current_user = crate::app::state::CurrentUserProfile {
            id: String::new(),
            username: String::new(),
            display_name: String::new(),
            discriminator: None,
            avatar_text: String::new(),
            avatar_url: None,
            custom_status: None,
            presence: crate::app::state::PresenceStatus::Offline,
            is_muted: false,
            is_deafened: false,
            avatar_decoration_data: None,
        };
        self.guilds.clear();
        self.members.clear();
        self.friends.clear();
        self.dm_channels.clear();
        self.messages_by_channel.clear();

        if let Err(error) = crate::protocol::TokenStorage::default().clear() {
            log::warn!("Falha ao remover credenciais salvas no logout: {:#}", error);
            self.auth_error = Some("Não foi possível remover as credenciais salvas.".to_string());
        }

        self.close_modal();
    }

    /// Disconnects from live gateway
    pub fn disconnect_gateway(&mut self) {
        self.is_connected_to_gateway = false;
        self.auth_token = None;
        self.auth_token_type = None;
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
