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

/// Mention foreground (#cdd7ff). Source: `.theme-dark --mention-foreground` -> `var(--blurple-8)`.
pub const HEX_MENTION_FOREGROUND: u32 = 0xcdd7ff;

/// Border subtle with alpha (#97979f1f). Source: `.theme-dark --border-subtle` -> `hsl(var(--opacity-12-hsl)/0.1215)`.
pub const HEX_BORDER_SUBTLE: u32 = 0x97979f1f;

/// Channels default text (#999aa1). Source: `.theme-dark --channels-default` -> `var(--neutral-28)`.
pub const HEX_CHANNELS_DEFAULT: u32 = 0x999aa1;

/// Interactive normal text (#c5c6ca). Source: `.theme-dark --interactive-text-default` -> `var(--neutral-16)`.
pub const HEX_INTERACTIVE_NORMAL: u32 = 0xc5c6ca;

/// Reaction text default (#c5c6ca). Source: `.theme-dark --reaction-text-default` -> `var(--neutral-16)`.
pub const HEX_REACTION_TEXT_DEFAULT: u32 = 0xc5c6ca;

/// Reaction text reacted (#dfe4ff). Source: `.theme-dark --reaction-text-reacted-default` -> `var(--blurple-3)`.
pub const HEX_REACTION_TEXT_REACTED_DEFAULT: u32 = 0xdfe4ff;

/// Status danger red (#da3e44). Source: `.theme-dark --status-danger` -> `var(--red-400)` -> `var(--red-new-46)`.
pub const HEX_STATUS_DANGER: u32 = 0xda3e44;

/// Status positive green (#3d9e60). Source: `.theme-dark --status-positive` -> `var(--green-360)` -> `var(--green-new-40)`.
pub const HEX_STATUS_POSITIVE: u32 = 0x3d9e60;

/// Status warning yellow (#fdb833). Source: `.theme-dark --status-warning` -> `var(--yellow-300)` -> `var(--yellow-new-36)`.
pub const HEX_STATUS_WARNING: u32 = 0xfdb833;

/// Badge brand bubble pink stop (#eb459e). Source: `--bg-animated-gradient-background-pink-1`.
pub const HEX_BADGE_BRAND_BUBBLE: u32 = 0xeb459e;

/// COR ANTIGA SEM ORIGEM no Discord atual: header secondary (#949ba4). Não existe variável --header-secondary no DOM.
pub const HEX_LEGACY_949BA4: u32 = 0x949ba4;

/// COR ANTIGA SEM ORIGEM no Discord atual: background modifier hover (#282a2e). Não existe variável --background-modifier-hover no DOM.
pub const HEX_LEGACY_282A2E: u32 = 0x282a2e;

/// COR ANTIGA SEM ORIGEM no Discord atual: background modifier accent (#3f4147). Não existe variável --background-modifier-accent no DOM.
pub const HEX_LEGACY_3F4147: u32 = 0x3f4147;

/// COR ANTIGA SEM ORIGEM no Discord atual: text danger (#fa777c). Não existe variável --text-danger no DOM.
pub const HEX_LEGACY_FA777C: u32 = 0xfa777c;

/// COR ANTIGA SEM ORIGEM no Discord atual: button danger background hover (#da373c). Não existe variável --button-danger-background-hover no DOM.
pub const HEX_LEGACY_DA373C: u32 = 0xda373c;

/// COR ANTIGA SEM ORIGEM no Discord atual: text warning (#fee75c). Não existe variável --text-warning no DOM.
pub const HEX_LEGACY_FEE75C: u32 = 0xfee75c;

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

/// Mention foreground (#cdd7ff)
#[inline]
pub fn mention_foreground() -> Rgba {
    active_palette().mention_foreground
}

/// Border subtle (#97979f1f)
#[inline]
pub fn border_subtle() -> Rgba {
    active_palette().border_subtle
}

/// Channels default text (#999aa1)
#[inline]
pub fn channels_default() -> Rgba {
    active_palette().channels_default
}

/// Interactive normal text (#c5c6ca)
#[inline]
pub fn interactive_normal() -> Rgba {
    active_palette().interactive_normal
}

/// Reaction text default (#c5c6ca)
#[inline]
pub fn reaction_text_default() -> Rgba {
    active_palette().reaction_text_default
}

/// Reaction text reacted (#dfe4ff)
#[inline]
pub fn reaction_text_reacted_default() -> Rgba {
    active_palette().reaction_text_reacted_default
}

/// Status danger red (#da3e44)
#[inline]
pub fn status_danger() -> Rgba {
    active_palette().status_danger
}

/// Status positive green (#3d9e60)
#[inline]
pub fn status_positive() -> Rgba {
    active_palette().status_positive
}

/// Status warning yellow (#fdb833)
#[inline]
pub fn status_warning() -> Rgba {
    active_palette().status_warning
}

/// Badge brand bubble pink stop (#eb459e)
#[inline]
pub fn badge_brand_bubble() -> Rgba {
    active_palette().badge_brand_bubble
}

/// COR ANTIGA SEM ORIGEM no Discord atual: header secondary (#949ba4)
#[inline]
pub fn legacy_949ba4() -> Rgba {
    active_palette().legacy_949ba4
}

/// COR ANTIGA SEM ORIGEM no Discord atual: background modifier hover (#282a2e)
#[inline]
pub fn legacy_282a2e() -> Rgba {
    active_palette().legacy_282a2e
}

/// COR ANTIGA SEM ORIGEM no Discord atual: background modifier accent (#3f4147)
#[inline]
pub fn legacy_3f4147() -> Rgba {
    active_palette().legacy_3f4147
}

/// COR ANTIGA SEM ORIGEM no Discord atual: text danger (#fa777c)
#[inline]
pub fn legacy_fa777c() -> Rgba {
    active_palette().legacy_fa777c
}

/// COR ANTIGA SEM ORIGEM no Discord atual: button danger background hover (#da373c)
#[inline]
pub fn legacy_da373c() -> Rgba {
    active_palette().legacy_da373c
}

/// COR ANTIGA SEM ORIGEM no Discord atual: text warning (#fee75c)
#[inline]
pub fn legacy_fee75c() -> Rgba {
    active_palette().legacy_fee75c
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
// Semantic Theme Struct & Palette
// ============================================================================

/// Canonical Discord Color Palette (standard dark reference theme).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscordPalette {
    pub bg_server: Rgba,
    pub bg_channels: Rgba,
    pub bg_chat: Rgba,
    pub bg_chat_hover: Rgba,
    pub bg_user_bar: Rgba,
    pub bg_input: Rgba,
    pub bg_floating: Rgba,
    pub blurple: Rgba,
    pub blurple_hover: Rgba,
    pub blurple_active: Rgba,
    pub status_online: Rgba,
    pub status_ausente: Rgba,
    pub status_dnd: Rgba,
    pub status_offline: Rgba,
    pub status_danger: Rgba,
    pub status_positive: Rgba,
    pub status_warning: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_titles: Rgba,
    pub text_muted: Rgba,
    pub borders: Rgba,
    pub border_subtle: Rgba,
    pub channel_hover: Rgba,
    pub channel_active: Rgba,
    pub channels_default: Rgba,
    pub interactive_normal: Rgba,
    pub mention_bg: Rgba,
    pub mention_hover: Rgba,
    pub mention_foreground: Rgba,
    pub reaction_text_default: Rgba,
    pub reaction_text_reacted_default: Rgba,
    pub modal_backdrop: Rgba,
    pub scrollbar_thumb: Rgba,
    pub white: Rgba,
    pub neutral_67: Rgba,
    pub transparent: Rgba,
    pub opacity_white_8: Rgba,
    pub opacity_white_20: Rgba,
    pub control_primary_border: Rgba,
    pub control_secondary_bg_default: Rgba,
    pub control_secondary_bg_hover: Rgba,
    pub control_secondary_bg_active: Rgba,
    pub control_critical_primary_bg_default: Rgba,
    pub control_critical_primary_bg_hover: Rgba,
    pub control_critical_primary_bg_active: Rgba,
    pub control_critical_secondary_text_default: Rgba,
    pub control_icon_only_icon_default: Rgba,
    pub premium_tier_2_purple: Rgba,
    pub bg_animated_gradient_pink_1: Rgba,
    pub platform_xbox: Rgba,
    pub platform_spotify: Rgba,
    pub role_purple: Rgba,
    pub platform_reddit: Rgba,
    pub badge_brand_bubble: Rgba,
    pub legacy_949ba4: Rgba,
    pub legacy_282a2e: Rgba,
    pub legacy_3f4147: Rgba,
    pub legacy_fa777c: Rgba,
    pub legacy_da373c: Rgba,
    pub legacy_fee75c: Rgba,
}

impl Default for DiscordPalette {
    fn default() -> Self {
        Self::dark()
    }
}

impl DiscordPalette {
    /// Selects the palette corresponding to the given account appearance theme.
    ///
    /// Currently only `DiscordPalette::dark()` is implemented in discord-rs.
    /// Unsupported themes (such as Light, Midnight, or custom gradients) fallback to Dark,
    /// as specified in `docs/THEME_PER_ACCOUNT.md`.
    pub fn for_theme(_theme: crate::protocol::user_settings_proto::Theme) -> Self {
        Self::dark()
    }

    /// Returns the exact Discord Dark Theme reference palette.
    pub fn dark() -> Self {
        Self {
            bg_server: rgb(HEX_BG_SERVER),
            bg_channels: rgb(HEX_BG_CHANNELS),
            bg_chat: rgb(HEX_BG_CHAT),
            bg_chat_hover: rgb(HEX_BG_CHAT_HOVER),
            bg_user_bar: rgb(HEX_BG_USER_BAR),
            bg_input: rgb(HEX_BG_INPUT),
            bg_floating: rgb(HEX_BG_FLOATING),
            blurple: rgb(HEX_BLURPLE),
            blurple_hover: rgb(HEX_BLURPLE_HOVER),
            blurple_active: rgb(HEX_BLURPLE_ACTIVE),
            status_online: rgb(HEX_STATUS_ONLINE),
            status_ausente: rgb(HEX_STATUS_AUSENTE),
            status_dnd: rgb(HEX_STATUS_DND),
            status_offline: rgb(HEX_STATUS_OFFLINE),
            status_danger: rgb(HEX_STATUS_DANGER),
            status_positive: rgb(HEX_STATUS_POSITIVE),
            status_warning: rgb(HEX_STATUS_WARNING),
            text_primary: rgb(HEX_TEXT_PRIMARY),
            text_secondary: rgb(HEX_TEXT_SECONDARY),
            text_titles: rgb(HEX_TEXT_TITLES),
            text_muted: rgb(HEX_TEXT_MUTED),
            borders: rgb(HEX_BORDERS),
            border_subtle: rgba(HEX_BORDER_SUBTLE),
            channel_hover: rgb(HEX_CHANNEL_HOVER),
            channel_active: rgb(HEX_CHANNEL_ACTIVE),
            channels_default: rgb(HEX_CHANNELS_DEFAULT),
            interactive_normal: rgb(HEX_INTERACTIVE_NORMAL),
            mention_bg: rgba(HEX_MENTION_BG),
            mention_hover: rgba(HEX_MENTION_HOVER),
            mention_foreground: rgb(HEX_MENTION_FOREGROUND),
            reaction_text_default: rgb(HEX_REACTION_TEXT_DEFAULT),
            reaction_text_reacted_default: rgb(HEX_REACTION_TEXT_REACTED_DEFAULT),
            modal_backdrop: rgba(HEX_MODAL_BACKDROP),
            scrollbar_thumb: rgb(HEX_SCROLLBAR_THUMB),
            white: rgb(HEX_WHITE),
            neutral_67: rgb(HEX_NEUTRAL_67),
            transparent: rgba(HEX_TRANSPARENT),
            opacity_white_8: rgba(HEX_OPACITY_WHITE_8),
            opacity_white_20: rgba(HEX_OPACITY_WHITE_20),
            control_primary_border: rgba(HEX_CONTROL_PRIMARY_BORDER),
            control_secondary_bg_default: rgba(HEX_CONTROL_SECONDARY_BG_DEFAULT),
            control_secondary_bg_hover: rgba(HEX_CONTROL_SECONDARY_BG_HOVER),
            control_secondary_bg_active: rgba(HEX_CONTROL_SECONDARY_BG_ACTIVE),
            control_critical_primary_bg_default: rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_DEFAULT),
            control_critical_primary_bg_hover: rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_HOVER),
            control_critical_primary_bg_active: rgb(HEX_CONTROL_CRITICAL_PRIMARY_BG_ACTIVE),
            control_critical_secondary_text_default: rgb(HEX_CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT),
            control_icon_only_icon_default: rgb(HEX_CONTROL_ICON_ONLY_ICON_DEFAULT),
            premium_tier_2_purple: rgb(HEX_PREMIUM_TIER_2_PURPLE),
            bg_animated_gradient_pink_1: rgb(HEX_BG_ANIMATED_GRADIENT_PINK_1),
            platform_xbox: rgb(HEX_PLATFORM_XBOX),
            platform_spotify: rgb(HEX_PLATFORM_SPOTIFY),
            role_purple: rgb(HEX_ROLE_PURPLE),
            platform_reddit: rgb(HEX_PLATFORM_REDDIT),
            badge_brand_bubble: rgb(HEX_BADGE_BRAND_BUBBLE),
            legacy_949ba4: rgb(HEX_LEGACY_949BA4),
            legacy_282a2e: rgb(HEX_LEGACY_282A2E),
            legacy_3f4147: rgb(HEX_LEGACY_3F4147),
            legacy_fa777c: rgb(HEX_LEGACY_FA777C),
            legacy_da373c: rgb(HEX_LEGACY_DA373C),
            legacy_fee75c: rgb(HEX_LEGACY_FEE75C),
        }
    }
}

/// Returns the current active theme palette (Discord Dark Theme reference).
#[inline]
pub fn active_palette() -> DiscordPalette {
    DiscordPalette::dark()
}

/// Aggregated Discord Dark Theme color palette (for backwards compatibility).
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
        let p = active_palette();
        Self {
            bg_server: p.bg_server,
            bg_channels: p.bg_channels,
            bg_chat: p.bg_chat,
            bg_chat_hover: p.bg_chat_hover,
            bg_user_bar: p.bg_user_bar,
            bg_input: p.bg_input,
            blurple: p.blurple,
            blurple_hover: p.blurple_hover,
            blurple_active: p.blurple_active,
            status_online: p.status_online,
            status_ausente: p.status_ausente,
            status_dnd: p.status_dnd,
            status_offline: p.status_offline,
            text_secondary: p.text_secondary,
            text_primary: p.text_primary,
            text_titles: p.text_titles,
            borders: p.borders,
        }
    }
}
