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
use std::time::{Duration, Instant};

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
                                    if let Ok(config) =
                                        serde_json::from_value::<ProfileEffectConfig>(item.clone())
                                    {
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

/// Returns the real image source for a layer. Randomized catalog entries are
/// all valid official assets; choosing the first one keeps the source stable
/// for the lifetime of the mounted effect instead of changing every frame.
fn effect_layer_source(layer: &ProfileEffectLayer) -> &str {
    layer
        .randomized_sources
        .first()
        .map(|source| source.src.as_str())
        .filter(|src| !src.is_empty())
        .unwrap_or(layer.src.as_str())
}

/// Next timestamp at which this layer can change visibility.
///
/// The renderer schedules one executor timer for this boundary instead of
/// requesting an animation frame on every display refresh. The image itself
/// remains a native APNG and plays smoothly without re-rendering the app.
fn effect_next_boundary_at(layer: &ProfileEffectLayer, t_ms: u64) -> Option<u64> {
    if layer.duration == 0 {
        return None;
    }
    if t_ms < layer.start {
        return Some(layer.start);
    }

    let elapsed = t_ms - layer.start;
    if !layer.is_looping {
        let end = layer.start.saturating_add(layer.duration);
        return (t_ms < end).then_some(end);
    }

    let period = layer.duration.saturating_add(layer.loop_delay);
    if period == 0 {
        return None;
    }
    let phase = elapsed % period;
    let remaining = if phase < layer.duration {
        layer.duration - phase
    } else {
        period - phase
    };
    t_ms.checked_add(remaining)
}

fn effect_root(banner_height: Pixels, banner_width: Option<Pixels>) -> Div {
    let mut root = div()
        .absolute()
        .top_0()
        .left_0()
        .h(banner_height)
        .overflow_hidden();
    if let Some(width) = banner_width {
        root = root.w(width);
    } else {
        root = root.w_full();
    }
    root
}

/// Renders one catalog layer on its own timeline. GPUI owns the animation
/// lifecycle, so unmounting the profile stops it without a background thread.
struct ProfileEffectLayerElement {
    id: ElementId,
    layer: ProfileEffectLayer,
    source: String,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
}

struct ProfileEffectTimelineState {
    layer: ProfileEffectLayer,
    started_at: Instant,
    scheduled_boundary: Option<u64>,
    timer_pending: bool,
    // Dropping the element state cancels a still-pending boundary wake-up.
    timer_task: Option<Task<()>>,
}

impl IntoElement for ProfileEffectLayerElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ProfileEffectLayerElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        window.with_element_state(
            global_id.expect("profile effect layer has a stable id"),
            |state, window| {
                let now = Instant::now();
                let mut timeline: ProfileEffectTimelineState =
                    state.unwrap_or_else(|| ProfileEffectTimelineState {
                        layer: self.layer.clone(),
                        started_at: now,
                        scheduled_boundary: None,
                        timer_pending: false,
                        timer_task: None,
                    });
                if timeline.layer != self.layer {
                    timeline.layer = self.layer.clone();
                    timeline.started_at = now;
                    timeline.scheduled_boundary = None;
                    timeline.timer_pending = false;
                    timeline.timer_task = None;
                }

                let elapsed = timeline
                    .started_at
                    .elapsed()
                    .as_millis()
                    .min(u64::MAX as u128) as u64;
                if timeline.timer_pending
                    && timeline
                        .scheduled_boundary
                        .is_some_and(|boundary| elapsed >= boundary)
                {
                    timeline.timer_pending = false;
                    timeline.scheduled_boundary = None;
                    timeline.timer_task = None;
                }

                let mut root = effect_root(self.banner_height, self.banner_width);
                if is_layer_visible_at(&self.layer, elapsed) {
                    root = root.child(
                        img(self.source.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    );
                }

                if let Some(boundary) = effect_next_boundary_at(&self.layer, elapsed) {
                    if !timeline.timer_pending || timeline.scheduled_boundary != Some(boundary) {
                        timeline.timer_pending = true;
                        timeline.scheduled_boundary = Some(boundary);
                        // Wake just after the boundary so millisecond truncation and
                        // executor scheduling cannot leave the layer stuck hidden.
                        let delay = Duration::from_millis(
                            boundary.saturating_sub(elapsed).saturating_add(2),
                        );
                        let view = window.current_view();
                        timeline.timer_task = Some(window.spawn(cx, async move |cx| {
                            cx.background_executor().timer(delay).await;
                            cx.update(move |_, cx| cx.notify(view)).ok();
                        }));
                    }
                }

                let mut root = root.into_any_element();
                ((root.request_layout(window, cx), root), timeline)
            },
        )
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::Element::prepaint(element, id, inspector_id, bounds, &mut (), window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::Element::paint(
            element,
            id,
            inspector_id,
            bounds,
            &mut (),
            _prepaint,
            window,
            cx,
        );
    }
}

fn render_effect_layer(
    layer: &ProfileEffectLayer,
    layer_index: usize,
    sku_id: &str,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> AnyElement {
    let source = effect_layer_source(layer);
    if layer.duration == 0 || source.is_empty() {
        return div().into_any_element();
    }

    ProfileEffectLayerElement {
        id: ElementId::Name(format!("profile-effect-{sku_id}-{layer_index}").into()),
        layer: layer.clone(),
        source: source.to_string(),
        banner_height,
        banner_width,
    }
    .into_any_element()
}

fn effect_static_source(config: &ProfileEffectConfig) -> Option<&str> {
    config
        .reduced_motion_src
        .as_deref()
        .filter(|src| !src.is_empty())
        .or_else(|| {
            config
                .thumbnail_preview_src
                .as_deref()
                .filter(|src| !src.is_empty())
        })
}

/// Renders a real Discord profile effect from its catalog configuration,
/// animating visible layers over time or respecting reduced motion.
///
/// Each layer keeps its own `start`, `duration`, `loop` and `loopDelay`
/// timeline. Intro layers never repeat just because another layer loops, and
/// looping layers never restart on a shared hourly budget. Layers are mounted
/// in ascending `z_index` order and native APNG assets provide smooth motion
/// without a timer that re-renders the whole application at a fixed FPS.
pub fn render_profile_effect_animated(
    config: Option<&ProfileEffectConfig>,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
    reduce_motion: bool,
) -> Div {
    let Some(config) = config else {
        return div();
    };

    if reduce_motion {
        return match effect_static_source(config) {
            Some(src) => effect_root(banner_height, banner_width).child(
                img(src.to_string())
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            ),
            None => div(),
        };
    }

    if config.effects.is_empty() {
        return match config
            .thumbnail_preview_src
            .as_deref()
            .filter(|src| !src.is_empty())
        {
            Some(src) => effect_root(banner_height, banner_width).child(
                img(src.to_string())
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            ),
            None => div(),
        };
    }

    let mut root = effect_root(banner_height, banner_width);
    let mut layers: Vec<_> = config.effects.iter().enumerate().collect();
    layers.sort_by_key(|(_, layer)| layer.z_index);
    for (layer_index, layer) in layers {
        root = root.child(render_effect_layer(
            layer,
            layer_index,
            &config.sku_id,
            banner_height,
            banner_width,
        ));
    }
    root
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
    render_profile_effect_for_user_with_motion(
        user_effect,
        catalog,
        banner_height,
        banner_width,
        false,
    )
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

    #[::core::prelude::v1::test]
    fn test_effect_timer_only_targets_visibility_boundaries() {
        let intro = ProfileEffectLayer {
            src: "intro.png".into(),
            is_looping: false,
            duration: 2_000,
            start: 500,
            loop_delay: 0,
            z_index: 0,
            randomized_sources: vec![],
        };
        assert_eq!(effect_next_boundary_at(&intro, 0), Some(500));
        assert_eq!(effect_next_boundary_at(&intro, 1_000), Some(2_500));
        assert_eq!(effect_next_boundary_at(&intro, 2_500), None);

        let delayed_loop = ProfileEffectLayer {
            src: "loop.png".into(),
            is_looping: true,
            duration: 3_000,
            start: 2_000,
            loop_delay: 1_000,
            z_index: 1,
            randomized_sources: vec![],
        };
        assert_eq!(effect_next_boundary_at(&delayed_loop, 0), Some(2_000));
        assert_eq!(effect_next_boundary_at(&delayed_loop, 2_500), Some(5_000));
        assert_eq!(effect_next_boundary_at(&delayed_loop, 5_500), Some(6_000));
        assert_eq!(effect_next_boundary_at(&delayed_loop, 6_500), Some(9_000));

        let zero_duration = ProfileEffectLayer {
            duration: 0,
            ..delayed_loop
        };
        assert_eq!(effect_next_boundary_at(&zero_duration, 0), None);
    }

    #[::core::prelude::v1::test]
    fn test_randomized_layer_source_is_real_and_stable() {
        let layer = ProfileEffectLayer {
            src: "fallback.png".into(),
            is_looping: false,
            duration: 1_000,
            start: 0,
            loop_delay: 0,
            z_index: 0,
            randomized_sources: vec![ProfileEffectLayerSource {
                src: "https://cdn.discordapp.com/catalog-variant.png".into(),
            }],
        };
        assert_eq!(
            effect_layer_source(&layer),
            "https://cdn.discordapp.com/catalog-variant.png"
        );
        assert_eq!(effect_layer_source(&layer), effect_layer_source(&layer));
    }

    #[::core::prelude::v1::test]
    fn test_reduced_motion_source_precedence_and_empty_fallback() {
        let mut config = ProfileEffectConfig {
            sku_id: "synth-reduced-source".into(),
            title: None,
            description: None,
            accessibility_label: None,
            reduced_motion_src: Some("https://cdn.discordapp.com/static.png".into()),
            thumbnail_preview_src: Some("https://cdn.discordapp.com/thumb.png".into()),
            effects: vec![ProfileEffectLayer {
                src: "animated.png".into(),
                is_looping: true,
                duration: 1_000,
                start: 0,
                loop_delay: 0,
                z_index: 0,
                randomized_sources: vec![],
            }],
            animation_type: None,
            effect_type: Some(1),
        };
        assert_eq!(
            effect_static_source(&config),
            Some("https://cdn.discordapp.com/static.png")
        );
        config.reduced_motion_src = None;
        assert_eq!(
            effect_static_source(&config),
            Some("https://cdn.discordapp.com/thumb.png")
        );
        config.thumbnail_preview_src = None;
        assert_eq!(effect_static_source(&config), None);
    }
}
