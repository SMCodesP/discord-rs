//! Discord RS - Chat View Component
//! Complete reactive chat stream combining kit v_virtual_list message stream,
//! welcome header, hover actions, active replies, popover media picker, and message editor.

use gpui::component::input::{InputEvent, TextareaState};
use gpui::*;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::app::state::AppState;
use crate::chat::editor::{ChatComposer, EditorActionCallback, EditorSendCallback};
use crate::chat::media_picker::{
    gif_query_from_picker, new_gifs_client, MediaPickerConfig, MediaPickerGuildSection,
    MediaPickerModal, MediaPickerTab, OnCloseMediaPickerCallback, OnPlaySoundCallback,
    OnSearchQueryCallback, OnSelectEmojiCallback, OnSelectGifCallback, OnSelectGifCategoryCallback,
    OnSelectGuildCallback, OnSelectStickerCallback, OnTabChangeCallback,
};
use crate::chat::message_list::{
    build_chat_list_items, render_date_separator_element, render_message_context_menu,
    render_message_row, render_welcome_banner, ChatListItem, MessageActionCallback,
    MessageActionPosCallback, MessageContextMenuConfig, MessageRowConfig,
};
use crate::chat::reactions::{AddReactionCallback, ReactionActionCallback};
use crate::domain::message::Message;
use crate::protocol::gifs_client::{GifItem, GifsClient};
use crate::protocol::media_cache::MediaCacheManager;
use crate::tokens::colors::white;
use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PendingPinAction {
    pub message: Message,
    pub is_unpin: bool,
}

fn render_chat_error_toast(
    message: String,
    retry_label: Option<&'static str>,
    on_retry: Option<EditorActionCallback>,
    on_dismiss: Option<EditorActionCallback>,
) -> Option<AnyElement> {
    let mut actions = div().flex().flex_row().items_center().gap(space_2());
    if let (Some(label), Some(handler)) = (retry_label, on_retry) {
        actions = actions.child(
            div()
                .cursor_pointer()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_BOLD)
                .text_color(blurple())
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    handler(window, cx);
                })
                .child(label),
        );
    }
    if let Some(handler) = on_dismiss {
        actions = actions.child(
            div()
                .cursor_pointer()
                .text_size(font_size_xs())
                .text_color(status_danger())
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    handler(window, cx);
                })
                .child("✕"),
        );
    }

    Some(
        div()
            .mx(space_4())
            .mb(space_2())
            .p(space_2())
            .rounded(radius_sm())
            .bg(with_alpha(HEX_STATUS_DANGER, 0.15))
            .border_1()
            .border_color(status_danger())
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(space_2())
            .child(
                div()
                    .text_size(font_size_xs())
                    .text_color(status_danger())
                    .child(message),
            )
            .child(actions)
            .into_any_element(),
    )
}

/// Reactive Chat View managing real messages and input state for the active channel.
pub struct ChatView {
    pub state: Entity<AppState>,
    pub replying_to: Option<Message>,
    pub reply_mention_author: bool,
    pub typed_text: String,
    pub typing_users: Vec<String>,
    pub focus_handle: FocusHandle,
    pub search_focus_handle: FocusHandle,
    pub scroll_handle: ScrollHandle,
    pub list_state: ListState,
    pub last_channel_id: Option<String>,
    pub last_message_count: usize,
    pub cached_chat_items: Arc<Vec<ChatListItem>>,
    pub cached_chat_items_channel: Option<String>,
    pub cached_chat_items_len: usize,
    pub cached_chat_items_first_id: Option<String>,
    pub cached_chat_items_last_id: Option<String>,
    pub cached_chat_items_signature: u64,
    pub media_picker_open: bool,
    pub media_picker_tab: MediaPickerTab,
    pub media_picker_search: String,
    pub media_picker_react_message_id: Option<String>,
    pub media_picker_focus_handle: FocusHandle,
    pub media_picker_selected_guild_id: Option<String>,
    pub media_picker_list_state: ListState,
    pub media_picker_gifs: Vec<GifItem>,
    pub gif_request_seq: u64,
    pub gifs_client: Arc<GifsClient>,
    pub active_context_menu: Option<(String, Point<Pixels>)>,
    pub pending_delete_message: Option<Message>,
    pub is_deleting: bool,
    pub delete_error: Option<String>,
    pub pending_pin_action: Option<PendingPinAction>,
    pub editing_message: Option<Message>,
    pub edit_error: Option<String>,
    pub is_saving_edit: bool,
    pub edit_request_sequence: u64,
    pub pin_error: Option<String>,
    pub send_error: Option<String>,
    pub retry_send: Option<(String, String)>,
    pub is_sending_message: bool,
    pub history_loading: bool,
    pub history_error: Option<String>,
    pub last_history_limit: usize,
    pub scroll_handler_installed: bool,
    pub typing_refresh_task: Option<Task<()>>,
    pub composer_state: Option<Entity<TextareaState>>,
    pub composer_sub: Option<Subscription>,
    pub pending_jump_request: Option<(String, String, u64)>,
    pub jump_request_seq: u64,
    pub highlight_clear_task: Option<Task<()>>,
}

fn chat_items_signature(messages: &[Message]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    messages.len().hash(&mut hasher);
    for msg in messages {
        msg.id.hash(&mut hasher);
        msg.edited_timestamp.hash(&mut hasher);
        msg.is_pinned.hash(&mut hasher);
        msg.is_pending.hash(&mut hasher);
        msg.reactions.len().hash(&mut hasher);
        for reaction in &msg.reactions {
            reaction.emoji.hash(&mut hasher);
            reaction.count.hash(&mut hasher);
            reaction.me.hash(&mut hasher);
        }
    }
    hasher.finish()
}

impl ChatView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let search_focus_handle = cx.focus_handle();
        let media_picker_focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        // Virtual ListState configured with bottom alignment (canonical chat stream)
        let list_state = ListState::new(0, ListAlignment::Bottom, px(250.0));
        list_state.set_follow_mode(FollowMode::Tail);
        // Top-aligned ListState for rich media picker popup with 150px overdraw windowing
        let media_picker_list_state = ListState::new(0, ListAlignment::Top, px(150.0));
        let gifs_client = Arc::new(new_gifs_client(reqwest::Client::new()));
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        Self {
            state,
            replying_to: None,
            reply_mention_author: true,
            typed_text: String::new(),
            typing_users: Vec::new(),
            focus_handle,
            search_focus_handle,
            media_picker_focus_handle,
            scroll_handle,
            list_state,
            last_channel_id: None,
            last_message_count: 0,
            cached_chat_items: Arc::new(Vec::new()),
            cached_chat_items_channel: None,
            cached_chat_items_len: 0,
            cached_chat_items_first_id: None,
            cached_chat_items_last_id: None,
            cached_chat_items_signature: 0,
            media_picker_open: false,
            media_picker_tab: MediaPickerTab::Emojis,
            media_picker_search: String::new(),
            media_picker_react_message_id: None,
            media_picker_selected_guild_id: None,
            media_picker_list_state,
            media_picker_gifs: Vec::new(),
            gif_request_seq: 0,
            gifs_client,
            active_context_menu: None,
            pending_delete_message: None,
            is_deleting: false,
            delete_error: None,
            pending_pin_action: None,
            editing_message: None,
            edit_error: None,
            is_saving_edit: false,
            edit_request_sequence: 0,
            pin_error: None,
            send_error: None,
            retry_send: None,
            is_sending_message: false,
            history_loading: false,
            history_error: None,
            last_history_limit: 0,
            scroll_handler_installed: false,
            typing_refresh_task: None,
            composer_state: None,
            composer_sub: None,
            pending_jump_request: None,
            jump_request_seq: 0,
            highlight_clear_task: None,
        }
    }

    pub fn set_typed_text(&mut self, text: impl Into<String>) {
        self.typed_text = text.into();
    }

    pub fn set_replying_to(&mut self, message: Option<Message>) {
        self.replying_to = message;
        if self.replying_to.is_some() {
            self.reply_mention_author = true;
        }
    }

    pub fn set_typing_users(&mut self, users: Vec<String>) {
        self.typing_users = users;
    }

    pub fn start_editing_message(
        &mut self,
        message: Message,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let content = message.content.clone();
        self.editing_message = Some(message);
        self.edit_error = None;
        self.is_saving_edit = false;
        self.edit_request_sequence = self.edit_request_sequence.wrapping_add(1);
        self.replying_to = None;
        self.typed_text = content.clone();
        if let (Some(window), Some(state)) = (window, &self.composer_state) {
            state.update(cx, |state, cx| {
                state.set_value(content, window, cx);
                let line_count = state.value().lines().count().max(1) as u32;
                let last_line = state
                    .value()
                    .lines()
                    .last()
                    .unwrap_or_default()
                    .chars()
                    .count() as u32;
                state.set_cursor_position(
                    gpui::component::input::Position {
                        line: line_count - 1,
                        character: last_line,
                    },
                    window,
                    cx,
                );
            });
        }
        cx.notify();
    }

    pub fn cancel_editing(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.edit_request_sequence = self.edit_request_sequence.wrapping_add(1);
        self.editing_message = None;
        self.edit_error = None;
        self.is_saving_edit = false;
        self.typed_text.clear();
        if let (Some(window), Some(state)) = (window, &self.composer_state) {
            state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }
        cx.notify();
    }

    /// Envia a edição de uma mensagem via ClientManager sem mutação otimista.
    pub fn submit_edit_message(
        &mut self,
        channel_id: String,
        message_id: String,
        text: String,
        cm: Option<crate::protocol::ClientManager>,
        cx: &mut Context<Self>,
    ) {
        if self.is_saving_edit {
            return;
        }
        let Some(cm) = cm else {
            self.edit_error =
                Some("Não conectado ao Discord: cliente não inicializado".to_string());
            cx.notify();
            return;
        };

        self.edit_error = None;
        self.is_saving_edit = true;
        let request_sequence = self.edit_request_sequence;
        let expected_message_id = message_id.clone();
        let this_entity = cx.entity().clone();
        cx.notify();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                match cm.edit_message(&channel_id, &message_id, &text).await {
                    Ok(updated_message) => {
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                if chat.edit_request_sequence == request_sequence
                                    && chat
                                        .editing_message
                                        .as_ref()
                                        .map(|message| message.id.as_str())
                                        == Some(expected_message_id.as_str())
                                {
                                    chat.cancel_editing(None, cx);
                                }
                            });
                        });
                        cm.emit_event(crate::protocol::TransportEvent::MessageUpdate(
                            updated_message,
                        ));
                    }
                    Err(error) => {
                        let error = error.to_string();
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                if chat.edit_request_sequence == request_sequence
                                    && chat
                                        .editing_message
                                        .as_ref()
                                        .map(|message| message.id.as_str())
                                        == Some(expected_message_id.as_str())
                                {
                                    chat.is_saving_edit = false;
                                    chat.edit_error = Some(error);
                                    cx.notify();
                                }
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn submit_composer_message(
        &mut self,
        channel_id: String,
        text: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_sending_message {
            return;
        }
        let Some(client_manager) = self.state.read(cx).client_manager.clone() else {
            self.send_error =
                Some("Não conectado ao Discord: cliente não inicializado".to_string());
            cx.notify();
            return;
        };
        let temp_id = match self.state.update(cx, |state, _cx| {
            state.begin_message_send(&channel_id, &text)
        }) {
            Ok(temp_id) => temp_id,
            Err(error) => {
                self.send_error = Some(error);
                cx.notify();
                return;
            }
        };

        self.send_error = None;
        self.history_error = None;
        self.retry_send = None;
        self.is_sending_message = true;
        cx.notify();

        let this_entity = cx.entity().clone();
        let target_channel = channel_id.clone();
        let content = text.clone();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let request = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    client_manager.send_message(&target_channel, &content),
                )
                .await;
                let result = match request {
                    Ok(result) => result,
                    Err(_) => Err(anyhow::anyhow!(
                        "Tempo limite de 15s excedido ao confirmar o envio"
                    )),
                };
                match result {
                    Ok(confirmed) => {
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                chat.state.update(cx, |state, _cx| {
                                    state.confirm_message_send(&temp_id, confirmed);
                                    state.replying_to_message = None;
                                    state.active_draft.clear();
                                });
                                chat.is_sending_message = false;
                                if chat.last_channel_id.as_deref() == Some(target_channel.as_str())
                                {
                                    chat.typed_text.clear();
                                    chat.set_replying_to(None);
                                    chat.list_state.scroll_to_end();
                                    chat.scroll_handle.scroll_to_bottom();
                                }
                                cx.notify();
                            });
                        });
                    }
                    Err(error) => {
                        let error = error.to_string();
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                let retry = chat
                                    .state
                                    .update(cx, |state, _cx| state.fail_message_send(&temp_id));
                                chat.is_sending_message = false;
                                if let Some(retry) = retry {
                                    chat.send_error =
                                        Some(format!("Falha ao enviar mensagem: {}", error));
                                    chat.retry_send = Some(retry);
                                } else {
                                    // Gateway confirmation may have arrived while
                                    // the REST request was failing or timing out.
                                    // Do not report a false failure or keep the
                                    // composer in a sending state.
                                    chat.send_error = None;
                                    chat.retry_send = None;
                                    chat.state.update(cx, |state, _cx| {
                                        state.replying_to_message = None;
                                        state.active_draft.clear();
                                    });
                                    if chat.last_channel_id.as_deref()
                                        == Some(target_channel.as_str())
                                    {
                                        chat.typed_text.clear();
                                        chat.set_replying_to(None);
                                        chat.list_state.scroll_to_end();
                                        chat.scroll_handle.scroll_to_bottom();
                                    }
                                }
                                cx.notify();
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn retry_failed_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((channel_id, content)) = self.retry_send.clone() else {
            return;
        };
        self.retry_send = None;
        self.submit_composer_message(channel_id, content, window, cx);
    }

    fn refresh_media_picker_gifs(&mut self, cx: &mut Context<Self>) {
        if !self.media_picker_open || self.media_picker_tab != MediaPickerTab::Gifs {
            return;
        }
        self.gif_request_seq = self.gif_request_seq.wrapping_add(1);
        let seq = self.gif_request_seq;
        let query = gif_query_from_picker(&self.media_picker_search);
        let client = self.gifs_client.clone();
        let this_entity = cx.entity().clone();

        cx.spawn(move |_view, async_cx: &mut AsyncApp| {
            let mut async_cx = async_cx.clone();
            async move {
                let result = if query.query.trim().is_empty() {
                    client.trending(query).await
                } else {
                    client.search(query).await
                };
                let _ = this_entity.update(&mut async_cx, |this, cx| {
                    if seq != this.gif_request_seq {
                        return;
                    }
                    match result {
                        Ok(gifs) => {
                            this.media_picker_gifs = gifs;
                        }
                        Err(err) => {
                            log::warn!("Falha ao carregar GIFs: {}", err);
                            this.media_picker_gifs.clear();
                        }
                    }
                    this.media_picker_list_state.reset(0);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn load_more_history(&mut self, cx: &mut Context<Self>) {
        if self.history_loading {
            return;
        }
        let Some(client_manager) = self.state.read(cx).client_manager.clone() else {
            self.history_error =
                Some("Não conectado ao Discord: cliente não inicializado".to_string());
            cx.notify();
            return;
        };
        let Some(channel_id) = self.state.read(cx).selected_channel_id.clone() else {
            return;
        };
        let (message_count, anchor_message_id) = {
            let state = self.state.read(cx);
            let messages = state.get_active_messages();
            (
                messages.len(),
                messages.first().map(|message| message.id.clone()),
            )
        };
        if message_count == 0 {
            return;
        }

        // The current transport only exposes `get_messages(limit)`. Grow the
        // latest-page window once, up to Discord's maximum of 100. A true
        // `before` cursor requires a protocol/transport change outside this task.
        let requested_limit = (message_count + 50).min(100);
        if requested_limit <= self.last_history_limit {
            self.history_error = Some("Histórico mais antigo indisponível: o transporte atual não oferece cursor `before`.".to_string());
            cx.notify();
            return;
        }
        self.last_history_limit = requested_limit;
        self.history_loading = true;
        self.history_error = None;
        cx.notify();

        let this_entity = cx.entity().clone();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                match client_manager
                    .get_messages(&channel_id, requested_limit)
                    .await
                {
                    Ok(older) => {
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                chat.state.update(cx, |state, _cx| {
                                    state.merge_older_messages(&channel_id, older);
                                });
                                chat.history_loading = false;
                                if let Some(anchor_id) = anchor_message_id {
                                    let target_index = {
                                        let state = chat.state.read(cx);
                                        let items =
                                            build_chat_list_items(state.get_active_messages());
                                        items.iter().position(|item| match item {
                                            ChatListItem::Message { message_idx, .. } => state
                                                .get_active_messages()
                                                .get(*message_idx)
                                                .map(|message| message.id == anchor_id)
                                                .unwrap_or(false),
                                            _ => false,
                                        })
                                    };
                                    if let Some(index) = target_index {
                                        chat.list_state.scroll_to_reveal_item(index);
                                    }
                                }
                                cx.notify();
                            });
                        });
                    }
                    Err(error) => {
                        let _ = cx.update(|cx| {
                            let _ = this_entity.update(cx, |chat, cx| {
                                chat.history_loading = false;
                                chat.history_error =
                                    Some(format!("Falha ao carregar mensagens antigas: {}", error));
                                cx.notify();
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn ensure_scroll_handler(&mut self, cx: &mut Context<Self>) {
        if self.scroll_handler_installed {
            return;
        }
        let this_entity = cx.entity().clone();
        self.list_state
            .set_scroll_handler(move |event, _window, cx| {
                if event.is_scrolled && event.visible_range.start <= 1 {
                    this_entity.update(cx, |chat, cx| chat.load_more_history(cx));
                }
            });
        self.scroll_handler_installed = true;
    }

    fn schedule_highlight_clear(&mut self, message_id: String, cx: &mut Context<Self>) {
        let this_entity = cx.entity().clone();
        self.highlight_clear_task = Some(cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = cx.update(|cx| {
                    let _ = this_entity.update(cx, |chat, cx| {
                        let is_same = chat
                            .state
                            .read(cx)
                            .highlighted_message_id
                            .as_deref()
                            == Some(message_id.as_str());
                        if is_same {
                            chat.state.update(cx, |state, _cx| {
                                state.clear_message_highlight();
                            });
                            cx.notify();
                        }
                    });
                });
            }
        }));
    }

    fn resolve_target_jump(&mut self, cx: &mut Context<Self>) {
        let (target_id, jump_channel_id, selected_channel_id, client_manager) = {
            let state = self.state.read(cx);
            (
                state.target_message_id.clone(),
                state.jump_target_channel_id.clone(),
                state.selected_channel_id.clone(),
                state.client_manager.clone(),
            )
        };
        let Some(target_id) = target_id else {
            return;
        };
        let Some(channel_id) = jump_channel_id.or(selected_channel_id) else {
            self.state.update(cx, |state, _cx| {
                state.target_message_id = None;
                state.jump_target_channel_id = None;
                state.set_jump_error(Some(
                    "Mensagem não encontrada: nenhum canal selecionado.".to_string(),
                ));
            });
            return;
        };

        let (channel_loaded, message_exists) = {
            let state = self.state.read(cx);
            let loaded = state.selected_channel_id.as_deref() == Some(channel_id.as_str());
            let exists = loaded
                && state
                    .get_active_messages()
                    .iter()
                    .any(|m| m.id == target_id);
            (loaded, exists)
        };
        if !channel_loaded {
            return;
        }
        if message_exists {
            self.state.update(cx, |state, _cx| {
                state.target_message_id = None;
                state.jump_target_channel_id = None;
                state.set_jump_error(None);
                state.highlighted_message_id = Some(target_id.clone());
            });
            self.schedule_highlight_clear(target_id, cx);
            cx.notify();
            return;
        }

        if self
            .pending_jump_request
            .as_ref()
            .is_some_and(|(cid, mid, _)| cid == &channel_id && mid == &target_id)
        {
            return;
        }

        let Some(cm) = client_manager else {
            self.state.update(cx, |state, _cx| {
                state.target_message_id = None;
                state.jump_target_channel_id = None;
                state.set_jump_error(Some(
                    "Não conectado à API do Discord para carregar a mensagem.".to_string(),
                ));
            });
            return;
        };

        self.jump_request_seq = self.jump_request_seq.wrapping_add(1);
        let seq = self.jump_request_seq;
        self.pending_jump_request = Some((channel_id.clone(), target_id.clone(), seq));
        self.state.update(cx, |state, _cx| {
            state.set_jump_error(None);
        });

        let this_entity = cx.entity().clone();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = cm.get_messages_around(&channel_id, &target_id, 50).await;
                let _ = cx.update(|cx| {
                    let _ = this_entity.update(cx, |chat, cx| {
                        let is_current = chat
                            .pending_jump_request
                            .as_ref()
                            .is_some_and(|(cid, mid, s)| {
                                *s == seq && cid == &channel_id && mid == &target_id
                            });
                        if !is_current {
                            return;
                        }
                        chat.pending_jump_request = None;
                        match result {
                            Ok(messages) => {
                                let found = messages.iter().any(|m| m.id == target_id);
                                chat.state.update(cx, |state, _cx| {
                                    state.set_channel_messages(&channel_id, messages);
                                    if found {
                                        state.target_message_id = Some(target_id.clone());
                                        state.set_jump_error(None);
                                    } else {
                                        state.target_message_id = None;
                                        state.jump_target_channel_id = None;
                                        state.highlighted_message_id = None;
                                        state.set_jump_error(Some(
                                            "Mensagem não encontrada ou sem permissão para acessá-la."
                                                .to_string(),
                                        ));
                                    }
                                });
                                if found {
                                    // Next render will scroll/highlight once item exists.
                                }
                                cx.notify();
                            }
                            Err(err) => {
                                let msg = err.to_string();
                                let is_not_found = msg.contains("404")
                                    || msg.to_lowercase().contains("not found")
                                    || msg.to_lowercase().contains("unknown message");
                                let is_forbidden = msg.contains("403")
                                    || msg.to_lowercase().contains("forbidden")
                                    || msg.to_lowercase().contains("missing access")
                                    || msg.to_lowercase().contains("sem permissão");
                                let user_msg = if is_not_found || is_forbidden {
                                    "Mensagem não encontrada ou sem permissão para acessá-la."
                                        .to_string()
                                } else {
                                    format!("Falha ao carregar mensagem: {}", msg)
                                };
                                chat.state.update(cx, |state, _cx| {
                                    state.target_message_id = None;
                                    state.jump_target_channel_id = None;
                                    state.highlighted_message_id = None;
                                    state.set_jump_error(Some(user_msg));
                                });
                                cx.notify();
                            }
                        }
                    });
                });
            }
        })
        .detach();
    }

    pub fn ensure_composer_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextareaState> {
        if let Some(ref st) = self.composer_state {
            return st.clone();
        }

        let st = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .submit_on_enter(true)
        });

        if !self.typed_text.is_empty() {
            let initial = self.typed_text.clone();
            st.update(cx, |s, cx| {
                s.set_value(initial, window, cx);
            });
        }

        let st_for_sub = st.clone();
        let sub = cx.subscribe_in(
            &st,
            window,
            move |this, _textarea, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => {
                    if this.is_sending_message {
                        return;
                    }
                    let content = st_for_sub.read(cx).value().to_string();
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        let text = trimmed.to_string();
                        if let Some(editing_msg) = this.editing_message.clone() {
                            let cid = if let Some((_, ch)) =
                                this.state.read(cx).get_selected_channel()
                            {
                                ch.id.clone()
                            } else if let Some(ref cid) = this.state.read(cx).selected_channel_id {
                                cid.clone()
                            } else {
                                editing_msg.channel_id.clone()
                            };
                            let mid = editing_msg.id.clone();
                            let cm = this.state.read(cx).client_manager.clone();
                            this.submit_edit_message(cid, mid, text, cm, cx);
                            return;
                        }

                        let channel_id = {
                            let state = this.state.read(cx);
                            state
                                .get_selected_channel()
                                .map(|(_, channel)| channel.id.clone())
                                .or_else(|| state.selected_channel_id.clone())
                        };
                        let Some(channel_id) = channel_id else {
                            this.send_error = Some("Nenhum canal selecionado".to_string());
                            cx.notify();
                            return;
                        };
                        this.submit_composer_message(channel_id, text, window, cx);
                    }
                }
                InputEvent::Change => {
                    let v = st_for_sub.read(cx).value().to_string();
                    this.typed_text = v;
                }
                _ => {}
            },
        );

        self.composer_state = Some(st.clone());
        self.composer_sub = Some(sub);
        st
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_scroll_handler(cx);
        let composer_state = self.ensure_composer_state(window, cx);
        if composer_state.read(cx).value() != self.typed_text.as_str() {
            let t = self.typed_text.clone();
            composer_state.update(cx, |s, cx| {
                s.set_value(t, window, cx);
            });
        }
        let failed_sends = self
            .state
            .update(cx, |state, _cx| state.take_failed_message_sends());
        if let Some((channel_id, content, error)) = failed_sends.into_iter().last() {
            self.send_error = Some(format!("Falha ao enviar mensagem: {}", error));
            self.retry_send = Some((channel_id, content));
        }
        let app_state = self.state.read(cx);
        let selected_channel_opt = if let Some((_, ch)) = app_state.get_selected_channel() {
            Some(ch.clone())
        } else if let Some(ref cid) = app_state.selected_channel_id {
            app_state.dm_channels.iter().find(|d| &d.id == cid).cloned()
        } else {
            None
        };
        let channel_name = selected_channel_opt
            .as_ref()
            .map(|channel| channel.name.clone())
            .or_else(|| app_state.selected_channel_id.clone())
            .unwrap_or_default();
        let channel_id = selected_channel_opt
            .as_ref()
            .map(|channel| channel.id.clone())
            .or_else(|| app_state.selected_channel_id.clone())
            .unwrap_or_default();
        let is_dm = app_state.selected_guild_id.is_none()
            || app_state.dm_channels.iter().any(|d| d.id == channel_id);
        let channel_topic = selected_channel_opt
            .as_ref()
            .and_then(|c| c.topic.clone())
            .unwrap_or_else(|| {
                if channel_name.is_empty() {
                    String::new()
                } else if is_dm {
                    format!(
                        "Este é o início do seu histórico de mensagens diretas com {}.",
                        channel_name
                    )
                } else {
                    format!("Este é o início do canal #{}.", channel_name)
                }
            });

        // Resolve DM recipient avatar and decoration if this is a DM channel
        let dm_channel_state = if is_dm {
            app_state.dm_channels.iter().find(|d| d.id == channel_id)
        } else {
            None
        };
        let recipient_friend = if let Some(dm) = dm_channel_state {
            dm.recipient_ids
                .first()
                .and_then(|rid| app_state.friends.iter().find(|f| &f.id == rid))
        } else {
            None
        };
        let all_messages = app_state.get_active_messages();
        let recipient_msg_author = if is_dm {
            all_messages
                .iter()
                .find(|m| m.author.id != app_state.current_user.id)
                .map(|m| &m.author)
        } else {
            None
        };
        let dm_avatar_src: Option<SharedString> = if is_dm {
            dm_channel_state
                .and_then(|d| d.icon_url.as_ref())
                .or_else(|| recipient_friend.and_then(|f| f.avatar_url.as_ref()))
                .or_else(|| recipient_msg_author.and_then(|a| a.avatar.as_ref()))
                .map(|s| SharedString::from(s.clone()))
        } else {
            None
        };
        let dm_avatar_decoration = if is_dm {
            dm_channel_state
                .and_then(|d| d.avatar_decoration_data.clone())
                .or_else(|| recipient_friend.and_then(|f| f.avatar_decoration_data.clone()))
                .or_else(|| recipient_msg_author.and_then(|a| a.avatar_decoration_data.clone()))
        } else {
            None
        };

        // ZERO DEEP CLONES: borrow direct slice from AppState
        let message_count = all_messages.len();
        let first_id = all_messages.first().map(|m| m.id.as_str());
        let last_id = all_messages.last().map(|m| m.id.as_str());
        let signature = chat_items_signature(all_messages);

        let channel_changed = self.last_channel_id.as_deref() != Some(&channel_id);
        let switched_from_channel = self.last_channel_id.is_some();
        let items_changed = channel_changed
            || self.cached_chat_items_channel.as_deref() != Some(&channel_id)
            || self.cached_chat_items_len != message_count
            || self.cached_chat_items_first_id.as_deref() != first_id
            || self.cached_chat_items_last_id.as_deref() != last_id
            || self.cached_chat_items_signature != signature
            || self.list_state.item_count() == 0;
        if items_changed {
            self.cached_chat_items = Arc::new(build_chat_list_items(all_messages));
            self.cached_chat_items_channel = Some(channel_id.clone());
            self.cached_chat_items_len = message_count;
            self.cached_chat_items_first_id = first_id.map(str::to_string);
            self.cached_chat_items_last_id = last_id.map(str::to_string);
            self.cached_chat_items_signature = signature;
        }
        let chat_items = self.cached_chat_items.clone();
        let total_items = chat_items.len();

        // Synchronize scroll state when channel switches or message count changes.
        // Preserve the visible anchor while the user is reading older messages.
        let count_changed = message_count != self.last_message_count;
        if channel_changed {
            self.last_channel_id = Some(channel_id.clone());
            self.last_history_limit = 0;
            self.history_loading = false;
            self.history_error = None;
            self.send_error = None;
            self.retry_send = None;
            if switched_from_channel {
                self.replying_to = None;
                self.editing_message = None;
                self.is_saving_edit = false;
                self.edit_request_sequence = self.edit_request_sequence.wrapping_add(1);
                self.typed_text.clear();
            }
            self.list_state.set_follow_mode(FollowMode::Tail);
        }
        if channel_changed || count_changed || self.list_state.item_count() != total_items {
            let at_bottom = channel_changed
                || self.list_state.is_following_tail()
                || self.list_state.is_scrolled_to_end() != Some(false)
                || self.list_state.logical_scroll_top().item_ix
                    >= self.list_state.item_count().saturating_sub(1);
            let anchor_id = if at_bottom {
                None
            } else {
                let top_index = self.list_state.logical_scroll_top().item_ix;
                chat_items.get(top_index).and_then(|item| match item {
                    ChatListItem::Message { message_idx, .. } => all_messages
                        .get(*message_idx)
                        .map(|message| message.id.clone()),
                    _ => None,
                })
            };
            self.last_message_count = message_count;
            self.list_state.reset(total_items);
            if at_bottom {
                self.list_state.scroll_to_end();
                self.scroll_handle.scroll_to_bottom();
            } else if let Some(anchor_id) = anchor_id {
                if let Some(index) = chat_items.iter().position(|item| match item {
                    ChatListItem::Message { message_idx, .. } => all_messages
                        .get(*message_idx)
                        .map(|message| message.id == anchor_id)
                        .unwrap_or(false),
                    _ => false,
                }) {
                    self.list_state.scroll_to_reveal_item(index);
                }
            }
        }

        if switched_from_channel && channel_changed {
            composer_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }

        let app_entity = self.state.clone();
        let self_entity = cx.entity().clone();

        // 1. Message Action: React -> opens emoji picker for that specific message
        let self_for_react = self_entity.clone();
        let app_for_react = app_entity.clone();
        let on_react: Option<MessageActionCallback> = Some(Arc::new(
            move |message_id: &str, window: &mut Window, cx: &mut App| {
                let msg_id = message_id.to_string();
                app_for_react.update(cx, |state, _cx| {
                    state.sync_guild_media();
                });
                self_for_react.update(cx, |this, cx| {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Emojis;
                    this.media_picker_react_message_id = Some(msg_id);
                    this.media_picker_list_state.reset(0);
                    this.active_context_menu = None;
                    window.focus(&this.media_picker_focus_handle, cx);
                    cx.notify();
                });
            },
        ));

        // 2. Message Action: Reply
        let cid_for_reply = channel_id.clone();
        let app_for_reply = app_entity.clone();
        let self_for_reply = self_entity.clone();
        let on_reply: Option<MessageActionCallback> = Some(Arc::new(
            move |message_id: &str, _window: &mut Window, cx: &mut App| {
                let msg_id = message_id.to_string();
                let target_msg = app_for_reply
                    .read(cx)
                    .messages_by_channel
                    .get(&cid_for_reply)
                    .and_then(|msgs| msgs.iter().find(|m| m.id == msg_id).cloned());
                if let Some(msg) = target_msg {
                    self_for_reply.update(cx, |this, cx| {
                        this.set_replying_to(Some(msg));
                        cx.notify();
                    });
                }
            },
        ));

        // 3. Reaction Click: toggle/add specific emoji
        let cid_for_rx_click = channel_id.clone();
        let app_for_rx_click = app_entity.clone();
        let on_reaction_click: Option<ReactionActionCallback> = Some(Arc::new(
            move |message_id: &str, emoji: &str, _window: &mut Window, cx: &mut App| {
                let msg_id = message_id.to_string();
                let em = emoji.to_string();
                let cid = cid_for_rx_click.clone();
                app_for_rx_click.update(cx, |state, cx| {
                    state.add_reaction_to_message(&cid, &msg_id, &em);
                    cx.notify();
                });
            },
        ));

        // 4. Add Reaction (+) button -> opens emoji picker for that specific message
        let self_for_add_rx = self_entity.clone();
        let app_for_add_rx = app_entity.clone();
        let on_add_reaction: Option<AddReactionCallback> = Some(Arc::new(
            move |message_id: &str, window: &mut Window, cx: &mut App| {
                let msg_id = message_id.to_string();
                app_for_add_rx.update(cx, |state, _cx| {
                    state.sync_guild_media();
                });
                self_for_add_rx.update(cx, |this, cx| {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Emojis;
                    this.media_picker_react_message_id = Some(msg_id);
                    this.media_picker_list_state.reset(0);
                    this.active_context_menu = None;
                    window.focus(&this.media_picker_focus_handle, cx);
                    cx.notify();
                });
            },
        ));

        // 5. More Action: (•••) button -> opens message context menu
        let self_for_more = self_entity.clone();
        let on_more: Option<MessageActionPosCallback> = Some(Arc::new(
            move |message_id: &str, pos: Point<Pixels>, _window: &mut Window, cx: &mut App| {
                let msg_id = message_id.to_string();
                self_for_more.update(cx, |this, cx| {
                    this.active_context_menu = Some((msg_id, pos));
                    cx.notify();
                });
            },
        ));

        // 6. Send message
        let cid_for_send = channel_id.clone();
        let app_for_send = app_entity.clone();
        let self_for_send = self_entity.clone();

        let on_send: Option<EditorSendCallback> = Some(Arc::new(
            move |content: &str, _reply_id: Option<&str>, window: &mut Window, cx: &mut App| {
                if self_for_send.read(cx).is_sending_message {
                    return;
                }
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    return;
                }

                let text = trimmed.to_string();
                let editing_msg_opt = self_for_send.read(cx).editing_message.clone();
                if let Some(editing_msg) = editing_msg_opt {
                    let cid = cid_for_send.clone();
                    let mid = editing_msg.id.clone();
                    let cm = app_for_send.read(cx).client_manager.clone();
                    self_for_send.update(cx, |this, cx| {
                        this.submit_edit_message(cid, mid, text, cm, cx);
                    });
                    return;
                }

                if cid_for_send.is_empty() {
                    self_for_send.update(cx, |this, cx| {
                        this.send_error = Some("Nenhum canal selecionado".to_string());
                        cx.notify();
                    });
                    return;
                }
                self_for_send.update(cx, |this, cx| {
                    this.submit_composer_message(cid_for_send.clone(), text, window, cx);
                });
            },
        ));

        // 7. Dismiss / Cancel Reply
        let self_for_cancel = self_entity.clone();
        let on_cancel_reply: Option<EditorActionCallback> =
            Some(Arc::new(move |_window: &mut Window, cx: &mut App| {
                self_for_cancel.update(cx, |this, cx| {
                    this.set_replying_to(None);
                    cx.notify();
                });
            }));

        let self_for_toggle_mention = self_entity.clone();
        let on_toggle_mention: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_for_toggle_mention.update(cx, |this, cx| {
                this.reply_mention_author = !this.reply_mention_author;
                cx.notify();
            });
        }));

        let self_for_jump_reply = self_entity.clone();
        let app_for_jump_reply = app_entity.clone();
        let on_jump_to_reply: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            let target = self_for_jump_reply
                .read(cx)
                .replying_to
                .as_ref()
                .map(|message| (message.channel_id.clone(), message.id.clone()));
            if let Some((channel_id, message_id)) = target {
                app_for_jump_reply.update(cx, |state, cx| {
                    state.jump_to_message(&channel_id, &message_id);
                    cx.notify();
                });
            }
        }));

        // Virtualized list items rendering via GPUI list
        let items_arc = chat_items.clone();
        let ch_name = channel_name.clone();
        let ch_topic = channel_topic.clone();
        let is_dm_welcome = is_dm;
        let dm_avatar_src_item = dm_avatar_src.clone();
        let dm_avatar_deco_item = dm_avatar_decoration.clone();
        let app_entity_list = app_entity.clone();
        let on_react_item = on_react.clone();
        let on_reply_item = on_reply.clone();
        let on_more_item = on_more.clone();
        let on_rx_click_item = on_reaction_click.clone();
        let on_add_rx_item = on_add_reaction.clone();

        let virtual_list = list(self.list_state.clone(), move |ix, _window, cx| {
            if ix >= items_arc.len() {
                return div().into_any_element();
            }

            match &items_arc[ix] {
                ChatListItem::WelcomeHeader => {
                    render_welcome_banner(
                        &ch_name,
                        &ch_topic,
                        is_dm_welcome,
                        dm_avatar_src_item.clone(),
                        dm_avatar_deco_item.clone(),
                    ).into_any_element()
                }
                ChatListItem::DateSeparator(date) => {
                    render_date_separator_element(*date).into_any_element()
                }
                ChatListItem::Message { message_idx, is_grouped } => {
                    let state = app_entity_list.read(cx);
                    let msgs = state.get_active_messages();
                    if let Some(msg) = msgs.get(*message_idx) {
                        let author_role_color = state
                            .members
                            .iter()
                            .find(|m| m.id == msg.author.id && m.role_color != 0)
                            .map(|m| m.role_color)
                            .or_else(|| {
                                state.selected_guild_id.as_deref().and_then(|gid| {
                                    crate::protocol::cache::lazy_guilds::get_cached_author_role_color(gid, &msg.author.id)
                                })
                            });

                        let is_grouped = *is_grouped && msg.edited_timestamp.is_none();
                        let mut display_message = msg.clone();
                        let attachments = std::mem::take(&mut display_message.attachments);
                        let embeds = std::mem::take(&mut display_message.embeds);
                        let is_highlighted = state.highlighted_message_id.as_deref() == Some(&msg.id);
                        let row = render_message_row(
                            &display_message,
                            MessageRowConfig {
                                is_grouped,
                                role_color: author_role_color,
                                is_highlighted,
                                on_react: on_react_item.clone(),
                                on_reply: on_reply_item.clone(),
                                on_more: on_more_item.clone(),
                                on_reaction_click: on_rx_click_item.clone(),
                                on_add_reaction: on_add_rx_item.clone(),
                            },
                        );
                        if attachments.is_empty() && embeds.is_empty() {
                            row.into_any_element()
                        } else {
                            // Align rich media with the message content column:
                            // 16px row padding + 40px avatar + 8px gutter.
                            div()
                                .flex()
                                .flex_col()
                                .w_full()
                                .child(row)
                                .child(
                                    div()
                                        .pl(px(64.0))
                                        .pr(space_8())
                                        .flex()
                                        .flex_col()
                                        .child(crate::chat::markdown::render_rich_attachments(&attachments))
                                        .child(crate::chat::markdown::render_rich_embeds(&embeds)),
                                )
                                .into_any_element()
                        }
                    } else {
                        div().into_any_element()
                    }
                }
                ChatListItem::BottomSpacer => {
                    div().id("chat-bottom-anchor").h(px(8.0)).into_any_element()
                }
            }
        })
        .size_full();

        // Resolve target message jump (fetch around if needed), then scroll/highlight.
        self.resolve_target_jump(cx);
        let target_message_id = self.state.read(cx).target_message_id.clone();
        if let Some(ref target_id) = target_message_id {
            let target_idx = {
                let state = self.state.read(cx);
                let messages = state.get_active_messages();
                chat_items.iter().position(|item| match item {
                    ChatListItem::Message { message_idx, .. } => messages
                        .get(*message_idx)
                        .map(|message| message.id == *target_id)
                        .unwrap_or(false),
                    _ => false,
                })
            };
            if let Some(target_idx) = target_idx {
                self.list_state.scroll_to_reveal_item(target_idx);
                self.state.update(cx, |state, _cx| {
                    state.target_message_id = None;
                    state.jump_target_channel_id = None;
                    state.highlighted_message_id = Some(target_id.clone());
                    state.set_jump_error(None);
                });
                self.schedule_highlight_clear(target_id.clone(), cx);
            }
        }

        // Query active typing users directly from AppState's coalescing tracker.
        let (typing_users, typing_delay, is_search_open) = {
            let state = self.state.read(cx);
            let active_typing = state.get_active_typing_users(&channel_id);
            (
                if active_typing.is_empty() {
                    self.typing_users.clone()
                } else {
                    active_typing
                },
                state.typing_refresh_delay(&channel_id),
                state.is_search_open,
            )
        };
        if self.typing_refresh_task.is_none() {
            if let Some(delay) = typing_delay {
                let typing_entity = self_entity.clone();
                self.typing_refresh_task = Some(cx.spawn(move |_view, async_cx: &mut AsyncApp| {
                    let mut async_cx = async_cx.clone();
                    async move {
                        tokio::time::sleep(delay).await;
                        let _ = typing_entity.update(&mut async_cx, |chat, cx| {
                            chat.typing_refresh_task = None;
                            cx.notify();
                        });
                    }
                }));
            }
        }

        // Render search flyout if open.
        let search_panel = if is_search_open {
            let state = self.state.read(cx);
            Some(crate::chat::search::render_search_flyout(
                app_entity.clone(),
                &state,
                &self.search_focus_handle,
            ))
        } else {
            None
        };

        let is_pins_panel_open = app_state_ref.is_pins_panel_open;
        let pins_panel = if is_pins_panel_open {
            let state = self.state.read(cx);
            Some(render_pinned_messages_panel(
                app_entity.clone(),
                self_entity.clone(),
                &state,
                &channel_id,
            ))
        } else {
            None
        };

        // 8. Rich Media Picker Callbacks & Popover
        let self_for_attachment = self_entity.clone();
        let on_attachment: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_for_attachment.update(cx, |this, cx| {
                this.send_error = Some("Envio de anexos indisponível: o transporte atual aceita apenas texto e figurinhas.".to_string());
                this.retry_send = None;
                cx.notify();
            });
        }));

        let self_for_gif = self_entity.clone();
        let app_for_gif = app_entity.clone();
        let on_gif: Option<EditorActionCallback> = Some(Arc::new(move |window, cx| {
            app_for_gif.update(cx, |state, _cx| {
                state.sync_guild_media();
            });
            self_for_gif.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Gifs {
                    this.media_picker_open = false;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Gifs;
                    this.media_picker_list_state.reset(0);
                    window.focus(&this.media_picker_focus_handle, cx);
                    this.refresh_media_picker_gifs(cx);
                }
                cx.notify();
            });
        }));

        let self_for_sticker = self_entity.clone();
        let app_for_sticker = app_entity.clone();
        let on_sticker: Option<EditorActionCallback> = Some(Arc::new(move |window, cx| {
            app_for_sticker.update(cx, |state, _cx| {
                state.sync_guild_media();
            });
            self_for_sticker.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Stickers {
                    this.media_picker_open = false;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Stickers;
                    this.media_picker_list_state.reset(0);
                    window.focus(&this.media_picker_focus_handle, cx);
                }
                cx.notify();
            });
        }));

        let self_for_emoji = self_entity.clone();
        let app_for_emoji = app_entity.clone();
        let on_emoji: Option<EditorActionCallback> = Some(Arc::new(move |window, cx| {
            app_for_emoji.update(cx, |state, _cx| {
                state.sync_guild_media();
            });
            self_for_emoji.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Emojis {
                    this.media_picker_open = false;
                    this.media_picker_react_message_id = None;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Emojis;
                    this.media_picker_react_message_id = None;
                    this.media_picker_list_state.reset(0);
                    window.focus(&this.media_picker_focus_handle, cx);
                }
                cx.notify();
            });
        }));

        let media_picker_overlay = if self.media_picker_open {
            let self_close = self_entity.clone();
            let on_close: Option<OnCloseMediaPickerCallback> = Some(Arc::new(move |_win, cx| {
                self_close.update(cx, |this, cx| {
                    this.media_picker_open = false;
                    this.media_picker_react_message_id = None;
                    cx.notify();
                });
            }));

            let self_tab = self_entity.clone();
            let on_tab_change: Option<OnTabChangeCallback> =
                Some(Arc::new(move |tab, _win, cx| {
                    self_tab.update(cx, |this, cx| {
                        this.media_picker_tab = tab;
                        this.media_picker_list_state.reset(0);
                        if tab == MediaPickerTab::Gifs {
                            this.refresh_media_picker_gifs(cx);
                        }
                        cx.notify();
                    });
                }));

            let self_emoji = self_entity.clone();
            let app_emoji = app_entity.clone();
            let cid_emoji = channel_id.clone();
            let on_select_emoji: Option<OnSelectEmojiCallback> =
                Some(Arc::new(move |emoji, _win, cx| {
                    let em = emoji.to_string();
                    let _cid = cid_emoji.clone();
                    self_emoji.update(cx, |this, cx| {
                        if let Some(target_msg_id) = this.media_picker_react_message_id.take() {
                            let target_id = target_msg_id.clone();
                            let em_clone = em.clone();
                            app_emoji.update(cx, |state, cx| {
                                state.add_reaction(&target_id, &em_clone);
                                state.record_emoji_usage(&em_clone);
                                cx.notify();
                            });
                            this.media_picker_open = false;
                        } else {
                            this.typed_text.push_str(&em);
                            if let Some(ref st) = this.composer_state {
                                let new_val = format!("{}{}", st.read(cx).value(), em);
                                st.update(cx, |s, cx| {
                                    s.set_value(new_val, _win, cx);
                                });
                            }
                            let em_clone = em.clone();
                            app_emoji.update(cx, |state, _cx| {
                                state.record_emoji_usage(&em_clone);
                            });
                        }
                        cx.notify();
                    });
                }));

            let self_sticker = self_entity.clone();
            let app_sticker = app_entity.clone();
            let cid_sticker = channel_id.clone();
            let on_select_sticker: Option<OnSelectStickerCallback> =
                Some(Arc::new(move |sticker, _win, cx| {
                    let st = sticker.clone();
                    let cid = cid_sticker.clone();
                    app_sticker.update(cx, |state, cx| {
                        state.send_sticker(&cid, &st);
                        cx.notify();
                    });
                    self_sticker.update(cx, |this, cx| {
                        this.media_picker_open = false;
                        cx.notify();
                    });
                }));

            let self_gif = self_entity.clone();
            let cid_gif = channel_id.clone();
            let on_select_gif: Option<OnSelectGifCallback> =
                Some(Arc::new(move |gif_url, window, cx| {
                    let url = gif_url.to_string();
                    let cid = cid_gif.clone();
                    self_gif.update(cx, |this, cx| {
                        this.media_picker_open = false;
                        this.submit_composer_message(cid, url, window, cx);
                    });
                }));

            let on_play_sound: Option<OnPlaySoundCallback> =
                Some(Arc::new(move |sound, _win, _cx| {
                    log::info!("Soundboard sound preview: {}", sound.name);
                }));

            let self_sound = self_entity.clone();
            let cid_sound = channel_id.clone();
            let on_send_sound: Option<OnPlaySoundCallback> =
                Some(Arc::new(move |sound, window, cx| {
                    let sound_text = if let Some(ref emoji_name) = sound.emoji_name {
                        format!("[Som da Soundboard: {} {}]", emoji_name, sound.name)
                    } else {
                        format!("[Som da Soundboard: {}]", sound.name)
                    };
                    let cid = cid_sound.clone();
                    self_sound.update(cx, |this, cx| {
                        this.media_picker_open = false;
                        this.submit_composer_message(cid, sound_text, window, cx);
                    });
                }));

            let self_search = self_entity.clone();
            let on_search_query: Option<OnSearchQueryCallback> =
                Some(Arc::new(move |query, _win, cx| {
                    let q = query.to_string();
                    self_search.update(cx, |this, cx| {
                        this.media_picker_search = q;
                        this.media_picker_list_state.reset(0);
                        if this.media_picker_tab == MediaPickerTab::Gifs {
                            this.refresh_media_picker_gifs(cx);
                        }
                        cx.notify();
                    });
                }));

            let self_cat = self_entity.clone();
            let on_select_gif_category: Option<OnSelectGifCategoryCallback> =
                Some(Arc::new(move |cat, _win, cx| {
                    let c = cat.to_string();
                    self_cat.update(cx, |this, cx| {
                        this.media_picker_search = c;
                        this.media_picker_list_state.reset(0);
                        if this.media_picker_tab == MediaPickerTab::Gifs {
                            this.refresh_media_picker_gifs(cx);
                        }
                        cx.notify();
                    });
                }));

            let self_guild = self_entity.clone();
            let on_select_guild: Option<OnSelectGuildCallback> =
                Some(Arc::new(move |gid_opt, _win, cx| {
                    let next_gid = gid_opt.map(|s| s.to_string());
                    self_guild.update(cx, |this, cx| {
                        this.media_picker_selected_guild_id = next_gid;
                        this.media_picker_list_state.reset(0);
                        cx.notify();
                    });
                }));

            let (guild_sections, recent_emojis, custom_emojis, stickers) = {
                let state = self.state.read(cx);
                let guild_sections = state
                    .guilds
                    .iter()
                    .map(|guild| MediaPickerGuildSection {
                        guild_id: guild.id.clone(),
                        guild_name: guild.name.clone(),
                        icon_url: guild.icon_url(),
                        acronym: guild.acronym.clone(),
                        emojis: guild.emojis.clone(),
                        stickers: guild.stickers.clone(),
                    })
                    .collect();
                (
                    guild_sections,
                    state.media_cache.get_recent_emojis_sync(),
                    state.get_current_custom_emojis(),
                    state.get_current_stickers(),
                )
            };

            let is_picker_focused = self.media_picker_focus_handle.is_focused(window);

            let modal_content = MediaPickerModal::render(MediaPickerConfig {
                active_tab: self.media_picker_tab,
                search_query: self.media_picker_search.clone(),
                recent_emojis,
                custom_emojis,
                stickers,
                gifs: self.media_picker_gifs.clone(),
                sounds: Vec::new(),
                hovered_info: None,
                guild_sections,
                selected_guild_id: self.media_picker_selected_guild_id.clone(),
                focus_handle: Some(self.media_picker_focus_handle.clone()),
                is_focused: is_picker_focused,
                list_state: Some(self.media_picker_list_state.clone()),
                on_select_emoji,
                on_select_sticker,
                on_select_gif,
                on_play_sound,
                on_send_sound,
                on_close,
                on_tab_change,
                on_search_query,
                on_select_gif_category,
                on_select_guild,
            });

            let self_dismiss_picker = self_entity.clone();
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                        self_dismiss_picker.update(cx, |this, cx| {
                            this.media_picker_open = false;
                            this.media_picker_react_message_id = None;
                            cx.notify();
                        });
                    })
                    .on_scroll_wheel(|_ev, _win, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .absolute()
                            .bottom(px(68.0))
                            .right(px(16.0))
                            .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                cx.stop_propagation();
                            })
                            .on_scroll_wheel(|_ev, _win, cx| {
                                cx.stop_propagation();
                            })
                            .child(modal_content),
                    ),
            )
        } else {
            None
        };

        let context_menu_overlay = if let Some((ref target_id, click_pos)) =
            self.active_context_menu
        {
            let (target_msg, current_user_id, guild_part) = {
                let state = self.state.read(cx);
                (
                    state
                        .get_active_messages()
                        .iter()
                        .find(|message| &message.id == target_id)
                        .cloned(),
                    state.current_user.id.clone(),
                    state
                        .selected_guild_id
                        .clone()
                        .unwrap_or_else(|| "@me".to_string()),
                )
            };
            if let Some(msg) = target_msg {
                let msg_id = msg.id.clone();
                let msg_content = msg.content.clone();
                let is_pinned = msg.pinned;
                let is_own = msg.author.id == current_user_id;

                let self_close = self_entity.clone();
                let on_close: Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static> =
                    Arc::new(move |_win, cx| {
                        self_close.update(cx, |this, cx| {
                            this.active_context_menu = None;
                            cx.notify();
                        });
                    });

                let self_add_rx = self_entity.clone();
                let on_cm_add_rx: Option<MessageActionCallback> =
                    Some(Arc::new(move |m_id: &str, _win, cx| {
                        let id = m_id.to_string();
                        self_add_rx.update(cx, |this, cx| {
                            this.media_picker_open = true;
                            this.media_picker_tab = MediaPickerTab::Emojis;
                            this.media_picker_react_message_id = Some(id);
                            this.media_picker_list_state.reset(0);
                            this.active_context_menu = None;
                            _win.focus(&this.media_picker_focus_handle, cx);
                            cx.notify();
                        });
                    }));

                let self_cm_reply = self_entity.clone();
                let msg_clone = msg.clone();
                let on_cm_reply: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        let m = msg_clone.clone();
                        self_cm_reply.update(cx, |this, cx| {
                            this.set_replying_to(Some(m));
                            this.active_context_menu = None;
                            cx.notify();
                        });
                    }));

                let content_for_copy = msg_content.clone();
                let on_cm_copy_text: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(content_for_copy.clone()));
                    }));

                let link_target = format!(
                    "https://discord.com/channels/{}/{}/{}",
                    guild_part, channel_id, msg.id
                );
                let on_cm_copy_link: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(link_target.clone()));
                    }));

                let id_for_copy = msg_id.clone();
                let on_cm_copy_id: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(id_for_copy.clone()));
                    }));

                let self_cm_edit = self_entity.clone();
                let msg_for_edit = msg.clone();
                let on_cm_edit: Option<MessageActionCallback> = if is_own {
                    Some(Arc::new(
                        move |_m_id: &str, window: &mut Window, cx: &mut App| {
                            let m = msg_for_edit.clone();
                            self_cm_edit.update(cx, |this, cx| {
                                this.start_editing_message(m, Some(window), cx);
                                this.active_context_menu = None;
                            });
                        },
                    ))
                } else {
                    None
                };

                let self_cm_pin = self_entity.clone();
                let msg_pin = msg.clone();
                let on_cm_pin: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        let m = msg_pin.clone();
                        let is_currently_pinned = m.pinned;
                        self_cm_pin.update(cx, |this, cx| {
                            this.pending_pin_action = Some(PendingPinAction {
                                message: m,
                                is_unpin: is_currently_pinned,
                            });
                            this.pin_error = None;
                            this.active_context_menu = None;
                            cx.notify();
                        });
                    }));

                // Normal click: abre o modal canônico de confirmação de exclusão
                let self_cm_del = self_entity.clone();
                let msg_for_del = msg.clone();
                let on_cm_delete: Option<MessageActionCallback> =
                    Some(Arc::new(move |_m_id: &str, _win, cx| {
                        let m = msg_for_del.clone();
                        self_cm_del.update(cx, |this, cx| {
                            this.pending_delete_message = Some(m);
                            this.is_deleting = false;
                            this.delete_error = None;
                            this.active_context_menu = None;
                            cx.notify();
                        });
                    }));

                // Shift+click: apaga direto chamando a API sem exibir confirmação
                let app_cm_del_shift = app_entity.clone();
                let cid_del_shift = channel_id.clone();
                let self_cm_del_shift = self_entity.clone();
                let on_cm_delete_shift: Option<MessageActionCallback> = Some(Arc::new(
                    move |m_id: &str, _win, cx| {
                        let mid = m_id.to_string();
                        let cid = cid_del_shift.clone();
                        let cm = app_cm_del_shift.read(cx).client_manager.clone();
                        if let Some(cm) = cm {
                            self_cm_del_shift.update(cx, |this, cx| {
                                this.delete_error = None;
                                this.active_context_menu = None;
                                cx.notify();
                            });
                            let self_async = self_cm_del_shift.clone();
                            cx.spawn(|cx: &mut AsyncApp| {
                            let cx = cx.clone();
                            async move {
                                match cm.delete_message(&cid, &mid).await {
                                    Ok(()) => {
                                        cm.emit_event(crate::protocol::TransportEvent::MessageDelete {
                                            channel_id: cid,
                                            message_id: mid,
                                        });
                                    }
                                    Err(e) => {
                                        let err_msg = e.to_string();
                                        let _ = cx.update(|cx| {
                                            let _ = self_async.update(cx, |chat, cx| {
                                                chat.delete_error = Some(err_msg.clone());
                                                cx.notify();
                                            });
                                        });
                                        cm.emit_event(crate::protocol::TransportEvent::MessageDeleteFailed {
                                            channel_id: cid,
                                            message_id: mid,
                                            error: err_msg,
                                        });
                                    }
                                }
                            }
                        }).detach();
                        } else {
                            let err_msg =
                                "Não conectado ao Discord: cliente não inicializado".to_string();
                            self_cm_del_shift.update(cx, |this, cx| {
                                this.delete_error = Some(err_msg);
                                this.active_context_menu = None;
                                cx.notify();
                            });
                        }
                    },
                ));

                let self_dismiss = self_entity.clone();
                let self_dismiss_r = self_entity.clone();
                let self_close_key = self_entity.clone();

                Some(
                    div()
                        .id("message-context-menu-layer")
                        .key_context("MessageContextMenu")
                        .absolute()
                        .inset_0()
                        .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                            if event.keystroke.key == "escape" {
                                self_close_key.update(cx, |this, cx| {
                                    this.active_context_menu = None;
                                    cx.notify();
                                });
                            }
                        })
                        .child(
                            div()
                                .id("message-context-menu-dismiss-backdrop")
                                .absolute()
                                .inset_0()
                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                    self_dismiss.update(cx, |this, cx| {
                                        this.active_context_menu = None;
                                        cx.notify();
                                    });
                                })
                                .on_mouse_down(MouseButton::Right, move |_ev, _win, cx| {
                                    self_dismiss_r.update(cx, |this, cx| {
                                        this.active_context_menu = None;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(deferred(
                            anchored()
                                .position(click_pos)
                                .snap_to_window_with_margin(px(8.0))
                                .child(
                                    div()
                                        .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_down(MouseButton::Right, |_ev, _win, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(render_message_context_menu(
                                            MessageContextMenuConfig {
                                                message_id: msg_id,
                                                message_content: msg_content,
                                                is_pinned,
                                                is_own,
                                                on_add_reaction: on_cm_add_rx,
                                                on_edit: on_cm_edit,
                                                on_reply: on_cm_reply,
                                                on_copy_text: on_cm_copy_text,
                                                on_copy_link: on_cm_copy_link,
                                                on_copy_id: on_cm_copy_id,
                                                on_mark_unread: None,
                                                on_pin: on_cm_pin,
                                                on_delete: on_cm_delete,
                                                on_delete_shift: on_cm_delete_shift,
                                                on_close: Some(on_close),
                                            },
                                        )),
                                ),
                        )),
                )
            } else {
                None
            }
        } else {
            None
        };

        let delete_modal_overlay = if let Some(ref del_msg) = self.pending_delete_message {
            let msg_to_delete = del_msg.clone();
            let cid_confirm = channel_id.clone();
            let app_confirm = app_entity.clone();
            let self_cancel = self_entity.clone();
            let self_confirm = self_entity.clone();
            let self_esc = self_entity.clone();
            let self_backdrop = self_entity.clone();

            Some(
                div()
                    .id("delete-message-modal-layer")
                    .key_context("DeleteMessageModal")
                    .absolute()
                    .inset_0()
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            self_esc.update(cx, |this, cx| {
                                this.pending_delete_message = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .id("delete-modal-backdrop")
                            .absolute()
                            .inset_0()
                            .bg(modal_backdrop())
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                self_backdrop.update(cx, |this, cx| {
                                    this.pending_delete_message = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(440.0))
                                    .bg(bg_floating())
                                    .border_1()
                                    .border_color(bg_user_bar())
                                    .rounded(radius_md())
                                    .shadow_xl()
                                    .overflow_hidden()
                                    .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                        cx.stop_propagation();
                                    })
                                    .child(
                                        // Header
                                        div()
                                            .p(px(16.0))
                                            .pb(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(18.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(white())
                                                    .child("Excluir Mensagem"),
                                            )
                                            .child(
                                                div()
                                                    .pt(px(8.0))
                                                    .text_size(font_size_sm())
                                                    .text_color(text_normal())
                                                    .child("Tem certeza de que deseja excluir esta mensagem? Isto não pode ser desfeito."),
                                            ),
                                    )
                                    .children(self.delete_error.as_ref().map(|err| {
                                        div()
                                            .mx(px(16.0))
                                            .mt(px(8.0))
                                            .p(px(8.0))
                                            .rounded(radius_sm())
                                            .bg(with_alpha(HEX_STATUS_DANGER, 0.15))
                                            .border_1()
                                            .border_color(status_danger())
                                            .text_size(font_size_xs())
                                            .text_color(status_danger())
                                            .child(format!("Erro ao excluir mensagem: {}", err))
                                    }))
                                    .child(
                                        // Message Preview Card
                                        div()
                                            .mx(px(16.0))
                                            .my(px(8.0))
                                            .p(px(10.0))
                                            .bg(bg_chat())
                                            .border_1()
                                            .border_color(bg_user_bar())
                                            .rounded(radius_md())
                                            .flex()
                                            .flex_row()
                                            .gap(px(10.0))
                                            .child(
                                                crate::components::avatar::Avatar::new(msg_to_delete.author.display_name().to_string())
                                                    .size(crate::components::avatar::AvatarSize::Px40)
                                                    .opt_src(msg_to_delete.author.avatar_url())
                                                    .opt_decoration(msg_to_delete.author.avatar_decoration_data.clone())
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_baseline()
                                                            .gap(px(6.0))
                                                            .child(
                                                                div()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_size(font_size_sm())
                                                                    .text_color(text_titles())
                                                                    .child(msg_to_delete.author.display_name().to_string()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(font_size_xs())
                                                                    .text_color(text_muted())
                                                                    .child(crate::chat::message_list::format_message_timestamp(msg_to_delete.timestamp)),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .pt(px(4.0))
                                                            .text_size(font_size_sm())
                                                            .text_color(text_normal())
                                                            .child(msg_to_delete.content.clone()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        // Actions Footer
                                        div()
                                            .p(px(16.0))
                                            .bg(bg_user_bar())
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .px(px(16.0))
                                                    .py(px(8.0))
                                                    .rounded(radius_sm())
                                                    .cursor_pointer()
                                                    .hover(|s| s.underline())
                                                    .text_color(white())
                                                    .text_size(font_size_sm())
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        self_cancel.update(cx, |this, cx| {
                                                            this.pending_delete_message = None;
                                                            this.is_deleting = false;
                                                            this.delete_error = None;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .child("Cancelar"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(24.0))
                                                    .py(px(8.0))
                                                    .bg(status_danger())
                                                    .hover(|s| s.bg(with_alpha(HEX_STATUS_DANGER, 0.85)))
                                                    .rounded(radius_sm())
                                                    .cursor_pointer()
                                                    .text_color(white())
                                                    .text_size(font_size_sm())
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        let mid = msg_to_delete.id.clone();
                                                        let cid = cid_confirm.clone();
                                                        let cm = app_confirm.read(cx).client_manager.clone();
                                                        if let Some(cm) = cm {
                                                            self_confirm.update(cx, |this, cx| {
                                                                if this.is_deleting {
                                                                    return;
                                                                }
                                                                this.is_deleting = true;
                                                                this.delete_error = None;
                                                                cx.notify();
                                                            });
                                                            let self_async = self_confirm.clone();
                                                            cx.spawn(|cx: &mut AsyncApp| {
                                                                let cx = cx.clone();
                                                                async move {
                                                                    match cm.delete_message(&cid, &mid).await {
                                                                        Ok(()) => {
                                                                            let _ = cx.update(|cx| {
                                                                                let _ = self_async.update(cx, |chat, cx| {
                                                                                    chat.pending_delete_message = None;
                                                                                    chat.is_deleting = false;
                                                                                    chat.delete_error = None;
                                                                                    cx.notify();
                                                                                });
                                                                            });
                                                                            cm.emit_event(crate::protocol::TransportEvent::MessageDelete {
                                                                                channel_id: cid,
                                                                                message_id: mid,
                                                                            });
                                                                        }
                                                                        Err(e) => {
                                                                            let err_msg = e.to_string();
                                                                            let _ = cx.update(|cx| {
                                                                                let _ = self_async.update(cx, |chat, cx| {
                                                                                    chat.is_deleting = false;
                                                                                    chat.delete_error = Some(err_msg.clone());
                                                                                    cx.notify();
                                                                                });
                                                                            });
                                                                            cm.emit_event(crate::protocol::TransportEvent::MessageDeleteFailed {
                                                                                channel_id: cid,
                                                                                message_id: mid,
                                                                                error: err_msg,
                                                                            });
                                                                        }
                                                                    }
                                                                }
                                                            }).detach();
                                                        } else {
                                                            self_confirm.update(cx, |this, cx| {
                                                                this.is_deleting = false;
                                                                this.delete_error = Some("Não conectado ao Discord: cliente não inicializado".to_string());
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .child(if self.is_deleting { "Excluindo..." } else { "Excluir" }),
                                            ),
                                    ),
                            ),
                    ),
            )
        } else {
            None
        };

        let pin_modal_overlay = if let Some(ref pin_act) = self.pending_pin_action {
            let msg_to_pin = pin_act.message.clone();
            let is_unpin = pin_act.is_unpin;
            let cid_confirm = channel_id.clone();
            let app_confirm = app_entity.clone();
            let self_cancel = self_entity.clone();
            let self_confirm = self_entity.clone();
            let self_esc = self_entity.clone();
            let self_backdrop = self_entity.clone();

            let title = if is_unpin {
                "Desafixar Mensagem"
            } else {
                "Fixar Mensagem"
            };

            let subtitle = if is_unpin {
                "Tem certeza de que deseja remover esta mensagem fixada?".to_string()
            } else if is_dm {
                "Você tem certeza de que deseja fixar esta mensagem para a posteridade?".to_string()
            } else {
                format!(
                    "Você tem certeza de que deseja fixar esta mensagem em #{} para a posteridade?",
                    channel_name
                )
            };

            let confirm_label = if is_unpin {
                "Sim, desafixar"
            } else {
                "Sim, fixar"
            };

            let confirm_color = if is_unpin { status_danger() } else { blurple() };

            Some(
                div()
                    .id("pin-message-modal-layer")
                    .key_context("PinMessageModal")
                    .absolute()
                    .inset_0()
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            self_esc.update(cx, |this, cx| {
                                this.pending_pin_action = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .id("pin-modal-backdrop")
                            .absolute()
                            .inset_0()
                            .bg(modal_backdrop())
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                self_backdrop.update(cx, |this, cx| {
                                    this.pending_pin_action = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(440.0))
                                    .bg(bg_floating())
                                    .border_1()
                                    .border_color(bg_user_bar())
                                    .rounded(radius_md())
                                    .shadow_xl()
                                    .overflow_hidden()
                                    .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                        cx.stop_propagation();
                                    })
                                    .child(
                                        // Header
                                        div()
                                            .p(px(16.0))
                                            .pb(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(18.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(white())
                                                    .child(title),
                                            )
                                            .child(
                                                div()
                                                    .pt(px(8.0))
                                                    .text_size(font_size_sm())
                                                    .text_color(text_normal())
                                                    .child(subtitle),
                                            ),
                                    )
                                    .child(
                                        // Message Preview Card
                                        div()
                                            .mx(px(16.0))
                                            .my(px(8.0))
                                            .p(px(10.0))
                                            .bg(bg_chat())
                                            .border_1()
                                            .border_color(bg_user_bar())
                                            .rounded(radius_md())
                                            .flex()
                                            .flex_row()
                                            .gap(px(10.0))
                                            .child(
                                                crate::components::avatar::Avatar::new(msg_to_pin.author.display_name().to_string())
                                                    .size(crate::components::avatar::AvatarSize::Px40)
                                                    .opt_src(msg_to_pin.author.avatar_url())
                                                    .opt_decoration(msg_to_pin.author.avatar_decoration_data.clone())
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_baseline()
                                                            .gap(px(6.0))
                                                            .child(
                                                                div()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_size(font_size_sm())
                                                                    .text_color(text_titles())
                                                                    .child(msg_to_pin.author.display_name().to_string()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(font_size_xs())
                                                                    .text_color(text_muted())
                                                                    .child(crate::chat::message_list::format_message_timestamp(msg_to_pin.timestamp)),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .pt(px(4.0))
                                                            .text_size(font_size_sm())
                                                            .text_color(text_normal())
                                                            .child(if msg_to_pin.content.is_empty() {
                                                                "[Anexo ou mensagem sem texto]".to_string()
                                                            } else {
                                                                msg_to_pin.content.clone()
                                                            }),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        // Actions Footer
                                        div()
                                            .p(px(16.0))
                                            .bg(bg_user_bar())
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .px(px(16.0))
                                                    .py(px(8.0))
                                                    .rounded(radius_sm())
                                                    .cursor_pointer()
                                                    .hover(|s| s.underline())
                                                    .text_color(white())
                                                    .text_size(font_size_sm())
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        self_cancel.update(cx, |this, cx| {
                                                            this.pending_pin_action = None;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .child("Cancelar"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(24.0))
                                                    .py(px(8.0))
                                                    .bg(confirm_color)
                                                    .hover(move |s| s.opacity(0.85))
                                                    .rounded(radius_sm())
                                                    .cursor_pointer()
                                                    .text_color(white())
                                                    .text_size(font_size_sm())
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        let mid = msg_to_pin.id.clone();
                                                        let cid = cid_confirm.clone();
                                                        let cm = app_confirm.read(cx).client_manager.clone();
                                                        self_confirm.update(cx, |this, cx| {
                                                            this.pending_pin_action = None;
                                                            this.pin_error = None;
                                                            cx.notify();
                                                        });
                                                        if let Some(cm) = cm {
                                                            let self_async = self_confirm.clone();
                                                            let app_async = app_confirm.clone();
                                                            cx.spawn(move |cx: &mut AsyncApp| {
                                                                let mut cx = cx.clone();
                                                                async move {
                                                                    let res = if is_unpin {
                                                                        cm.unpin_message(&cid, &mid).await
                                                                    } else {
                                                                        cm.pin_message(&cid, &mid).await
                                                                    };
                                                                    match res {
                                                                        Ok(()) => {
                                                                            let _ = app_async.update(&mut cx, |state, cx| {
                                                                                state.set_message_pinned(&cid, &mid, !is_unpin);
                                                                                cx.notify();
                                                                            });
                                                                        }
                                                                        Err(error) => {
                                                                            let error = error.to_string();
                                                                            let _ = cx.update(|cx| {
                                                                                let _ = self_async.update(cx, |chat, cx| {
                                                                                    chat.pin_error = Some(error);
                                                                                    cx.notify();
                                                                                });
                                                                            });
                                                                        }
                                                                    }
                                                                }
                                                            }).detach();
                                                        } else {
                                                            self_confirm.update(cx, |this, cx| {
                                                                this.pin_error = Some("Não conectado ao Discord: cliente não inicializado".to_string());
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .child(confirm_label),
                                            ),
                                    ),
                            ),
                    )
            )
        } else {
            None
        };

        let delete_error_toast = if let Some(ref err) = self.delete_error {
            if self.pending_delete_message.is_none() {
                let self_dismiss_err = self_entity.clone();
                Some(
                    div()
                        .mx(px(16.0))
                        .mb(px(8.0))
                        .p(px(8.0))
                        .rounded(radius_sm())
                        .bg(with_alpha(HEX_STATUS_DANGER, 0.15))
                        .border_1()
                        .border_color(status_danger())
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(font_size_xs())
                                .text_color(status_danger())
                                .child(format!("Erro ao excluir mensagem: {}", err)),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .text_size(font_size_xs())
                                .text_color(status_danger())
                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                    self_dismiss_err.update(cx, |this, cx| {
                                        this.delete_error = None;
                                        cx.notify();
                                    });
                                })
                                .child("✕"),
                        ),
                )
            } else {
                None
            }
        } else {
            None
        };

        let edit_error_toast = if let Some(ref err) = self.edit_error {
            let self_dismiss_err = self_entity.clone();
            Some(
                div()
                    .mx(px(16.0))
                    .mb(px(8.0))
                    .p(px(8.0))
                    .rounded(radius_sm())
                    .bg(with_alpha(HEX_STATUS_DANGER, 0.15))
                    .border_1()
                    .border_color(status_danger())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(font_size_xs())
                            .text_color(status_danger())
                            .child(format!("Erro ao editar mensagem: {}", err)),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(font_size_xs())
                            .text_color(status_danger())
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                self_dismiss_err.update(cx, |this, cx| {
                                    this.edit_error = None;
                                    cx.notify();
                                });
                            })
                            .child("✕"),
                    ),
            )
        } else {
            None
        };

        let pin_error_toast = if let Some(ref err) = self.pin_error {
            let self_dismiss_err = self_entity.clone();
            Some(
                div()
                    .mx(px(16.0))
                    .mb(px(8.0))
                    .p(px(8.0))
                    .rounded(radius_sm())
                    .bg(with_alpha(HEX_STATUS_DANGER, 0.15))
                    .border_1()
                    .border_color(status_danger())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(font_size_xs())
                            .text_color(status_danger())
                            .child(format!("Erro ao fixar/desafixar mensagem: {}", err)),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(font_size_xs())
                            .text_color(status_danger())
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                self_dismiss_err.update(cx, |this, cx| {
                                    this.pin_error = None;
                                    cx.notify();
                                });
                            })
                            .child("✕"),
                    ),
            )
        } else {
            None
        };

        let send_retry = if self.retry_send.is_some() {
            let self_retry = self_entity.clone();
            Some(Arc::new(move |window: &mut Window, cx: &mut App| {
                self_retry.update(cx, |this, cx| this.retry_failed_send(window, cx));
            }) as EditorActionCallback)
        } else {
            None
        };
        let self_dismiss_send = self_entity.clone();
        let dismiss_send: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_dismiss_send.update(cx, |this, cx| {
                this.send_error = None;
                this.retry_send = None;
                cx.notify();
            });
        }));
        let send_error_toast = self.send_error.clone().and_then(|message| {
            render_chat_error_toast(
                message,
                if self.retry_send.is_some() {
                    Some("Tentar novamente")
                } else {
                    None
                },
                send_retry.clone(),
                dismiss_send.clone(),
            )
        });

        let self_dismiss_history = self_entity.clone();
        let dismiss_history: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_dismiss_history.update(cx, |this, cx| {
                this.history_error = None;
                cx.notify();
            });
        }));
        let history_error_toast = self
            .history_error
            .clone()
            .and_then(|message| render_chat_error_toast(message, None, None, dismiss_history));

        let self_for_cancel_edit = self_entity.clone();
        let on_cancel_edit: Option<EditorActionCallback> =
            Some(Arc::new(move |window: &mut Window, cx: &mut App| {
                self_for_cancel_edit.update(cx, |this, cx| {
                    this.cancel_editing(Some(window), cx);
                });
            }));

        let composer_channel_name = if channel_name.is_empty() {
            String::new()
        } else if is_dm {
            if channel_name.starts_with('@') {
                channel_name.clone()
            } else {
                format!("@{}", channel_name)
            }
        } else {
            channel_name.clone()
        };

        let main_chat_col = div()
            .relative()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .justify_between()
            .bg(bg_chat())
            .overflow_hidden()
            // 1. Fully Virtualized Message Stream Area via kit v_virtual_list
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .min_w(px(0.0))
                    .w_full()
                    .max_w_full()
                    .overflow_hidden()
                    .id("messages-scroll-area")
                    .child(virtual_list),
            )
            .children(delete_error_toast)
            .children(edit_error_toast)
            .children(pin_error_toast)
            .children(send_error_toast)
            .children(history_error_toast)
            .children({
                let err = self.state.read(cx).jump_error.clone();
                err.and_then(|message| {
                    let on_dismiss: Option<EditorActionCallback> = Some(Arc::new({
                        let entity = self_entity.clone();
                        move |_window: &mut Window, cx: &mut App| {
                            entity.update(cx, |this, cx| {
                                this.state.update(cx, |state, _cx| state.set_jump_error(None));
                                cx.notify();
                            });
                        }
                    }));
                    render_chat_error_toast(message, None, None, on_dismiss)
                })
            })
            // 2. Chat Input Editor Bar
            .child(
                ChatComposer::new("chat-composer", &composer_state)
                    .channel_name(composer_channel_name)
                    .replying_to(self.replying_to.clone())
                    .editing_message(self.editing_message.clone())
                    .disabled(self.is_saving_edit || self.is_sending_message)
                    .on_cancel_edit(on_cancel_edit)
                    .on_attachment(on_attachment)
                    .mention_author(self.reply_mention_author)
                    .on_toggle_mention(on_toggle_mention)
                    .typing_users(typing_users)
                    .active_media_tab(if self.media_picker_open {
                        Some(self.media_picker_tab)
                    } else {
                        None
                    })
                    .on_gif(on_gif)
                    .on_sticker(on_sticker)
                    .on_emoji(on_emoji)
                    .on_cancel_reply(on_cancel_reply)
                    .on_jump_to_reply(on_jump_to_reply)
                    .on_send(on_send),
            )
            .children(media_picker_overlay);

        let app_entity_shortcut = app_entity.clone();
        let self_keys = self_entity.clone();
        let app_keys = app_entity.clone();

        div()
            .size_full()
            .min_h(px(0.0))
            .min_w(px(0.0))
            .flex()
            .flex_row()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                let key = ev.keystroke.key.as_str();
                let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
                if is_ctrl && ev.keystroke.key.to_lowercase() == "f" {
                    app_entity_shortcut.update(cx, |state, cx| {
                        state.toggle_search();
                        cx.notify();
                    });
                    return;
                }

                if key == "escape" || key == "Escape" {
                    let app_esc = app_entity_shortcut.clone();
                    self_keys.update(cx, |this, cx| {
                        if this.media_picker_open {
                            this.media_picker_open = false;
                            this.media_picker_react_message_id = None;
                            cx.notify();
                        } else if this.pending_pin_action.is_some() {
                            this.pending_pin_action = None;
                            cx.notify();
                        } else if this.pending_delete_message.is_some() {
                            this.pending_delete_message = None;
                            cx.notify();
                        } else if this.editing_message.is_some() {
                            this.cancel_editing(Some(window), cx);
                        } else if this.replying_to.is_some() {
                            this.set_replying_to(None);
                            cx.notify();
                        } else if this.active_context_menu.is_some() {
                            this.active_context_menu = None;
                            cx.notify();
                        } else {
                            let was_open = app_esc.read(cx).is_pins_panel_open;
                            if was_open {
                                app_esc.update(cx, |state, cx| {
                                    state.close_pins_panel();
                                    cx.notify();
                                });
                            }
                        }
                    });
                    return;
                }

                if key == "up" || key == "arrowup" || key == "ArrowUp" {
                    self_keys.update(cx, |this, cx| {
                        if this.editing_message.is_none() && this.typed_text.trim().is_empty() {
                            let state = app_keys.read(cx);
                            let msgs = state.get_active_messages();
                            if let Some(last_own) = msgs
                                .iter()
                                .rev()
                                .find(|message| {
                                    !message.is_pending
                                        && message.author.id == state.current_user.id
                                        && !message.content.trim().is_empty()
                                })
                                .cloned()
                            {
                                this.start_editing_message(last_own, Some(window), cx);
                            }
                        }
                    });
                    return;
                }
            })
            .child(main_chat_col)
            .children(search_panel)
            .children(pins_panel)
            .children(context_menu_overlay)
            .children(delete_modal_overlay)
            .children(pin_modal_overlay)
    }
}

fn render_pinned_messages_panel(
    app_entity: Entity<AppState>,
    chat_entity: Entity<ChatView>,
    state: &AppState,
    channel_id: &str,
) -> AnyElement {
    use crate::components::{Avatar, AvatarSize, IconButton};
    use crate::tokens::icons::{icon_close, icon_pin};

    let close_entity = app_entity.clone();
    let header = div()
        .h(px(48.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(border_subtle())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(icon_pin(px(16.0), text_titles()))
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .child("Mensagens fixadas"),
                ),
        )
        .child(
            IconButton::with_icon("pins-panel-close", |sz, col| icon_close(sz, col).into_any_element())
                .size(px(28.0))
                .icon_size(px(14.0))
                .rounded(px(4.0))
                .tooltip("Fechar")
                .on_click(move |_ev, _win, cx| {
                    close_entity.update(cx, |state, cx| {
                        state.close_pins_panel();
                        cx.notify();
                    });
                }),
        );

    let mut body = div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .p(px(12.0))
        .flex()
        .flex_col()
        .gap(px(8.0));

    if state.is_pins_loading {
        body = body.child(
            div()
                .text_color(text_muted())
                .text_size(font_size_sm())
                .child("Carregando mensagens fixadas..."),
        );
    } else if let Some(ref err) = state.pinned_messages_error {
        body = body.child(
            div()
                .p(px(10.0))
                .rounded(radius_sm())
                .bg(with_alpha(HEX_STATUS_DANGER, 0.12))
                .border_1()
                .border_color(status_danger())
                .text_color(status_danger())
                .text_size(font_size_sm())
                .child(err.clone()),
        );
    } else if state.pinned_messages.is_empty() {
        body = body.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .py(px(24.0))
                .child(icon_pin(px(32.0), text_muted()))
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(font_size_sm())
                        .child("Nenhuma mensagem fixada"),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(font_size_xs())
                        .child("As mensagens fixadas neste canal aparecerão aqui."),
                ),
        );
    } else {
        let can_manage = state
            .selected_guild_id
            .is_none()
            || state.selected_channel_id.as_deref() == Some(channel_id);
        for msg in &state.pinned_messages {
            let msg_jump = msg.clone();
            let msg_unpin = msg.clone();
            let app_jump = app_entity.clone();
            let app_unpin = app_entity.clone();
            let chat_unpin = chat_entity.clone();
            let cid = channel_id.to_string();
            let content = if msg.content.trim().is_empty() {
                if !msg.attachments.is_empty() {
                    format!("{} anexo(s)", msg.attachments.len())
                } else {
                    "Mensagem sem texto".to_string()
                }
            } else {
                msg.content.clone()
            };

            let mut card = div()
                .p(px(10.0))
                .rounded(radius_md())
                .bg(bg_channels())
                .border_1()
                .border_color(border_subtle())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            Avatar::new(msg.author.display_name().to_string())
                                .size(AvatarSize::Px24)
                                .opt_src(msg.author.avatar_url.clone())
                                .opt_decoration(msg.author.avatar_decoration_data.clone())
                                .cutout_color(bg_channels()),
                        )
                        .child(
                            div()
                                .text_color(text_titles())
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::BOLD)
                                .child(msg.author.display_name().to_string()),
                        ),
                )
                .child(
                    div()
                        .text_color(text_primary())
                        .text_size(font_size_sm())
                        .overflow_hidden()
                        .max_h(px(72.0))
                        .child(content),
                );

            let mut actions = div().flex().flex_row().gap(px(8.0)).child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(radius_sm())
                    .bg(channel_hover())
                    .cursor_pointer()
                    .text_size(font_size_xs())
                    .text_color(text_titles())
                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                        let cid_jump = msg_jump.channel_id.clone();
                        let mid = msg_jump.id.clone();
                        app_jump.update(cx, |state, cx| {
                            state.jump_to_message(&cid_jump, &mid);
                            state.close_pins_panel();
                            cx.notify();
                        });
                    })
                    .child("Ir para mensagem"),
            );

            if can_manage {
                actions = actions.child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(radius_sm())
                        .bg(with_alpha(HEX_STATUS_DANGER, 0.12))
                        .cursor_pointer()
                        .text_size(font_size_xs())
                        .text_color(status_danger())
                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                            let mid = msg_unpin.id.clone();
                            let cid_unpin = cid.clone();
                            let cm = app_unpin.read(cx).client_manager.clone();
                            if let Some(cm) = cm {
                                let app_async = app_unpin.clone();
                                let chat_async = chat_unpin.clone();
                                cx.spawn(|cx: &mut gpui::AsyncApp| {
                                    let mut cx = cx.clone();
                                    async move {
                                        let res = cm.unpin_message(&cid_unpin, &mid).await;
                                        match res {
                                            Ok(()) => {
                                                let _ = cx.update(|cx| {
                                                    let _ = app_async.update(cx, |state, cx| {
                                                        state.set_message_pinned(&cid_unpin, &mid, false);
                                                        cx.notify();
                                                    });
                                                });
                                            }
                                            Err(err) => {
                                                let err = err.to_string();
                                                let _ = cx.update(|cx| {
                                                    let _ = chat_async.update(cx, |chat, cx| {
                                                        chat.pin_error = Some(err);
                                                        cx.notify();
                                                    });
                                                });
                                            }
                                        }
                                    }
                                })
                                .detach();
                            } else {
                                chat_unpin.update(cx, |chat, cx| {
                                    chat.pin_error = Some("Não conectado à API do Discord".to_string());
                                    cx.notify();
                                });
                            }
                        })
                        .child("Desafixar"),
                );
            }

            card = card.child(actions);
            body = body.child(card);
        }
    }

    div()
        .w(px(420.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg_channels())
        .border_l_1()
        .border_color(bg_server())
        .flex_shrink_0()
        .child(header)
        .child(body)
        .into_any_element()
}
