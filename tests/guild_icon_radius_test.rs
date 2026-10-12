//! Tests for guild icon rounding, folder previews, and guild rail dimensions.
//! Parity reference: ~/.cache/discord-rs-parity/discord/guild-rail.md

use discord_rs::components::guild_item::{GuildFolderChild, GuildFolderItem, GuildItem};
use discord_rs::tokens::colors::*;
use discord_rs::tokens::spacing::*;
use gpui_kit::px;

#[test]
fn test_guild_icon_and_rail_tokens() {
    // Parity with Discord DOM measurements:
    // Rail width is exactly 72px (nav[aria-label] rect 72x718)
    assert_eq!(RAW_RAIL_WIDTH, 72.0);
    assert_eq!(rail_width(), px(72.0));

    // Guild icon squircle corner radius in Discord visual-refresh is 16px (--radius-lg)
    assert_eq!(RAW_RADIUS_SERVER_HOVER, 16.0);
    assert_eq!(radius_server_hover(), px(16.0));
}

#[test]
fn test_guild_item_builder_states() {
    // 1. Guild item in rest state with initials
    let rest_item = GuildItem::new("guild-1", "Comunidade Rust")
        .selected(false)
        .unread(false);
    assert_eq!(rest_item.get_name(), "Comunidade Rust");
    assert_eq!(rest_item.get_initials(), "CR");
    assert!(!rest_item.is_selected());
    assert!(!rest_item.is_unread());
    assert_eq!(rest_item.get_icon_url(), None);

    // 2. Guild item selected with image URL
    let selected_item = GuildItem::new("guild-2", "Aqua Scans")
        .selected(true)
        .unread(true)
        .mention_count(3)
        .icon_url("https://cdn.discordapp.com/icons/123/abc.webp");
    assert_eq!(selected_item.get_name(), "Aqua Scans");
    assert!(selected_item.is_selected());
    assert!(selected_item.is_unread());
    assert_eq!(selected_item.get_mention_count(), 3);
    assert_eq!(
        selected_item.get_icon_url(),
        Some("https://cdn.discordapp.com/icons/123/abc.webp")
    );

    // 3. Guild item with local file path (test / fixture icon)
    let local_icon_item = GuildItem::new("guild-3", "Local Test")
        .icon_url("/tmp/guild_icon_test.png");
    assert_eq!(
        local_icon_item.get_icon_url(),
        Some("/tmp/guild_icon_test.png")
    );

    // 4. Guild item with file:// URL
    let file_uri_item = GuildItem::new("guild-4", "File URI Test")
        .icon_url("file:///tmp/guild_icon_test.png");
    assert_eq!(
        file_uri_item.get_icon_url(),
        Some("file:///tmp/guild_icon_test.png")
    );
}

#[test]
fn test_guild_initials_computation() {
    assert_eq!(GuildItem::compute_initials("Rust Lang"), "RL");
    assert_eq!(GuildItem::compute_initials("Open Source Community"), "OSC");
    assert_eq!(GuildItem::compute_initials("Single"), "Sin");
    assert_eq!(GuildItem::compute_initials(""), "?");
}

#[test]
fn test_guild_folder_collapsed_and_expanded() {
    let child_1 = GuildFolderChild::new("c1", "Alpha")
        .icon_url("https://cdn.discordapp.com/icons/c1/icon.png")
        .mention_count(2)
        .unread(true);

    let child_2 = GuildFolderChild::new("c2", "Beta")
        .selected(true)
        .mention_count(4);

    let child_3 = GuildFolderChild::new("c3", "Gamma")
        .icon_url("/tmp/test_gamma.png");

    let child_4 = GuildFolderChild::new("c4", "Delta");

    // Collapsed folder
    let collapsed_folder = GuildFolderItem::new("f1", "Jogos")
        .color(blurple())
        .expanded(false)
        .child_guild(child_1.clone())
        .child_guild(child_2.clone())
        .child_guild(child_3.clone())
        .child_guild(child_4.clone());

    assert_eq!(collapsed_folder.get_name(), "Jogos");
    assert!(!collapsed_folder.is_expanded());
    assert_eq!(collapsed_folder.get_guilds().len(), 4);
    assert!(collapsed_folder.is_unread());
    assert_eq!(collapsed_folder.get_mention_count(), 6);

    // Expanded folder
    let expanded_folder = GuildFolderItem::new("f1", "Jogos")
        .color(blurple())
        .expanded(true)
        .child_guild(child_1)
        .child_guild(child_2);

    assert!(expanded_folder.is_expanded());
    assert_eq!(expanded_folder.get_guilds().len(), 2);
}
