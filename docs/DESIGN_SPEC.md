# Discord RS - Design Specification

## Overview

**Discord RS** is a high-performance native desktop client built with [GPUI](https://gpui.rs) in Rust. This document outlines the official Design System specifications, tokens, layout metrics, typography hierarchy, iconography, and internationalization guidelines aligned with Discord's Dark Theme.

---

## 1. Color System (Discord Dark Theme)

All colors are defined in [`src/tokens/colors.rs`](file:///home/smcodes/personal/discord-rs/src/tokens/colors.rs). Colors return `gpui::Rgba` instances via `gpui::rgb` and `gpui::rgba`.

### Core Palette

| Token Function | Constant | Hex Code | Semantic Role |
|---|---|---|---|
| `bg_server()` | `HEX_BG_SERVER` | `#1e1f22` | Server navigation rail background |
| `bg_channels()` | `HEX_BG_CHANNELS` | `#2b2d31` | Channels and categories sidebar |
| `bg_chat()` | `HEX_BG_CHAT` | `#313338` | Main message area and viewport |
| `bg_chat_hover()` | `HEX_BG_CHAT_HOVER` | `#383a40` | Message row hover highlight |
| `bg_user_bar()` | `HEX_BG_USER_BAR` | `#232428` | Bottom user profile and voice control panel |
| `bg_input()` | `HEX_BG_INPUT` | `#111214` | Message input textarea, toolbars, popouts |
| `blurple()` | `HEX_BLURPLE` | `#5865f2` | Primary brand color, action buttons, active pill |
| `status_online()` | `HEX_STATUS_ONLINE` | `#23a55a` | Online presence status indicator |
| `status_ausente()` | `HEX_STATUS_AUSENTE` | `#f0b232` | Idle / Away presence indicator |
| `status_dnd()` | `HEX_STATUS_DND` | `#f23f43` | Do Not Disturb presence indicator |
| `text_secondary()` | `HEX_TEXT_SECONDARY` | `#80848e` | Secondary/muted labels, timestamps, icons |
| `text_primary()` | `HEX_TEXT_PRIMARY` | `#dbdee1` | Standard message text, member names, inputs |
| `text_titles()` | `HEX_TEXT_TITLES` | `#f2f3f5` | Headers, active channels, title bars |
| `borders()` | `HEX_BORDERS` | `#4e5058` | Dividers, card borders, subtle separators |

### Extended Palette

| Token Function | Constant | Hex Code | Description |
|---|---|---|---|
| `blurple_hover()` | `HEX_BLURPLE_HOVER` | `#4752c4` | Hover state for blurple buttons |
| `blurple_active()` | `HEX_BLURPLE_ACTIVE` | `#3c45a5` | Pressed/active state for blurple buttons |
| `channel_hover()` | `HEX_CHANNEL_HOVER` | `#35373c` | Channel item hover background |
| `channel_active()` | `HEX_CHANNEL_ACTIVE` | `#3f4248` | Selected channel item background |
| `mention_bg()` | `HEX_MENTION_BG` | `#5865f226` | @mention message background tint |
| `modal_backdrop()` | `HEX_MODAL_BACKDROP` | `#000000cc` | Dimmed overlay for modals and dialogs |

### GPUI Usage Example

```rust
use discord_rs::tokens::*;
use gpui::*;

fn render_message_row() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .bg(bg_chat())
        .hover(|s| s.bg(bg_chat_hover()))
        .text_color(text_primary())
        .border_b_1()
        .border_color(borders())
}
```

---

## 2. Typography Scale

Defined in [`src/tokens/typography.rs`](file:///home/smcodes/personal/discord-rs/src/tokens/typography.rs).

### Font Families
- **Primary Stack**: `gg sans, Noto Sans, Helvetica Neue, Helvetica, Arial, sans-serif`
- **Monospace Stack**: `Consolas, Andale Mono, Monaco, Courier New, monospace`

### Font Sizes

| Token | Pixels | Use Case |
|---|---|---|
| `font_size_xxs()` | `10px` | Notification badge count, micro indicators |
| `font_size_xs()` | `12px` | Category headers, timestamps, subtext, tooltips |
| `font_size_sm()` | `14px` | Channel items, user panel subtext, compact buttons |
| `font_size_base()` / `font_size_md()` | `16px` | Message body text, input fields, dropdown items |
| `font_size_lg()` | `18px` | Channel topic, section titles |
| `font_size_xl()` | `20px` | Server header, modal titles |
| `font_size_2xl()` | `24px` | Large dialog headings, welcome channel titles |
| `font_size_3xl()` | `32px` | Hero headings, landing banner titles |

### Font Weights
- `WEIGHT_LIGHT`: `300`
- `WEIGHT_NORMAL`: `400` (Body messages, input text)
- `WEIGHT_MEDIUM`: `500` (Usernames, channel names, timestamps)
- `WEIGHT_SEMIBOLD`: `600` (User profile names, buttons, modal titles)
- `WEIGHT_BOLD`: `700` (Category headers, server names, title bars)
- `WEIGHT_EXTRA_BOLD`: `800` (Accent callouts)

### Line Heights
- `line_height_tight()`: `14px`
- `line_height_xs()`: `16px`
- `line_height_sm()`: `18px`
- `line_height_base()` / `line_height_md()`: `22px` (standard 1.375rem for 16px text)
- `line_height_lg()`: `24px`
- `line_height_xl()`: `26px`
- `line_height_2xl()`: `30px`
- `line_height_3xl()`: `38px`

---

## 3. Spacing, Layout & Radiuses

Defined in [`src/tokens/spacing.rs`](file:///home/smcodes/personal/discord-rs/src/tokens/spacing.rs).

### Layout Column Widths & Heights

| Token | Dimension | Description |
|---|---|---|
| `rail_width()` | `72px` | Server navigation rail width |
| `channels_width()` | `240px` | Channel list sidebar width |
| `members_width()` | `240px` | Server members sidebar width |
| `header_height()` | `48px` | Top channel title & actions header |
| `user_bar_height()` | `52px` | Bottom user profile, mute/deafen controls |
| `chat_input_height()` | `44px` | Minimum height for chat message input box |
| `server_icon_size()` | `48px` | Default server icon square dimension |

### Spacing Scale

| Token | Value | Standard Use |
|---|---|---|
| `space_1()` | `2px` | Indicator margin, pill gap |
| `space_2()` | `4px` | Badge padding, micro item gap |
| `space_3()` | `6px` | Compact button vertical padding |
| `space_4()` | `8px` | Standard element gap, button padding |
| `space_5()` | `10px` | Channel item vertical padding |
| `space_6()` | `12px` | Server icon gap, container padding |
| `space_8()` | `16px` | Chat gutter horizontal margin, card padding |
| `space_10()` | `20px` | Large margins |
| `space_12()` | `24px` | Dialog container padding |
| `space_16()` | `32px` | Section margins |

### Border Radiuses

| Token | Value | Application |
|---|---|---|
| `radius_none()` | `0px` | Squared edges |
| `radius_xs()` | `2px` | Tiny status indicators |
| `radius_sm()` | `4px` | Notification tags, small badges |
| `radius_md()` | `8px` | Discord buttons, text inputs, channel item hover box |
| `radius_lg()` | `12px` | Modals, context menus, large popouts |
| `radius_server_hover()`| `16px` | Server icon squircle shape on hover / active |
| `radius_server_default()`| `24px` | Server icon resting circle shape (48px / 2) |
| `radius_pill()` | `20px` | Pill badges, tag chips |
| `radius_full()` | `9999px` | Circular avatars, status pips |

---

## 4. Iconography

Defined in [`src/tokens/icons.rs`](file:///home/smcodes/personal/discord-rs/src/tokens/icons.rs).

All icons are built on a standard `24x24` viewBox with `fill="currentColor"`.

| Icon Identifier | Enum Variant | Constant | Functionality |
|---|---|---|---|
| `hash` | `DiscordIcon::Hash` | `SVG_HASH` | Text channels |
| `speaker` | `DiscordIcon::Speaker` | `SVG_SPEAKER` | Voice channels / audio output |
| `megaphone` | `DiscordIcon::Megaphone` | `SVG_MEGAPHONE` | Announcement channels |
| `gear` | `DiscordIcon::Gear` | `SVG_GEAR` | Settings and preferences |
| `mic` | `DiscordIcon::Mic` | `SVG_MIC` | Microphone toggle (mute/unmute) |
| `headphones` | `DiscordIcon::Headphones` | `SVG_HEADPHONES` | Headphone toggle (deafen/undeafen) |
| `pin` | `DiscordIcon::Pin` | `SVG_PIN` | Pinned messages panel toggle |
| `search` | `DiscordIcon::Search` | `SVG_SEARCH` | Channel & server search |
| `inbox` | `DiscordIcon::Inbox` | `SVG_INBOX` | Mentions and unread inbox |
| `members` | `DiscordIcon::Members` | `SVG_MEMBERS` | Member list sidebar toggle |
| `gift` | `DiscordIcon::Gift` | `SVG_GIFT` | Nitro gifting action |
| `gif` | `DiscordIcon::Gif` | `SVG_GIF` | GIF picker modal opener |
| `sticker` | `DiscordIcon::Sticker` | `SVG_STICKER` | Sticker drawer toggle |
| `emoji` | `DiscordIcon::Emoji` | `SVG_EMOJI` | Emoji drawer toggle |
| `plus` | `DiscordIcon::Plus` | `SVG_PLUS` | Add server / upload file attachment |
| `compass` | `DiscordIcon::Compass` | `SVG_COMPASS` | Server discovery directory |

---

## 5. Internationalization (i18n)

Located in [`src/i18n/`](file:///home/smcodes/personal/discord-rs/src/i18n/).

- **Default Locale**: Brazilian Portuguese (`pt-BR`).
- **Fallback Locale**: English US (`en-US`).
- **Translation API**:
  - `t("status.online")` -> `"Online"`
  - `t_with("chat.placeholder", &[("channel", "geral")])` -> `"Conversar em #geral"`
  - `t_fmt("members.online_count", &[("count", "12")])` -> `"ONLINE — 12"`
  - `t!("chat.placeholder", channel = "geral")` -> Macro syntax

---

## Verification & Parity

To verify the tokens against the codebase:

```bash
cargo check
cargo test
```
