use super::*;

    // =========================================================================
    // Search Engine & Search Flyout Subsystem
    // =========================================================================

impl AppState {
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

    /// Executes search across active channel and all cached channels in memory
    pub fn execute_search(&mut self, query_str: &str) {
        self.search_query = query_str.to_string();
        self.is_searching = true;
        let query = crate::chat::search::SearchQuery::parse(query_str);

        if query.is_empty() {
            self.search_results.clear();
            self.is_searching = false;
            return;
        }

        let mut results = Vec::new();

        // 1. Search in current channel first if selected
        if let Some(ref current_cid) = self.selected_channel_id {
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
            }
        }

        // 2. Search across other cached channels
        for (cid, msgs) in &self.messages_by_channel {
            if self.selected_channel_id.as_deref() == Some(cid) {
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

        self.search_results = results;
        self.is_searching = false;
    }

    /// Clears search query and result list
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.is_searching = false;
    }

    /// Jumps to and highlights a specific message in the chat
    pub fn jump_to_message(&mut self, channel_id: &str, message_id: &str) {
        if self.selected_channel_id.as_deref() != Some(channel_id) {
            self.select_channel(channel_id.to_string());
        }
        self.target_message_id = Some(message_id.to_string());
    }
}
