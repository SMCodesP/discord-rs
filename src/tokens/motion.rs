//! Discord RS - Motion & Animation Tokens
//! Comprehensive catalog of animation timings, cubic-bezier easing curves, and transition presets.

use std::time::Duration;

// ============================================================================
// Raw Millisecond Durations
// ============================================================================

pub const MS_INSTANT: u64 = 0;
pub const MS_FASTER: u64 = 50;
pub const MS_FAST: u64 = 100;
pub const MS_NORMAL: u64 = 150;
pub const MS_MODERATE: u64 = 200;
pub const MS_SLOW: u64 = 300;
pub const MS_SLOWER: u64 = 400;
pub const MS_DELIBERATE: u64 = 500;

// ============================================================================
// Duration Accessors (std::time::Duration)
// ============================================================================

/// Instant (0ms) - Immediate state updates
pub const DURATION_INSTANT: Duration = Duration::from_millis(MS_INSTANT);

/// Faster (50ms) - Very fast micro feedback (active states, press effect)
pub const DURATION_FASTER: Duration = Duration::from_millis(MS_FASTER);

/// Fast (100ms) - Micro-interactions (button hover, channel item hover)
pub const DURATION_FAST: Duration = Duration::from_millis(MS_FAST);

/// Normal (150ms) - Standard Discord transitions (server icon morph, tooltip fade)
pub const DURATION_NORMAL: Duration = Duration::from_millis(MS_NORMAL);

/// Moderate (200ms) - Server pill expansion, popup animations
pub const DURATION_MODERATE: Duration = Duration::from_millis(MS_MODERATE);

/// Slow (300ms) - Modal enters, sidebar slide-ins, accordion collapse
pub const DURATION_SLOW: Duration = Duration::from_millis(MS_SLOW);

/// Slower (400ms) - Large element repositioning
pub const DURATION_SLOWER: Duration = Duration::from_millis(MS_SLOWER);

/// Deliberate (500ms) - Hero transitions, onboarding reveals
pub const DURATION_DELIBERATE: Duration = Duration::from_millis(MS_DELIBERATE);

// ============================================================================
// Cubic Bezier Easing Curves
// ============================================================================

/// Represents a cubic bezier curve: (p1x, p1y, p2x, p2y) where p0=(0,0) and p3=(1,1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub p1x: f32,
    pub p1y: f32,
    pub p2x: f32,
    pub p2y: f32,
}

impl CubicBezier {
    pub const fn new(p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> Self {
        Self {
            p1x,
            p1y,
            p2x,
            p2y,
        }
    }

    /// Returns the CSS cubic-bezier string format
    pub fn to_css(&self) -> String {
        format!(
            "cubic-bezier({:.3}, {:.3}, {:.3}, {:.3})",
            self.p1x, self.p1y, self.p2x, self.p2y
        )
    }

    /// Evaluates y for a given progress parameter t [0.0, 1.0] using Newton-Raphson.
    pub fn solve(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }

        // Approximate parameter `u` where B_x(u) == t
        let mut u = t;
        for _ in 0..8 {
            let x = 3.0 * (1.0 - u) * (1.0 - u) * u * self.p1x
                + 3.0 * (1.0 - u) * u * u * self.p2x
                + u * u * u;
            let dx = 3.0 * (1.0 - u) * (1.0 - u) * self.p1x
                + 6.0 * (1.0 - u) * u * (self.p2x - self.p1x)
                + 3.0 * u * u * (1.0 - self.p2x);
            if dx.abs() < 1e-6 {
                break;
            }
            u -= (x - t) / dx;
            u = u.clamp(0.0, 1.0);
        }

        // Compute B_y(u)
        3.0 * (1.0 - u) * (1.0 - u) * u * self.p1y
            + 3.0 * (1.0 - u) * u * u * self.p2y
            + u * u * u
    }
}

// Standard Web / Discord curves
pub const EASE_LINEAR: CubicBezier = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
pub const EASE_DEFAULT: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
pub const EASE_IN: CubicBezier = CubicBezier::new(0.4, 0.0, 1.0, 1.0);
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.2, 1.0);
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);

// Discord Custom Bezier Curves
/// Server icon shape morph (circle to rounded rectangle): smooth acceleration and deceleration
pub const EASE_SERVER_ICON: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);

/// Server unread/selected pill transition: snappy out
pub const EASE_PILL: CubicBezier = CubicBezier::new(0.0, 0.0, 0.2, 1.0);

/// Bouncy popups and reaction badges (subtle overshoot spring)
pub const EASE_SPRING: CubicBezier = CubicBezier::new(0.175, 0.885, 0.32, 1.275);

/// Modal scale & fade enter curve
pub const EASE_MODAL_ENTER: CubicBezier = CubicBezier::new(0.18, 0.89, 0.32, 1.28);

/// Drawer / panel sliding in curve
pub const EASE_DRAWER_SLIDE: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);

// ============================================================================
// Transition Presets
// ============================================================================

/// High-level transition configuration combining timing and easing
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionPreset {
    pub duration: Duration,
    pub easing: CubicBezier,
    pub name: &'static str,
}

impl TransitionPreset {
    pub const fn new(duration: Duration, easing: CubicBezier, name: &'static str) -> Self {
        Self {
            duration,
            easing,
            name,
        }
    }
}

/// Server icon morph on hover / active (150ms ease-in-out)
pub const TRANSITION_SERVER_ICON: TransitionPreset =
    TransitionPreset::new(DURATION_NORMAL, EASE_SERVER_ICON, "server_icon_morph");

/// Server pill indicator expand / collapse (150ms ease-out)
pub const TRANSITION_SERVER_PILL: TransitionPreset =
    TransitionPreset::new(DURATION_NORMAL, EASE_PILL, "server_pill_expand");

/// Interactive button background color hover (100ms ease-default)
pub const TRANSITION_BUTTON_HOVER: TransitionPreset =
    TransitionPreset::new(DURATION_FAST, EASE_DEFAULT, "button_hover");

/// Channel item background hover (100ms ease-default)
pub const TRANSITION_CHANNEL_HOVER: TransitionPreset =
    TransitionPreset::new(DURATION_FAST, EASE_DEFAULT, "channel_hover");

/// Tooltip fade in/out (150ms ease-out)
pub const TRANSITION_TOOLTIP: TransitionPreset =
    TransitionPreset::new(DURATION_NORMAL, EASE_OUT, "tooltip_fade");

/// Modal pop-in transition (200ms spring/overshoot)
pub const TRANSITION_MODAL_ENTER: TransitionPreset =
    TransitionPreset::new(DURATION_MODERATE, EASE_MODAL_ENTER, "modal_enter");

/// Emoji / reaction button pop (200ms spring)
pub const TRANSITION_REACTION_POP: TransitionPreset =
    TransitionPreset::new(DURATION_MODERATE, EASE_SPRING, "reaction_pop");

/// Sidebar or drawer slide transition (300ms smooth drawer slide)
pub const TRANSITION_SIDEBAR_SLIDE: TransitionPreset =
    TransitionPreset::new(DURATION_SLOW, EASE_DRAWER_SLIDE, "sidebar_slide");
