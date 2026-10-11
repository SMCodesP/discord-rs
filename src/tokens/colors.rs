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
// Parity Tokens (Direct from Discord CSS variables in tokens.md)
// ============================================================================

/// Neutral 67: Dark neutral surface / border (#35363c). Source: `--neutral-67`
pub const HEX_NEUTRAL_67: u32 = 0x35363c;

/// Transparent (#00000000). Source: `--transparent`
pub const HEX_TRANSPARENT: u32 = 0x00000000;

/// Opacity White 8%: White at ~8% opacity (#ffffff14). Source: `--opacity-white-8` / `--control-primary-border-default`
pub const HEX_OPACITY_WHITE_8: u32 = 0xffffff14;
pub const HEX_CONTROL_PRIMARY_BORDER: u32 = HEX_OPACITY_WHITE_8;

/// Opacity White 20%: White at 20% opacity (#ffffff33). Source: `--opacity-white-20`
pub const HEX_OPACITY_WHITE_20: u32 = 0xffffff33;

/// Nitro Tier 2 Purple for gradients (#8547c6). Source: `--premium-tier-2-purple-for-gradients`
pub const HEX_PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS: u32 = 0x8547c6;
pub const HEX_PREMIUM_TIER_2_PURPLE: u32 = HEX_PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS;

/// Animated gradient background pink stop (#eb459e). Source: `--bg-animated-gradient-background-pink-1`
pub const HEX_BG_ANIMATED_GRADIENT_PINK_1: u32 = 0xeb459e;

/// Platform Xbox brand green (#107c10). Source: `--platform-xbox`
pub const HEX_PLATFORM_XBOX: u32 = 0x107c10;

/// Platform Spotify brand green (#1db954). Source: `--platform-spotify`
pub const HEX_PLATFORM_SPOTIFY: u32 = 0x1db954;

/// Role color purple (#9b59b6). Source: `--role-purple`
pub const HEX_ROLE_PURPLE: u32 = 0x9b59b6;

/// Platform Reddit brand red/orange (#ff4500). Source: `--platform-reddit`
pub const HEX_PLATFORM_REDDIT: u32 = 0xff4500;

// Button control tokens:

/// Control secondary background default (#97979f1f). Source: `--control-secondary-background-default`
pub const HEX_CONTROL_SECONDARY_BG_DEFAULT: u32 = 0x97979f1f;

/// Control secondary background hover (#97979f29). Source: `--control-secondary-background-hover`
pub const HEX_CONTROL_SECONDARY_BG_HOVER: u32 = 0x97979f29;

/// Control secondary background active (#97979f33). Source: `--control-secondary-background-active`
pub const HEX_CONTROL_SECONDARY_BG_ACTIVE: u32 = 0x97979f33;

/// Control critical primary background default (#d22d39). Source: `--control-critical-primary-background-default`
pub const HEX_CONTROL_CRITICAL_PRIMARY_BG_DEFAULT: u32 = 0xd22d39;

/// Control critical primary background hover (#a9232e). Source: `--control-critical-primary-background-hover`
pub const HEX_CONTROL_CRITICAL_PRIMARY_BG_HOVER: u32 = 0xa9232e;

/// Control critical primary background active (#971d28). Source: `--control-critical-primary-background-active`
pub const HEX_CONTROL_CRITICAL_PRIMARY_BG_ACTIVE: u32 = 0x971d28;

/// Control critical secondary text default (#f87e7a). Source: `--control-critical-secondary-text-default`
pub const HEX_CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT: u32 = 0xf87e7a;

/// Control icon-only icon default (#abacb2). Source: `--control-icon-only-icon-default`
pub const HEX_CONTROL_ICON_ONLY_ICON_DEFAULT: u32 = 0xabacb2;

/// Mention foreground (#a7b2ff). Source: `--mention-foreground`
pub const HEX_MENTION_FOREGROUND: u32 = 0xa7b2ff;

/// Border subtle with alpha (#635a731f). Source: `--border-subtle`
pub const HEX_BORDER_SUBTLE: u32 = 0x635a731f;

/// Channels default text (#8989aa). Source: `--channels-default`
pub const HEX_CHANNELS_DEFAULT: u32 = 0x8989aa;

/// Interactive normal text (#9999b9). Source: `--interactive-normal`
pub const HEX_INTERACTIVE_NORMAL: u32 = 0x9999b9;

/// Badge brand bubble pink stop (#eb459e). Source: `--badge-brand-bubble`
pub const HEX_BADGE_BRAND_BUBBLE: u32 = 0xeb459e;

/// Text warning yellow (#fee75c). Source: `--text-warning`
pub const HEX_TEXT_WARNING: u32 = 0xfee75c;

/// Background modifier hover (#282a2e). Source: `--background-modifier-hover`
pub const HEX_BACKGROUND_MODIFIER_HOVER: u32 = 0x282a2e;

/// Header secondary text (#949ba4). Source: `--header-secondary`
pub const HEX_HEADER_SECONDARY: u32 = 0x949ba4;

/// Background modifier accent (#3f4147). Source: `--background-modifier-accent`
pub const HEX_BACKGROUND_MODIFIER_ACCENT: u32 = 0x3f4147;

/// Text danger (#fa777c). Source: `--text-danger`
pub const HEX_TEXT_DANGER: u32 = 0xfa777c;

/// Button danger background hover (#da373c). Source: `--button-danger-background-hover`
pub const HEX_BUTTON_DANGER_BACKGROUND_HOVER: u32 = 0xda373c;

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

/// Neutral 67 (#35363c)
#[inline]
pub fn neutral_67() -> Rgba {
    rgb(HEX_NEUTRAL_67)
}

/// Fully transparent (#00000000)
#[inline]
pub fn transparent() -> Rgba {
    rgba(HEX_TRANSPARENT)
}

/// Opacity White 8% (#ffffff14)
#[inline]
pub fn opacity_white_8() -> Rgba {
    rgba(HEX_OPACITY_WHITE_8)
}

/// Control primary border alias (#ffffff14)
#[inline]
pub fn control_primary_border() -> Rgba {
    opacity_white_8()
}

/// Opacity White 20% (#ffffff33)
#[inline]
pub fn opacity_white_20() -> Rgba {
    rgba(HEX_OPACITY_WHITE_20)
}

/// Premium Tier 2 Purple for gradients (#8547c6)
#[inline]
pub fn premium_tier_2_purple_for_gradients() -> Rgba {
    rgb(HEX_PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS)
}

/// Alias for premium_tier_2_purple_for_gradients (#8547c6)
#[inline]
pub fn premium_tier_2_purple() -> Rgba {
    premium_tier_2_purple_for_gradients()
}

/// Background animated gradient pink stop (#eb459e)
#[inline]
pub fn bg_animated_gradient_pink_1() -> Rgba {
    rgb(HEX_BG_ANIMATED_GRADIENT_PINK_1)
}

/// Platform Xbox green (#107c10)
#[inline]
pub fn platform_xbox() -> Rgba {
    rgb(HEX_PLATFORM_XBOX)
}

/// Platform Spotify green (#1db954)
#[inline]
pub fn platform_spotify() -> Rgba {
    rgb(HEX_PLATFORM_SPOTIFY)
}

/// Role purple (#9b59b6)
#[inline]
pub fn role_purple() -> Rgba {
    rgb(HEX_ROLE_PURPLE)
}

/// Platform Reddit red/orange (#ff4500)
#[inline]
pub fn platform_reddit() -> Rgba {
    rgb(HEX_PLATFORM_REDDIT)
}

/// Control secondary background default (#97979f1f)
#[inline]
pub fn control_secondary_bg_default() -> Rgba {
    rgba(HEX_CONTROL_SECONDARY_BG_DEFAULT)
}

/// Control secondary background hover (#97979f29)
#[inline]
pub fn control_secondary_bg_hover() -> Rgba {
    rgba(HEX_CONTROL_SECONDARY_BG_HOVER)
}

/// Control secondary background active (#97979f33)
#[inline]
pub fn control_secondary_bg_active() -> Rgba {
    rgba(HEX_CONTROL_SECONDARY_BG_ACTIVE)
}

/// Control critical primary background default (#d22d39)
#[inline]
pub fn control_critical_primary_bg_default() -> Rgba {
    rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_DEFAULT)
}

/// Control critical primary background hover (#a9232e)
#[inline]
pub fn control_critical_primary_bg_hover() -> Rgba {
    rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_HOVER)
}

/// Control critical primary background active (#971d28)
#[inline]
pub fn control_critical_primary_bg_active() -> Rgba {
    rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_ACTIVE)
}

/// Control critical secondary text default (#f87e7a)
#[inline]
pub fn control_critical_secondary_text_default() -> Rgba {
    rgb(HEX_CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)
}

/// Control icon-only icon default (#abacb2)
#[inline]
pub fn control_icon_only_icon_default() -> Rgba {
    rgb(HEX_CONTROL_ICON_ONLY_ICON_DEFAULT)
}

/// Mention foreground (#a7b2ff)
#[inline]
pub fn mention_foreground() -> Rgba {
    rgb(HEX_MENTION_FOREGROUND)
}

/// Border subtle (#635a731f)
#[inline]
pub fn border_subtle() -> Rgba {
    rgba(HEX_BORDER_SUBTLE)
}

/// Channels default text (#8989aa)
#[inline]
pub fn channels_default() -> Rgba {
    rgb(HEX_CHANNELS_DEFAULT)
}

/// Interactive normal text (#9999b9)
#[inline]
pub fn interactive_normal() -> Rgba {
    rgb(HEX_INTERACTIVE_NORMAL)
}

/// Badge brand bubble pink stop (#eb459e)
#[inline]
pub fn badge_brand_bubble() -> Rgba {
    rgb(HEX_BADGE_BRAND_BUBBLE)
}

/// Text warning yellow (#fee75c)
#[inline]
pub fn text_warning() -> Rgba {
    rgb(HEX_TEXT_WARNING)
}

/// Background modifier hover (#282a2e)
#[inline]
pub fn background_modifier_hover() -> Rgba {
    rgb(HEX_BACKGROUND_MODIFIER_HOVER)
}

/// Header secondary text (#949ba4)
#[inline]
pub fn header_secondary() -> Rgba {
    rgb(HEX_HEADER_SECONDARY)
}

/// Background modifier accent (#3f4147)
#[inline]
pub fn background_modifier_accent() -> Rgba {
    rgb(HEX_BACKGROUND_MODIFIER_ACCENT)
}

/// Text danger (#fa777c)
#[inline]
pub fn text_danger() -> Rgba {
    rgb(HEX_TEXT_DANGER)
}

/// Button danger background hover (#da373c)
#[inline]
pub fn button_danger_background_hover() -> Rgba {
    rgb(HEX_BUTTON_DANGER_BACKGROUND_HOVER)
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
