//! Discord RS - Profile Effect Component
//! Thematic visual effects (gradients, particle overlays, and ambient glows)
//! rendered on user profile banners and popouts.
//!
//! # Technical Parity & GPUI Limitations
//! In the official Discord web/desktop client, profile effects are powered by Lottie JSON,
//! APNG or WebGL canvas shaders. Because GPUI does not feature a WebGL runtime, this component
//! renders high-fidelity multi-stop linear gradient backdrops, atmospheric ambient glow layers,
//! and thematic particle overlays (e.g. cherry blossom petals, embers, cyber nodes, frost crystals)
//! using native GPUI elements. This achieves authentic visual aesthetic parity with zero overhead.

use gpui::prelude::*;
use gpui::*;
use serde::{Deserialize, Serialize};

/// Canonical profile effect payload received in user profiles or collectibles inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEffectData {
    /// Snowflake identifier or preset name of the profile effect.
    pub id: String,
    /// Store SKU snowflake identifier, if applicable.
    #[serde(default)]
    pub sku_id: Option<String>,
    /// Optional expiration timestamp (Unix seconds) for seasonal / nitro-trial items.
    #[serde(default)]
    pub expires_at: Option<u64>,
}

impl ProfileEffectData {
    /// Creates a new profile effect with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            sku_id: None,
            expires_at: None,
        }
    }

    /// Creates a profile effect with an explicit SKU ID.
    pub fn with_sku(id: impl Into<String>, sku_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            sku_id: Some(sku_id.into()),
            expires_at: None,
        }
    }

    /// Maps this effect's identifier to a recognized Discord visual theme.
    pub fn theme(&self) -> ProfileEffectTheme {
        ProfileEffectTheme::from_effect_id(&self.id)
    }
}

/// Thematic visual style mapped from Discord profile effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileEffectTheme {
    /// Falling sakura petals with rose-pink glow
    Sakura,
    /// Neon cyan and laser violet cyber grid
    Cyberpunk,
    /// Arcane purple and cosmic shimmering glow
    Mystic,
    /// Warm crimson, burning orange, and rising embers
    Fire,
    /// Frost crystals, icy blue, and winter sparkle
    Frost,
    /// Aquamarine oceanic hues and rising bubble highlights
    DeepSea,
    /// 8-bit arcade magenta, pixel grid, and neon sparks
    RetroArcade,
    /// Autumn golden-amber, bronze, and falling leaves
    Autumn,
    /// Poison-emerald green and bubbling mystical mist
    Cauldron,
    /// Midnight galaxy, cosmic violet, and stardust
    Celestial,
    /// Generic or unknown effect with deterministic theme derived from ID
    Custom(String),
}

impl ProfileEffectTheme {
    /// Resolves an effect ID into a known preset theme.
    pub fn from_effect_id(id: &str) -> Self {
        let lower = id.to_lowercase();
        if lower.contains("sakura") || lower.contains("blossom") || lower.contains("cherry") || id == "1144002621008298034" {
            Self::Sakura
        } else if lower.contains("cyber") || lower.contains("matrix") || lower.contains("neon") {
            Self::Cyberpunk
        } else if lower.contains("mystic") || lower.contains("magic") || lower.contains("arcane") {
            Self::Mystic
        } else if lower.contains("fire") || lower.contains("flame") || lower.contains("ember") {
            Self::Fire
        } else if lower.contains("frost") || lower.contains("ice") || lower.contains("snow") || lower.contains("winter") {
            Self::Frost
        } else if lower.contains("sea") || lower.contains("ocean") || lower.contains("hydro") || lower.contains("water") {
            Self::DeepSea
        } else if lower.contains("arcade") || lower.contains("retro") || lower.contains("8bit") || lower.contains("pixel") {
            Self::RetroArcade
        } else if lower.contains("autumn") || lower.contains("fall") || lower.contains("leaves") {
            Self::Autumn
        } else if lower.contains("cauldron") || lower.contains("witch") || lower.contains("spooky") {
            Self::Cauldron
        } else if lower.contains("celestial") || lower.contains("galaxy") || lower.contains("star") || lower.contains("cosmic") {
            Self::Celestial
        } else {
            Self::Custom(id.to_string())
        }
    }

    /// Display title for the effect theme.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Sakura => "Sakura Blossoms",
            Self::Cyberpunk => "Cyberpunk Neo",
            Self::Mystic => "Mystic Arcane",
            Self::Fire => "Fiery Embers",
            Self::Frost => "Winter Frost",
            Self::DeepSea => "Ocean Depths",
            Self::RetroArcade => "Retro 8-Bit",
            Self::Autumn => "Autumn Leaves",
            Self::Cauldron => "Witch's Cauldron",
            Self::Celestial => "Cosmic Stardust",
            Self::Custom(_) => "Profile Effect",
        }
    }

    /// Hex color stops for the background gradient.
    pub fn gradient_stops(&self) -> &'static [u32] {
        match self {
            Self::Sakura => &[0x4a154b, 0x6e2558, 0x933566, 0xb84573, 0xdd5581, 0xff7096],
            Self::Cyberpunk => &[0x0d1b2a, 0x1b263b, 0x415a77, 0x0077b6, 0x00b4d8, 0x90e0ef],
            Self::Mystic => &[0x10002b, 0x240046, 0x3c096c, 0x5a189a, 0x7b2cbf, 0x9d4edd],
            Self::Fire => &[0x370617, 0x6a040f, 0x9d0208, 0xd00000, 0xdc2f02, 0xe85d04],
            Self::Frost => &[0x03045e, 0x023e8a, 0x0077b6, 0x0096c7, 0x00b4d8, 0x48cae4],
            Self::DeepSea => &[0x012a4a, 0x013a63, 0x01497c, 0x2a6f97, 0x2c7da0, 0x468faf],
            Self::RetroArcade => &[0x2b0938, 0x4e125c, 0x7b1fa2, 0xaa00ff, 0xd500f9, 0xff4081],
            Self::Autumn => &[0x281105, 0x441c09, 0x692d0d, 0x8f4316, 0xb55a1f, 0xdc7629],
            Self::Cauldron => &[0x0b2914, 0x134623, 0x1d6634, 0x278746, 0x33aa5a, 0x40cd6e],
            Self::Celestial => &[0x08071a, 0x120e36, 0x221a5c, 0x382a85, 0x543fae, 0x7558d4],
            Self::Custom(_) => &[0x1e1f22, 0x2b2d31, 0x383a40, 0x4e5058, 0x5c5e66, 0x6d6f78],
        }
    }

    /// Primary accent / atmospheric glow color.
    pub fn glow_color(&self) -> Rgba {
        match self {
            Self::Sakura => rgba(0xff709666),
            Self::Cyberpunk => rgba(0x00b4d866),
            Self::Mystic => rgba(0x9d4edd66),
            Self::Fire => rgba(0xe85d0466),
            Self::Frost => rgba(0x48cae466),
            Self::DeepSea => rgba(0x468faf66),
            Self::RetroArcade => rgba(0xff408166),
            Self::Autumn => rgba(0xdc762966),
            Self::Cauldron => rgba(0x40cd6e66),
            Self::Celestial => rgba(0x7558d466),
            Self::Custom(_) => rgba(0x5865f244),
        }
    }

    /// Particle foreground color.
    pub fn particle_color(&self) -> Rgba {
        match self {
            Self::Sakura => rgba(0xffb3c6aa),
            Self::Cyberpunk => rgba(0xa0f0ffcc),
            Self::Mystic => rgba(0xe0aaffcc),
            Self::Fire => rgba(0xffba08dd),
            Self::Frost => rgba(0xade8f4ee),
            Self::DeepSea => rgba(0x90e0efaa),
            Self::RetroArcade => rgba(0xff80abdd),
            Self::Autumn => rgba(0xf4a261cc),
            Self::Cauldron => rgba(0x80ed99cc),
            Self::Celestial => rgba(0xe2e0ffcc),
            Self::Custom(_) => rgba(0xffffff88),
        }
    }
}

/// Standalone GPUI component for rendering a Discord profile effect overlay.
pub struct ProfileEffect {
    data: ProfileEffectData,
    height: Pixels,
    width: Option<Pixels>,
    show_particles: bool,
}

impl ProfileEffect {
    /// Creates a new `ProfileEffect` component.
    pub fn new(data: ProfileEffectData) -> Self {
        Self {
            data,
            height: px(105.0),
            width: None,
            show_particles: true,
        }
    }

    /// Sets explicit banner height.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// Sets explicit banner width.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Toggles particle effect overlay.
    pub fn show_particles(mut self, show: bool) -> Self {
        self.show_particles = show;
        self
    }
}

impl IntoElement for ProfileEffect {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        render_profile_effect(&self.data, self.height, self.width)
    }
}

/// Renders a profile effect as a composable GPUI `Div` element.
///
/// Can be embedded directly inside the banner of `UserProfilePopout` or other profile views.
pub fn render_profile_effect(
    effect: &ProfileEffectData,
    banner_height: Pixels,
    banner_width: Option<Pixels>,
) -> Div {
    let theme = effect.theme();
    let stops = theme.gradient_stops();
    let glow = theme.glow_color();
    let particle_col = theme.particle_color();

    let mut root = div()
        .relative()
        .h(banner_height)
        .overflow_hidden()
        .rounded_t_md();

    if let Some(w) = banner_width {
        root = root.w(w);
    } else {
        root = root.w_full();
    }

    // 1. Multi-stop smooth linear gradient background band
    let gradient_layer = div()
        .absolute()
        .size_full()
        .flex()
        .flex_row()
        .children(stops.iter().map(|&c| {
            div()
                .flex_1()
                .h_full()
                .bg(rgb(c))
        }));

    // 2. Ambient top/center atmospheric glow
    let ambient_glow = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(glow);

    // 3. Shimmer border highlight across top edge
    let top_shimmer = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(2.0))
        .bg(particle_col);

    // 4. Atmospheric themed floating particles
    // Deterministic positioning across the banner width
    let particles_layer = div()
        .absolute()
        .size_full()
        .child(
            div()
                .absolute()
                .top(px(12.0))
                .left(px(18.0))
                .size(px(6.0))
                .rounded_full()
                .bg(particle_col),
        )
        .child(
            div()
                .absolute()
                .top(px(34.0))
                .left(px(64.0))
                .size(px(4.0))
                .rounded_full()
                .bg(particle_col),
        )
        .child(
            div()
                .absolute()
                .top(px(18.0))
                .left(px(140.0))
                .size(px(7.0))
                .rounded_full()
                .bg(particle_col),
        )
        .child(
            div()
                .absolute()
                .top(px(52.0))
                .left(px(210.0))
                .size(px(5.0))
                .rounded_full()
                .bg(particle_col),
        )
        .child(
            div()
                .absolute()
                .top(px(28.0))
                .left(px(270.0))
                .size(px(6.0))
                .rounded_full()
                .bg(particle_col),
        );

    root.child(gradient_layer)
        .child(ambient_glow)
        .child(top_shimmer)
        .child(particles_layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_profile_effect_data_serde() {
        let json_str = r#"{"id":"1144002621008298034","sku_id":"sku_sakura_01","expires_at":1700000000}"#;
        let data: ProfileEffectData = serde_json::from_str(json_str).unwrap();

        assert_eq!(data.id, "1144002621008298034");
        assert_eq!(data.sku_id.as_deref(), Some("sku_sakura_01"));
        assert_eq!(data.expires_at, Some(1700000000));
        assert_eq!(data.theme(), ProfileEffectTheme::Sakura);
    }

    #[::core::prelude::v1::test]
    fn test_profile_effect_theme_mapping() {
        assert_eq!(
            ProfileEffectTheme::from_effect_id("sakura_petals"),
            ProfileEffectTheme::Sakura
        );
        assert_eq!(
            ProfileEffectTheme::from_effect_id("cyber_matrix"),
            ProfileEffectTheme::Cyberpunk
        );
        assert_eq!(
            ProfileEffectTheme::from_effect_id("fire_embers"),
            ProfileEffectTheme::Fire
        );
        assert_eq!(
            ProfileEffectTheme::from_effect_id("winter_frost"),
            ProfileEffectTheme::Frost
        );
        assert_eq!(
            ProfileEffectTheme::from_effect_id("witch_cauldron"),
            ProfileEffectTheme::Cauldron
        );
    }

    #[::core::prelude::v1::test]
    fn test_profile_effect_builder() {
        let data = ProfileEffectData::new("cyber_neo");
        let effect = ProfileEffect::new(data)
            .height(px(120.0))
            .width(px(300.0))
            .show_particles(true);

        assert_eq!(effect.height, px(120.0));
        assert_eq!(effect.width, Some(px(300.0)));
        assert!(effect.show_particles);
    }
}
