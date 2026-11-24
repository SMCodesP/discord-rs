use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::guild::{Category, Role};
use discord_rs::domain::hierarchy::{
    compare_categories, compare_channels, compare_role_rank, compare_snowflakes, highest_role,
    order_channel_groups, order_channel_groups_from_channels, sort_categories, sort_channels,
    sort_roles, sorted_categories, sorted_channels, sorted_roles, ChannelGroup,
};
use std::cmp::Ordering;

fn text(id: &str, name: &str, position: i32, category_id: Option<&str>) -> Channel {
    let mut channel = Channel::text(id, name).with_position(position);
    if let Some(category_id) = category_id {
        channel = channel.with_category_id(category_id);
    }
    channel
}

fn category(id: &str, name: &str, position: i32) -> Category {
    Category::new(id, name, position)
}

fn role(id: &str, name: &str, position: i32) -> Role {
    Role::new(id, name).with_position(position)
}

#[test]
fn channel_order_is_position_then_snowflake_id() {
    let mut channels = vec![
        text("900", "third", 3, None),
        text("200", "first-b", 1, None),
        text("100", "first-a", 1, None),
        text("800", "second", 2, None),
    ];

    sort_channels(&mut channels);

    let ids: Vec<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
    assert_eq!(ids, vec!["100", "200", "800", "900"]);
    assert_eq!(compare_channels(&channels[0], &channels[1]), Ordering::Less);
    assert_eq!(compare_channels(&channels[1], &channels[0]), Ordering::Greater);
}

#[test]
fn category_order_is_position_then_snowflake_id() {
    let mut categories = vec![
        category("900", "Last", 30),
        category("200", "Tie B", 10),
        category("100", "Tie A", 10),
    ];

    sort_categories(&mut categories);

    let ids: Vec<&str> = categories
        .iter()
        .map(|category| category.id.as_str())
        .collect();
    assert_eq!(ids, vec!["100", "200", "900"]);
    assert_eq!(compare_categories(&categories[0], &categories[1]), Ordering::Less);
}

#[test]
fn role_rank_is_descending_position_then_newer_id() {
    let mut roles = vec![
        role("100", "Older Mod", 10),
        role("200", "Newer Mod", 10),
        role("300", "Admin", 20),
        role("050", "Helper", 5),
    ];

    sort_roles(&mut roles);

    let names: Vec<&str> = roles.iter().map(|role| role.name.as_str()).collect();
    assert_eq!(names, vec!["Admin", "Newer Mod", "Older Mod", "Helper"]);
    assert_eq!(compare_role_rank(&roles[0], &roles[1]), Ordering::Greater);
    assert_eq!(highest_role(&roles).map(|role| role.name.as_str()), Some("Admin"));
}

#[test]
fn sorted_helpers_do_not_mutate_inputs() {
    let channels = vec![text("300", "c", 3, None), text("100", "a", 1, None)];
    let categories = vec![category("300", "C", 3), category("100", "A", 1)];
    let roles = vec![role("100", "Low", 1), role("200", "High", 2)];

    let ordered_channels = sorted_channels(&channels);
    let ordered_categories = sorted_categories(&categories);
    let ordered_roles = sorted_roles(&roles);

    assert_eq!(channels[0].id, "300");
    assert_eq!(categories[0].id, "300");
    assert_eq!(roles[0].id, "100");
    assert_eq!(ordered_channels[0].id, "100");
    assert_eq!(ordered_categories[0].id, "100");
    assert_eq!(ordered_roles[0].id, "200");
}

#[test]
fn uncategorized_channels_form_a_real_unlabeled_group() {
    let channels = vec![
        text("300", "rules", 3, None),
        text("100", "general", 1, None),
        text("200", "help", 2, None),
    ];

    let groups = order_channel_groups(&[], &channels);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0], ChannelGroup {
        category: None,
        channels: sorted_channels(&channels),
    });
    assert!(groups[0].is_uncategorized());
    assert_eq!(groups[0].id(), None);
    assert_eq!(groups[0].name(), None);
}

#[test]
fn empty_real_categories_are_preserved_in_official_order() {
    let categories = vec![
        category("400", "Voice", 20),
        category("100", "Text", 10),
        category("300", "Empty", 15),
        category("200", "Announcements", 10),
    ];
    let channels = vec![
        text("510", "voice-lounge", 2, Some("400")),
        text("500", "voice-stage", 1, Some("400")),
        text("210", "general", 2, Some("100")),
        text("200", "welcome", 1, Some("100")),
    ];

    let groups = order_channel_groups(&categories, &channels);

    let names: Vec<&str> = groups.iter().map(|group| group.name().unwrap()).collect();
    assert_eq!(names, vec!["Text", "Announcements", "Empty", "Voice"]);
    assert!(groups[2].channels.is_empty());

    let text_names: Vec<&str> = groups[0]
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(text_names, vec!["welcome", "general"]);
}

#[test]
fn orphan_channels_never_receive_invented_category_names() {
    let categories = vec![category("100", "Text", 10)];
    let channels = vec![
        text("500", "orphan", 1, Some("999")),
        text("400", "top", 0, None),
        text("210", "general", 1, Some("100")),
    ];

    let groups = order_channel_groups(&categories, &channels);

    assert_eq!(groups.len(), 2);
    assert!(groups[0].is_uncategorized());
    let names: Vec<&str> = groups[0]
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(names, vec!["top", "orphan"]);

    for group in &groups {
        if let Some(name) = group.name() {
            assert_ne!(name, "GERAL");
            assert_ne!(name, "OUTROS");
        }
    }
}

#[test]
fn category_header_channels_are_not_rendered_as_members() {
    let channels = vec![
        Channel::category("100", "TEXT CHANNELS").with_position(10),
        Channel::category("300", "VOICE CHANNELS").with_position(20),
        text("200", "general", 1, Some("100")),
        text("400", "lobby", 1, Some("300")),
    ];

    let groups = order_channel_groups_from_channels(&channels);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name(), Some("TEXT CHANNELS"));
    assert_eq!(groups[1].name(), Some("VOICE CHANNELS"));
    assert_eq!(groups[0].channels.len(), 1);
    assert_eq!(groups[1].channels.len(), 1);
    assert!(groups
        .iter()
        .flat_map(|group| group.channels.iter())
        .all(|channel| channel.channel_type != ChannelType::Category));
}

#[test]
fn shuffled_payload_produces_identical_order() {
    let categories = vec![
        category("300", "C", 5),
        category("100", "A", 10),
        category("200", "B", 10),
    ];
    let channels = vec![
        text("900", "b2", 1, Some("200")),
        text("800", "b1", 1, Some("200")),
        text("700", "a2", 2, Some("100")),
        text("600", "a1", 2, Some("100")),
        text("500", "c1", 1, Some("300")),
    ];

    let groups = order_channel_groups(&categories, &channels);

    let mut reversed_categories = categories.clone();
    reversed_categories.reverse();
    let mut reversed_channels = channels.clone();
    reversed_channels.reverse();
    let reversed_groups = order_channel_groups(&reversed_categories, &reversed_channels);

    assert_eq!(groups, reversed_groups);
    let names: Vec<&str> = groups
        .iter()
        .map(|group| group.name().unwrap_or("<uncategorized>"))
        .collect();
    assert_eq!(names, vec!["C", "A", "B"]);
    assert_eq!(groups[1].channels[0].name, "a1");
    assert_eq!(groups[2].channels[0].name, "b1");
}

#[test]
fn snowflake_comparison_handles_numeric_and_synthetic_ids() {
    assert_eq!(compare_snowflakes("9", "10"), Ordering::Less);
    assert_eq!(compare_snowflakes("10", "9"), Ordering::Greater);
    assert_eq!(compare_snowflakes("alpha", "beta"), Ordering::Less);
    assert_eq!(compare_snowflakes("synthetic", "synthetic"), Ordering::Equal);
}
