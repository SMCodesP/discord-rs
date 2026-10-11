use discord_rs::i18n::*;
use discord_rs::tokens::*;

#[test]
fn test_all_design_tokens_and_colors() {
    assert_eq!(HEX_BG_SERVER, 0x1e1f22);
    assert_eq!(HEX_BG_CHANNELS, 0x2b2d31);
    assert_eq!(HEX_BG_CHAT, 0x313338);
    assert_eq!(HEX_BG_CHAT_HOVER, 0x383a40);
    assert_eq!(HEX_BG_USER_BAR, 0x232428);
    assert_eq!(HEX_BG_INPUT, 0x111214);
    assert_eq!(HEX_BLURPLE, 0x5865f2);
    assert_eq!(HEX_STATUS_ONLINE, 0x23a55a);
    assert_eq!(HEX_STATUS_AUSENTE, 0xf0b232);
    assert_eq!(HEX_STATUS_DND, 0xf23f43);
    assert_eq!(HEX_TEXT_SECONDARY, 0x80848e);
    assert_eq!(HEX_TEXT_PRIMARY, 0xdbdee1);
    assert_eq!(HEX_TEXT_TITLES, 0xf2f3f5);
    assert_eq!(HEX_BORDERS, 0x4e5058);

    // Color utility functions
    assert_eq!(bg_server(), rgb(0x1e1f22));
    assert_eq!(bg_channels(), rgb(0x2b2d31));
    assert_eq!(bg_chat(), rgb(0x313338));
    assert_eq!(bg_chat_hover(), rgb(0x383a40));
    assert_eq!(bg_user_bar(), rgb(0x232428));
    assert_eq!(bg_input(), rgb(0x111214));
    assert_eq!(blurple(), rgb(0x5865f2));
    assert_eq!(status_online(), rgb(0x23a55a));
    assert_eq!(status_ausente(), rgb(0xf0b232));
    assert_eq!(status_dnd(), rgb(0xf23f43));
    assert_eq!(text_secondary(), rgb(0x80848e));
    assert_eq!(text_primary(), rgb(0xdbdee1));
    assert_eq!(text_titles(), rgb(0xf2f3f5));
    assert_eq!(borders(), rgb(0x4e5058));
}

#[test]
fn test_new_parity_tokens_and_colors() {
    assert_eq!(HEX_NEUTRAL_67, 0x35363c);
    assert_eq!(HEX_TRANSPARENT, 0x00000000);
    assert_eq!(HEX_OPACITY_WHITE_8, 0xffffff14);
    assert_eq!(HEX_CONTROL_PRIMARY_BORDER, 0xffffff14);
    assert_eq!(HEX_OPACITY_WHITE_20, 0xffffff33);
    assert_eq!(HEX_PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS, 0x8547c6);
    assert_eq!(HEX_PREMIUM_TIER_2_PURPLE, 0x8547c6);
    assert_eq!(HEX_BG_ANIMATED_GRADIENT_PINK_1, 0xeb459e);
    assert_eq!(HEX_PLATFORM_XBOX, 0x107c10);
    assert_eq!(HEX_PLATFORM_SPOTIFY, 0x1db954);
    assert_eq!(HEX_ROLE_PURPLE, 0x9b59b6);
    assert_eq!(HEX_PLATFORM_REDDIT, 0xff4500);

    // Button controls
    assert_eq!(HEX_CONTROL_SECONDARY_BG_DEFAULT, 0x97979f1f);
    assert_eq!(HEX_CONTROL_SECONDARY_BG_HOVER, 0x97979f29);
    assert_eq!(HEX_CONTROL_SECONDARY_BG_ACTIVE, 0x97979f33);
    assert_eq!(HEX_CONTROL_CRITICAL_PRIMARY_BG_DEFAULT, 0xd22d39);
    assert_eq!(HEX_CONTROL_CRITICAL_PRIMARY_BG_HOVER, 0xa9232e);
    assert_eq!(HEX_CONTROL_CRITICAL_PRIMARY_BG_ACTIVE, 0x971d28);
    assert_eq!(HEX_CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT, 0xf87e7a);
    assert_eq!(HEX_CONTROL_ICON_ONLY_ICON_DEFAULT, 0xabacb2);

    // Accessors
    assert_eq!(neutral_67(), rgb(0x35363c));
    assert_eq!(transparent(), rgba(0x00000000));
    assert_eq!(opacity_white_8(), rgba(0xffffff14));
    assert_eq!(control_primary_border(), rgba(0xffffff14));
    assert_eq!(opacity_white_20(), rgba(0xffffff33));
    assert_eq!(premium_tier_2_purple_for_gradients(), rgb(0x8547c6));
    assert_eq!(premium_tier_2_purple(), rgb(0x8547c6));
    assert_eq!(bg_animated_gradient_pink_1(), rgb(0xeb459e));
    assert_eq!(platform_xbox(), rgb(0x107c10));
    assert_eq!(platform_spotify(), rgb(0x1db954));
    assert_eq!(role_purple(), rgb(0x9b59b6));
    assert_eq!(platform_reddit(), rgb(0xff4500));
    assert_eq!(control_secondary_bg_default(), rgba(0x97979f1f));
    assert_eq!(control_secondary_bg_hover(), rgba(0x97979f29));
    assert_eq!(control_secondary_bg_active(), rgba(0x97979f33));
    assert_eq!(control_critical_primary_bg_default(), rgb(0xd22d39));
    assert_eq!(control_critical_primary_bg_hover(), rgb(0xa9232e));
    assert_eq!(control_critical_primary_bg_active(), rgb(0x971d28));
    assert_eq!(control_critical_secondary_text_default(), rgb(0xf87e7a));
    assert_eq!(control_icon_only_icon_default(), rgb(0xabacb2));
}

#[test]
fn test_parity_added_tokens_and_accessors() {
    assert_eq!(HEX_MENTION_FOREGROUND, 0xa7b2ff);
    assert_eq!(HEX_BORDER_SUBTLE, 0x635a731f);
    assert_eq!(HEX_CHANNELS_DEFAULT, 0x8989aa);
    assert_eq!(HEX_INTERACTIVE_NORMAL, 0x9999b9);
    assert_eq!(HEX_BADGE_BRAND_BUBBLE, 0xeb459e);
    assert_eq!(HEX_TEXT_WARNING, 0xfee75c);
    assert_eq!(HEX_BACKGROUND_MODIFIER_HOVER, 0x282a2e);
    assert_eq!(HEX_HEADER_SECONDARY, 0x949ba4);
    assert_eq!(HEX_BACKGROUND_MODIFIER_ACCENT, 0x3f4147);
    assert_eq!(HEX_TEXT_DANGER, 0xfa777c);
    assert_eq!(HEX_BUTTON_DANGER_BACKGROUND_HOVER, 0xda373c);

    // Accessors
    assert_eq!(mention_foreground(), rgb(0xa7b2ff));
    assert_eq!(border_subtle(), rgba(0x635a731f));
    assert_eq!(channels_default(), rgb(0x8989aa));
    assert_eq!(interactive_normal(), rgb(0x9999b9));
    assert_eq!(badge_brand_bubble(), rgb(0xeb459e));
    assert_eq!(text_warning(), rgb(0xfee75c));
    assert_eq!(background_modifier_hover(), rgb(0x282a2e));
    assert_eq!(header_secondary(), rgb(0x949ba4));
    assert_eq!(background_modifier_accent(), rgb(0x3f4147));
    assert_eq!(text_danger(), rgb(0xfa777c));
    assert_eq!(button_danger_background_hover(), rgb(0xda373c));
}

#[test]
fn test_no_duplicate_tokens_with_conflicting_values() {
    // Verified that alias pairs have identical values
    assert_eq!(HEX_STATUS_IDLE, HEX_STATUS_AUSENTE);
    assert_eq!(HEX_TEXT_MUTED, HEX_TEXT_SECONDARY);
    assert_eq!(HEX_TEXT_NORMAL, HEX_TEXT_PRIMARY);
    assert_eq!(HEX_TEXT_HEADER, HEX_TEXT_TITLES);
    assert_eq!(HEX_CONTROL_PRIMARY_BORDER, HEX_OPACITY_WHITE_8);
    assert_eq!(HEX_PREMIUM_TIER_2_PURPLE, HEX_PREMIUM_TIER_2_PURPLE_FOR_GRADIENTS);
}

#[test]
fn test_spacing_and_dimensions() {
    assert_eq!(RAW_RAIL_WIDTH, 72.0);
    assert_eq!(RAW_CHANNELS_WIDTH, 240.0);
    assert_eq!(RAW_MEMBERS_WIDTH, 240.0);
}

#[test]
fn test_all_16_discord_icons() {
    let icons = [
        DiscordIcon::Hash,
        DiscordIcon::Speaker,
        DiscordIcon::Megaphone,
        DiscordIcon::Gear,
        DiscordIcon::Mic,
        DiscordIcon::Headphones,
        DiscordIcon::Pin,
        DiscordIcon::Search,
        DiscordIcon::Inbox,
        DiscordIcon::Members,
        DiscordIcon::Gift,
        DiscordIcon::Gif,
        DiscordIcon::Sticker,
        DiscordIcon::Emoji,
        DiscordIcon::Plus,
        DiscordIcon::Compass,
    ];

    for icon in icons {
        assert!(!icon.name().is_empty());
        assert!(!icon.path_data().is_empty());
        assert!(icon.svg_data().contains("<svg"));
    }
}

#[test]
fn test_auth_and_connection_icons() {
    let auth_icons = [
        DiscordIcon::Login,
        DiscordIcon::Key,
        DiscordIcon::Wifi,
        DiscordIcon::Connection,
        DiscordIcon::Lock,
        DiscordIcon::Warning,
        DiscordIcon::Info,
        DiscordIcon::Bot,
        DiscordIcon::User,
        DiscordIcon::Eye,
        DiscordIcon::EyeOff,
        DiscordIcon::Paste,
        DiscordIcon::Bell,
        DiscordIcon::Send,
        DiscordIcon::Close,
        DiscordIcon::Check,
    ];

    for icon in auth_icons {
        assert!(!icon.name().is_empty());
        assert!(!icon.path_data().is_empty());
        assert!(icon.svg_data().contains("<svg"));
        assert!(icon.svg_data().contains("viewBox=\"0 0 24 24\""));
    }
}

#[test]
fn test_i18n_pt_br() {
    set_locale(Locale::PtBr);
    assert_eq!(t("status.online"), "Online");
    assert_eq!(t("status.idle"), "Ausente");
    assert_eq!(t("status.dnd"), "Não perturbar");
    assert_eq!(t("channels.text_channels"), "CANAIS DE TEXTO");
    assert_eq!(t("channels.voice_channels"), "CANAIS DE VOZ");
    assert_eq!(t("user.mute"), "Silenciar");

    let formatted = t_fmt("chat.placeholder", &[("channel", "geral")]);
    assert_eq!(formatted, "Conversar em #geral");
}
