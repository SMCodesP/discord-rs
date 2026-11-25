//! Memoized view ordering caches keyed by generation counters.
//!
//! These caches avoid recomputing category/channel ordering and member-by-role
//! grouping on every render when the underlying data generation has not changed.

use crate::app::state::{CategoryItemState, ChannelItemState, MemberItemState, PresenceStatus};

/// Generation-keyed memo slot. Recomputes only when `generation` changes.
#[derive(Debug, Clone)]
pub struct GenerationMemo<T> {
    generation: u64,
    value: Option<T>,
}

impl<T> Default for GenerationMemo<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            value: None,
        }
    }
}

impl<T> GenerationMemo<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get(&self, generation: u64) -> Option<&T> {
        if self.value.is_some() && self.generation == generation {
            self.value.as_ref()
        } else {
            None
        }
    }

    pub fn get_or_compute_with<F>(&mut self, generation: u64, compute: F) -> &T
    where
        F: FnOnce() -> T,
    {
        let needs_recompute = self.value.is_none() || self.generation != generation;
        if needs_recompute {
            self.value = Some(compute());
            self.generation = generation;
        }
        self.value.as_ref().expect("memo value just computed")
    }

    pub fn invalidate(&mut self) {
        self.value = None;
    }
}

/// Ordered category/channel snapshot for view rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedCategory {
    pub id: String,
    pub name: String,
    pub channels: Vec<ChannelItemState>,
}

impl From<&CategoryItemState> for OrderedCategory {
    fn from(category: &CategoryItemState) -> Self {
        Self {
            id: category.id.clone(),
            name: category.name.clone(),
            channels: category.channels.clone(),
        }
    }
}

/// Memoized ordering for guild categories and their channels.
///
/// Ordering rule: categories keep their source order (already position-sorted by
/// domain hierarchy), channels within each category are ordered by
/// (kind rank, name lowercase, id) for deterministic display.
#[derive(Debug, Clone, Default)]
pub struct CategoryChannelMemo {
    slot: GenerationMemo<Vec<OrderedCategory>>,
}

impl CategoryChannelMemo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.slot.generation()
    }

    pub fn get(&self, generation: u64) -> Option<&[OrderedCategory]> {
        self.slot.get(generation).map(Vec::as_slice)
    }

    pub fn get_or_compute(
        &mut self,
        generation: u64,
        categories: &[CategoryItemState],
    ) -> &[OrderedCategory] {
        self.slot
            .get_or_compute_with(generation, || order_categories(categories))
            .as_slice()
    }

    pub fn invalidate(&mut self) {
        self.slot.invalidate();
    }
}

fn channel_kind_rank(kind: &crate::app::state::ChannelKind) -> u8 {
    use crate::app::state::ChannelKind;
    match kind {
        ChannelKind::Text => 0,
        ChannelKind::Announcement => 1,
        ChannelKind::Forum => 2,
        ChannelKind::Directory => 3,
        ChannelKind::AnnouncementThread => 4,
        ChannelKind::PublicThread => 5,
        ChannelKind::PrivateThread => 6,
        ChannelKind::Voice => 7,
        ChannelKind::StageVoice => 8,
    }
}

pub fn order_categories(categories: &[CategoryItemState]) -> Vec<OrderedCategory> {
    categories
        .iter()
        .map(|category| {
            let mut channels = category.channels.clone();
            channels.sort_by(|a, b| {
                channel_kind_rank(&a.kind)
                    .cmp(&channel_kind_rank(&b.kind))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    .then_with(|| a.id.cmp(&b.id))
            });
            OrderedCategory {
                id: category.id.clone(),
                name: category.name.clone(),
                channels,
            }
        })
        .collect()
}

/// A role-grouped member section for sidebar rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRoleGroup {
    pub role_group: String,
    pub members: Vec<MemberItemState>,
}

/// Memoized member grouping/sorting by role.
///
/// Ordering rule: online/non-offline groups preserve first-seen role group order;
/// members inside a group are sorted by (name lowercase, id). Offline members are
/// grouped last under "OFFLINE", also sorted by (name lowercase, id).
#[derive(Debug, Clone, Default)]
pub struct MemberRoleMemo {
    slot: GenerationMemo<Vec<MemberRoleGroup>>,
}

impl MemberRoleMemo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.slot.generation()
    }

    pub fn get(&self, generation: u64) -> Option<&[MemberRoleGroup]> {
        self.slot.get(generation).map(Vec::as_slice)
    }

    pub fn get_or_compute(
        &mut self,
        generation: u64,
        members: &[MemberItemState],
    ) -> &[MemberRoleGroup] {
        self.slot
            .get_or_compute_with(generation, || group_members_by_role(members))
            .as_slice()
    }

    pub fn invalidate(&mut self) {
        self.slot.invalidate();
    }
}

pub fn group_members_by_role(members: &[MemberItemState]) -> Vec<MemberRoleGroup> {
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<MemberItemState>> =
        std::collections::HashMap::new();
    let mut offline: Vec<MemberItemState> = Vec::new();

    for member in members {
        if member.is_unloaded() {
            continue;
        }
        if member.presence == PresenceStatus::Offline {
            offline.push(member.clone());
            continue;
        }

        let role_group = if member.role_group.trim().is_empty() {
            if member.is_bot {
                "BOTS".to_string()
            } else {
                "DISPONÍVEL".to_string()
            }
        } else {
            member.role_group.clone()
        };

        if !groups.contains_key(&role_group) {
            group_order.push(role_group.clone());
        }
        groups.entry(role_group).or_default().push(member.clone());
    }

    let mut result: Vec<MemberRoleGroup> = Vec::with_capacity(group_order.len() + 1);
    for role_group in group_order {
        if let Some(mut group_members) = groups.remove(&role_group) {
            sort_members(&mut group_members);
            result.push(MemberRoleGroup {
                role_group,
                members: group_members,
            });
        }
    }

    if !offline.is_empty() {
        sort_members(&mut offline);
        result.push(MemberRoleGroup {
            role_group: "OFFLINE".to_string(),
            members: offline,
        });
    }

    result
}

fn sort_members(members: &mut [MemberItemState]) {
    members.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}
