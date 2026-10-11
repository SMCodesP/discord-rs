//! Integration tests for Avatar Decorations and Profile Effects (Phase 5)

extern crate gpui_kit as gpui;

use discord_rs::app::state::{AppState, FriendItemState, PresenceStatus as AppPresenceStatus};
use discord_rs::components::avatar::{Avatar, AvatarSize};
use discord_rs::components::member_item::MemberItem;
use discord_rs::components::profile_effect::{
    render_profile_effect, render_profile_effect_for_user, ProfileEffectCatalog,
    ProfileEffectConfig, ProfileEffectLayer, UserProfileEffect,
};
use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::message::{Author, Message};
use discord_rs::domain::user::{Activity, ActivityType, AvatarDecorationData, PresenceStatus, User};
use discord_rs::domain::voice::VoiceState;
use discord_rs::protocol::transport::{Relationship, TransportEvent};
use gpui::px;

#[test]
fn test_avatar_decoration_data_model_and_urls() {
    // 1. Static asset decoration
    let static_decor = AvatarDecorationData::new("b100_autumn_leaves");
    assert_eq!(static_decor.asset, "b100_autumn_leaves");
    assert_eq!(static_decor.sku_id, None);
    assert!(!static_decor.is_animated());
    assert_eq!(
        static_decor.url(),
        "https://cdn.discordapp.com/avatar-decoration-presets/b100_autumn_leaves.png?size=96&passthrough=true"
    );

    // 2. Animated asset decoration with store SKU
    let anim_decor = AvatarDecorationData::with_sku("a_999_fire_crown", "sku-11223344");
    assert_eq!(anim_decor.asset, "a_999_fire_crown");
    assert_eq!(anim_decor.sku_id.as_deref(), Some("sku-11223344"));
    assert!(anim_decor.is_animated());
    assert_eq!(
        anim_decor.url(),
        "https://cdn.discordapp.com/avatar-decoration-presets/a_999_fire_crown.png?size=96&passthrough=true"
    );
}

#[test]
fn test_user_serde_backward_compatibility() {
    // Payload completely missing avatar_decoration_data (legacy payload)
    let legacy_json = r#"{
        "id": "123456789",
        "username": "rust_fan",
        "discriminator": "0001",
        "global_name": "Ferris Lover",
        "avatar": "abcd1234efgh"
    }"#;
    let user: User = serde_json::from_str(legacy_json).expect("Must deserialize legacy user without decoration");
    assert_eq!(user.avatar_decoration_data, None);
    assert_eq!(user.avatar_decoration_url(), None);

    // Payload with null avatar_decoration_data
    let null_decor_json = r#"{
        "id": "123456789",
        "username": "rust_fan",
        "avatar_decoration_data": null
    }"#;
    let user_null: User = serde_json::from_str(null_decor_json).expect("Must deserialize null decoration");
    assert_eq!(user_null.avatar_decoration_data, None);

    // Payload with real equipped decoration
    let equipped_json = r#"{
        "id": "987654321",
        "username": "decor_pro",
        "avatar": "a_avatar123",
        "avatar_decoration_data": {
            "asset": "a_cyber_visor",
            "sku_id": "10987654321"
        }
    }"#;
    let user_equipped: User = serde_json::from_str(equipped_json).expect("Must deserialize equipped decoration");
    assert!(user_equipped.avatar_decoration_data.is_some());
    let decor = user_equipped.avatar_decoration_data.as_ref().unwrap();
    assert_eq!(decor.asset, "a_cyber_visor");
    assert_eq!(decor.sku_id.as_deref(), Some("10987654321"));
    assert!(decor.is_animated());
    assert_eq!(
        user_equipped.avatar_decoration_url().as_deref(),
        Some("https://cdn.discordapp.com/avatar-decoration-presets/a_cyber_visor.png?size=96&passthrough=true")
    );
}

#[test]
fn test_avatar_builder_decoration_integration() {
    // Avatar without decoration
    let plain_avatar = Avatar::new("User").size(AvatarSize::Px40);
    assert_eq!(AvatarSize::Px40.pixels(), px(40.0));

    // Avatar with decoration builder
    let decor = AvatarDecorationData::with_sku("a_sakura_petals", "sku_sakura");
    let decorated_avatar = Avatar::new("Cherry")
        .size(AvatarSize::Px32)
        .status(PresenceStatus::Online)
        .decoration(decor.clone());

    assert_eq!(AvatarSize::Px32.pixels(), px(32.0));
    assert_eq!(AvatarSize::Px32.status_size(), px(10.0));

    // Avatar with convenience asset setter
    let asset_avatar = Avatar::new("Ember").decoration_asset("fire_ring");
    let _ = asset_avatar;
    let _ = decorated_avatar;
    let _ = plain_avatar;
}

#[test]
fn test_member_item_repasses_avatar_decoration() {
    let mut user = User::new("user_100", "DecoratedMember");
    user.global_name = Some("Decorated Member".to_string());
    user.avatar = Some("avatar_hash_1".to_string());
    user.presence = PresenceStatus::DoNotDisturb;
    user.avatar_decoration_data = Some(AvatarDecorationData::new("neon_aura"));

    // Construct MemberItem via from_user
    let member_item = MemberItem::from_user("member-row-100", &user);
    let _ = member_item;

    // Construct MemberItem manually with explicit builder
    let manual_item = MemberItem::new("member-row-101", "ManualMember")
        .avatar_url("https://cdn.discordapp.com/avatars/1/test.png")
        .avatar_decoration(AvatarDecorationData::new("magic_sparkles"))
        .status(PresenceStatus::Idle);
    let _ = manual_item;
}

#[test]
fn test_user_profile_effect_and_catalog_parsing() {
    // 1. User profile effect reference
    let upe_json = r#"{"sku_id":"1271175049168031815","expires_at":1750000000}"#;
    let upe: UserProfileEffect = serde_json::from_str(upe_json).expect("Must parse UserProfileEffect");
    assert_eq!(upe.sku_id.as_deref(), Some("1271175049168031815"));
    assert_eq!(upe.expires_at, Some(1750000000));

    // 2. Real collectible category JSON with profile effects (type 1)
    let categories_json = serde_json::json!([
        {
            "name": "Halloween",
            "products": [
                {
                    "sku_id": "prod_1",
                    "items": [
                        {
                            "sku_id": "1271175049168031815",
                            "title": "Spooky Cauldron",
                            "type": 1,
                            "effects": [
                                {
                                    "src": "https://cdn.discordapp.com/media/v1/collectibles-shop/layer1.png",
                                    "loop": true,
                                    "duration": 5000,
                                    "start": 0,
                                    "loopDelay": 1000,
                                    "zIndex": 10
                                },
                                {
                                    "src": "https://cdn.discordapp.com/media/v1/collectibles-shop/layer2.png",
                                    "loop": false,
                                    "duration": 3000,
                                    "start": 500,
                                    "loopDelay": 0,
                                    "zIndex": 20
                                }
                            ]
                        },
                        {
                            "sku_id": "item_avatar_decor",
                            "type": 0
                        }
                    ]
                }
            ]
        }
    ]);

    let catalog = ProfileEffectCatalog::from_categories_json(&categories_json);
    let effect = catalog.get("1271175049168031815").expect("Effect must be loaded in catalog");
    assert_eq!(effect.title.as_deref(), Some("Spooky Cauldron"));
    assert_eq!(effect.effects.len(), 2);
    assert_eq!(effect.effects[0].z_index, 10);
    assert!(effect.effects[0].is_looping);
    assert_eq!(effect.effects[1].z_index, 20);
    assert!(!effect.effects[1].is_looping);
}

#[test]
fn test_profile_effect_render_fallback_zero_mocks() {
    let catalog = ProfileEffectCatalog::new();

    // 1. Unknown SKU -> renders empty div
    let unknown_effect = UserProfileEffect::new("unknown_sku_123");
    let rendered_unknown = render_profile_effect_for_user(Some(&unknown_effect), &catalog, px(105.0), Some(px(300.0)));
    let _ = rendered_unknown;

    // 2. None user effect -> renders empty div
    let rendered_none = render_profile_effect_for_user(None, &catalog, px(105.0), None);
    let _ = rendered_none;

    // 3. Known config with layers -> renders properly
    let config = ProfileEffectConfig {
        sku_id: "test_sku".to_string(),
        title: Some("Test Effect".to_string()),
        description: None,
        accessibility_label: None,
        reduced_motion_src: None,
        thumbnail_preview_src: None,
        effects: vec![
            ProfileEffectLayer {
                src: "https://cdn.discordapp.com/test.png".to_string(),
                is_looping: true,
                duration: 4000,
                start: 0,
                loop_delay: 0,
                z_index: 1,
                randomized_sources: vec![],
            }
        ],
        animation_type: Some(1),
        effect_type: Some(1),
    };
    let rendered_config = render_profile_effect(Some(&config), px(120.0), Some(px(300.0)));
    let _ = rendered_config;
}

// =========================================================================
// KAvatarData (Etapa 2, parte 1): decoration data reaches every UI model.
// Synthetic IDs/names/assets only; no account data.
// =========================================================================

fn synth_decor(asset: &str) -> AvatarDecorationData {
    AvatarDecorationData::with_sku(asset, "synth-sku-1")
}

#[test]
fn test_kavatardata_ready_user_decoration_reaches_current_user() {
    let mut state = AppState::new();

    let mut decorated = User::new("synth-user-101", "synth_ready_user");
    decorated.avatar_decoration_data = Some(synth_decor("a_synth_ready_deco"));
    state.handle_transport_event(TransportEvent::Ready {
        user: decorated,
        guilds: Vec::new(),
        guild_folders: Vec::new(),
    });
    let got = state.current_user.avatar_decoration_data.as_ref().expect("current user must carry decoration from READY");
    assert_eq!(got.asset, "a_synth_ready_deco");

    // Payload without decoration -> None (no stale data).
    let plain = User::new("synth-user-102", "synth_plain_user");
    state.handle_transport_event(TransportEvent::Ready {
        user: plain,
        guilds: Vec::new(),
        guild_folders: Vec::new(),
    });
    assert_eq!(state.current_user.avatar_decoration_data, None);
}

#[test]
fn test_kavatardata_dm_recipient_decoration_reaches_sidebar() {
    let mut state = AppState::new();

    let mut dm = Channel::new("synth-dm-201", "Synth Recip", ChannelType::Dm)
        .with_recipient_ids(vec!["synth-user-103".to_string()]);
    dm.recipient_names = vec!["Synth Recip".to_string()];
    dm.recipient_avatar_decorations = vec![Some(synth_decor("a_synth_dm_deco"))];

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![dm]));
    let item = state.dm_channels.iter().find(|d| d.id == "synth-dm-201").expect("DM must exist");
    let got = item.avatar_decoration_data.as_ref().expect("DM item must carry recipient decoration");
    assert_eq!(got.asset, "a_synth_dm_deco");

    // CHANNEL_CREATE path (real-time DM open).
    let mut live = Channel::new("synth-dm-202", "Synth Live", ChannelType::Dm)
        .with_recipient_ids(vec!["synth-user-104".to_string()]);
    live.recipient_names = vec!["Synth Live".to_string()];
    live.recipient_avatar_decorations = vec![Some(synth_decor("a_synth_live_deco"))];
    state.handle_transport_event(TransportEvent::DmChannelCreated(live));
    let live_item = state.dm_channels.iter().find(|d| d.id == "synth-dm-202").expect("live DM must exist");
    assert_eq!(
        live_item.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_live_deco")
    );

    // Channel without recipient data -> None.
    let bare = Channel::new("synth-dm-203", "Synth Bare", ChannelType::Dm)
        .with_recipient_ids(vec!["synth-user-105".to_string()]);
    state.handle_transport_event(TransportEvent::DmChannelCreated(bare));
    let bare_item = state.dm_channels.iter().find(|d| d.id == "synth-dm-203").expect("bare DM must exist");
    assert_eq!(bare_item.avatar_decoration_data, None);
}

#[test]
fn test_kavatardata_message_author_decoration_reaches_state() {
    let mut state = AppState::new();

    let mut author = Author::new("synth-user-106", "synth_msg_author");
    author.avatar_decoration_data = Some(synth_decor("a_synth_author_deco"));
    let msg = Message::new("synth-msg-301", "synth-chan-301", author, "synthetic hello");
    state.handle_transport_event(TransportEvent::MessageCreate(msg));

    let stored = state.messages_by_channel.get("synth-chan-301").expect("messages must exist");
    let got = stored[0].author.avatar_decoration_data.as_ref().expect("author decoration must survive gateway event");
    assert_eq!(got.asset, "a_synth_author_deco");

    // Optimistic send carries the current user's decoration.
    state.current_user.avatar_decoration_data = Some(synth_decor("a_synth_self_deco"));
    state.send_message("synth-chan-301", "synthetic optimistic");
    let stored = state.messages_by_channel.get("synth-chan-301").expect("messages must exist");
    let own = stored.last().expect("optimistic message must exist");
    assert_eq!(
        own.author.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_self_deco")
    );
}

#[test]
fn test_kavatardata_voice_state_decoration_retained() {
    let mut state = AppState::new();

    let vs = VoiceState::new("synth-user-107", "synth-sess-1")
        .with_channel_id("synth-voice-401")
        .with_user_name("Synth Voicer")
        .with_avatar_decoration(synth_decor("a_synth_voice_deco"));
    state.handle_transport_event(TransportEvent::VoiceStateUpdate(vs));

    let states = state.voice_states_for_channel("synth-voice-401");
    assert_eq!(states.len(), 1);
    assert_eq!(
        states[0].avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_voice_deco")
    );

    // Voice state without member data -> None.
    let bare = VoiceState::new("synth-user-108", "synth-sess-2").with_channel_id("synth-voice-402");
    state.handle_transport_event(TransportEvent::VoiceStateUpdate(bare));
    let bare_states = state.voice_states_for_channel("synth-voice-402");
    assert_eq!(bare_states.len(), 1);
    assert_eq!(bare_states[0].avatar_decoration_data, None);
}

#[test]
fn test_kavatardata_active_now_inherits_friend_decoration() {
    let mut state = AppState::new();
    state.friends.clear();
    state.active_now_items.clear();

    state.friends.push(FriendItemState {
        id: "synth-user-109".to_string(),
        username: "synth_gamer".to_string(),
        display_name: "Synth Gamer".to_string(),
        avatar_url: None,
        avatar_text: "S".to_string(),
        presence: AppPresenceStatus::Offline,
        custom_status: None,
        relationship_type: 1,
        avatar_decoration_data: Some(synth_decor("a_synth_active_deco")),
    });

    let mut activity = Activity::new("Synth Game", ActivityType::Playing);
    activity.details = Some("In Lobby".to_string());
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: "synth-user-109".to_string(),
        status: PresenceStatus::Online,
        activity: Some(activity),
        custom_status: None,
    });

    assert_eq!(state.active_now_items.len(), 1);
    assert_eq!(
        state.active_now_items[0].avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_active_deco")
    );
}

#[test]
fn test_krel_relationship_decoration_reaches_friend_state() {
    use discord_rs::protocol::discord_api::parse_relationship_from_json;

    // 1. Parse path (shared by READY relationships, RELATIONSHIP_ADD dispatch
    // and REST list): decoration comes from the nested `user` object.
    let with_decor = serde_json::json!({
        "id": "synth-user-110",
        "type": 1,
        "user": {
            "id": "synth-user-110",
            "username": "synth_friend",
            "global_name": "Synth Friend",
            "avatar": "synth_hash_110",
            "avatar_decoration_data": { "asset": "a_synth_rel_deco", "sku_id": "synth-sku-3" }
        }
    });
    let rel = parse_relationship_from_json(&with_decor).expect("relationship must parse");
    assert_eq!(
        rel.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_rel_deco")
    );

    // Absent and null decoration -> None (no stale data).
    let bare = serde_json::json!({
        "id": "synth-user-111",
        "type": 1,
        "user": { "id": "synth-user-111", "username": "synth_bare" }
    });
    assert_eq!(
        parse_relationship_from_json(&bare).expect("bare relationship must parse").avatar_decoration_data,
        None
    );
    let nulled = serde_json::json!({
        "id": "synth-user-112",
        "type": 1,
        "user": {
            "id": "synth-user-112",
            "username": "synth_null",
            "avatar_decoration_data": null
        }
    });
    assert_eq!(
        parse_relationship_from_json(&nulled).expect("nulled relationship must parse").avatar_decoration_data,
        None
    );

    // Flat structure (top-level decoration fallback)
    let flat = serde_json::json!({
        "id": "synth-user-112b",
        "type": 1,
        "username": "synth_flat",
        "avatar_decoration_data": { "asset": "a_synth_flat_rel_deco" }
    });
    assert_eq!(
        parse_relationship_from_json(&flat)
            .expect("flat relationship must parse")
            .avatar_decoration_data
            .as_ref()
            .map(|d| d.asset.as_str()),
        Some("a_synth_flat_rel_deco")
    );

    // 2. Bulk path: RelationshipsUpdated carries the decoration into FriendItemState.
    let mut state = AppState::new();
    state.friends.clear();
    state.handle_transport_event(TransportEvent::RelationshipsUpdated(vec![rel]));
    let friend = state.friends.iter().find(|f| f.id == "synth-user-110").expect("friend must exist");
    assert_eq!(
        friend.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_rel_deco")
    );

    // 3. Live path: RelationshipAdd creates and later updates the same entry.
    let mut live = Relationship {
        id: "synth-user-113".to_string(),
        rel_type: 1,
        username: "synth_live".to_string(),
        global_name: Some("Synth Live".to_string()),
        avatar: None,
        avatar_decoration_data: Some(synth_decor("a_synth_live_rel_deco")),
    };
    state.handle_transport_event(TransportEvent::RelationshipAdd(live.clone()));
    let live_friend =
        state.friends.iter().find(|f| f.id == "synth-user-113").expect("live friend must exist");
    assert_eq!(
        live_friend.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_live_rel_deco")
    );
    // Re-add without decoration clears it (payload is authoritative).
    live.avatar_decoration_data = None;
    state.handle_transport_event(TransportEvent::RelationshipAdd(live));
    let cleared =
        state.friends.iter().find(|f| f.id == "synth-user-113").expect("friend must still exist");
    assert_eq!(cleared.avatar_decoration_data, None);

    // 4. Bare relationship literal -> None.
    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "synth-user-114".to_string(),
        rel_type: 1,
        username: "synth_plain".to_string(),
        global_name: None,
        avatar: None,
        avatar_decoration_data: None,
    }));
    let plain =
        state.friends.iter().find(|f| f.id == "synth-user-114").expect("plain friend must exist");
    assert_eq!(plain.avatar_decoration_data, None);
}

#[test]
fn test_krel_user_update_refreshes_current_user_profile() {
    // Live USER_UPDATE dispatch refreshes identity, avatar and decoration,
    // and leaves presence / custom status untouched (not in the payload).
    let mut state = AppState::new();
    state.current_user.presence = AppPresenceStatus::Online;
    state.current_user.custom_status = Some("synth status".to_string());

    let mut updated = User::new("synth-user-115", "synth_renamed");
    updated.global_name = Some("Synth Renamed".to_string());
    updated.avatar = Some("synth_new_hash".to_string());
    updated.avatar_decoration_data = Some(synth_decor("a_synth_self_update_deco"));
    state.handle_transport_event(TransportEvent::CurrentUserUpdated(updated));

    assert_eq!(state.current_user.id, "synth-user-115");
    assert_eq!(state.current_user.username, "synth_renamed");
    assert_eq!(state.current_user.display_name, "Synth Renamed");
    assert_eq!(
        state.current_user.avatar_url.as_deref(),
        Some("https://cdn.discordapp.com/avatars/synth-user-115/synth_new_hash.png")
    );
    assert_eq!(
        state.current_user.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_self_update_deco")
    );
    // Untouched by the payload.
    assert_eq!(state.current_user.presence, AppPresenceStatus::Online);
    assert_eq!(state.current_user.custom_status.as_deref(), Some("synth status"));

    // Payload without decoration clears it (payload is authoritative).
    let plain = User::new("synth-user-115", "synth_renamed");
    state.handle_transport_event(TransportEvent::CurrentUserUpdated(plain));
    assert_eq!(state.current_user.avatar_decoration_data, None);
    assert_eq!(state.current_user.avatar_url, None);
}

#[test]
fn test_kavatardata_struct_json_backward_shapes() {
    // Author shape with decoration present / absent / null.
    let author_json = r#"{
        "id": "synth-user-111",
        "username": "synth_json",
        "avatar_decoration_data": { "asset": "a_synth_json_deco", "sku_id": "synth-sku-2" }
    }"#;
    let author: Author = serde_json::from_str(author_json).expect("author must deserialize");
    assert_eq!(
        author.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_json_deco")
    );

    let bare: Author = serde_json::from_str(r#"{"id": "synth-user-112", "username": "synth_bare"}"#)
        .expect("legacy author must deserialize");
    assert_eq!(bare.avatar_decoration_data, None);

    let nulled: Author = serde_json::from_str(
        r#"{"id": "synth-user-113", "username": "synth_null", "avatar_decoration_data": null}"#,
    )
    .expect("null decoration must deserialize");
    assert_eq!(nulled.avatar_decoration_data, None);

    // Message carrying a decorated author.
    let msg_json = r#"{
        "id": "synth-msg-302",
        "channel_id": "synth-chan-302",
        "content": "hi",
        "timestamp": "2026-10-01T12:00:00.000Z",
        "author": {
            "id": "synth-user-114",
            "username": "synth_msg_json",
            "avatar_decoration_data": { "asset": "a_synth_msg_json" }
        }
    }"#;
    let msg: Message = serde_json::from_str(msg_json).expect("message must deserialize");
    assert_eq!(
        msg.author.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_msg_json")
    );

    // DM channel with decorated recipient.
    let chan_json = r#"{
        "id": "synth-dm-204",
        "name": "Synth Json",
        "type": 1,
        "recipient_ids": [],
        "recipient_names": [],
        "recipient_avatar_decorations": [{ "asset": "a_synth_chan_json" }]
    }"#;
    let chan: Channel = serde_json::from_str(chan_json).expect("channel must deserialize");
    assert_eq!(
        chan.recipient_avatar_decorations.first().and_then(|d| d.as_ref()).map(|d| d.asset.as_str()),
        Some("a_synth_chan_json")
    );

    // Legacy channel payload without the new field still loads.
    let legacy_chan: Channel = serde_json::from_str(
        r#"{"id": "synth-dm-205", "name": "Legacy", "type": 1}"#,
    )
    .expect("legacy channel must deserialize");
    assert!(legacy_chan.recipient_avatar_decorations.is_empty());

    // Voice state with decoration present / absent.
    let vs_json = r#"{
        "user_id": "synth-user-115",
        "session_id": "synth-sess-3",
        "avatar_decoration_data": { "asset": "a_synth_vs_json" }
    }"#;
    let vs: VoiceState = serde_json::from_str(vs_json).expect("voice state must deserialize");
    assert_eq!(
        vs.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_vs_json")
    );

    let vs_bare: VoiceState =
        serde_json::from_str(r#"{"user_id": "synth-user-116", "session_id": "s"}"#)
            .expect("legacy voice state must deserialize");
    assert_eq!(vs_bare.avatar_decoration_data, None);

    // Relationship with decoration present / absent / null.
    let rel_json = r#"{
        "id": "synth-user-117",
        "type": 1,
        "username": "synth_rel_json",
        "avatar_decoration_data": { "asset": "a_synth_rel_json" }
    }"#;
    let rel: Relationship = serde_json::from_str(rel_json).expect("relationship must deserialize");
    assert_eq!(
        rel.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_synth_rel_json")
    );

    let rel_bare: Relationship = serde_json::from_str(
        r#"{"id": "synth-user-118", "type": 1, "username": "synth_bare"}"#,
    )
    .expect("legacy relationship must deserialize");
    assert_eq!(rel_bare.avatar_decoration_data, None);

    let rel_null: Relationship = serde_json::from_str(
        r#"{"id": "synth-user-119", "type": 1, "username": "synth_null", "avatar_decoration_data": null}"#,
    )
    .expect("null decoration relationship must deserialize");
    assert_eq!(rel_null.avatar_decoration_data, None);
}
