use super::*;
use crate::domain::message::Message;
use crate::domain::message::Reaction;
use crate::protocol::TransportEvent;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Maximum time an optimistic message may stay pending without a REST/Gateway
/// confirmation before it is rolled back and offered for retry. Mirrors the
/// composer's request timeout so both paths expire together.
pub const MESSAGE_SEND_TIMEOUT: Duration = Duration::from_secs(15);

/// Correlation metadata for one optimistic send. Discord's REST transport in
/// this repository does not yet expose a nonce parameter, so the nonce is kept
/// here as a stable client correlation token and reconciled by the REST message
/// ID plus channel/author/content. This still makes REST/Gateway confirmation
/// idempotent and prevents duplicate rows.
#[derive(Debug, Clone)]
pub struct PendingMessageSend {
    pub temp_id: String,
    pub client_nonce: String,
    pub channel_id: String,
    pub content: String,
    pub author_id: String,
    pub created_at: Instant,
    pub confirmed_id: Option<String>,
}

static PENDING_MESSAGE_SENDS: LazyLock<
    Mutex<std::collections::HashMap<String, PendingMessageSend>>,
> = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
static FAILED_MESSAGE_SENDS: LazyLock<Mutex<Vec<(String, String, String, String)>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static SEND_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn next_client_nonce() -> String {
    // Hex timestamp + monotonic sequence stays below Discord's 25-character
    // nonce limit while remaining unique within this process.
    format!(
        "{:x}{:x}",
        chrono::Utc::now().timestamp_millis(),
        SEND_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn register_pending_send(pending: PendingMessageSend) {
    if let Ok(mut sends) = PENDING_MESSAGE_SENDS.lock() {
        sends.retain(|_, existing| existing.created_at.elapsed() < Duration::from_secs(300));
        sends.insert(pending.temp_id.clone(), pending);
    }
}

fn pending_send(temp_id: &str) -> Option<PendingMessageSend> {
    PENDING_MESSAGE_SENDS
        .lock()
        .ok()
        .and_then(|sends| sends.get(temp_id).cloned())
}

fn take_pending_send(temp_id: &str) -> Option<PendingMessageSend> {
    PENDING_MESSAGE_SENDS
        .lock()
        .ok()
        .and_then(|mut sends| sends.remove(temp_id))
}

fn mark_pending_send_confirmed(temp_id: &str, confirmed_id: &str) {
    if let Ok(mut sends) = PENDING_MESSAGE_SENDS.lock() {
        if let Some(pending) = sends.get_mut(temp_id) {
            pending.confirmed_id = Some(confirmed_id.to_string());
        }
    }
}

fn mark_message_send_failed(temp_id: &str, channel_id: &str, content: &str, error: &str) {
    if let Ok(mut failures) = FAILED_MESSAGE_SENDS.lock() {
        failures.retain(|(existing_temp, _, _, _)| existing_temp != temp_id);
        failures.push((
            temp_id.to_string(),
            channel_id.to_string(),
            content.to_string(),
            error.to_string(),
        ));
    }
}

fn prewarm_message_markdown(message: &Message) {
    crate::chat::markdown::prewarm_message_markdown_cache(&message.id, &message.content);
    crate::chat::markdown::prewarm_markdown_cache(&message.content);
    for (embed_idx, embed) in message.embeds.iter().enumerate() {
        if let Some(description) = &embed.description {
            let key = format!("{}:embed:{embed_idx}:desc", message.id);
            crate::chat::markdown::prewarm_message_markdown_cache(&key, description);
            crate::chat::markdown::prewarm_markdown_cache(description);
        }
        for (field_idx, field) in embed.fields.iter().enumerate() {
            let key = format!("{}:embed:{embed_idx}:field:{field_idx}", message.id);
            crate::chat::markdown::prewarm_message_markdown_cache(&key, &field.value);
            crate::chat::markdown::prewarm_markdown_cache(&field.value);
        }
    }
}

impl AppState {
    /// Gets message history for the currently selected channel without copying
    pub fn get_active_messages(&self) -> &[Message] {
        if let Some(ref cid) = self.selected_channel_id {
            self.messages_by_channel
                .get(cid)
                .map(|v| v.as_slice())
                .unwrap_or(&[])
        } else {
            &[]
        }
    }

    /// Injects or replaces a message in a channel's history by Discord ID.
    pub fn inject_message(&mut self, channel_id: &str, message: Message) {
        prewarm_message_markdown(&message);
        let messages = self
            .messages_by_channel
            .entry(channel_id.to_string())
            .or_default();
        if let Some(existing) = messages
            .iter_mut()
            .find(|existing| existing.id == message.id)
        {
            *existing = message;
        } else {
            messages.push(message);
        }
    }

    /// Adds an emoji reaction to a message in state
    pub fn add_reaction_to_message(&mut self, channel_id: &str, message_id: &str, emoji: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                if let Some(rx) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                    rx.count += 1;
                    rx.me = true;
                } else {
                    msg.reactions.push(Reaction {
                        emoji: emoji.to_string(),
                        count: 1,
                        me: true,
                    });
                }
            }
        }
    }

    /// Deletes a message by ID from the specified channel in local state.
    /// Used when the API or Gateway confirms deletion via `TransportEvent::MessageDelete`.
    pub fn delete_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            messages.retain(|m| m.id != message_id);
        }
    }

    /// Dispatches message deletion request asynchronously through the ClientManager / DiscordTransport.
    /// Uses the official rate limiter, User-Agent, and headers of DiscordApiClient (or MockProvider).
    /// ONLY upon successful API response or Gateway event is the message removed from state.
    pub fn request_delete_message(&mut self, channel_id: &str, message_id: &str) {
        let cid = channel_id.to_string();
        let mid = message_id.to_string();
        if let Some(ref cm) = self.client_manager {
            let cm_clone = cm.clone();
            tokio::spawn(async move {
                match cm_clone.delete_message(&cid, &mid).await {
                    Ok(()) => {
                        log::info!("Mensagem {} deletada com sucesso no backend", mid);
                        cm_clone.emit_event(TransportEvent::MessageDelete {
                            channel_id: cid,
                            message_id: mid,
                        });
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        log::warn!("Falha ao deletar mensagem {} na API: {}", mid, err_msg);
                        cm_clone.emit_event(TransportEvent::MessageDeleteFailed {
                            channel_id: cid,
                            message_id: mid,
                            error: err_msg,
                        });
                    }
                }
            });
        } else {
            log::warn!(
                "Não conectado ao Discord: client_manager ausente ao tentar deletar mensagem {}",
                mid
            );
        }
    }

    /// Sets the pinned status from a confirmed API response.
    pub fn set_message_pinned(&mut self, channel_id: &str, message_id: &str, pinned: bool) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(message) = messages.iter_mut().find(|message| message.id == message_id) {
                message.pinned = pinned;
            }
        }
    }

    /// Toggles the pinned status of a message by ID
    pub fn pin_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.pinned = !msg.pinned;
            }
        }
    }

    /// Sets / synchronizes real messages received from the API/Gateway for a channel.
    /// Pending sends are preserved until their correlated REST/Gateway message is
    /// present; content matching is only a fallback for legacy pending rows.
    pub fn set_channel_messages(&mut self, channel_id: &str, mut messages: Vec<Message>) {
        for message in &messages {
            prewarm_message_markdown(message);
        }

        let mut seen_ids: HashSet<String> =
            messages.iter().map(|message| message.id.clone()).collect();
        if let Some(existing) = self.messages_by_channel.get(channel_id) {
            for message in existing {
                if seen_ids.contains(&message.id) {
                    continue;
                }
                let keep = if message.is_pending || message.id.starts_with("temp-") {
                    // Only a correlated confirmation may drop an optimistic row.
                    // An older message with the same text must never remove a
                    // newer pending send.
                    let correlated = pending_send(&message.id)
                        .and_then(|pending| pending.confirmed_id)
                        .map(|confirmed_id| seen_ids.contains(&confirmed_id))
                        .unwrap_or(false);
                    !correlated
                } else {
                    // Keep live rows not included in a stale history response.
                    true
                };
                if keep {
                    seen_ids.insert(message.id.clone());
                    messages.push(message.clone());
                }
            }
        }
        messages.sort_by_key(|message| message.timestamp);

        if let Some(ref gid) = self.selected_guild_id {
            self.request_missing_author_colors(gid, &messages);
        }
        self.messages_by_channel
            .insert(channel_id.to_string(), messages);
    }

    /// Merges an older REST page into the current history without duplicating
    /// messages or dropping live/pending rows. Returns the number of new rows.
    pub fn merge_older_messages(&mut self, channel_id: &str, mut older: Vec<Message>) -> usize {
        let messages = self
            .messages_by_channel
            .entry(channel_id.to_string())
            .or_default();
        let mut seen_ids: HashSet<String> =
            messages.iter().map(|message| message.id.clone()).collect();
        let mut added = 0;
        for message in older.drain(..) {
            prewarm_message_markdown(&message);
            if seen_ids.insert(message.id.clone()) {
                messages.push(message);
                added += 1;
            }
        }
        messages.sort_by_key(|message| message.timestamp);
        added
    }

    /// Requests missing author roles and colors for historical messages via Gateway Opcode 8
    pub fn request_missing_author_colors(&self, guild_id: &str, messages: &[Message]) {
        let mut missing_uids = Vec::new();
        for msg in messages {
            let uid = &msg.author.id;
            if uid.is_empty() || uid == "user-me" || uid.starts_with("temp-") {
                continue;
            }
            if crate::protocol::cache::lazy_guilds::is_op8_requested(guild_id, uid) {
                continue;
            }
            let has_color = self
                .members
                .iter()
                .any(|m| &m.id == uid && m.role_color != 0)
                || crate::protocol::cache::lazy_guilds::get_cached_author_role_color(guild_id, uid)
                    .is_some()
                || crate::protocol::cache::lazy_guilds::get_cached_author_roles(guild_id, uid)
                    .is_some();
            if !has_color && !missing_uids.contains(uid) {
                missing_uids.push(uid.clone());
            }
        }

        if !missing_uids.is_empty() {
            crate::protocol::cache::lazy_guilds::mark_op8_batch_requested(guild_id, &missing_uids);
            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                let gid = guild_id.to_string();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        for chunk in missing_uids.chunks(100) {
                            let _ = cm.request_guild_members(&gid, chunk.to_vec()).await;
                        }
                    });
                }
            }
        }
    }
}

impl AppState {
    /// Sends a chat message to the currently selected channel via ClientManager
    pub fn send_chat_message(&mut self, content: &str) {
        if let Some(cid) = self.selected_channel_id.clone() {
            self.send_message(&cid, content);
        }
    }

    /// Creates a correlated optimistic row without dispatching it. The composer
    /// uses this API so REST failures can be shown and retried immediately.
    pub fn begin_message_send(
        &mut self,
        channel_id: &str,
        content: &str,
    ) -> Result<String, String> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err("Mensagem vazia".to_string());
        }
        if self.client_manager.is_none() {
            return Err("Não conectado ao Discord: cliente não inicializado".to_string());
        }

        let cid = channel_id.to_string();
        if let Some(ref tel) = self.telemetry {
            tel.track_message_send(&cid, trimmed.len(), true, 0);
            tel.track_button_click("send_message", "Enviar Mensagem", 0);
        }

        let client_nonce = next_client_nonce();
        let temp_id = format!("temp-{}", client_nonce);
        let mut message = Message::new(
            temp_id.clone(),
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: self.current_user.avatar_url.clone(),
                bot: false,
                avatar_decoration_data: self.current_user.avatar_decoration_data.clone(),
            },
            trimmed,
        )
        .with_pending(true);
        if let Some(guild_id) = &self.selected_guild_id {
            message.guild_id = Some(guild_id.clone());
        }

        register_pending_send(PendingMessageSend {
            temp_id: temp_id.clone(),
            client_nonce,
            channel_id: cid.clone(),
            content: trimmed.to_string(),
            author_id: self.current_user.id.clone(),
            created_at: Instant::now(),
            confirmed_id: None,
        });
        self.inject_message(&cid, message);
        if let Some(dm) = self
            .dm_channels
            .iter_mut()
            .find(|channel| channel.id == cid)
        {
            dm.last_message_id = Some(temp_id.clone());
            self.sort_dm_channels();
        }
        Ok(temp_id)
    }

    /// Removes one optimistic row from every channel it may have been migrated
    /// to (temporary DM -> resolved DM). Returns the channel that contained it.
    fn remove_message_row(&mut self, temp_id: &str) -> Option<String> {
        let mut found = None;
        for (channel_id, messages) in self.messages_by_channel.iter_mut() {
            if let Some(position) = messages.iter().position(|message| message.id == temp_id) {
                messages.remove(position);
                if found.is_none() {
                    found = Some(channel_id.clone());
                }
            }
        }
        found
    }

    /// Inserts or replaces a confirmed message, keeping channel ordering and the
    /// DM last-message pointer in sync.
    fn upsert_confirmed_message(&mut self, mut confirmed: Message) {
        prewarm_message_markdown(&confirmed);
        confirmed.is_pending = false;
        let channel_id = confirmed.channel_id.clone();
        let messages = self
            .messages_by_channel
            .entry(channel_id.clone())
            .or_default();
        if let Some(existing) = messages
            .iter_mut()
            .find(|message| message.id == confirmed.id)
        {
            *existing = confirmed;
        } else {
            let insert_at = messages
                .iter()
                .position(|message| message.timestamp > confirmed.timestamp)
                .unwrap_or(messages.len());
            messages.insert(insert_at, confirmed);
        }
        if let Some(dm) = self
            .dm_channels
            .iter_mut()
            .find(|channel| channel.id == channel_id)
        {
            dm.last_message_id = messages.last().map(|message| message.id.clone());
            self.sort_dm_channels();
        }
    }

    /// Reconciles a successful REST response with its optimistic row. Safe to
    /// call before or after the duplicate Gateway `MESSAGE_CREATE` event.
    pub fn confirm_message_send(&mut self, temp_id: &str, confirmed: Message) {
        if let Some(pending) = take_pending_send(temp_id) {
            log::debug!(
                "Confirmando mensagem otimista {} com nonce de correlação {}",
                pending.temp_id,
                pending.client_nonce
            );
        }
        self.remove_message_row(temp_id);
        self.upsert_confirmed_message(confirmed);
    }

    /// Correlates a Gateway `MESSAGE_CREATE` with one optimistic send. Matching
    /// is anchored on the registered temp ID/nonce; text+author is only used to
    /// locate a legacy offline row that has no correlation metadata. Pending
    /// entries stay registered after this call so a later REST success/failure
    /// can still be resolved idempotently.
    pub fn reconcile_gateway_message_create(&mut self, message: &Message) -> bool {
        let already_confirmed: Vec<String> = PENDING_MESSAGE_SENDS
            .lock()
            .map(|sends| {
                sends
                    .values()
                    .filter(|pending| pending.confirmed_id.as_deref() == Some(message.id.as_str()))
                    .map(|pending| pending.temp_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if !already_confirmed.is_empty() {
            for temp_id in already_confirmed {
                self.remove_message_row(&temp_id);
            }
            self.upsert_confirmed_message(message.clone());
            return true;
        }

        let matched = PENDING_MESSAGE_SENDS
            .lock()
            .ok()
            .and_then(|sends| {
                sends
                    .values()
                    .filter(|pending| {
                        pending.confirmed_id.is_none()
                            && pending.content == message.content
                            && pending.author_id == message.author.id
                            && (pending.channel_id == message.channel_id
                                || pending.channel_id.starts_with("dm-"))
                    })
                    .min_by(|left, right| {
                        left.created_at
                            .cmp(&right.created_at)
                            .then_with(|| left.temp_id.cmp(&right.temp_id))
                    })
                    .map(|pending| pending.temp_id.clone())
            });
        if let Some(temp_id) = matched {
            if let Ok(mut sends) = PENDING_MESSAGE_SENDS.lock() {
                if let Some(pending) = sends.get_mut(&temp_id) {
                    pending.confirmed_id = Some(message.id.clone());
                }
            }
            self.remove_message_row(&temp_id);
            self.upsert_confirmed_message(message.clone());
            return true;
        }

        // Legacy rows created by the offline dispatcher never registered a
        // nonce. They may still be confirmed once, but only a row that matches
        // this exact author/content pair is consumed.
        let legacy = self.find_legacy_pending_row(message);
        if let Some(temp_id) = legacy {
            self.remove_message_row(&temp_id);
            self.upsert_confirmed_message(message.clone());
            return true;
        }
        false
    }

    fn find_legacy_pending_row(&self, message: &Message) -> Option<String> {
        let registered: HashSet<String> = PENDING_MESSAGE_SENDS
            .lock()
            .map(|sends| sends.keys().cloned().collect())
            .unwrap_or_default();
        let mut best: Option<(i64, String)> = None;
        for (channel_id, messages) in &self.messages_by_channel {
            if channel_id != &message.channel_id && !channel_id.starts_with("dm-") {
                continue;
            }
            for candidate in messages {
                if !(candidate.is_pending || candidate.id.starts_with("temp-")) {
                    continue;
                }
                if registered.contains(&candidate.id) {
                    continue;
                }
                if candidate.content != message.content {
                    continue;
                }
                if candidate.author.id != message.author.id
                    && candidate.author.username != message.author.username
                {
                    continue;
                }
                let candidate_timestamp = candidate.timestamp.timestamp_millis();
                let is_older = best
                    .as_ref()
                    .map(|(timestamp, _)| candidate_timestamp < *timestamp)
                    .unwrap_or(true);
                if is_older {
                    best = Some((candidate_timestamp, candidate.id.clone()));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Removes a failed optimistic row and returns its channel/content for retry.
    /// If the Gateway/REST already confirmed the send, no error is returned and
    /// the temporary row is still rolled back.
    pub fn fail_message_send(&mut self, temp_id: &str) -> Option<(String, String)> {
        let pending = take_pending_send(temp_id)?;
        let actual_channel = self
            .remove_message_row(temp_id)
            .unwrap_or_else(|| pending.channel_id.clone());
        if pending.confirmed_id.is_some() {
            return None;
        }
        Some((actual_channel, pending.content))
    }

    /// Records a transport-level send failure without touching a send that the
    /// Gateway already confirmed. The UI drains it through
    /// `take_failed_message_sends`.
    pub fn handle_message_send_failure_event(
        &mut self,
        temp_id: &str,
        channel_id: &str,
        content: &str,
        error: &str,
    ) {
        if pending_send(temp_id)
            .map(|pending| pending.confirmed_id.is_some())
            .unwrap_or(false)
        {
            return;
        }
        mark_message_send_failed(temp_id, channel_id, content, error);
    }

    /// Expires one pending send when it exceeds `max_age`. Returns the retry
    /// payload only when no REST/Gateway confirmation arrived.
    pub fn expire_pending_message_send(
        &mut self,
        temp_id: &str,
        max_age: Duration,
    ) -> Option<(String, String, String)> {
        let expired = PENDING_MESSAGE_SENDS.lock().ok().and_then(|mut sends| {
            let should_expire = sends
                .get(temp_id)
                .map(|pending| {
                    pending.confirmed_id.is_none() && pending.created_at.elapsed() >= max_age
                })
                .unwrap_or(false);
            if should_expire {
                sends.remove(temp_id)
            } else {
                None
            }
        })?;
        let actual_channel = self
            .remove_message_row(temp_id)
            .unwrap_or_else(|| expired.channel_id.clone());
        Some((
            actual_channel,
            expired.content,
            format!(
                "Tempo limite de {}s excedido ao confirmar o envio",
                MESSAGE_SEND_TIMEOUT.as_secs()
            ),
        ))
    }

    /// Rolls back every optimistic row older than `max_age` that never received
    /// a confirmation, so a lost request can never leave an infinite pending row.
    pub fn expire_message_sends_older_than(
        &mut self,
        max_age: Duration,
    ) -> Vec<(String, String, String)> {
        let expired_ids: Vec<String> = PENDING_MESSAGE_SENDS
            .lock()
            .map(|sends| {
                sends
                    .values()
                    .filter(|pending| {
                        pending.confirmed_id.is_none() && pending.created_at.elapsed() >= max_age
                    })
                    .map(|pending| pending.temp_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut result = Vec::new();
        for temp_id in expired_ids {
            if let Some(failure) = self.expire_pending_message_send(&temp_id, max_age) {
                result.push(failure);
            }
        }
        result
    }

    /// Production timeout sweep used before the UI drains failures.
    pub fn expire_timed_out_message_sends(&mut self) -> Vec<(String, String, String)> {
        self.expire_message_sends_older_than(MESSAGE_SEND_TIMEOUT)
    }

    /// Drains failures from the legacy `send_message` dispatcher and expired
    /// optimistic sends, removes their temporary rows and returns
    /// `(channel_id, content, error)` for UI retry.
    pub fn take_failed_message_sends(&mut self) -> Vec<(String, String, String)> {
        let mut result = self.expire_timed_out_message_sends();
        let failures = FAILED_MESSAGE_SENDS
            .lock()
            .map(|mut failures| std::mem::take(&mut *failures))
            .unwrap_or_default();
        for (temp_id, channel_id, content, error) in failures {
            let pending = take_pending_send(&temp_id);
            let confirmed = pending
                .as_ref()
                .map(|pending| pending.confirmed_id.is_some())
                .unwrap_or(false);
            let actual_channel = self
                .remove_message_row(&temp_id)
                .unwrap_or(channel_id);
            if confirmed {
                // A late Gateway MESSAGE_CREATE already proved the send; the
                // failure must not be reported or retried.
                continue;
            }
            result.push((actual_channel, content, error));
        }
        result
    }

    /// Sends a message to a specific channel via ClientManager with optimistic UI update.
    /// Kept for non-composer callers; the composer uses `begin_message_send` for
    /// explicit failure/retry handling.
    pub fn send_message(&mut self, channel_id: &str, content: &str) {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.client_manager.is_some() {
            let Ok(temp_id) = self.begin_message_send(channel_id, trimmed) else {
                return;
            };
            let Some(cm) = self.client_manager.clone() else {
                return;
            };
            let target_cid = channel_id.to_string();
            let text = trimmed.to_string();
            tokio::spawn(async move {
                let result = tokio::time::timeout(
                    MESSAGE_SEND_TIMEOUT,
                    cm.send_message(&target_cid, &text),
                )
                .await;
                let result = match result {
                    Ok(result) => result,
                    Err(_) => Err(anyhow::anyhow!(
                        "Tempo limite de {}s excedido ao confirmar o envio",
                        MESSAGE_SEND_TIMEOUT.as_secs()
                    )),
                };
                match result {
                    Ok(message) => mark_pending_send_confirmed(&temp_id, &message.id),
                    Err(error) => {
                        let error = error.to_string();
                        // Keep the correlation entry until the UI drains the
                        // failure. A late Gateway MESSAGE_CREATE can still prove
                        // that the request succeeded even when REST failed.
                        mark_message_send_failed(&temp_id, &target_cid, &text, &error);
                        cm.emit_event(TransportEvent::MessageSendFailed {
                            temp_id,
                            channel_id: target_cid,
                            content: text,
                            error: error.clone(),
                        });
                        log::warn!("Falha ao enviar mensagem via ClientManager: {:?}", error);
                    }
                }
            });
            return;
        }

        // Offline/local callers still get a clearly pending row, never a
        // fabricated confirmation. The production composer rejects this state.
        let temp_id = format!("temp-{}", next_client_nonce());
        self.inject_message(
            channel_id,
            Message::new(
                temp_id,
                channel_id.to_string(),
                crate::domain::message::Author {
                    id: self.current_user.id.clone(),
                    username: self.current_user.username.clone(),
                    global_name: Some(self.current_user.display_name.clone()),
                    avatar: self.current_user.avatar_url.clone(),
                    bot: false,
                    avatar_decoration_data: self.current_user.avatar_decoration_data.clone(),
                },
                trimmed,
            )
            .with_pending(true),
        );
        self.replying_to_message = None;
        self.active_draft.clear();
    }

    /// Sends a sticker message to a specific channel immediately with authentic sticker rendering
    pub fn send_sticker(
        &mut self,
        channel_id: &str,
        sticker: &crate::protocol::media_cache::DiscordSticker,
    ) {
        let cid = channel_id.to_string();
        let new_id = format!("temp-{}", next_client_nonce());

        let mut new_msg = Message::new(
            new_id.clone(),
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: None,
                bot: false,
                avatar_decoration_data: self.current_user.avatar_decoration_data.clone(),
            },
            "",
        )
        .with_pending(true);

        new_msg
            .sticker_items
            .push(crate::domain::message::MessageStickerItem::new(
                sticker.id.clone(),
                sticker.name.clone(),
                sticker.format_type as u8,
            ));

        self.inject_message(&cid, new_msg);
        if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == cid) {
            dm.last_message_id = Some(new_id.clone());
            self.sort_dm_channels();
        }
        self.replying_to_message = None;

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let target_cid = cid;
            let sticker_id = sticker.id.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.send_sticker(&target_cid, &sticker_id).await {
                    log::warn!("Falha ao enviar figurinha via ClientManager: {:?}", e);
                }
            });
        }
    }

    /// Records emoji usage in local media cache
    pub fn record_emoji_usage(&self, emoji: &str) {
        let mgr = self.media_cache.clone();
        let em = emoji.to_string();
        tokio::spawn(async move {
            mgr.record_emoji_use(&em, &em, false, None).await;
        });
    }

    /// Returns custom emojis for the currently selected guild, or across all guilds
    pub fn get_current_custom_emojis(&self) -> Vec<crate::protocol::media_cache::DiscordEmoji> {
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                if !guild.emojis.is_empty() {
                    return guild.emojis.clone();
                }
            }
        }
        let mut all = Vec::new();
        for g in &self.guilds {
            all.extend(g.emojis.clone());
        }
        all
    }

    /// Returns custom stickers and favorites for the currently selected guild
    pub fn get_current_stickers(&self) -> Vec<crate::protocol::media_cache::DiscordSticker> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. Figurinhas favoritas persistidas
        for s in self.media_cache.get_favorite_stickers_sync() {
            if seen.insert(s.id.clone()) {
                result.push(s);
            }
        }

        // 2. Figurinhas do servidor selecionado (ou todos)
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                for s in &guild.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        } else {
            for g in &self.guilds {
                for s in &g.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        }
        result
    }

    /// Carrega emojis e figurinhas customizados de cada guilda a partir do cache em disco imediatamente
    /// e busca da API em segundo plano caso algum servidor ainda não os possua.
    pub fn sync_guild_media(&mut self) {
        let mut missing_emojis = Vec::new();

        // 1. Fast-path: carrega do cache particionado em disco
        for guild in &mut self.guilds {
            if guild.emojis.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_emojis_from_disk(&guild.id) {
                    guild.emojis = cached;
                } else {
                    missing_emojis.push(guild.id.clone());
                }
            }
            if guild.stickers.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_stickers_from_disk(&guild.id) {
                    guild.stickers = cached;
                }
            }
        }

        // 2. Background-path sob demanda: busca figurinhas APENAS da guilda selecionada se ainda não estiver em cache
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            if let Some(ref gid) = self.selected_guild_id {
                let target_gid = gid.clone();
                let needs_stickers = self
                    .guilds
                    .iter()
                    .any(|g| g.id == target_gid && g.stickers.is_empty())
                    && self
                        .media_cache
                        .load_guild_stickers_from_disk(&target_gid)
                        .is_none();
                if needs_stickers {
                    let cm_stickers = cm.clone();
                    tokio::spawn(async move {
                        if let Ok(stickers) = cm_stickers.get_guild_stickers(&target_gid).await {
                            if !stickers.is_empty() {
                                cm_stickers.emit_event(TransportEvent::GuildStickersUpdated {
                                    guild_id: target_gid,
                                    stickers,
                                });
                            }
                        }
                    });
                }
            }

            if !missing_emojis.is_empty() {
                tokio::spawn(async move {
                    for gid in missing_emojis {
                        if let Ok(emojis) = cm.get_guild_emojis(&gid).await {
                            if !emojis.is_empty() {
                                cm.emit_event(TransportEvent::GuildEmojisUpdated {
                                    guild_id: gid,
                                    emojis,
                                });
                            }
                        }
                    }
                });
            }
        }
    }

    /// Adds a reaction and dispatches it via ClientManager
    pub fn add_reaction(&mut self, message_id: &str, emoji: &str) {
        if let Some(ref tel) = self.telemetry {
            tel.track_reaction(emoji, message_id, true, 0);
        }
        let channel_id_opt = self.selected_channel_id.clone();
        if let Some(cid) = channel_id_opt {
            self.add_reaction_to_message(&cid, message_id, emoji);

            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                let cid = cid.clone();
                let mid = message_id.to_string();
                let em = emoji.to_string();
                tokio::spawn(async move {
                    if let Err(e) = cm.add_reaction(&cid, &mid, &em).await {
                        log::warn!("Falha ao adicionar reação via ClientManager: {:?}", e);
                    }
                });
            }
        }
    }

    // =========================================================================
    // Real-Time Coalesced Typing Tracker Subsystem
    // =========================================================================

    /// Records a typing event with 9-10 second coalescing window
    pub fn record_typing(&mut self, channel_id: &str, user_id: &str, username: Option<&str>) {
        if user_id == self.current_user.id {
            return;
        }
        let resolved_name = if let Some(un) = username {
            un.to_string()
        } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
            m.name.clone()
        } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
            f.display_name.clone()
        } else if let Some(dm) = self
            .dm_channels
            .iter()
            .find(|d| d.id == user_id || d.name.to_lowercase() == user_id.to_lowercase())
        {
            dm.name.clone()
        } else {
            user_id.to_string()
        };

        let channel_map = self
            .typing_tracker
            .entry(channel_id.to_string())
            .or_default();
        channel_map.insert(
            user_id.to_string(),
            (resolved_name, std::time::Instant::now()),
        );

        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Clears typing status for a user in a channel (e.g. upon MessageCreate)
    pub fn clear_typing(&mut self, channel_id: &str, user_id: &str) {
        if let Some(channel_map) = self.typing_tracker.get_mut(channel_id) {
            channel_map.remove(user_id);
        }
        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Retrieves active typers in a channel whose events arrived within the 9-second TTL window
    pub fn get_active_typing_users(&self, channel_id: &str) -> Vec<String> {
        let now = std::time::Instant::now();
        if let Some(channel_map) = self.typing_tracker.get(channel_id) {
            let mut users: Vec<String> = channel_map
                .values()
                .filter(|(_, ts)| now.duration_since(*ts).as_secs() <= 9)
                .map(|(name, _)| name.clone())
                .collect();
            users.sort();
            users.dedup();
            users
        } else {
            Vec::new()
        }
    }

    /// Delay until the composer should refresh the typing indicator. A one-second
    /// floor guarantees expired TYPING_START entries disappear without user input.
    pub fn typing_refresh_delay(&self, channel_id: &str) -> Option<Duration> {
        let now = std::time::Instant::now();
        let channel_map = self.typing_tracker.get(channel_id)?;
        let remaining = channel_map
            .values()
            .filter_map(|(_, timestamp)| {
                let elapsed = now.saturating_duration_since(*timestamp);
                Duration::from_secs(9).checked_sub(elapsed)
            })
            .min()?;
        Some(remaining.max(Duration::from_secs(1)))
    }

    /// Formats active typing users into localized natural language string
    pub fn get_active_typing_text(&self, channel_id: &str) -> Option<String> {
        let users = self.get_active_typing_users(channel_id);
        match users.len() {
            0 => None,
            1 => Some(format!("{} está digitando...", users[0])),
            2 => Some(format!("{} e {} estão digitando...", users[0], users[1])),
            3 => Some(format!(
                "{}, {} e {} estão digitando...",
                users[0], users[1], users[2]
            )),
            _ => Some("Várias pessoas estão digitando...".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Author;
    use crate::protocol::ClientManager;

    fn synthetic_state() -> AppState {
        let mut state = AppState::default();
        state.current_user.id = "user-synthetic-send".into();
        state.current_user.username = "synthetic_send".into();
        state.current_user.display_name = "Synthetic Send".into();
        state.client_manager = Some(ClientManager::new_mock().0);
        state.selected_channel_id = Some("channel-synthetic-send".into());
        state
    }

    fn confirmed_message(temp: &Message) -> Message {
        let mut message = Message::new(
            "message-synthetic-confirmed",
            temp.channel_id.clone(),
            temp.author.clone(),
            temp.content.clone(),
        );
        message.is_pending = false;
        message
    }

    #[test]
    fn optimistic_send_rest_confirmation_is_idempotent() {
        let mut state = synthetic_state();
        let temp_id = state
            .begin_message_send("channel-synthetic-send", "Mensagem sintética")
            .unwrap();
        assert!(pending_send(&temp_id).unwrap().client_nonce.len() <= 25);
        let pending = state
            .messages_by_channel
            .get("channel-synthetic-send")
            .unwrap()
            .iter()
            .find(|message| message.id == temp_id)
            .unwrap()
            .clone();
        let confirmed = confirmed_message(&pending);

        state.confirm_message_send(&temp_id, confirmed.clone());
        state.confirm_message_send(&temp_id, confirmed);

        let messages = &state.messages_by_channel["channel-synthetic-send"];
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "message-synthetic-confirmed");
        assert!(!messages[0].is_pending);
    }

    #[test]
    fn gateway_before_rest_does_not_duplicate_or_report_failure() {
        let mut state = synthetic_state();
        let temp_id = state
            .begin_message_send("channel-synthetic-send", "Gateway primeiro")
            .unwrap();
        let pending = state
            .messages_by_channel
            .get("channel-synthetic-send")
            .unwrap()
            .iter()
            .find(|message| message.id == temp_id)
            .unwrap()
            .clone();
        let confirmed = confirmed_message(&pending);

        state.handle_transport_event(TransportEvent::MessageCreate(confirmed.clone()));
        state.confirm_message_send(&temp_id, confirmed.clone());
        assert!(state.fail_message_send(&temp_id).is_none());
        assert_eq!(state.messages_by_channel["channel-synthetic-send"].len(), 1);
    }

    #[tokio::test]
    async fn gateway_confirmation_in_resolved_dm_suppresses_rest_failure() {
        let mut state = synthetic_state();
        state.open_dm_with_user("user-synthetic-dm", "Synthetic DM");
        let temp_channel = state.selected_channel_id.clone().unwrap();
        let temp_id = state
            .begin_message_send(&temp_channel, "DM sintética")
            .unwrap();
        let pending = state.messages_by_channel[&temp_channel]
            .iter()
            .find(|message| message.id == temp_id)
            .unwrap()
            .clone();

        let real_channel = crate::domain::channel::Channel::new(
            "channel-synthetic-dm-real",
            "Synthetic DM",
            crate::domain::channel::ChannelType::Dm,
        )
        .with_recipient_ids(vec!["user-synthetic-dm".to_string()]);
        state.handle_transport_event(TransportEvent::DmChannelResolved {
            temp_id: temp_channel,
            channel: real_channel.clone(),
        });

        let mut confirmed = confirmed_message(&pending);
        confirmed.channel_id = real_channel.id.clone();
        state.handle_transport_event(TransportEvent::MessageCreate(confirmed));

        assert!(state.fail_message_send(&temp_id).is_none());
        let messages = &state.messages_by_channel[&real_channel.id];
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "message-synthetic-confirmed");
        assert!(!messages[0].is_pending);
    }

    #[test]
    fn failed_send_is_removed_and_retry_payload_is_returned() {
        let mut state = synthetic_state();
        let temp_id = state
            .begin_message_send("channel-synthetic-send", "Falhou sinteticamente")
            .unwrap();
        let retry = state.fail_message_send(&temp_id);
        assert_eq!(
            retry,
            Some((
                "channel-synthetic-send".into(),
                "Falhou sinteticamente".into()
            ))
        );
        assert!(state.messages_by_channel["channel-synthetic-send"].is_empty());
    }

    #[test]
    fn retry_after_failure_confirms_exactly_one_row() {
        let mut state = synthetic_state();
        let first_temp = state
            .begin_message_send("channel-synthetic-send", "Mensagem repetida")
            .unwrap();
        let (channel_id, content) = state.fail_message_send(&first_temp).unwrap();

        let retry_temp = state.begin_message_send(&channel_id, &content).unwrap();
        let pending = state.messages_by_channel[&channel_id]
            .iter()
            .find(|message| message.id == retry_temp)
            .unwrap()
            .clone();
        state.confirm_message_send(&retry_temp, confirmed_message(&pending));

        let messages = &state.messages_by_channel[&channel_id];
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "message-synthetic-confirmed");
        assert!(!messages[0].is_pending);
    }

    #[test]
    fn older_history_merge_deduplicates_and_preserves_pending() {
        let mut state = synthetic_state();
        state
            .begin_message_send("channel-synthetic-send", "Pendente")
            .unwrap();
        let older = vec![
            Message::new(
                "old-1",
                "channel-synthetic-send",
                Author::new("user-old", "old"),
                "Antiga 1",
            ),
            Message::new(
                "old-2",
                "channel-synthetic-send",
                Author::new("user-old", "old"),
                "Antiga 2",
            ),
        ];
        assert_eq!(
            state.merge_older_messages("channel-synthetic-send", older.clone()),
            2
        );
        assert_eq!(
            state.merge_older_messages("channel-synthetic-send", older),
            0
        );
        let messages = &state.messages_by_channel["channel-synthetic-send"];
        assert_eq!(messages.len(), 3);
        assert!(messages.iter().any(|message| message.is_pending));
    }

    #[test]
    fn legacy_dispatch_failure_is_drained_without_pending_row() {
        let mut state = synthetic_state();
        let temp_id = state
            .begin_message_send("channel-synthetic-send", "Falha legada")
            .unwrap();
        mark_message_send_failed(
            &temp_id,
            "channel-synthetic-send",
            "Falha legada",
            "erro sintético",
        );
        let failures = state.take_failed_message_sends();
        assert!(failures.iter().any(|(channel, content, error)| {
            channel == "channel-synthetic-send"
                && content == "Falha legada"
                && error == "erro sintético"
        }));
        assert!(state.messages_by_channel["channel-synthetic-send"].is_empty());
    }

    #[test]
    fn typing_refresh_is_scheduled_only_for_active_users() {
        let mut state = synthetic_state();
        assert!(state
            .typing_refresh_delay("channel-synthetic-send")
            .is_none());
        state.record_typing("channel-synthetic-send", "user-other", Some("Other"));
        assert!(state
            .typing_refresh_delay("channel-synthetic-send")
            .is_some());
        let own_id = state.current_user.id.clone();
        state.record_typing("channel-synthetic-send", &own_id, Some("Eu"));
        assert_eq!(
            state.get_active_typing_users("channel-synthetic-send"),
            vec!["Other"]
        );
    }
}
