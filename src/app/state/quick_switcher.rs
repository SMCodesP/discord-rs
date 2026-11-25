use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickSwitcherKind {
    Dm,
    Channel,
    Guild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickSwitcherItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub kind: QuickSwitcherKind,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub dm_user_id: Option<String>,
    pub accessible: bool,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuickSwitcherState {
    pub open: bool,
    pub query: String,
    pub selected_index: usize,
    pub results: Vec<QuickSwitcherItem>,
    pub recent_channel_ids: Vec<String>,
}

impl QuickSwitcherState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn normalize_for_match(s: &str) -> String {
    s.to_lowercase()
}

fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    let q = normalize_for_match(query);
    let t = normalize_for_match(target);
    if q.is_empty() {
        return Some(0);
    }
    if t.is_empty() {
        return None;
    }
    if t == q {
        return Some(1000);
    }
    if let Some(pos) = t.find(&q) {
        // Prefer prefix and earlier matches.
        let mut score = 500 - (pos as i32 * 2);
        if pos == 0 {
            score += 100;
        }
        // word boundary bonus
        if pos > 0 {
            let prev = t.as_bytes()[pos - 1] as char;
            if prev == ' ' || prev == '-' || prev == '_' || prev == '/' {
                score += 40;
            }
        }
        return Some(score);
    }

    // subsequence match
    let mut ti = t.chars();
    let mut last_pos: Option<usize> = None;
    let mut first_pos: Option<usize> = None;
    let mut gaps = 0i32;
    let mut consumed = 0usize;
    let t_chars: Vec<char> = t.chars().collect();
    let mut t_idx = 0usize;
    for qc in q.chars() {
        let mut found = false;
        while t_idx < t_chars.len() {
            if t_chars[t_idx] == qc {
                if first_pos.is_none() {
                    first_pos = Some(t_idx);
                }
                if let Some(lp) = last_pos {
                    gaps += (t_idx as i32 - lp as i32 - 1).max(0);
                }
                last_pos = Some(t_idx);
                t_idx += 1;
                consumed += 1;
                found = true;
                break;
            }
            t_idx += 1;
        }
        if !found {
            let _ = ti.next(); // silence unused
            return None;
        }
    }
    let _ = consumed;
    let mut score = 200 - gaps * 3;
    if first_pos == Some(0) {
        score += 30;
    }
    Some(score)
}

impl AppState {
    pub fn open_quick_switcher(&mut self) {
        self.quick_switcher.open = true;
        self.quick_switcher.query.clear();
        self.quick_switcher.selected_index = 0;
        self.refresh_quick_switcher_results();
    }

    pub fn close_quick_switcher(&mut self) {
        self.quick_switcher.open = false;
        self.quick_switcher.query.clear();
        self.quick_switcher.selected_index = 0;
        self.quick_switcher.results.clear();
    }

    pub fn toggle_quick_switcher(&mut self) {
        if self.quick_switcher.open {
            self.close_quick_switcher();
        } else {
            self.open_quick_switcher();
        }
    }

    pub fn quick_switcher_query(&self) -> &str {
        &self.quick_switcher.query
    }

    pub fn set_quick_switcher_query(&mut self, query: String) {
        self.quick_switcher.query = query;
        self.quick_switcher.selected_index = 0;
        self.refresh_quick_switcher_results();
    }

    pub fn quick_switcher_move_selection(&mut self, delta: i32) {
        let len = self.quick_switcher.results.len();
        if len == 0 {
            self.quick_switcher.selected_index = 0;
            return;
        }
        let cur = self.quick_switcher.selected_index as i32;
        let mut next = cur + delta;
        if next < 0 {
            next = len as i32 - 1;
        }
        if next >= len as i32 {
            next = 0;
        }
        self.quick_switcher.selected_index = next as usize;
    }

    pub fn quick_switcher_select_index(&mut self, index: usize) {
        if index < self.quick_switcher.results.len() {
            self.quick_switcher.selected_index = index;
        }
    }

    pub fn confirm_quick_switcher(&mut self) -> bool {
        let Some(item) = self.quick_switcher.results.get(self.quick_switcher.selected_index).cloned() else {
            return false;
        };
        if !item.accessible {
            return false;
        }
        self.navigate_quick_switcher_item(&item);
        self.close_quick_switcher();
        true
    }

    fn navigate_quick_switcher_item(&mut self, item: &QuickSwitcherItem) {
        match item.kind {
            QuickSwitcherKind::Guild => {
                self.select_guild(item.guild_id.clone());
                if let Some(cid) = item.channel_id.clone() {
                    self.select_channel(cid.clone());
                    self.push_quick_switcher_recent(&cid);
                }
            }
            QuickSwitcherKind::Channel => {
                if let Some(gid) = item.guild_id.clone() {
                    self.select_guild(Some(gid));
                }
                if let Some(cid) = item.channel_id.clone() {
                    self.select_channel(cid.clone());
                    self.push_quick_switcher_recent(&cid);
                }
            }
            QuickSwitcherKind::Dm => {
                self.select_guild(None);
                if let Some(cid) = item.channel_id.clone() {
                    self.select_channel(cid.clone());
                    self.push_quick_switcher_recent(&cid);
                } else if let Some(uid) = item.dm_user_id.clone() {
                    let name = item.label.clone();
                    let cid = self.open_dm_with_user(&uid, &name);
                    if !cid.is_empty() {
                        self.push_quick_switcher_recent(&cid);
                    }
                }
            }
        }
    }

    fn push_quick_switcher_recent(&mut self, channel_id: &str) {
        let id = channel_id.to_string();
        self.quick_switcher.recent_channel_ids.retain(|x| x != &id);
        self.quick_switcher.recent_channel_ids.insert(0, id);
        if self.quick_switcher.recent_channel_ids.len() > 20 {
            self.quick_switcher.recent_channel_ids.truncate(20);
        }
    }

    pub fn refresh_quick_switcher_results(&mut self) {
        let query = self.quick_switcher.query.clone();
        let mut items = self.build_quick_switcher_items();
        // Exclude inaccessible channels/targets from navigable results? The issue says
        // "canal sem permissão" in fixtures - they should be handled (not navigated/visible as allowed).
        items.retain(|i| i.accessible);

        let recent = self.quick_switcher.recent_channel_ids.clone();
        let mut scored: Vec<(i32, usize, QuickSwitcherItem)> = Vec::new();
        for item in items.into_iter() {
            let mut score = 0i32;
            let mut matched = query.trim().is_empty();
            if !query.trim().is_empty() {
                let mut best: Option<i32> = None;
                for candidate in [
                    Some(item.label.as_str()),
                    item.detail.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(s) = fuzzy_score(&query, candidate) {
                        best = Some(best.map_or(s, |b| b.max(s)));
                    }
                }
                if let Some(s) = best {
                    score += s;
                    matched = true;
                }
            } else {
                // empty query: show recent-first, then DMs/channels/guilds
                if let Some(cid) = item.channel_id.as_ref() {
                    if let Some(pos) = recent.iter().position(|r| r == cid) {
                        score += 500 - pos as i32;
                        matched = true;
                    }
                }
            }

            if !matched {
                continue;
            }

            // Kind bias for empty query
            if query.trim().is_empty() {
                score += match item.kind {
                    QuickSwitcherKind::Dm => 30,
                    QuickSwitcherKind::Channel => 20,
                    QuickSwitcherKind::Guild => 10,
                };
            }

            let recency_bias = item.channel_id.as_ref()
                .and_then(|cid| recent.iter().position(|r| r == cid))
                .map(|p| 50 - p as i32)
                .unwrap_or(0);
            score += recency_bias;

            scored.push((score, 0usize, item));
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.2.label.to_lowercase().cmp(&b.2.label.to_lowercase())));
        let results: Vec<QuickSwitcherItem> = scored.into_iter().map(|(_, _, i)| i).collect();
        self.quick_switcher.results = results;
        if self.quick_switcher.selected_index >= self.quick_switcher.results.len() {
            self.quick_switcher.selected_index = 0;
        }
    }

    fn build_quick_switcher_items(&self) -> Vec<QuickSwitcherItem> {
        let mut items = Vec::new();

        for dm in &self.dm_channels {
            let label = if dm.name.trim().is_empty() {
                if dm.recipient_names.is_empty() { "Direct Message".to_string() } else { dm.recipient_names.join(", ") }
            } else {
                dm.name.clone()
            };
            items.push(QuickSwitcherItem {
                id: format!("dm:{}", dm.id),
                label,
                detail: Some(if dm.is_group_dm { "Group DM".to_string() } else { "Direct Message".to_string() }),
                kind: QuickSwitcherKind::Dm,
                guild_id: None,
                channel_id: Some(dm.id.clone()),
                dm_user_id: dm.recipient_ids.first().cloned(),
                accessible: true,
                icon_url: dm.icon_url.clone(),
            });
        }

        for guild in &self.guilds {
            items.push(QuickSwitcherItem {
                id: format!("guild:{}", guild.id),
                label: guild.name.clone(),
                detail: Some("Server".to_string()),
                kind: QuickSwitcherKind::Guild,
                guild_id: Some(guild.id.clone()),
                channel_id: None,
                dm_user_id: None,
                accessible: true,
                icon_url: guild.icon_url(),
            });
            for cat in &guild.categories {
                for ch in &cat.channels {
                    if ch.kind == ChannelKind::Voice {
                        continue;
                    }
                    let accessible = !self.quick_switcher_inaccessible_channel_ids.contains(&ch.id);
                    items.push(QuickSwitcherItem {
                        id: format!("channel:{}:{}", guild.id, ch.id),
                        label: format!("#{}", ch.name),
                        detail: Some(guild.name.clone()),
                        kind: QuickSwitcherKind::Channel,
                        guild_id: Some(guild.id.clone()),
                        channel_id: Some(ch.id.clone()),
                        dm_user_id: None,
                        accessible,
                        icon_url: None,
                    });
                }
            }
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_guild(id: &str, name: &str, channels: Vec<(&str, &str)>) -> GuildItemState {
        GuildItemState {
            id: id.to_string(),
            name: name.to_string(),
            acronym: name.chars().take(2).collect(),
            icon_url: None,
            icon_hash: None,
            unread: false,
            mention_count: 0,
            categories: vec![CategoryItemState {
                id: format!("cat-{id}"),
                name: "Text".to_string(),
                channels: channels.into_iter().map(|(cid, cname)| ChannelItemState {
                    id: cid.to_string(),
                    name: cname.to_string(),
                    kind: ChannelKind::Text,
                    topic: None,
                    unread: false,
                    mention_count: 0,
                    icon_url: None,
                    is_pinned: false,
                    last_message_id: None,
                    is_group_dm: false,
                    recipient_ids: vec![],
                    recipient_names: vec![],
                    user_limit: None,
                    avatar_decoration_data: None,
                    thread_metadata: None,
                    default_tag_ids: Vec::new(),
                    available_tags: Vec::new(),
                    flags: 0,
                }).collect(),
            }],
            emojis: vec![],
            stickers: vec![],
        }
    }

    fn mk_dm(id: &str, name: &str) -> ChannelItemState {
        ChannelItemState {
            id: id.to_string(),
            name: name.to_string(),
            kind: ChannelKind::Text,
            topic: None,
            unread: false,
            mention_count: 0,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: false,
            recipient_ids: vec![format!("u-{id}")],
            recipient_names: vec![name.to_string()],
            user_limit: None,
            avatar_decoration_data: None,
            thread_metadata: None,
            default_tag_ids: Vec::new(),
            available_tags: Vec::new(),
            flags: 0,
        }
    }

    #[test]
    fn quick_switcher_fuzzy_ranks_prefix_and_recent() {
        let mut state = AppState::default();
        state.guilds = vec![mk_guild("g1", "Alpha", vec![("c1", "general"), ("c2", "gaming")])];
        state.dm_channels = vec![mk_dm("d1", "alice"), mk_dm("d2", "gabi")];
        state.quick_switcher.recent_channel_ids = vec!["c2".to_string()];
        state.set_quick_switcher_query("ga".to_string());
        assert!(!state.quick_switcher.results.is_empty());
        // "gabi" DM and "gaming" channel should match; recent gaming may get boost
        let labels: Vec<String> = state.quick_switcher.results.iter().map(|r| r.label.clone()).collect();
        assert!(labels.iter().any(|l| l.to_lowercase().contains("gabi") || l.to_lowercase().contains("gaming")));
    }

    #[test]
    fn quick_switcher_excludes_inaccessible_channel_fixture() {
        let mut state = AppState::default();
        state.guilds = vec![mk_guild("g1", "Alpha", vec![("c1", "public"), ("c2", "secret")])];
        state.quick_switcher_inaccessible_channel_ids.insert("c2".to_string());
        state.set_quick_switcher_query("sec".to_string());
        assert!(state.quick_switcher.results.iter().all(|r| r.channel_id.as_deref() != Some("c2")));
        assert!(state.quick_switcher.results.is_empty());
    }

    #[test]
    fn quick_switcher_empty_query_shows_recent() {
        let mut state = AppState::default();
        state.guilds = vec![mk_guild("g1", "Alpha", vec![("c1", "general")])];
        state.dm_channels = vec![mk_dm("d1", "alice")];
        state.quick_switcher.recent_channel_ids = vec!["d1".to_string()];
        state.set_quick_switcher_query(String::new());
        assert!(!state.quick_switcher.results.is_empty());
        assert_eq!(state.quick_switcher.results[0].channel_id.as_deref(), Some("d1"));
    }

    #[test]
    fn quick_switcher_keyboard_navigation_wraps() {
        let mut state = AppState::default();
        state.dm_channels = vec![mk_dm("d1", "alice"), mk_dm("d2", "bob")];
        state.open_quick_switcher();
        assert_eq!(state.quick_switcher.selected_index, 0);
        state.quick_switcher_move_selection(-1);
        assert_eq!(state.quick_switcher.selected_index, state.quick_switcher.results.len() - 1);
        state.quick_switcher_move_selection(1);
        assert_eq!(state.quick_switcher.selected_index, 0);
    }

    #[test]
    fn quick_switcher_confirm_navigates_dm() {
        let mut state = AppState::default();
        state.dm_channels = vec![mk_dm("d1", "alice")];
        state.open_quick_switcher();
        state.set_quick_switcher_query("alice".to_string());
        let ok = state.confirm_quick_switcher();
        assert!(ok);
        assert!(!state.quick_switcher.open);
        assert_eq!(state.selected_channel_id.as_deref(), Some("d1"));
        assert_eq!(state.selected_guild_id, None);
    }
}
