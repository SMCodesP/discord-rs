use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Nível de assinatura Discord Nitro
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NitroTier {
    #[default]
    None = 0,
    Basic = 1,
    Full = 2,
}

impl NitroTier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "Sem Assinatura",
            Self::Basic => "Nitro Basic",
            Self::Full => "Nitro",
        }
    }

    pub fn upload_limit_mb(&self) -> u32 {
        match self {
            Self::None => 25,
            Self::Basic => 50,
            Self::Full => 500,
        }
    }

    pub fn streaming_resolution(&self) -> &'static str {
        match self {
            Self::None => "720p 30 FPS",
            Self::Basic => "1080p 60 FPS",
            Self::Full => "4K 60 FPS (Ultra)",
        }
    }
}

/// Assinatura ativa de faturamento (/users/@me/billing/subscriptions)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub tier: NitroTier,
    pub status: String, // "active", "canceled", "past_due"
    pub current_period_end: DateTime<Utc>,
    pub plan_name: String,
    pub payment_gateway: String, // "Stripe", "PayPal", "Apple"
}

/// Impulso de servidor ativo (/users/@me/guilds/premium/subscriptions)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerBoost {
    pub id: String,
    pub guild_id: String,
    pub guild_name: String,
    pub guild_icon: Option<String>,
    pub cooldown_ends: DateTime<Utc>,
}

/// Conta externa vinculada (/users/@me/connections)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountConnection {
    pub platform: String, // "github", "spotify", "steam", "youtube", "twitch", "playstation", "xbox"
    pub id: String,
    pub name: String,
    pub verified: bool,
    pub show_on_profile: bool,
    pub show_activity: bool,
}

impl AccountConnection {
    /// Identificador canônico do ícone/marca para a plataforma (sem emojis)
    pub fn icon(&self) -> &'static str {
        match self.platform.as_str() {
            "github" => "github",
            "spotify" => "spotify",
            "steam" => "steam",
            "youtube" => "youtube",
            "twitch" => "twitch",
            "playstation" => "playstation",
            "xbox" => "xbox",
            "battlenet" => "battlenet",
            "reddit" => "reddit",
            "tiktok" => "tiktok",
            _ => "link",
        }
    }

    /// Tag de marca curta em caixa alta para distintivo
    pub fn brand_tag(&self) -> &'static str {
        match self.platform.as_str() {
            "github" => "GITHUB",
            "spotify" => "SPOTIFY",
            "steam" => "STEAM",
            "youtube" => "YOUTUBE",
            "twitch" => "TWITCH",
            "playstation" => "PLAYSTATION",
            "xbox" => "XBOX",
            "battlenet" => "BATTLENET",
            "reddit" => "REDDIT",
            "tiktok" => "TIKTOK",
            _ => "LINK",
        }
    }

    /// Cor de destaque oficial da marca
    pub fn brand_color_hex(&self) -> u32 {
        match self.platform.as_str() {
            "github" => 0x24292e,
            "spotify" => 0x1db954,
            "steam" => 0x171a21,
            "youtube" => 0xff0000,
            "twitch" => 0x9146ff,
            "playstation" => 0x003791,
            "xbox" => 0x107c10,
            "battlenet" => 0x00aeef,
            "reddit" => 0xff4500,
            "tiktok" => 0xfe2c55,
            _ => 0x5865f2,
        }
    }

    pub fn platform_display_name(&self) -> &'static str {
        match self.platform.as_str() {
            "github" => "GitHub",
            "spotify" => "Spotify",
            "steam" => "Steam",
            "youtube" => "YouTube",
            "twitch" => "Twitch",
            "playstation" => "PlayStation Network",
            "xbox" => "Xbox Live",
            "battlenet" => "Battle.net",
            "reddit" => "Reddit",
            "tiktok" => "TikTok",
            _ => "Outro",
        }
    }
}

/// Aplicação OAuth2 autorizada (/oauth2/tokens)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedApp {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub scopes: Vec<String>,
    pub authorized_at: DateTime<Utc>,
}

/// Vínculo supervisionado no Centro da Família (/family-center/summary)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyCenterLink {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub is_guardian: bool,
    pub linked_at: DateTime<Utc>,
}

/// Opções de Tema Visual Discord
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    Midnight, // OLED Puro (#000000)
    SyncWithSystem,
}

impl ThemeChoice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark => "Escuro (Padrão)",
            Self::Light => "Claro",
            Self::Midnight => "Midnight (OLED #000000)",
            Self::SyncWithSystem => "Sincronizar com o Computador",
        }
    }

    pub fn bg_hex(&self) -> u32 {
        match self {
            Self::Dark => 0x313338,
            Self::Light => 0xffffff,
            Self::Midnight => 0x000000,
            Self::SyncWithSystem => 0x313338,
        }
    }
}

/// Modo de Exibição das Mensagens de Chat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageDisplayMode {
    #[default]
    Cozy,
    Compact,
}

impl MessageDisplayMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cozy => "Aconchegante (Avatares grandes e modernos)",
            Self::Compact => "Compacto (Denso estilo IRC)",
        }
    }
}

/// Configurações avançadas de acessibilidade
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilitySettings {
    pub saturation: f32, // 0.0 a 1.0 (Padrão 1.0)
    pub reduce_motion: bool,
    pub auto_play_gifs: bool,
    pub auto_play_stickers: bool,
    pub chat_font_size: f32, // 12.0 a 24.0 (Padrão 16.0)
    pub zoom_level: f32,      // 0.7 a 1.25 (Padrão 1.0)
    #[serde(default)]
    pub high_contrast: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            saturation: 1.0,
            reduce_motion: false,
            auto_play_gifs: true,
            auto_play_stickers: true,
            chat_font_size: 16.0,
            zoom_level: 1.0,
            high_contrast: false,
        }
    }
}
