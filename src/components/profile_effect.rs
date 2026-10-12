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

/// Pure function that computes whether an individual profile effect layer is visible
/// at timestamp `t_ms` (in milliseconds elapsed since effect animation began).
///
/// Official collectibles catalog parity:
/// - A layer with `duration == 0` is never visible.
/// - If `t_ms < start`: the layer hasn't started yet and is not visible.
/// - If `!is_looping`: visible only during `start .. (start + duration)`. Once the duration elapses,
///   it finishes and is no longer visible.
/// - If `is_looping`: runs on a recurring timeline with cycle period = `duration + loop_delay`.
///   Within each cycle, it is visible during `0..duration`, followed by `loop_delay` ms of pause.
pub fn is_layer_visible_at(layer: &ProfileEffectLayer, t_ms: u64) -> bool {
    if layer.duration == 0 || t_ms < layer.start {
        return false;
    }
    let elapsed = t_ms - layer.start;
    if !layer.is_looping {
        elapsed < layer.duration
    } else {
        let period = layer.duration.saturating_add(layer.loop_delay);
        if period == 0 {
            false
        } else {
            (elapsed % period) < layer.duration
        }
    }
}

/// Pure function that filters and sorts all layers from a `ProfileEffectConfig`
/// visible at timestamp `t_ms`.
///
/// Layers are returned in ascending `z_index` order so lower z-index layers
/// are drawn underneath higher ones.
pub fn visible_layers_at<'a>(
    config: &'a ProfileEffectConfig,
    t_ms: u64,
) -> Vec<&'a ProfileEffectLayer> {
    let mut visible: Vec<&'a ProfileEffectLayer> = config
        .effects
        .iter()
        .filter(|layer| is_layer_visible_at(layer, t_ms))
        .collect();
    visible.sort_by_key(|layer| layer.z_index);
    visible
}

/// Renders a real Discord profile effect from its catalog configuration,
/// animating visible layers over time or respecting reduced motion.
///
/// Parity rules:
/// - When `reduce_motion` is enabled: displays static `reduced_motion_src` if available,
///   otherwise thumbnail preview or empty div. Does NOT animate.
/// - When animated: uses GPUI's native `AnimationExt::with_animation` with an element ID
///   scoped to the sku_id. When the popout closes or unmounts, animation automatically terminates;
///   no background thread or external timer runs.
/// - If the configuration is None or has no valid layers, returns empty Div or thumbnail fallback.
pub fn render_profile_effect_animated(
    config: Option<&ProfileEffectConfig>,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
    reduce_motion: bool,
) -> Div {
    let Some(config) = config else {
        return div();
    };

    // Reduced motion handling
    if reduce_motion {
        if let Some(ref static_src) = config.reduced_motion_src {
            let mut root = div()
                .absolute()
                .top_0()
                .left_0()
                .h(banner_height)
                .overflow_hidden()
                .child(img(static_src.clone()).size_full());
            if let Some(w) = banner_width {
                root = root.w(w);
            } else {
                root = root.w_full();
            }
            return root;
        } else if let Some(ref thumb) = config.thumbnail_preview_src {
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

    let is_any_looping = config.effects.iter().any(|layer| layer.is_looping);
    let max_timeline_ms = if is_any_looping {
        3_600_000u64 // 1 hour persistent loop duration budget
    } else {
        config
            .effects
            .iter()
            .map(|l| l.start.saturating_add(l.duration))
            .max()
            .unwrap_or(0)
            .max(100)
    };

    let mut anim = Animation::new(std::time::Duration::from_millis(max_timeline_ms))
        .with_max_fps(30.0);
    if is_any_looping {
        anim = anim.repeat();
    }

    let cfg_clone = config.clone();
    let anim_layer = div()
        .size_full()
        .relative()
        .with_animation(
            ElementId::Name(format!("profile-effect-anim-{}", config.sku_id).into()),
            anim,
            move |container, delta| {
                let t_ms = (delta * max_timeline_ms as f32).round() as u64;
                let visible = visible_layers_at(&cfg_clone, t_ms);
                let mut layer_root = container;
                for layer in visible {
                    if !layer.src.is_empty() {
                        layer_root = layer_root.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(img(layer.src.clone()).size_full()),
                        );
                    }
                }
                layer_root
            },
        );

    root.child(anim_layer)
}

/// Renders a real Discord profile effect from its catalog configuration.
pub fn render_profile_effect(
    config: Option<&ProfileEffectConfig>,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> Div {
    render_profile_effect_animated(config, banner_height, banner_width, false)
}

/// Helper that looks up the effect in the catalog and renders it.
pub fn render_profile_effect_for_user(
    user_effect: Option<&UserProfileEffect>,
    catalog: &ProfileEffectCatalog,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> Div {
    render_profile_effect_for_user_with_motion(user_effect, catalog, banner_height, banner_width, false)
}

/// Helper that looks up the effect in the catalog and renders it with reduced motion support.
pub fn render_profile_effect_for_user_with_motion(
    user_effect: Option<&UserProfileEffect>,
    catalog: &ProfileEffectCatalog,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
    reduce_motion: bool,
) -> Div {
    let config = user_effect
        .and_then(|ue| ue.sku_id.as_deref())
        .and_then(|sku| catalog.get(sku));

    render_profile_effect_animated(config, banner_height, banner_width, reduce_motion)
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
        let catalog = ProfileEffectCatalog::new();
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

    #[::core::prelude::v1::test]
    fn test_is_layer_visible_at_pure_function() {
        // 1. Layer with 0 duration is never visible
        let zero_dur = ProfileEffectLayer {
            src: "zero.png".into(),
            is_looping: false,
            duration: 0,
            start: 0,
            loop_delay: 0,
            z_index: 0,
            randomized_sources: vec![],
        };
        assert!(!is_layer_visible_at(&zero_dur, 0));
        assert!(!is_layer_visible_at(&zero_dur, 100));

        // 2. Non-looping layer: start=1000, duration=2000 (active 1000..3000)
        let non_loop = ProfileEffectLayer {
            src: "intro.png".into(),
            is_looping: false,
            duration: 2000,
            start: 1000,
            loop_delay: 0,
            z_index: 10,
            randomized_sources: vec![],
        };
        assert!(!is_layer_visible_at(&non_loop, 0));
        assert!(!is_layer_visible_at(&non_loop, 999));
        assert!(is_layer_visible_at(&non_loop, 1000));
        assert!(is_layer_visible_at(&non_loop, 2000));
        assert!(is_layer_visible_at(&non_loop, 2999));
        assert!(!is_layer_visible_at(&non_loop, 3000));
        assert!(!is_layer_visible_at(&non_loop, 5000));

        // 3. Looping layer without loopDelay: start=500, duration=1000, loop=true, loop_delay=0
        let loop_no_delay = ProfileEffectLayer {
            src: "continuous.png".into(),
            is_looping: true,
            duration: 1000,
            start: 500,
            loop_delay: 0,
            z_index: 20,
            randomized_sources: vec![],
        };
        assert!(!is_layer_visible_at(&loop_no_delay, 499));
        assert!(is_layer_visible_at(&loop_no_delay, 500));
        assert!(is_layer_visible_at(&loop_no_delay, 1499));
        assert!(is_layer_visible_at(&loop_no_delay, 1500)); // Next loop
        assert!(is_layer_visible_at(&loop_no_delay, 99999));

        // 4. Looping layer with loopDelay: start=1000, duration=2000, loopDelay=1000 (period=3000)
        // Cycle 0: active 1000..3000, pause 3000..4000
        // Cycle 1: active 4000..6000, pause 6000..7000
        let loop_with_delay = ProfileEffectLayer {
            src: "loop.png".into(),
            is_looping: true,
            duration: 2000,
            start: 1000,
            loop_delay: 1000,
            z_index: 30,
            randomized_sources: vec![],
        };
        assert!(!is_layer_visible_at(&loop_with_delay, 0));
        assert!(!is_layer_visible_at(&loop_with_delay, 999));
        assert!(is_layer_visible_at(&loop_with_delay, 1000));
        assert!(is_layer_visible_at(&loop_with_delay, 2500));
        assert!(is_layer_visible_at(&loop_with_delay, 2999));
        assert!(!is_layer_visible_at(&loop_with_delay, 3000)); // In loop delay
        assert!(!is_layer_visible_at(&loop_with_delay, 3500)); // In loop delay
        assert!(!is_layer_visible_at(&loop_with_delay, 3999)); // In loop delay
        assert!(is_layer_visible_at(&loop_with_delay, 4000)); // Cycle 1 start
        assert!(is_layer_visible_at(&loop_with_delay, 5999));
        assert!(!is_layer_visible_at(&loop_with_delay, 6000)); // Cycle 1 loop delay
        assert!(is_layer_visible_at(&loop_with_delay, 7000)); // Cycle 2 start
    }

    #[::core::prelude::v1::test]
    fn test_visible_layers_at_timeline_sorting() {
        let config = ProfileEffectConfig {
            sku_id: "test_sku_timeline".into(),
            title: Some("Test Timeline".into()),
            description: None,
            accessibility_label: None,
            reduced_motion_src: Some("https://cdn.discordapp.com/static.png".into()),
            thumbnail_preview_src: Some("https://cdn.discordapp.com/thumb.png".into()),
            effects: vec![
                ProfileEffectLayer {
                    src: "layer_high.png".into(),
                    is_looping: false,
                    duration: 3000,
                    start: 0,
                    loop_delay: 0,
                    z_index: 100,
                    randomized_sources: vec![],
                },
                ProfileEffectLayer {
                    src: "layer_low.png".into(),
                    is_looping: false,
                    duration: 3000,
                    start: 0,
                    loop_delay: 0,
                    z_index: 10,
                    randomized_sources: vec![],
                },
                ProfileEffectLayer {
                    src: "layer_late.png".into(),
                    is_looping: true,
                    duration: 2000,
                    start: 4000,
                    loop_delay: 1000,
                    z_index: 50,
                    randomized_sources: vec![],
                },
            ],
            animation_type: Some(1),
            effect_type: Some(1),
        };

        // At t = 1000: layers low (z=10) and high (z=100) are visible, sorted by z_index
        let layers_t1000 = visible_layers_at(&config, 1000);
        assert_eq!(layers_t1000.len(), 2);
        assert_eq!(layers_t1000[0].src, "layer_low.png");
        assert_eq!(layers_t1000[1].src, "layer_high.png");

        // At t = 3500: none visible (first two finished, late hasn't started)
        let layers_t3500 = visible_layers_at(&config, 3500);
        assert_eq!(layers_t3500.len(), 0);

        // At t = 4500: only layer_late is visible
        let layers_t4500 = visible_layers_at(&config, 4500);
        assert_eq!(layers_t4500.len(), 1);
        assert_eq!(layers_t4500[0].src, "layer_late.png");

        // Reduced motion render
        let rendered_reduced = render_profile_effect_animated(Some(&config), px(100.0), None, true);
        let _ = rendered_reduced;
    }
}
