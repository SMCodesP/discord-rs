use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use super::*;
use crate::chat::search::{SearchResultGroup, SearchSortOrder};

// =========================================================================
// Search Engine & Search Flyout Subsystem
// =========================================================================

#[derive(Debug, Clone, Default)]
pub struct SearchSessionMetadata {
    pub total_results: u64,
    pub offset: u32,
    pub sort_order: SearchSortOrder,
    pub error: Option<String>,
    pub is_indexing: bool,
    pub retry_after: Option<f64>,
}

impl SearchSessionMetadata {
    fn new() -> Self {
        Self {
            total_results: 0,
            offset: 0,
            sort_order: SearchSortOrder::Desc,
            error: None,
            is_indexing: false,
            retry_after: None,
        }
    }
}

static SEARCH_METADATA: LazyLock<RwLock<HashMap<usize, SearchSessionMetadata>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

impl AppState {
    #[inline]
    fn state_key(&self) -> usize {
        self as *const Self as usize
    }

    /// Opens the search flyout panel
    pub fn open_search(&mut self) {
        self.is_search_open = true;
        if !self.search_query.is_empty() {
            let q = self.search_query.clone();
            self.execute_search(&q);
        }
    }

    /// Closes the search flyout panel
    pub fn close_search(&mut self) {
        self.is_search_open = false;
    }

    /// Toggles the search flyout panel
    pub fn toggle_search(&mut self) {
        if self.is_search_open {
            self.close_search();
        } else {
            self.open_search();
        }
    }

    /// Total matching results count returned by the Discord API.
    /// Falls back to `search_results.len()` if no API count was recorded.
    pub fn search_total_results(&self) -> u64 {
        let key = self.state_key();
        let map = SEARCH_METADATA.read().unwrap();
        if let Some(meta) = map.get(&key) {
            if meta.total_results > 0 {
                return meta.total_results;
            }
        }
        self.search_results.len() as u64
    }

    /// Current pagination offset (0, 25, 50...)
    pub fn search_offset(&self) -> u32 {
        let key = self.state_key();
        SEARCH_METADATA.read().unwrap().get(&key).map(|m| m.offset).unwrap_or(0)
    }

    /// Sets the pagination offset for subsequent search calls
    pub fn set_search_offset(&mut self, offset: u32) {
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        map.entry(key).or_insert_with(SearchSessionMetadata::new).offset = offset;
    }

    /// Current sort order (Desc / newest first, Asc / oldest first)
    pub fn search_sort_order(&self) -> SearchSortOrder {
        let key = self.state_key();
        SEARCH_METADATA.read().unwrap().get(&key).map(|m| m.sort_order).unwrap_or(SearchSortOrder::Desc)
    }

    /// Updates sort order and resets pagination offset to 0
    pub fn set_search_sort_order(&mut self, order: SearchSortOrder) {
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
        entry.sort_order = order;
        entry.offset = 0;
    }

    /// Current search error message, if any
    pub fn search_error(&self) -> Option<String> {
        let key = self.state_key();
        SEARCH_METADATA.read().unwrap().get(&key).and_then(|m| m.error.clone())
    }

    /// Sets or clears the search error message
    pub fn set_search_error(&mut self, error: Option<String>) {
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
        entry.error = error;
        entry.is_indexing = false;
        entry.retry_after = None;
    }

    /// True if search returned HTTP 202 indexing in progress
    pub fn is_search_indexing(&self) -> bool {
        let key = self.state_key();
        SEARCH_METADATA.read().unwrap().get(&key).map(|m| m.is_indexing).unwrap_or(false)
    }

    /// Retry-after duration in seconds when indexing
    pub fn search_retry_after(&self) -> Option<f64> {
        let key = self.state_key();
        SEARCH_METADATA.read().unwrap().get(&key).and_then(|m| m.retry_after)
    }

    /// Marks the search state as indexing (HTTP 202 Accepted)
    pub fn set_search_indexing(&mut self, retry_after: f64) {
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
        entry.is_indexing = true;
        entry.retry_after = Some(retry_after);
        entry.error = None;
        self.is_searching = true;
    }

    /// Sets search results received from the Discord REST API
    pub fn set_search_results_api(
        &mut self,
        total: u64,
        offset: u32,
        results: Vec<SearchResultGroup>,
    ) {
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
        entry.total_results = total;
        entry.offset = offset;
        entry.error = None;
        entry.is_indexing = false;
        entry.retry_after = None;
        self.search_results = results;
        self.is_searching = false;
    }

    /// Executes search across active scope.
    /// If not connected to Gateway/API, transitions to "Não conectado à API do Discord" error.
    /// Never falls back to local in-memory search in place of Discord API.
    pub fn execute_search(&mut self, query_str: &str) {
        self.search_query = query_str.to_string();
        let query = crate::chat::search::SearchQuery::parse(query_str);

        if query.is_empty() {
            self.clear_search();
            return;
        }

        self.is_searching = true;
        {
            let key = self.state_key();
            let mut map = SEARCH_METADATA.write().unwrap();
            let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
            entry.error = None;
            entry.is_indexing = false;
            entry.retry_after = None;
        }

        // Sem conexão: estado de erro "não conectado" — NÃO caia para busca local fingindo ser a busca do Discord.
        let cm_opt = self.client_manager.clone();
        let is_connected = self.is_connected_to_gateway && cm_opt.is_some();

        if !is_connected {
            self.is_searching = false;
            self.search_results.clear();
            self.set_search_error(Some("Não conectado à API do Discord".to_string()));
            return;
        }

        if let Some(cm) = cm_opt {
            let gid = self.selected_guild_id.clone();
            let cid = self.selected_channel_id.clone();
            let offset = self.search_offset();
            let sort_order = self.search_sort_order().as_api_str().to_string();
            let key = self.state_key();

            tokio::spawn(async move {
                match cm
                    .search_messages(
                        gid.as_deref(),
                        cid.as_deref(),
                        &query,
                        offset,
                        Some(&sort_order),
                    )
                    .await
                {
                    Ok(crate::chat::search::SearchApiResponse::Ready(payload)) => {
                        let total = payload.total_results;
                        let mut map = SEARCH_METADATA.write().unwrap();
                        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
                        entry.total_results = total;
                        entry.offset = offset;
                        entry.error = None;
                        entry.is_indexing = false;
                        entry.retry_after = None;
                    }
                    Ok(crate::chat::search::SearchApiResponse::Indexing { retry_after }) => {
                        let mut map = SEARCH_METADATA.write().unwrap();
                        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
                        entry.is_indexing = true;
                        entry.retry_after = Some(retry_after);
                    }
                    Err(e) => {
                        let mut map = SEARCH_METADATA.write().unwrap();
                        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
                        entry.error = Some(format!("Falha na busca: {}", e));
                        entry.is_indexing = false;
                    }
                }
            });
        }
    }

    /// Local in-memory search fallback for channel cache testing.
    pub fn execute_local_search_fallback(&mut self, query_str: &str) {
        self.search_query = query_str.to_string();
        self.is_searching = true;
        let query = crate::chat::search::SearchQuery::parse(query_str);

        if query.is_empty() {
            self.search_results.clear();
            self.is_searching = false;
            return;
        }

        let mut results = Vec::new();
        let mut searched_channels = std::collections::HashSet::new();

        let is_dm_mode = self.selected_guild_id.is_none();
        let current_gid = self.selected_guild_id.clone();

        // 1. Search in current channel first if selected and in scope
        if let Some(ref current_cid) = self.selected_channel_id {
            let channel_in_scope = if is_dm_mode {
                self.dm_channels.iter().any(|d| &d.id == current_cid)
                    || self.find_channel(current_cid).is_none()
            } else if let Some(ref gid) = current_gid {
                self.find_channel(current_cid).map(|(g, _)| &g.id == gid).unwrap_or(false)
            } else {
                true
            };

            if channel_in_scope {
                if let Some(msgs) = self.messages_by_channel.get(current_cid) {
                    let ch_name = self.find_channel(current_cid).map(|(_, c)| c.name.as_str())
                        .or_else(|| self.dm_channels.iter().find(|d| &d.id == current_cid).map(|d| d.name.as_str()));
                    results.extend(crate::chat::search::LocalSearchEngine::search_channel_messages(
                        msgs,
                        &query,
                        current_cid,
                        ch_name,
                        1,
                    ));
                    searched_channels.insert(current_cid.clone());
                }
            }
        }

        // 2. Search across other cached channels within the same scope
        for (cid, msgs) in &self.messages_by_channel {
            if searched_channels.contains(cid) {
                continue;
            }

            let channel_in_scope = if is_dm_mode {
                self.dm_channels.iter().any(|d| &d.id == cid)
                    || self.find_channel(cid).is_none()
            } else if let Some(ref gid) = current_gid {
                self.find_channel(cid).map(|(g, _)| &g.id == gid).unwrap_or(false)
            } else {
                true
            };

            if !channel_in_scope {
                continue;
            }

            let ch_name = self.find_channel(cid).map(|(_, c)| c.name.as_str())
                .or_else(|| self.dm_channels.iter().find(|d| &d.id == cid).map(|d| d.name.as_str()));
            results.extend(crate::chat::search::LocalSearchEngine::search_channel_messages(
                msgs,
                &query,
                cid,
                ch_name,
                1,
            ));
        }

        // 3. Sort results chronologically descending (newest hit first)
        results.sort_by(|a, b| b.hit_message.timestamp.cmp(&a.hit_message.timestamp));

        self.search_results = results;
        self.is_searching = false;
    }

    /// Clears search query and result list
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.is_searching = false;
        let key = self.state_key();
        let mut map = SEARCH_METADATA.write().unwrap();
        let entry = map.entry(key).or_insert_with(SearchSessionMetadata::new);
        entry.total_results = 0;
        entry.offset = 0;
        entry.sort_order = crate::chat::search::SearchSortOrder::Desc;
        entry.error = None;
        entry.is_indexing = false;
        entry.retry_after = None;
    }

    /// Jumps to and highlights a specific message in the chat
    pub fn jump_to_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some((guild, _)) = self.find_channel(channel_id) {
            let gid = guild.id.clone();
            if self.selected_guild_id.as_deref() != Some(&gid) {
                self.selected_guild_id = Some(gid);
            }
        }
        if self.selected_channel_id.as_deref() != Some(channel_id) {
            self.select_channel(channel_id.to_string());
        }
        self.target_message_id = Some(message_id.to_string());
    }
}
