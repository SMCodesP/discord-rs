//! Chat blocked-user message grouping.
//!
//! A blocked group is a maximal run of adjacent messages written by the same
//! blocked author in the same channel. The original [`Message`] values are
//! never removed or rewritten: the stream stores their indices and the caller
//! decides whether the group is collapsed or expanded.
//!
//! Expansion is viewer state, keyed by channel and author. Collapsing a group
//! only hides its indices from [`visible_message_indices`]; the message cache
//! remains intact.

use gpui::prelude::*;
use gpui::*;
use std::collections::HashSet;
use std::sync::Arc;

use crate::domain::message::Message;
use crate::i18n::Locale;
use crate::tokens::colors::{bg_chat_hover, blurple, text_muted};
use crate::tokens::spacing::{space_2, space_4, space_8};
use crate::tokens::typography::{font_size_sm, line_height_sm, WEIGHT_MEDIUM};

/// Callback fired when the viewer activates "Show messages" / "Collapse messages".
pub type BlockedGroupToggleCallback =
    Arc<dyn Fn(&BlockedGroupKey, &mut Window, &mut App) + Send + Sync + 'static>;

/// Identifies one expandable blocked group inside a channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockedGroupKey {
    pub channel_id: String,
    pub author_id: String,
}

impl BlockedGroupKey {
    pub fn new(channel_id: impl Into<String>, author_id: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            author_id: author_id.into(),
        }
    }
}

/// Consecutive messages from a single blocked author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedMessageGroup {
    pub key: BlockedGroupKey,
    /// Display name read from the real message author; never synthesized.
    pub author_name: String,
    /// Indices into the message slice used to build the stream.
    pub message_indices: Vec<usize>,
}

impl BlockedMessageGroup {
    pub fn count(&self) -> usize {
        self.message_indices.len()
    }

    pub fn first_index(&self) -> Option<usize> {
        self.message_indices.first().copied()
    }

    pub fn last_index(&self) -> Option<usize> {
        self.message_indices.last().copied()
    }

    pub fn first_message<'a>(&self, messages: &'a [Message]) -> Option<&'a Message> {
        self.first_index().and_then(|index| messages.get(index))
    }

    pub fn message_ids<'a>(&self, messages: &'a [Message]) -> Vec<&'a str> {
        self.message_indices
            .iter()
            .filter_map(|index| messages.get(*index))
            .map(|message| message.id.as_str())
            .collect()
    }
}

/// Flattened chat stream where blocked runs are replaced by a single group item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockedStreamItem {
    Message { message_index: usize },
    BlockedGroup { group_index: usize },
}

/// Result of partitioning a channel history by blocked authors.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockedStream {
    pub items: Vec<BlockedStreamItem>,
    pub groups: Vec<BlockedMessageGroup>,
    pub blocked_message_count: usize,
}

impl BlockedStream {
    /// Partitions `messages` without mutating or cloning them.
    ///
    /// A group is broken by any visible message, by a channel change and by a
    /// different author. Only IDs present in `blocked_user_ids` are hidden.
    pub fn build(messages: &[Message], blocked_user_ids: &HashSet<String>) -> Self {
        let mut items = Vec::with_capacity(messages.len());
        let mut groups = Vec::new();
        let mut blocked_message_count = 0;
        let mut index = 0;

        while index < messages.len() {
            let message = &messages[index];

            if !blocked_user_ids.contains(&message.author.id) {
                items.push(BlockedStreamItem::Message {
                    message_index: index,
                });
                index += 1;
                continue;
            }

            let author_id = message.author.id.clone();
            let channel_id = message.channel_id.clone();
            let author_name = message.author.display_name().to_string();
            let mut message_indices = vec![index];
            index += 1;

            while index < messages.len() {
                let next = &messages[index];
                if next.author.id != author_id
                    || next.channel_id != channel_id
                    || !blocked_user_ids.contains(&next.author.id)
                {
                    break;
                }
                message_indices.push(index);
                index += 1;
            }

            blocked_message_count += message_indices.len();
            let group_index = groups.len();
            groups.push(BlockedMessageGroup {
                key: BlockedGroupKey::new(channel_id, author_id),
                author_name,
                message_indices,
            });
            items.push(BlockedStreamItem::BlockedGroup { group_index });
        }

        Self {
            items,
            groups,
            blocked_message_count,
        }
    }

    pub fn group(&self, group_index: usize) -> Option<&BlockedMessageGroup> {
        self.groups.get(group_index)
    }

    pub fn group_for_key(&self, key: &BlockedGroupKey) -> Option<&BlockedMessageGroup> {
        self.groups.iter().find(|group| &group.key == key)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Viewer expansion state for blocked groups.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockedGroupsState {
    blocked_user_ids: HashSet<String>,
    expanded: HashSet<BlockedGroupKey>,
}

impl BlockedGroupsState {
    pub fn new(blocked_user_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            blocked_user_ids: blocked_user_ids.into_iter().collect(),
            expanded: HashSet::new(),
        }
    }

    pub fn blocked_user_ids(&self) -> &HashSet<String> {
        &self.blocked_user_ids
    }

    pub fn is_user_blocked(&self, user_id: &str) -> bool {
        self.blocked_user_ids.contains(user_id)
    }

    pub fn is_expanded(&self, key: &BlockedGroupKey) -> bool {
        self.expanded.contains(key)
    }

    pub fn expand(&mut self, key: BlockedGroupKey) -> bool {
        self.expanded.insert(key)
    }

    pub fn collapse(&mut self, key: &BlockedGroupKey) -> bool {
        self.expanded.remove(key)
    }

    /// Toggles visibility and returns the new expanded state.
    pub fn toggle(&mut self, key: BlockedGroupKey) -> bool {
        if self.expanded.remove(&key) {
            false
        } else {
            self.expanded.insert(key);
            true
        }
    }

    /// Applies a real relationship update.
    ///
    /// Blocking hides/removes any previous expansion for that author so content
    /// is not exposed before the viewer asks for it. Unblocking removes the
    /// author from the blocked set and clears its expansion state. Returns the
    /// expansion keys that were discarded.
    pub fn apply_user_relationship(&mut self, user_id: &str, blocked: bool) -> Vec<BlockedGroupKey> {
        let removed = self.remove_expansions_for_author(user_id);
        if blocked {
            self.blocked_user_ids.insert(user_id.to_string());
        } else {
            self.blocked_user_ids.remove(user_id);
        }
        removed
    }

    /// Replaces the blocked set with the latest relationship snapshot.
    ///
    /// Expansions for authors that are no longer blocked are discarded. Returns
    /// the discarded keys so the caller can refresh the affected channel.
    pub fn sync_blocked_users(
        &mut self,
        blocked_user_ids: impl IntoIterator<Item = String>,
    ) -> Vec<BlockedGroupKey> {
        let next: HashSet<String> = blocked_user_ids.into_iter().collect();
        let stale_authors: HashSet<String> = self
            .blocked_user_ids
            .difference(&next)
            .cloned()
            .collect();
        self.blocked_user_ids = next;

        let discarded: Vec<BlockedGroupKey> = self
            .expanded
            .iter()
            .filter(|key| stale_authors.contains(&key.author_id))
            .cloned()
            .collect();
        self.expanded
            .retain(|key| !stale_authors.contains(&key.author_id));
        discarded
    }

    /// Collapses every blocked group in one channel. Returns how many were open.
    pub fn collapse_channel(&mut self, channel_id: &str) -> usize {
        let before = self.expanded.len();
        self.expanded.retain(|key| key.channel_id != channel_id);
        before - self.expanded.len()
    }

    pub fn clear(&mut self) {
        self.expanded.clear();
    }

    fn remove_expansions_for_author(&mut self, user_id: &str) -> Vec<BlockedGroupKey> {
        let affected: Vec<BlockedGroupKey> = self
            .expanded
            .iter()
            .filter(|key| key.author_id == user_id)
            .cloned()
            .collect();
        for key in &affected {
            self.expanded.remove(key);
        }
        affected
    }
}

/// Message indices that should be rendered for the supplied expansion state.
///
/// Collapsed blocked groups contribute nothing. Expanded groups contribute the
/// exact original indices, preserving order and content.
pub fn visible_message_indices(
    stream: &BlockedStream,
    state: &BlockedGroupsState,
) -> Vec<usize> {
    let mut visible = Vec::new();

    for item in &stream.items {
        match item {
            BlockedStreamItem::Message { message_index } => visible.push(*message_index),
            BlockedStreamItem::BlockedGroup { group_index } => {
                if let Some(group) = stream.group(*group_index)
                    && state.is_expanded(&group.key)
                {
                    visible.extend(group.message_indices.iter().copied());
                }
            }
        }
    }

    visible
}

/// Localized blocked-group labels, sourced from Discord's client strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockedGroupLocale {
    #[default]
    PtBr,
    EnUs,
}

impl From<Locale> for BlockedGroupLocale {
    fn from(locale: Locale) -> Self {
        match locale {
            Locale::PtBr => Self::PtBr,
            Locale::EnUs => Self::EnUs,
        }
    }
}

impl BlockedGroupLocale {
    pub fn current() -> Self {
        Self::from(crate::i18n::current_locale())
    }

    pub fn count_label(&self, count: usize) -> String {
        match self {
            Self::PtBr => match count {
                1 => "1 Mensagem bloqueada".to_string(),
                count => format!("{} Mensagens bloqueadas", count),
            },
            Self::EnUs => match count {
                1 => "1 Blocked Message".to_string(),
                count => format!("{} Blocked Messages", count),
            },
        }
    }

    pub fn action_label(&self, expanded: bool) -> &'static str {
        match (self, expanded) {
            (Self::PtBr, false) => "Mostrar mensagens",
            (Self::PtBr, true) => "Recolher mensagens",
            (Self::EnUs, false) => "Show messages",
            (Self::EnUs, true) => "Collapse messages",
        }
    }
}

/// Renders the collapsed/expanded notice for one blocked group.
///
/// The raw messages are rendered by the caller using the indices returned by
/// [`visible_message_indices`]. This header never invents content or a count.
pub fn render_blocked_group_notice(
    group: &BlockedMessageGroup,
    expanded: bool,
    locale: BlockedGroupLocale,
    on_toggle: Option<BlockedGroupToggleCallback>,
) -> Div {
    let label = locale.count_label(group.count());
    let action_label = locale.action_label(expanded);

    let action_id = format!(
        "blocked-group-action-{}-{}",
        group.key.channel_id, group.key.author_id
    );
    let mut action = div()
        .id(ElementId::Name(action_id.into()))
        .flex_shrink_0()
        .invisible()
        .group_hover("blocked-group-row", |style| style.visible())
        .cursor_pointer()
        .text_size(font_size_sm())
        .line_height(line_height_sm())
        .font_weight(WEIGHT_MEDIUM)
        .text_color(blurple())
        .child(action_label);

    if let Some(handler) = on_toggle {
        let key = group.key.clone();
        action = action.on_click(move |_event, window, cx| handler(&key, window, cx));
    }

    div()
        .group("blocked-group-row")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .min_w(px(0.0))
        .py(px(2.0))
        .pl(space_8())
        .pr(space_4())
        .gap(space_2())
        .hover(|style| style.bg(bg_chat_hover()))
        .child(
            div()
                .min_w(px(0.0))
                .text_size(font_size_sm())
                .line_height(line_height_sm())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_muted())
                .child(label),
        )
        .child(action)
}
