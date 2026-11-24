//! Canonical ordering for guild channels, categories and roles.
//!
//! Discord exposes `position` for channels/categories/roles and a snowflake
//! `id` for every entity. The API does not guarantee that positions are
//! unique, so every comparison here is total and deterministic: `position`
//! first and the snowflake `id` as the tie-breaker.
//!
//! * Channels and categories are ordered by ascending `position`, then
//!   ascending [`Channel`]/[`Category`] id.
//! * Roles use the opposite direction because the highest role is the most
//!   powerful one: descending `position`, then descending role id (a newer
//!   role wins over an older role at the same position).

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::domain::channel::{Channel, ChannelType};
use crate::domain::guild::{Category, Role};

/// Compares two Discord snowflakes.
///
/// Snowflakes are decimal integers that grow over time. Non-numeric ids are
/// accepted for synthetic fixtures and compared lexicographically so the
/// ordering remains total.
pub fn compare_snowflakes(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

/// Canonical channel order: ascending `position`, then ascending channel id.
pub fn compare_channels(a: &Channel, b: &Channel) -> Ordering {
    a.position
        .cmp(&b.position)
        .then_with(|| compare_snowflakes(&a.id, &b.id))
}

/// Sorts channels in place using the canonical Discord order.
pub fn sort_channels(channels: &mut [Channel]) {
    channels.sort_by(compare_channels);
}

/// Returns a copy of `channels` in the canonical Discord order.
pub fn sorted_channels(channels: &[Channel]) -> Vec<Channel> {
    let mut ordered = channels.to_vec();
    sort_channels(&mut ordered);
    ordered
}

/// Canonical category order: ascending `position`, then ascending category id.
pub fn compare_categories(a: &Category, b: &Category) -> Ordering {
    a.position
        .cmp(&b.position)
        .then_with(|| compare_snowflakes(&a.id, &b.id))
}

/// Sorts categories in place using the canonical Discord order.
pub fn sort_categories(categories: &mut [Category]) {
    categories.sort_by(compare_categories);
}

/// Returns a copy of `categories` in the canonical Discord order.
pub fn sorted_categories(categories: &[Category]) -> Vec<Category> {
    let mut ordered = categories.to_vec();
    sort_categories(&mut ordered);
    ordered
}

/// Compares two roles by hierarchy power.
///
/// Returns [`Ordering::Greater`] when `a` outranks `b`. A role with a higher
/// `position` outranks a lower one; at equal positions the newer (higher
/// snowflake id) role outranks the older one.
pub fn compare_role_rank(a: &Role, b: &Role) -> Ordering {
    a.position
        .cmp(&b.position)
        .then_with(|| compare_snowflakes(&a.id, &b.id))
}

/// Sorts roles from highest rank to lowest rank.
pub fn sort_roles(roles: &mut [Role]) {
    roles.sort_by(|a, b| compare_role_rank(b, a));
}

/// Returns a copy of `roles` from highest rank to lowest rank.
pub fn sorted_roles(roles: &[Role]) -> Vec<Role> {
    let mut ordered = roles.to_vec();
    sort_roles(&mut ordered);
    ordered
}

/// Returns the highest-ranked role, if any.
pub fn highest_role(roles: &[Role]) -> Option<&Role> {
    roles.iter().max_by(|a, b| compare_role_rank(a, b))
}

/// A group of channels displayed under a category header.
///
/// `category` is `None` for the real, unlabeled top-level group: channels
/// without a parent category are never assigned an invented category name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelGroup {
    /// The real category header, or `None` for the uncategorized group.
    pub category: Option<Category>,
    /// Channels belonging to this group, in canonical order.
    pub channels: Vec<Channel>,
}

impl ChannelGroup {
    /// Category id, or `None` when this is the uncategorized group.
    pub fn id(&self) -> Option<&str> {
        self.category.as_ref().map(|category| category.id.as_str())
    }

    /// Real category name, or `None` when no header should be rendered.
    pub fn name(&self) -> Option<&str> {
        self.category.as_ref().map(|category| category.name.as_str())
    }

    /// Whether this is the unlabeled top-level group.
    pub fn is_uncategorized(&self) -> bool {
        self.category.is_none()
    }
}

/// Orders channels into the categories shown by Discord.
///
/// * The unlabeled group of channels without a parent comes first, and only
///   exists when it has channels.
/// * Every real category is preserved, including empty ones, ordered by
///   [`compare_categories`].
/// * Channels whose `category_id` does not match a real category are treated
///   as uncategorized instead of being renamed to a synthetic "OTHER" group.
/// * Category header channels (`ChannelType::Category`) are not returned as
///   members; they become categories when not already present in
///   `categories`.
pub fn order_channel_groups(categories: &[Category], channels: &[Channel]) -> Vec<ChannelGroup> {
    let mut category_map: HashMap<String, Category> = HashMap::new();

    for category in categories {
        category_map
            .entry(category.id.clone())
            .or_insert_with(|| category.clone());
    }

    for channel in channels
        .iter()
        .filter(|channel| channel.channel_type == ChannelType::Category)
    {
        category_map.entry(channel.id.clone()).or_insert_with(|| Category {
            id: channel.id.clone(),
            name: channel.name.clone(),
            position: channel.position,
        });
    }

    let mut uncategorized = Vec::new();
    let mut grouped: HashMap<String, Vec<Channel>> = HashMap::new();

    for channel in channels {
        if channel.channel_type == ChannelType::Category {
            continue;
        }

        match channel.category_id.as_deref() {
            Some(category_id) if category_map.contains_key(category_id) => {
                grouped
                    .entry(category_id.to_string())
                    .or_default()
                    .push(channel.clone());
            }
            _ => uncategorized.push(channel.clone()),
        }
    }

    sort_channels(&mut uncategorized);
    for list in grouped.values_mut() {
        sort_channels(list);
    }

    let mut ordered_categories: Vec<Category> = category_map.into_values().collect();
    sort_categories(&mut ordered_categories);

    let mut groups = Vec::with_capacity(ordered_categories.len().saturating_add(1));

    if !uncategorized.is_empty() {
        groups.push(ChannelGroup {
            category: None,
            channels: uncategorized,
        });
    }

    for category in ordered_categories {
        let channels = grouped.remove(&category.id).unwrap_or_default();
        groups.push(ChannelGroup {
            category: Some(category),
            channels,
        });
    }

    groups
}

/// Orders channels into groups using only the category header channels.
///
/// This is a convenience wrapper for payloads where categories are represented
/// as [`ChannelType::Category`] entries inside the channel list.
pub fn order_channel_groups_from_channels(channels: &[Channel]) -> Vec<ChannelGroup> {
    order_channel_groups(&[], channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(id: &str, name: &str, position: i32) -> Channel {
        Channel::text(id, name).with_position(position)
    }

    fn category(id: &str, name: &str, position: i32) -> Category {
        Category::new(id, name, position)
    }

    #[test]
    fn channels_sort_by_position_then_id() {
        let mut channels = vec![
            channel("30", "third", 2),
            channel("10", "first", 1),
            channel("20", "second", 1),
        ];

        sort_channels(&mut channels);

        let ids: Vec<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
        assert_eq!(ids, vec!["10", "20", "30"]);
    }

    #[test]
    fn categories_sort_by_position_then_id() {
        let mut categories = vec![
            category("300", "C", 20),
            category("100", "A", 10),
            category("200", "B", 10),
        ];

        sort_categories(&mut categories);

        let ids: Vec<&str> = categories
            .iter()
            .map(|category| category.id.as_str())
            .collect();
        assert_eq!(ids, vec!["100", "200", "300"]);
    }

    #[test]
    fn roles_sort_by_descending_rank_with_id_tie_breaker() {
        let mut roles = vec![
            Role::new("10", "Old Mod").with_position(10),
            Role::new("30", "Admin").with_position(30),
            Role::new("20", "New Mod").with_position(10),
            Role::new("40", "Helper").with_position(-1),
        ];

        sort_roles(&mut roles);

        let names: Vec<&str> = roles.iter().map(|role| role.name.as_str()).collect();
        assert_eq!(names, vec!["Admin", "New Mod", "Old Mod", "Helper"]);
    }

    #[test]
    fn highest_role_uses_position_and_id() {
        let roles = vec![
            Role::new("10", "Old Mod").with_position(10),
            Role::new("20", "New Mod").with_position(10),
            Role::new("30", "Admin").with_position(20),
        ];

        assert_eq!(highest_role(&roles).map(|role| role.name.as_str()), Some("Admin"));

        let tied = vec![
            Role::new("10", "Old Mod").with_position(10),
            Role::new("20", "New Mod").with_position(10),
        ];
        assert_eq!(highest_role(&tied).map(|role| role.name.as_str()), Some("New Mod"));
    }

    #[test]
    fn uncategorized_channels_have_no_invented_name() {
        let channels = vec![
            channel("10", "rules", 1),
            channel("20", "general", 2),
        ];

        let groups = order_channel_groups(&[], &channels);

        assert_eq!(groups.len(), 1);
        assert!(groups[0].is_uncategorized());
        assert_eq!(groups[0].name(), None);
        let names: Vec<&str> = groups[0]
            .channels
            .iter()
            .map(|channel| channel.name.as_str())
            .collect();
        assert_eq!(names, vec!["rules", "general"]);
    }

    #[test]
    fn real_categories_keep_names_and_order_even_when_empty() {
        let categories = vec![
            category("300", "Voice", 20),
            category("100", "Text", 10),
            category("200", "Empty", 15),
        ];
        let channels = vec![
            Channel::text("410", "voice-lounge")
                .with_position(2)
                .with_category_id("300"),
            Channel::text("420", "voice-stage")
                .with_position(1)
                .with_category_id("300"),
            Channel::text("310", "general")
                .with_position(2)
                .with_category_id("100"),
            Channel::text("320", "announcements")
                .with_position(1)
                .with_category_id("100"),
        ];

        let groups = order_channel_groups(&categories, &channels);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].name(), Some("Text"));
        assert_eq!(groups[1].name(), Some("Empty"));
        assert_eq!(groups[2].name(), Some("Voice"));
        assert!(groups[1].channels.is_empty());

        let text_names: Vec<&str> = groups[0]
            .channels
            .iter()
            .map(|channel| channel.name.as_str())
            .collect();
        assert_eq!(text_names, vec!["announcements", "general"]);

        let voice_names: Vec<&str> = groups[2]
            .channels
            .iter()
            .map(|channel| channel.name.as_str())
            .collect();
        assert_eq!(voice_names, vec!["voice-stage", "voice-lounge"]);
    }

    #[test]
    fn orphan_and_uncategorized_channels_share_unlabeled_group() {
        let categories = vec![category("100", "Text", 10)];
        let channels = vec![
            Channel::text("500", "orphan")
                .with_position(1)
                .with_category_id("999"),
            channel("600", "top", 0),
            Channel::text("310", "general")
                .with_position(1)
                .with_category_id("100"),
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
        assert_eq!(groups[1].name(), Some("Text"));
    }

    #[test]
    fn category_header_channels_are_merged_and_not_listed_as_members() {
        let channels = vec![
            Channel::category("100", "Text").with_position(10),
            Channel::text("200", "general")
                .with_position(1)
                .with_category_id("100"),
            Channel::text("300", "lobby")
                .with_position(2)
                .with_category_id("400"),
            Channel::category("400", "Voice").with_position(20),
        ];

        let groups = order_channel_groups_from_channels(&channels);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name(), Some("Text"));
        assert_eq!(groups[0].channels.len(), 1);
        assert_eq!(groups[0].channels[0].name, "general");
        assert_eq!(groups[1].name(), Some("Voice"));
        assert_eq!(groups[1].channels.len(), 1);
        assert_eq!(groups[1].channels[0].name, "lobby");
    }

    #[test]
    fn shuffled_payload_is_deterministic() {
        let categories = vec![
            category("200", "B", 10),
            category("100", "A", 10),
            category("300", "C", 5),
        ];
        let channels = vec![
            Channel::text("900", "b2").with_position(1).with_category_id("200"),
            Channel::text("800", "b1").with_position(1).with_category_id("200"),
            Channel::text("700", "a2").with_position(2).with_category_id("100"),
            Channel::text("600", "a1").with_position(2).with_category_id("100"),
            Channel::text("500", "c1").with_position(1).with_category_id("300"),
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
}
