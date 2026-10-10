use discord_rs::app::state::{AppState, ChannelItemState, ChannelKind};
use discord_rs::components::avatar::AvatarSize;
use discord_rs::domain::user::{Activity, CustomStatus, CustomStatusEmoji, PresenceStatus};
use discord_rs::protocol::transport::TransportEvent;
use gpui::px;

#[test]
fn test_presence_status_dot_specs() {
    let size_32 = AvatarSize::Px32;
    assert_eq!(size_32.pixels(), px(32.0));
    assert_eq!(size_32.status_size(), px(10.0)); // 10x10 dot
    assert_eq!(size_32.cutout_border(), px(2.0)); // 2px mask border

    // Canonical colors
    assert_eq!(PresenceStatus::Online.color_rgb(), 0x23A55A);
    assert_eq!(PresenceStatus::Idle.color_rgb(), 0xF0B232);
    assert_eq!(PresenceStatus::DoNotDisturb.color_rgb(), 0xF23F43);
    assert_eq!(PresenceStatus::Offline.color_rgb(), 0x80848E);

    assert_eq!(discord_rs::tokens::colors::HEX_BG_CHANNELS, 0x2b2d31);
}

#[test]
fn test_custom_status_model_unicode_and_custom_emoji() {
    // 1. Unicode emoji
    let emoji_unicode = CustomStatusEmoji::unicode("🌸");
    assert_eq!(emoji_unicode.name, "🌸");
    assert_eq!(emoji_unicode.id, None);
    assert_eq!(emoji_unicode.image_url(), None);

    let status_unicode = CustomStatus::new(Some("Férias na praia".to_string()), Some(emoji_unicode));
    assert_eq!(status_unicode.text.as_deref(), Some("Férias na praia"));
    assert_eq!(status_unicode.emoji.as_ref().unwrap().name, "🌸");

    // 2. Custom Discord guild emoji
    let emoji_custom = CustomStatusEmoji::custom("1020304050", "ferris_party", true);
    assert_eq!(emoji_custom.name, "ferris_party");
    assert_eq!(emoji_custom.id.as_deref(), Some("1020304050"));
    assert!(emoji_custom.animated);
    assert_eq!(
        emoji_custom.image_url().as_deref(),
        Some("https://cdn.discordapp.com/emojis/1020304050.gif?size=48")
    );

    let status_custom = CustomStatus::from_text("Coding").with_emoji(emoji_custom);
    assert_eq!(status_custom.text.as_deref(), Some("Coding"));
    assert_eq!(
        status_custom.emoji.as_ref().unwrap().image_url().as_deref(),
        Some("https://cdn.discordapp.com/emojis/1020304050.gif?size=48")
    );
}

#[test]
fn test_app_state_presence_update_and_resolution() {
    let mut state = AppState::new();

    let user_id = "user-ogarcone-123".to_string();
    let cs = CustomStatus::new(
        Some("I found her... ( A )".to_string()),
        Some(CustomStatusEmoji::unicode("🌸")),
    );
    let game = Activity::playing("VALORANT");

    // Send presence update
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: user_id.clone(),
        status: PresenceStatus::Online,
        activity: Some(game.clone()),
        custom_status: Some(cs.clone()),
    });

    // Check cached presence
    let presence = state.get_user_presence(&user_id).expect("presence should be cached");
    assert_eq!(presence.status, PresenceStatus::Online);
    assert_eq!(presence.custom_status, Some(cs.clone()));
    assert_eq!(presence.activity, Some(game.clone()));

    // Associate user with a 1-on-1 DM channel
    state.dm_channels.push(ChannelItemState {
        user_limit: None,
        id: "dm-ogarcone".into(),
        name: "Amigo Teste".into(),
        kind: ChannelKind::Text,
        topic: None,
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: false,
        last_message_id: None,
        is_group_dm: false,
        recipient_ids: vec![user_id.clone()],
        recipient_names: vec!["Amigo Teste".into()],
    });

    let dm = state.dm_channels.first().unwrap();
    assert_eq!(dm.recipient_ids.first().unwrap(), &user_id);

    // Verify lookup for DM line
    let p_info = state.get_user_presence(dm.recipient_ids.first().unwrap()).unwrap();
    assert_eq!(p_info.status, PresenceStatus::Online);
    assert_eq!(
        p_info.custom_status.as_ref().unwrap().text.as_deref(),
        Some("I found her... ( A )")
    );
}

#[test]
fn test_presence_transition_to_offline_clears_active_subtitle_state() {
    let mut state = AppState::new();
    let user_id = "user-vini-456".to_string();

    // 1. User starts online with custom status
    state.update_user_presence(
        user_id.clone(),
        PresenceStatus::DoNotDisturb,
        None,
        Some(CustomStatus::from_text("Reunião urgente")),
    );

    let p1 = state.get_user_presence(&user_id).unwrap();
    assert_eq!(p1.status, PresenceStatus::DoNotDisturb);
    assert_eq!(p1.custom_status.as_ref().unwrap().text.as_deref(), Some("Reunião urgente"));

    // 2. User goes offline
    state.update_user_presence(
        user_id.clone(),
        PresenceStatus::Offline,
        None,
        None,
    );

    let p2 = state.get_user_presence(&user_id).unwrap();
    assert_eq!(p2.status, PresenceStatus::Offline);
    assert_eq!(p2.custom_status, None);
}

#[test]
fn test_unknown_presence_defaults_to_offline_and_updates() {
    use discord_rs::protocol::transport::Relationship;

    let mut state = AppState::new();
    let friend_id = "user-bob-999".to_string();

    // Iniciar relacionamentos sem evento prévio de presença para este usuário
    state.handle_transport_event(TransportEvent::RelationshipsUpdated(vec![
        Relationship {
            id: friend_id.clone(),
            rel_type: 1, // Friend
            username: "bob_dev".to_string(),
            global_name: Some("Bob".to_string()),
            avatar: None,
        }
    ]));

    // Deve iniciar OFFLINE (não Online)
    let friend = state.friends.iter().find(|f| f.id == friend_id).expect("friend should exist");
    assert_eq!(friend.presence, discord_rs::app::state::PresenceStatus::Offline);
    assert_eq!(friend.custom_status, None);

    // Ao chegar PRESENCE_UPDATE, atualiza para status real Online
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: friend_id.clone(),
        status: PresenceStatus::Online,
        activity: None,
        custom_status: Some(CustomStatus::from_text("Disponível")),
    });

    let friend_after = state.friends.iter().find(|f| f.id == friend_id).unwrap();
    assert_eq!(friend_after.presence, discord_rs::app::state::PresenceStatus::Online);
    assert_eq!(friend_after.custom_status.as_deref(), Some("Disponível"));
}

#[test]
fn test_clearing_custom_status_removes_from_friend_and_presence() {
    let mut state = AppState::new();
    let friend_id = "user-alice-111".to_string();

    // 1. Friend tem status customizado ativo
    state.update_user_presence(
        friend_id.clone(),
        PresenceStatus::Online,
        None,
        Some(CustomStatus::from_text("Trabalhando remotamente")),
    );

    let p = state.get_user_presence(&friend_id).unwrap();
    assert_eq!(p.custom_status.as_ref().unwrap().text.as_deref(), Some("Trabalhando remotamente"));

    // 2. Recebe presença sem custom status (None)
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: friend_id.clone(),
        status: PresenceStatus::Online,
        activity: None,
        custom_status: None,
    });

    let p_cleared = state.get_user_presence(&friend_id).unwrap();
    assert_eq!(p_cleared.custom_status, None);

    if let Some(friend) = state.friends.iter().find(|f| f.id == friend_id) {
        assert_eq!(friend.custom_status, None);
    }
}
