# Discord RS - Motion Catalog

## Overview

Motion in Discord creates a responsive, tactile, and fluid desktop experience. Every transition serves an informational and navigational purpose without adding artificial latency.

All motion tokens, easing curves, and presets are defined in [`src/tokens/motion.rs`](file:///home/smcodes/personal/discord-rs/src/tokens/motion.rs).

---

## 1. Timing Scale (Durations)

Discord's interaction model prioritizes responsiveness. Actions under `150ms` feel instantaneous; transitions between `150ms` and `300ms` convey spatial continuity.

| Token | Milliseconds | Duration Type | Typical Interaction |
|---|---|---|---|
| `DURATION_INSTANT` | `0ms` | Immediate | Instant state toggle, cursor change |
| `DURATION_FASTER` | `50ms` | Ultra-fast | Pressed/active feedback on buttons |
| `DURATION_FAST` | `100ms` | Fast | Channel item hover, button background color fade |
| `DURATION_NORMAL` | `150ms` | Standard | Server icon corner-radius morph, tooltip fade |
| `DURATION_MODERATE` | `200ms` | Moderate | Server indicator pill expansion, popup open |
| `DURATION_SLOW` | `300ms` | Deliberate | Sidebars sliding in/out, modal scale & enter |
| `DURATION_SLOWER` | `400ms` | Extended | Large container resizing, view switching |
| `DURATION_DELIBERATE`| `500ms` | Highlight | Onboarding cards, banner reveals |

---

## 2. Easing Curves (Cubic Bezier)

### Standard Curves

| Name | Constant | Bezier Control Points | CSS Equivalent |
|---|---|---|---|
| Linear | `EASE_LINEAR` | `(0.00, 0.00, 1.00, 1.00)` | `linear` |
| Default | `EASE_DEFAULT`| `(0.25, 0.10, 0.25, 1.00)` | `cubic-bezier(0.25, 0.1, 0.25, 1)` |
| Ease In | `EASE_IN` | `(0.40, 0.00, 1.00, 1.00)` | `cubic-bezier(0.4, 0, 1, 1)` |
| Ease Out | `EASE_OUT` | `(0.00, 0.00, 0.20, 1.00)` | `cubic-bezier(0, 0, 0.2, 1)` |
| Ease In Out | `EASE_IN_OUT`| `(0.40, 0.00, 0.20, 1.00)` | `cubic-bezier(0.4, 0, 0.2, 1)` |

### Discord Signature Curves

#### 1. Server Icon Squircle Morph (`EASE_SERVER_ICON`)
- **Control Points**: `(0.40, 0.00, 0.20, 1.00)`
- **Duration**: `150ms`
- **Behavior**: Smooth acceleration with rapid deceleration into the target squircle radius (from 24px circle to 16px rounded rectangle).

#### 2. Indicator Pill Expansion (`EASE_PILL`)
- **Control Points**: `(0.00, 0.00, 0.20, 1.00)`
- **Duration**: `150ms`
- **Behavior**: Snappy ease-out expansion from 8px unread pip to 20px hover pill or 40px active selection.

#### 3. Bouncy Spring (`EASE_SPRING`)
- **Control Points**: `(0.175, 0.885, 0.32, 1.275)`
- **Duration**: `200ms`
- **Behavior**: Subtle overshoot curve used for emoji reaction clicks, badge pops, and celebratory micro-interactions.

#### 4. Modal Dialog Scale (`EASE_MODAL_ENTER`)
- **Control Points**: `(0.18, 0.89, 0.32, 1.28)`
- **Duration**: `200ms`
- **Behavior**: Fast scale up from 95% to 102% before settling at 100%, producing a crisp, organic pop.

#### 5. Drawer & Sidebar Slide (`EASE_DRAWER_SLIDE`)
- **Control Points**: `(0.22, 1.00, 0.36, 1.00)`
- **Duration**: `300ms`
- **Behavior**: Decelerating slide-in for member list toggles and settings drawers.

---

## 3. Motion Choreography & Presets

Pre-packaged transition configurations available in `src/tokens/motion.rs`:

```rust
use discord_rs::tokens::*;

// Apply predefined transitions
let icon_preset = TRANSITION_SERVER_ICON;     // 150ms, ease-in-out
let pill_preset = TRANSITION_SERVER_PILL;     // 150ms, ease-out
let button_preset = TRANSITION_BUTTON_HOVER;  // 100ms, ease-default
let modal_preset = TRANSITION_MODAL_ENTER;    // 200ms, modal spring
let reaction_preset = TRANSITION_REACTION_POP;// 200ms, overshoot spring
```

### Motion Recipe Matrix

| Component | State Trigger | Animated Property | From $\rightarrow$ To | Preset |
|---|---|---|---|---|
| **Server Icon** | Hover / Focus | `border-radius` | `24px` $\rightarrow$ `16px` | `TRANSITION_SERVER_ICON` |
| **Server Icon** | Hover | `background-color` | `#313338` $\rightarrow$ `#5865f2` | `TRANSITION_SERVER_ICON` |
| **Server Pill** | Hover | `height` | `8px` $\rightarrow$ `20px` | `TRANSITION_SERVER_PILL` |
| **Server Pill** | Selected | `height` | `20px` $\rightarrow$ `40px` | `TRANSITION_SERVER_PILL` |
| **Channel Item**| Hover | `background-color` | `transparent` $\rightarrow$ `#35373c` | `TRANSITION_CHANNEL_HOVER` |
| **Channel Item**| Active | `background-color` | `#35373c` $\rightarrow$ `#3f4248` | `TRANSITION_CHANNEL_HOVER` |
| **Button** | Hover | `background-color` | `#5865f2` $\rightarrow$ `#4752c4` | `TRANSITION_BUTTON_HOVER` |
| **Button** | Active / Press | `background-color` | `#4752c4` $\rightarrow$ `#3c45a5` | `DURATION_FASTER` (50ms) |
| **Tooltip** | Hover delay (300ms) | `opacity` & `scale` | `0.0 (95%)` $\rightarrow$ `1.0 (100%)` | `TRANSITION_TOOLTIP` |
| **Modal** | Open | `opacity` & `scale` | `0.0 (90%)` $\rightarrow$ `1.0 (100%)` | `TRANSITION_MODAL_ENTER` |

---

## 4. GPUI Math & Curve Solver

The `CubicBezier` struct includes an analytic Newton-Raphson approximation solver:

```rust
use discord_rs::tokens::{CubicBezier, EASE_SPRING};

let progress = 0.5; // Halfway through the animation
let eased_value = EASE_SPRING.solve(progress);
```

This can be used within GPUI render loops, custom animated elements, or tick-driven timers to compute interpolated bounds, offsets, and alpha levels.
