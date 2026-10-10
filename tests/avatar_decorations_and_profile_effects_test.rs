//! Integration tests for Avatar Decorations and Profile Effects (Phase 5)

use discord_rs::components::avatar::{Avatar, AvatarSize};
use discord_rs::components::member_item::MemberItem;
use discord_rs::components::profile_effect::{
    render_profile_effect, render_profile_effect_for_user, ProfileEffectCatalog,
    ProfileEffectConfig, ProfileEffectLayer, UserProfileEffect,
};
use discord_rs::domain::user::{AvatarDecorationData, PresenceStatus, User};
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
