use super::*;
use crate::chat::search::{SearchResultGroup, SearchSortOrder};

// =========================================================================
// Search Engine & Search Flyout Subsystem
// =========================================================================

/// Request bundle produced by `begin_search`.
/// Encapsulates the target scope (guild/channel), parsed query, offset, sort order,
/// generation and client manager to execute the search asynchronously outside AppState.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub client_manager: crate::protocol::ClientManager,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub query: crate::chat::search::SearchQuery,
    pub query_string: String,
    pub offset: u32,
    pub sort_order: crate::chat::search::SearchSortOrder,
    pub generation: u64,
    pub retry_count: u32,
}

impl SearchRequest {
    pub async fn execute(&self) -> Result<crate::chat::search::SearchApiResponse, String> {
        let sort_str = self.sort_order.as_api_str().to_string();
        self.client_manager
            .search_messages(
                self.guild_id.as_deref(),
                self.channel_id.as_deref(),
                &self.query,
                self.offset,
                Some(&sort_str),
            )
            .await
            .map_err(|e| e.to_string())
    }

    /// Spawns the async execution of this SearchRequest with automatic HTTP 202 retry handling
    /// and dispatches `finish_search` onto AppState with generation safety.
    pub async fn run_loop(
        mut self,
        app_weak: gpui::WeakEntity<AppState>,
        cx: gpui::AsyncApp,
    ) {
        const MAX_INDEXING_RETRIES: u32 = 5;
        loop {
            let res = self.execute().await;
            match res {
                Ok(crate::chat::search::SearchApiResponse::Indexing { retry_after }) => {
                    let _ = cx.update(|cx| {
                        let _ = app_weak.update(cx, |s, cx| {
                            s.record_search_indexing(&self, retry_after);
                            cx.notify();
                        });
                    });
                    if self.retry_count >= MAX_INDEXING_RETRIES {
                        let _ = cx.update(|cx| {
                            let _ = app_weak.update(cx, |s, cx| {
                                s.finish_search(
                                    &self,
                                    Err("Tempo limite de indexação da API excedido.".to_string()),
                                );
                                cx.notify();
                            });
                        });
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs_f64(retry_after.max(1.0))).await;
                    self.retry_count += 1;
                }
                other => {
                    let _ = cx.update(|cx| {
                        let _ = app_weak.update(cx, |s, cx| {
                            s.finish_search(&self, other);
                            cx.notify();
                        });
                    });
                    break;
                }
            }
        }
    }
}

impl AppState {
    /// Opens the search flyout panel
    pub fn open_search(&mut self) {
        self.is_search_open = true;
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
        if self.search_total_results > 0 {
            self.search_total_results
        } else {
            self.search_results.len() as u64
        }
    }

    /// Current pagination offset (0, 25, 50...)
    pub fn search_offset(&self) -> u32 {
        self.search_offset
    }

    /// Sets the pagination offset for subsequent search calls
    pub fn set_search_offset(&mut self, offset: u32) {
        self.search_offset = offset;
    }

    /// Current sort order (Desc / newest first, Asc / oldest first)
    pub fn search_sort_order(&self) -> SearchSortOrder {
        self.search_sort_order
    }

    /// Updates sort order and resets pagination offset to 0
    pub fn set_search_sort_order(&mut self, order: SearchSortOrder) {
        self.search_sort_order = order;
        self.search_offset = 0;
    }

    /// Current search error message, if any
    pub fn search_error(&self) -> Option<String> {
        self.search_error.clone()
    }

    /// Sets or clears the search error message
    pub fn set_search_error(&mut self, error: Option<String>) {
        self.search_error = error;
        self.is_search_indexing = false;
        self.search_retry_after = None;
    }

    /// True if search returned HTTP 202 indexing in progress
    pub fn is_search_indexing(&self) -> bool {
        self.is_search_indexing
    }

    /// Retry-after duration in seconds when indexing
    pub fn search_retry_after(&self) -> Option<f64> {
        self.search_retry_after
    }

    /// Marks the search state as indexing (HTTP 202 Accepted)
    pub fn set_search_indexing(&mut self, retry_after: f64) {
        self.is_search_indexing = true;
        self.search_retry_after = Some(retry_after);
        self.search_error = None;
        self.is_searching = true;
    }

    /// Sets search results received from the Discord REST API directly
    pub fn set_search_results_api(
        &mut self,
        total: u64,
        offset: u32,
        results: Vec<SearchResultGroup>,
    ) {
        self.search_total_results = total;
        self.search_offset = offset;
        self.search_results = results;
        self.search_error = None;
        self.is_search_indexing = false;
        self.search_retry_after = None;
        self.is_searching = false;
    }

    /// First phase of message search:
    /// Validates query and client connection, bumps generation counter,
    /// sets `is_searching = true`, clears previous error and returns `Option<SearchRequest>`.
    /// In disconnected state, records error "Não conectado à API do Discord" (Zero Mocks) and returns None.
    pub fn begin_search(&mut self, query_str: &str) -> Option<SearchRequest> {
        self.search_query = query_str.to_string();
        let query = crate::chat::search::SearchQuery::parse(query_str);

        if query.is_empty() {
            self.clear_search();
            return None;
        }

        self.search_generation += 1;
        let generation = self.search_generation;

        self.is_searching = true;
        self.search_error = None;
        self.is_search_indexing = false;
        self.search_retry_after = None;

        let cm = match self.client_manager.as_ref().cloned() {
            Some(cm) => cm,
            None => {
                // ZERO MOCKS: sem client_manager não inventa resultados
                self.is_searching = false;
                self.search_results.clear();
                self.search_total_results = 0;
                self.search_error = Some("Não conectado à API do Discord".to_string());
                return None;
            }
        };

        Some(SearchRequest {
            client_manager: cm,
            guild_id: self.selected_guild_id.clone(),
            channel_id: self.selected_channel_id.clone(),
            query,
            query_string: query_str.to_string(),
            offset: self.search_offset,
            sort_order: self.search_sort_order,
            generation,
            retry_count: 0,
        })
    }

    /// Records intermediate HTTP 202 indexing status while waiting for retry timer.
    /// Discards stale responses from older search generations.
    pub fn record_search_indexing(&mut self, request: &SearchRequest, retry_after: f64) {
        if request.generation != self.search_generation {
            return;
        }
        self.is_search_indexing = true;
        self.search_retry_after = Some(retry_after);
        self.search_error = None;
        self.is_searching = true;
    }

    /// Second phase of message search:
    /// Consumes result from `SearchRequest::execute`.
    /// - Discards stale responses if `request.generation != self.search_generation`.
    /// - On `Ready(payload)`: populates `search_results`, updates `search_total_results`, resets indexing and marks `is_searching = false`.
    /// - On `Indexing { retry_after }`: marks `is_search_indexing = true` with `retry_after`.
    /// - On `Err(err)`: records visible error, clears results, and marks `is_searching = false`.
    pub fn finish_search(
        &mut self,
        request: &SearchRequest,
        result: Result<crate::chat::search::SearchApiResponse, String>,
    ) {
        if request.generation != self.search_generation {
            return;
        }

        match result {
            Ok(crate::chat::search::SearchApiResponse::Ready(payload)) => {
                self.search_total_results = payload.total_results;
                self.search_offset = request.offset;
                self.search_results = payload.into_result_groups();
                self.search_error = None;
                self.is_search_indexing = false;
                self.search_retry_after = None;
                self.is_searching = false;
            }
            Ok(crate::chat::search::SearchApiResponse::Indexing { retry_after }) => {
                self.is_search_indexing = true;
                self.search_retry_after = Some(retry_after);
                self.search_error = None;
                self.is_searching = true;
            }
            Err(err) => {
                self.search_error = Some(format!("Falha na busca: {}", err));
                self.search_results.clear();
                self.is_search_indexing = false;
                self.search_retry_after = None;
                self.is_searching = false;
            }
        }
    }

    /// Clears search query, result list and resets session metadata
    pub fn clear_search(&mut self) {
        self.search_generation += 1;
        self.search_query.clear();
        self.search_results.clear();
        self.is_searching = false;
        self.search_total_results = 0;
        self.search_offset = 0;
        self.search_sort_order = crate::chat::search::SearchSortOrder::Desc;
        self.search_error = None;
        self.is_search_indexing = false;
        self.search_retry_after = None;
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

