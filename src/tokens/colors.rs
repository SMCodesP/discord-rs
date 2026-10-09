//! Discord RS - Color Tokens
//! Exact Discord Dark Theme color palette and GPUI color utilities.

use gpui::Rgba;
pub use gpui::{rgb, rgba};

// ============================================================================
// Core Palette Hex Constants (Exact Discord Dark Theme)
// ============================================================================

/// Servidor: Server rail / sidebar background (#1e1f22)
pub const HEX_BG_SERVER: u32 = 0x1e1f22;

/// Canais: Channels sidebar background (#2b2d31)
pub const HEX_BG_CHANNELS: u32 = 0x2b2d31;

/// Chat: Main chat area background (#313338)
pub const HEX_BG_CHAT: u32 = 0x313338;

/// Hover Chat: Chat message hover highlight background (#383a40)
pub const HEX_BG_CHAT_HOVER: u32 = 0x383a40;

/// Usuário Inferior: Bottom user profile panel background (#232428)
pub const HEX_BG_USER_BAR: u32 = 0x232428;

/// Input: Chat textarea / input background (#111214)
pub const HEX_BG_INPUT: u32 = 0x111214;

/// Blurple: Discord brand accent / button background (#5865f2)
pub const HEX_BLURPLE: u32 = 0x5865f2;

/// Online: User status online indicator (#23a55a)
pub const HEX_STATUS_ONLINE: u32 = 0x23a55a;

/// Ausente / Idle: User status idle / away indicator (#f0b232)
pub const HEX_STATUS_AUSENTE: u32 = 0xf0b232;
pub const HEX_STATUS_IDLE: u32 = HEX_STATUS_AUSENTE;

/// Não Perturbar / DND: User status do not disturb indicator (#f23f43)
pub const HEX_STATUS_DND: u32 = 0xf23f43;

/// Texto Secundário: Secondary / muted text (#80848e)
pub const HEX_TEXT_SECONDARY: u32 = 0x80848e;
pub const HEX_TEXT_MUTED: u32 = HEX_TEXT_SECONDARY;

/// Texto Principal: Primary body text (#dbdee1)
pub const HEX_TEXT_PRIMARY: u32 = 0xdbdee1;
pub const HEX_TEXT_NORMAL: u32 = HEX_TEXT_PRIMARY;

/// Títulos: Header / title / active text (#f2f3f5)
pub const HEX_TEXT_TITLES: u32 = 0xf2f3f5;
pub const HEX_TEXT_HEADER: u32 = HEX_TEXT_TITLES;

/// Bordas: Dividers, borders and modifiers (#4e5058)
pub const HEX_BORDERS: u32 = 0x4e5058;

/// Fundo do painel Ativo Agora (#2b2d31)
pub const HEX_BG_ACTIVE_NOW: u32 = 0x2b2d31;

/// Cor da barra de progresso do Spotify (#23a55a)
pub const HEX_SPOTIFY_GREEN: u32 = 0x23a55a;

/// Fundo de botões de aceitar solicitação (#5865f2)
pub const HEX_BTN_ACCEPT: u32 = 0x5865f2;

/// Fundo de botões de ignorar solicitação (#35373c)
pub const HEX_BTN_IGNORE: u32 = 0x35373c;

// ============================================================================
// Extended Discord Dark Palette Constants
// ============================================================================

/// Hover blurple (#4752c4)
pub const HEX_BLURPLE_HOVER: u32 = 0x4752c4;

/// Active/Pressed blurple (#3c45a5)
pub const HEX_BLURPLE_ACTIVE: u32 = 0x3c45a5;

/// Status offline / invisible (#80848e)
pub const HEX_STATUS_OFFLINE: u32 = 0x80848e;

/// Channel item hover (#35373c)
pub const HEX_CHANNEL_HOVER: u32 = 0x35373c;

/// Channel item active/selected (#3f4248)
pub const HEX_CHANNEL_ACTIVE: u32 = 0x3f4248;

/// Channel item unread badge white (#ffffff)
pub const HEX_WHITE: u32 = 0xffffff;

/// Popout / context menu background (#111214)
pub const HEX_BG_FLOATING: u32 = 0x111214;

/// Modal backdrop dark tint with alpha
pub const HEX_MODAL_BACKDROP: u32 = 0x000000cc;

/// Mention background tint with alpha (Blurple ~15% alpha)
pub const HEX_MENTION_BG: u32 = 0x5865f226;

/// Mention hover tint with alpha (Blurple ~25% alpha)
pub const HEX_MENTION_HOVER: u32 = 0x5865f240;

/// Scrollbar thumb background (#1a1b1e)
pub const HEX_SCROLLBAR_THUMB: u32 = 0x1a1b1e;

// ============================================================================
// Token Accessor Functions (Returning gpui::Rgba)
// ============================================================================

/// Servidor: Server rail background (#1e1f22)
#[inline]
pub fn bg_server() -> Rgba {
    rgb(HEX_BG_SERVER)
}

/// Canais: Channel sidebar background (#2b2d31)
#[inline]
pub fn bg_channels() -> Rgba {
    rgb(HEX_BG_CHANNELS)
}

/// Chat: Main chat area background (#313338)
#[inline]
pub fn bg_chat() -> Rgba {
    rgb(HEX_BG_CHAT)
}

/// Hover Chat: Chat message hover highlight (#383a40)
#[inline]
pub fn bg_chat_hover() -> Rgba {
    rgb(HEX_BG_CHAT_HOVER)
}

/// Usuário Inferior: Bottom user panel background (#232428)
#[inline]
pub fn bg_user_bar() -> Rgba {
    rgb(HEX_BG_USER_BAR)
}

/// Input: Chat textarea / input background (#111214)
#[inline]
pub fn bg_input() -> Rgba {
    rgb(HEX_BG_INPUT)
}

/// Blurple: Discord brand accent (#5865f2)
#[inline]
pub fn blurple() -> Rgba {
    rgb(HEX_BLURPLE)
}

/// Blurple on hover (#4752c4)
#[inline]
pub fn blurple_hover() -> Rgba {
    rgb(HEX_BLURPLE_HOVER)
}

/// Blurple when active/pressed (#3c45a5)
#[inline]
pub fn blurple_active() -> Rgba {
    rgb(HEX_BLURPLE_ACTIVE)
}

/// Online status green (#23a55a)
#[inline]
pub fn status_online() -> Rgba {
    rgb(HEX_STATUS_ONLINE)
}

/// Ausente / Idle status yellow (#f0b232)
#[inline]
pub fn status_ausente() -> Rgba {
    rgb(HEX_STATUS_AUSENTE)
}

/// Alias for status_ausente (#f0b232)
#[inline]
pub fn status_idle() -> Rgba {
    rgb(HEX_STATUS_IDLE)
}

/// Não Perturbar / DND status red (#f23f43)
#[inline]
pub fn status_dnd() -> Rgba {
    rgb(HEX_STATUS_DND)
}

/// Offline status grey (#80848e)
#[inline]
pub fn status_offline() -> Rgba {
    rgb(HEX_STATUS_OFFLINE)
}

/// Texto Secundário: Secondary / muted text (#80848e)
#[inline]
pub fn text_secondary() -> Rgba {
    rgb(HEX_TEXT_SECONDARY)
}

/// Alias for text_secondary (#80848e)
#[inline]
pub fn text_muted() -> Rgba {
    rgb(HEX_TEXT_MUTED)
}

/// Texto Principal: Primary text (#dbdee1)
#[inline]
pub fn text_primary() -> Rgba {
    rgb(HEX_TEXT_PRIMARY)
}

/// Alias for text_primary (#dbdee1)
#[inline]
pub fn text_normal() -> Rgba {
    rgb(HEX_TEXT_NORMAL)
}

/// Títulos: Header / title text (#f2f3f5)
#[inline]
pub fn text_titles() -> Rgba {
    rgb(HEX_TEXT_TITLES)
}

/// Alias for text_titles (#f2f3f5)
#[inline]
pub fn text_header() -> Rgba {
    rgb(HEX_TEXT_HEADER)
}

/// Bordas: Borders and dividers (#4e5058)
#[inline]
pub fn borders() -> Rgba {
    rgb(HEX_BORDERS)
}

/// Channel item hover state (#35373c)
#[inline]
pub fn channel_hover() -> Rgba {
    rgb(HEX_CHANNEL_HOVER)
}

/// Channel item active/selected state (#3f4248)
#[inline]
pub fn channel_active() -> Rgba {
    rgb(HEX_CHANNEL_ACTIVE)
}

/// Pure white (#ffffff)
#[inline]
pub fn white() -> Rgba {
    rgb(HEX_WHITE)
}

/// Floating / popout menu background (#111214)
#[inline]
pub fn bg_floating() -> Rgba {
    rgb(HEX_BG_FLOATING)
}

/// Mention background highlight (with alpha)
#[inline]
pub fn mention_bg() -> Rgba {
    rgba(HEX_MENTION_BG)
}

/// Mention hover highlight (with alpha)
#[inline]
pub fn mention_hover() -> Rgba {
    rgba(HEX_MENTION_HOVER)
}

/// Modal backdrop scrim (with alpha)
#[inline]
pub fn modal_backdrop() -> Rgba {
    rgba(HEX_MODAL_BACKDROP)
}

/// Scrollbar thumb background (#1a1b1e)
#[inline]
pub fn scrollbar_thumb() -> Rgba {
    rgb(HEX_SCROLLBAR_THUMB)
}

// ============================================================================
// Color Utility Helpers
// ============================================================================

/// Creates an `Rgba` color from a 24-bit RGB hex value (`0xRRGGBB`).
#[inline]
pub fn from_hex(hex: u32) -> Rgba {
    rgb(hex)
}

/// Creates an `Rgba` color from a 32-bit RGBA hex value (`0xRRGGBBAA`).
#[inline]
pub fn from_hex_alpha(hex: u32) -> Rgba {
    rgba(hex)
}

/// Creates an `Rgba` color from a 24-bit hex value and an explicit alpha component [0.0, 1.0].
#[inline]
pub fn with_alpha(hex: u32, alpha: f32) -> Rgba {
    let mut color = rgb(hex);
    color.a = alpha.clamp(0.0, 1.0);
    color
}

/// Adjusts the opacity of an existing `Rgba` color.
#[inline]
pub fn set_alpha(mut color: Rgba, alpha: f32) -> Rgba {
    color.a = alpha.clamp(0.0, 1.0);
    color
}

// ============================================================================
// Semantic Theme Struct
// ============================================================================

/// Aggregated Discord Dark Theme color palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscordThemeColors {
    pub bg_server: Rgba,
    pub bg_channels: Rgba,
    pub bg_chat: Rgba,
    pub bg_chat_hover: Rgba,
    pub bg_user_bar: Rgba,
    pub bg_input: Rgba,
    pub blurple: Rgba,
    pub blurple_hover: Rgba,
    pub blurple_active: Rgba,
    pub status_online: Rgba,
    pub status_ausente: Rgba,
    pub status_dnd: Rgba,
    pub status_offline: Rgba,
    pub text_secondary: Rgba,
    pub text_primary: Rgba,
    pub text_titles: Rgba,
    pub borders: Rgba,
}

impl Default for DiscordThemeColors {
    fn default() -> Self {
        Self::dark()
    }
}

impl DiscordThemeColors {
    /// Returns the exact Discord Dark Theme palette.
    pub fn dark() -> Self {
        Self {
            bg_server: bg_server(),
            bg_channels: bg_channels(),
            bg_chat: bg_chat(),
            bg_chat_hover: bg_chat_hover(),
            bg_user_bar: bg_user_bar(),
            bg_input: bg_input(),
            blurple: blurple(),
            blurple_hover: blurple_hover(),
            blurple_active: blurple_active(),
            status_online: status_online(),
            status_ausente: status_ausente(),
            status_dnd: status_dnd(),
            status_offline: status_offline(),
            text_secondary: text_secondary(),
            text_primary: text_primary(),
            text_titles: text_titles(),
            borders: borders(),
        }
    }
}
