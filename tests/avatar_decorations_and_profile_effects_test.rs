//! Integration tests for Avatar Decorations and Profile Effects (Phase 5)

use discord_rs::components::avatar::{Avatar, AvatarSize};
use discord_rs::components::member_item::MemberItem;
use discord_rs::components::profile_effect::{
    render_profile_effect, ProfileEffect, ProfileEffectData, ProfileEffectTheme,
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
fn test_profile_effect_model_and_themes() {
    // 1. Deserialization of realistic Discord profile effect JSON
    let effect_json = r#"{
        "id": "1144002621008298034",
        "sku_id": "1144002621008298033",
        "expires_at": 1750000000
    }"#;
    let effect: ProfileEffectData = serde_json::from_str(effect_json).expect("Must parse profile effect");
    assert_eq!(effect.id, "1144002621008298034");
    assert_eq!(effect.sku_id.as_deref(), Some("1144002621008298033"));
    assert_eq!(effect.expires_at, Some(1750000000));
    assert_eq!(effect.theme(), ProfileEffectTheme::Sakura);
    assert_eq!(effect.theme().title(), "Sakura Blossoms");

    // 2. Preset theme resolution
    let cyber_effect = ProfileEffectData::new("cyber_neo_grid");
    assert_eq!(cyber_effect.theme(), ProfileEffectTheme::Cyberpunk);
    assert!(!cyber_effect.theme().gradient_stops().is_empty());

    let fire_effect = ProfileEffectData::new("ember_flame_burst");
    assert_eq!(fire_effect.theme(), ProfileEffectTheme::Fire);

    let frost_effect = ProfileEffectData::new("winter_snow_crystals");
    assert_eq!(frost_effect.theme(), ProfileEffectTheme::Frost);

    let ocean_effect = ProfileEffectData::new("deep_sea_abyss");
    assert_eq!(ocean_effect.theme(), ProfileEffectTheme::DeepSea);

    let arcade_effect = ProfileEffectData::new("arcade_retro_pixels");
    assert_eq!(arcade_effect.theme(), ProfileEffectTheme::RetroArcade);

    let cauldron_effect = ProfileEffectData::new("spooky_witch_cauldron");
    assert_eq!(cauldron_effect.theme(), ProfileEffectTheme::Cauldron);

    let celestial_effect = ProfileEffectData::new("cosmic_galaxy_stardust");
    assert_eq!(celestial_effect.theme(), ProfileEffectTheme::Celestial);

    let custom_effect = ProfileEffectData::new("unknown_effect_custom_999");
    assert!(matches!(custom_effect.theme(), ProfileEffectTheme::Custom(_)));
}

#[test]
fn test_profile_effect_component_and_render_function() {
    let effect = ProfileEffectData::with_sku("sakura_blossoms", "sku_sakura_1");

    // Test render_profile_effect helper function
    let rendered = render_profile_effect(&effect, px(105.0), Some(px(300.0)));
    let _ = rendered;

    // Test ProfileEffect IntoElement component builder
    let component = ProfileEffect::new(effect)
        .height(px(140.0))
        .width(px(340.0))
        .show_particles(true);
    let _ = component;
}
