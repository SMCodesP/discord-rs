//! Discord RS - Design Tokens
//! Unified design tokens module re-exporting colors, typography, spacing, motion, and icons.

pub mod colors;
pub mod icons;
pub mod motion;
pub mod spacing;
pub mod typography;

pub use colors::*;
pub use icons::*;
pub use motion::*;
pub use spacing::*;
pub use typography::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_core_palette_exact_hex_values() {
        assert_eq!(HEX_BG_SERVER, 0x1e1f22);
        assert_eq!(HEX_BG_CHANNELS, 0x2b2d31);
        assert_eq!(HEX_BG_CHAT, 0x313338);
        assert_eq!(HEX_BG_CHAT_HOVER, 0x383a40);
        assert_eq!(HEX_BG_USER_BAR, 0x232428);
        assert_eq!(HEX_BG_INPUT, 0x111214);
        assert_eq!(HEX_BLURPLE, 0x5865f2);
        assert_eq!(HEX_STATUS_ONLINE, 0x23a55a);
        assert_eq!(HEX_STATUS_AUSENTE, 0xf0b232);
        assert_eq!(HEX_STATUS_DND, 0xf23f43);
        assert_eq!(HEX_TEXT_SECONDARY, 0x80848e);
        assert_eq!(HEX_TEXT_PRIMARY, 0xdbdee1);
        assert_eq!(HEX_TEXT_TITLES, 0xf2f3f5);
        assert_eq!(HEX_BORDERS, 0x4e5058);
    }

    #[::core::prelude::v1::test]
    fn test_color_functions() {
        let _ = bg_server();
        let _ = bg_channels();
        let _ = bg_chat();
        let _ = bg_chat_hover();
        let _ = bg_user_bar();
        let _ = bg_input();
        let _ = blurple();
        let _ = status_online();
        let _ = status_ausente();
        let _ = status_dnd();
        let _ = text_secondary();
        let _ = text_primary();
        let _ = text_titles();
        let _ = borders();

        let custom_alpha = with_alpha(HEX_BLURPLE, 0.5);
        assert!((custom_alpha.a - 0.5).abs() < 1e-4);
    }

    #[::core::prelude::v1::test]
    fn test_layout_widths() {
        assert_eq!(RAW_RAIL_WIDTH, 72.0);
        assert_eq!(RAW_CHANNELS_WIDTH, 240.0);
        assert_eq!(RAW_MEMBERS_WIDTH, 240.0);
    }

    #[::core::prelude::v1::test]
    fn test_all_icons() {
        assert_eq!(ALL_ICONS.len(), 51);
        for icon in ALL_ICONS {
            assert!(!icon.name().is_empty());
            assert!(!icon.path_data().is_empty());
            assert!(icon.svg_data().contains("<svg"));
            assert!(icon.svg_data().contains("viewBox=\"0 0 24 24\""));
        }
    }

    #[::core::prelude::v1::test]
    fn test_motion_tokens() {
        assert_eq!(DURATION_FAST.as_millis(), 100);
        assert_eq!(DURATION_NORMAL.as_millis(), 150);
        assert_eq!(DURATION_MODERATE.as_millis(), 200);
        assert_eq!(DURATION_SLOW.as_millis(), 300);

        // Bezier solver test
        let val_start = EASE_SERVER_ICON.solve(0.0);
        let val_end = EASE_SERVER_ICON.solve(1.0);
        assert!((val_start - 0.0).abs() < 1e-5);
        assert!((val_end - 1.0).abs() < 1e-5);
    }
}
