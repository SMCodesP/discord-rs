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

    /// Executes search across active channel and cached channels within the active guild/DM scope.
    /// Results are chronologically sorted (most recent first) matching Discord behavior.
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

        // 3. Sort results chronologically descending (newest hit first, matching Discord specification)
        results.sort_by(|a, b| b.hit_message.timestamp.cmp(&a.hit_message.timestamp));

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
