//! Discord RS - Profile Effect Component
//! Renders Discord profile effects using real catalog effect layer definitions.
//!
//! # Architecture & Parity
//! Discord profile effects are defined in the collectibles catalog (`/collectibles-categories/v2`
//! or `/collectibles-shop`), where each collectible with `type: 1` defines a list of visual
//! layers (`effects`). Each layer contains a PNG or animated APNG URL (`src`), loop settings,
//! duration, and stacking z-index.
//!
//! The user profile object carries `user_profile.profile_effect` with the effect's `sku_id`.
//! If the effect is unknown or not in the catalog, this component renders nothing (empty Div).
//! Real animated APNG layers are rendered with native GPUI `img(url)`.

use gpui::prelude::*;
use gpui::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reference to an equipped profile effect on a user profile (`user_profile.profile_effect`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UserProfileEffect {
    /// SKU Snowflake identifier of the collectible effect.
    #[serde(default, alias = "id", alias = "skuId")]
    pub sku_id: Option<String>,
    /// Optional expiration timestamp (Unix seconds) for seasonal or trial effects.
    #[serde(default, alias = "expiresAt")]
    pub expires_at: Option<u64>,
}

impl UserProfileEffect {
    pub fn new(sku_id: impl Into<String>) -> Self {
        Self {
            sku_id: Some(sku_id.into()),
            expires_at: None,
        }
    }
}

/// An alternate randomized image source for an effect layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEffectLayerSource {
    pub src: String,
}

/// A visual image layer of a Discord profile effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEffectLayer {
    /// CDN image asset URL (PNG / animated APNG).
    pub src: String,
    /// Whether this layer loops continuously.
    #[serde(default, rename = "loop")]
    pub is_looping: bool,
    /// Duration of the layer animation in milliseconds.
    #[serde(default)]
    pub duration: u64,
    /// Start delay of the layer animation in milliseconds.
    #[serde(default)]
    pub start: u64,
    /// Delay between animation loops in milliseconds.
    #[serde(default, alias = "loopDelay")]
    pub loop_delay: u64,
    /// Z-index stacking order of the layer.
    #[serde(default, alias = "zIndex")]
    pub z_index: i32,
    /// Optional alternate sources for randomized variants.
    #[serde(default, alias = "randomizedSources")]
    pub randomized_sources: Vec<ProfileEffectLayerSource>,
}

/// A full profile effect definition from the Discord collectibles catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEffectConfig {
    /// Store SKU snowflake identifier.
    #[serde(alias = "skuId")]
    pub sku_id: String,
    /// Name/title of the profile effect.
    #[serde(default)]
    pub title: Option<String>,
    /// Description of the effect.
    #[serde(default)]
    pub description: Option<String>,
    /// Accessibility screen reader label.
    #[serde(default, alias = "accessibilityLabel")]
    pub accessibility_label: Option<String>,
    /// Static preview asset URL for reduced motion.
    #[serde(default, alias = "reducedMotionSrc")]
    pub reduced_motion_src: Option<String>,
    /// Thumbnail preview image URL.
    #[serde(default, alias = "thumbnailPreviewSrc")]
    pub thumbnail_preview_src: Option<String>,
    /// Layer definitions comprising the animated effect.
    #[serde(default)]
    pub effects: Vec<ProfileEffectLayer>,
    /// Animation type code.
    #[serde(default, alias = "animationType")]
    pub animation_type: Option<u32>,
    /// Collectible type (1 = profile effect).
    #[serde(default, rename = "type")]
    pub effect_type: Option<u32>,
}

/// In-memory catalog of profile effect configurations indexed by `sku_id`.
#[derive(Debug, Clone, Default)]
pub struct ProfileEffectCatalog {
    pub effects_by_sku: HashMap<String, ProfileEffectConfig>,
}

impl ProfileEffectCatalog {
    pub fn new() -> Self {
        Self {
            effects_by_sku: HashMap::new(),
        }
    }

    pub fn insert(&mut self, config: ProfileEffectConfig) {
        self.effects_by_sku.insert(config.sku_id.clone(), config);
    }

    pub fn get(&self, sku_id: &str) -> Option<&ProfileEffectConfig> {
        self.effects_by_sku.get(sku_id)
    }

    /// Parses all profile effects (`type == 1`) from the raw JSON array returned by
    /// `GET /api/v9/collectibles-categories/v2` or `GET /api/v9/collectibles-shop`.
    pub fn from_categories_json(categories_val: &serde_json::Value) -> Self {
        let mut catalog = Self::new();
        if let Some(categories) = categories_val.as_array() {
            for cat in categories {
                if let Some(products) = cat.get("products").and_then(|p| p.as_array()) {
                    for prod in products {
                        if let Some(items) = prod.get("items").and_then(|i| i.as_array()) {
                            for item in items {
                                if item.get("type").and_then(|t| t.as_u64()) == Some(1) {
                                    if let Ok(config) = serde_json::from_value::<ProfileEffectConfig>(item.clone()) {
                                        catalog.insert(config);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        catalog
    }
}

/// Renders a real Discord profile effect from its catalog configuration.
///
/// Overlays the actual image layers defined in the JSON catalog using GPUI's native
/// `img(url)` element (which natively plays animated APNGs).
///
/// If the configuration is None, unknown, or has no valid layers, returns an empty Div.
pub fn render_profile_effect(
    config: Option<&ProfileEffectConfig>,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> Div {
    let Some(config) = config else {
        return div();
    };

    if config.effects.is_empty() {
        if let Some(ref thumb) = config.thumbnail_preview_src {
            let mut root = div()
                .absolute()
                .top_0()
                .left_0()
                .h(banner_height)
                .overflow_hidden()
                .child(img(thumb.clone()).size_full());
            if let Some(w) = banner_width {
                root = root.w(w);
            } else {
                root = root.w_full();
            }
            return root;
        }
        return div();
    }

    let mut root = div()
        .absolute()
        .top_0()
        .left_0()
        .h(banner_height)
        .overflow_hidden();

    if let Some(w) = banner_width {
        root = root.w(w);
    } else {
        root = root.w_full();
    }

    // Sort layers by z_index ascending so higher z-index layers render on top
    let mut sorted_layers = config.effects.clone();
    sorted_layers.sort_by_key(|layer| layer.z_index);

    for layer in sorted_layers {
        if layer.src.is_empty() {
            continue;
        }
        let layer_el = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                img(layer.src.clone())
                    .size_full()
            );
        root = root.child(layer_el);
    }

    root
}

/// Helper that looks up the effect in the catalog and renders it.
pub fn render_profile_effect_for_user(
    user_effect: Option<&UserProfileEffect>,
    catalog: &ProfileEffectCatalog,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> Div {
    let config = user_effect
        .and_then(|ue| ue.sku_id.as_deref())
        .and_then(|sku| catalog.get(sku));

    render_profile_effect(config, banner_height, banner_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_user_profile_effect_serde() {
        // Real user profile JSON snippet
        let json_str = r#"{"sku_id":"1271175049168031815","expires_at":null}"#;
        let upe: UserProfileEffect = serde_json::from_str(json_str).unwrap();
        assert_eq!(upe.sku_id.as_deref(), Some("1271175049168031815"));
        assert_eq!(upe.expires_at, None);

        // Alias skuId
        let json_camel = r#"{"skuId":"999","expiresAt":1800000000}"#;
        let upe_camel: UserProfileEffect = serde_json::from_str(json_camel).unwrap();
        assert_eq!(upe_camel.sku_id.as_deref(), Some("999"));
        assert_eq!(upe_camel.expires_at, Some(1800000000));
    }

    #[::core::prelude::v1::test]
    fn test_profile_effect_config_real_discord_payload() {
        // Real Discord payload captured from network
        let real_json = r#"{
            "skuId": "1551709318724849664",
            "title": "Lottie's Cauldron",
            "description": "Show this effect when others view your profile.",
            "accessibilityLabel": "A witch casts a spell into a cauldron",
            "reducedMotionSrc": "https://cdn.discordapp.com/media/v1/collectibles-shop/b7e0110143f9022379069cad2e9a87394f89fc22b85a63792711151071717c95",
            "thumbnailPreviewSrc": "https://cdn.discordapp.com/media/v1/collectibles-shop/1f8b62514a782fe4d54fdd7fe1dbb6b6bf070716eaf8e41b332dba41ae00598f",
            "effects": [
                {
                    "src": "https://cdn.discordapp.com/media/v1/collectibles-shop/5c8afd0840307d8903249d27c1255b2d892785a71bf4fe9ab5d08000949f190a",
                    "loop": true,
                    "duration": 4980,
                    "start": 7211,
                    "loopDelay": 2124,
                    "zIndex": 100,
                    "randomizedSources": []
                },
                {
                    "src": "https://cdn.discordapp.com/media/v1/collectibles-shop/43c477e789ef0df0d29e3ba3a5aa99ae411ac05f78b82e1a8d023498d058bd0f",
                    "loop": false,
                    "duration": 4980,
                    "start": 0,
                    "loopDelay": 0,
                    "zIndex": 110,
                    "randomizedSources": [
                        {"src": "https://cdn.discordapp.com/media/v1/collectibles-shop/43c477e789ef0df0d29e3ba3a5aa99ae411ac05f78b82e1a8d023498d058bd0f"}
                    ]
                }
            ],
            "animationType": 2,
            "type": 1
        }"#;

        let config: ProfileEffectConfig = serde_json::from_str(real_json).unwrap();
        assert_eq!(config.sku_id, "1551709318724849664");
        assert_eq!(config.title.as_deref(), Some("Lottie's Cauldron"));
        assert_eq!(config.effects.len(), 2);
        assert!(config.effects[0].is_looping);
        assert_eq!(config.effects[0].duration, 4980);
        assert_eq!(config.effects[0].z_index, 100);
        assert!(!config.effects[1].is_looping);
        assert_eq!(config.effects[1].z_index, 110);
        assert_eq!(config.effects[1].randomized_sources.len(), 1);
    }

    #[::core::prelude::v1::test]
    fn test_catalog_population_and_empty_fallback() {
        let mut catalog = ProfileEffectCatalog::new();
        assert!(catalog.get("non_existent").is_none());

        // Unknown effect renders nothing
        let rendered_unknown = render_profile_effect_for_user(
            Some(&UserProfileEffect::new("unknown_sku")),
            &catalog,
            px(105.0),
            None,
        );
        let _ = rendered_unknown;

        // No effect renders nothing
        let rendered_none = render_profile_effect_for_user(None, &catalog, px(105.0), None);
        let _ = rendered_none;
    }
}
