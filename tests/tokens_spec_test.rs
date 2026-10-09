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
