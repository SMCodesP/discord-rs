//! Discord RS - Chat View Component
//! Complete reactive chat stream combining virtualized message list, welcome header,
//! hover actions, active replies, and message editor.

use gpui::*;
use std::sync::Arc;

use crate::app::state::AppState;
use crate::chat::editor::{
    render_chat_editor, ChatEditorConfig, EditorActionCallback, EditorSendCallback,
    EditorTextChangeCallback,
};
use crate::chat::message_list::{
    build_chat_list_items, render_date_separator_element, render_message_row,
    render_welcome_banner, ChatListItem, MessageActionCallback, MessageRowConfig,
};
use crate::chat::media_picker::{
    MediaPickerConfig, MediaPickerModal, MediaPickerTab,
    OnCloseMediaPickerCallback, OnPlaySoundCallback, OnSearchQueryCallback,
    OnSelectEmojiCallback, OnSelectGifCallback, OnSelectGifCategoryCallback,
    OnSelectStickerCallback, OnTabChangeCallback,
};
use crate::chat::reactions::{AddReactionCallback, ReactionActionCallback};
use crate::domain::message::Message;
use crate::tokens::colors::*;

/// Reactive Chat View managing real messages and input state for the active channel.
pub struct ChatView {
    pub state: Entity<AppState>,
    pub replying_to: Option<Message>,
    pub typed_text: String,
    pub typing_users: Vec<String>,
    pub focus_handle: FocusHandle,
    pub search_focus_handle: FocusHandle,
    pub scroll_handle: ScrollHandle,
    pub list_state: ListState,
    pub last_channel_id: Option<String>,
    pub last_message_count: usize,
    pub media_picker_open: bool,
    pub media_picker_tab: MediaPickerTab,
    pub media_picker_search: String,
}

impl ChatView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let search_focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        // Virtual ListState configured with bottom alignment (canonical chat stream)
        // and 250px overdraw to eliminate blank flashes during 144+ FPS scrolling
        let list_state = ListState::new(0, ListAlignment::Bottom, px(250.0));
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        Self {
            state,
            replying_to: None,
            typed_text: String::new(),
            typing_users: Vec::new(),
            focus_handle,
            search_focus_handle,
            scroll_handle,
            list_state,
            last_channel_id: None,
            last_message_count: 0,
            media_picker_open: false,
            media_picker_tab: MediaPickerTab::Emojis,
            media_picker_search: String::new(),
        }
    }

    pub fn set_typed_text(&mut self, text: impl Into<String>) {
        self.typed_text = text.into();
    }

    pub fn set_replying_to(&mut self, message: Option<Message>) {
        self.replying_to = message;
    }

    pub fn set_typing_users(&mut self, users: Vec<String>) {
        self.typing_users = users;
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .map(|c| c.name.clone())
            .unwrap_or_else(|| {
                if let Some(ref cid) = app_state.selected_channel_id {
                    cid.clone()
                } else {
                    "geral".to_string()
                }
            });
        let channel_id = selected_channel_opt
            .as_ref()
            .map(|c| c.id.clone())
            .or_else(|| app_state.selected_channel_id.clone())
            .unwrap_or_else(|| "chan-general".to_string());
        let channel_topic = selected_channel_opt
            .as_ref()
            .and_then(|c| c.topic.clone())
            .unwrap_or_else(|| format!("Este é o início do canal #{}.", channel_name));

        // ZERO DEEP CLONES: borrow direct slice from AppState
        let all_messages = app_state.get_active_messages();
        let message_count = all_messages.len();

        // Build flattened virtual item list with zero message cloning (O(N) index mapping)
        // No static limitation: thousands of messages are handled seamlessly
        let chat_items = build_chat_list_items(all_messages);
        let total_items = chat_items.len();

        // Synchronize ListState when channel switches or message count changes
        let channel_changed = self.last_channel_id.as_deref() != Some(&channel_id);
        let count_changed = message_count != self.last_message_count;
        if channel_changed || count_changed {
            self.last_channel_id = Some(channel_id.clone());
            self.last_message_count = message_count;
            self.list_state.reset(total_items);
            self.scroll_handle.scroll_to_bottom();
        } else if self.list_state.item_count() != total_items {
            self.list_state.reset(total_items);
        }

        let app_entity = self.state.clone();
        let self_entity = cx.entity().clone();

        // 1. Message Action: React (quick heart)
        let cid_for_react = channel_id.clone();
        let app_for_react = app_entity.clone();
        let on_react: Option<MessageActionCallback> = Some(Arc::new(move |message_id: &str, _window: &mut Window, cx: &mut App| {
            let msg_id = message_id.to_string();
            let cid = cid_for_react.clone();
            app_for_react.update(cx, |state, cx| {
                state.add_reaction_to_message(&cid, &msg_id, "❤️");
                cx.notify();
            });
        }));

        // 2. Message Action: Reply (zero messages vector clone - queries by ID on demand)
        let cid_for_reply = channel_id.clone();
        let app_for_reply = app_entity.clone();
        let self_for_reply = self_entity.clone();
        let on_reply: Option<MessageActionCallback> = Some(Arc::new(move |message_id: &str, _window: &mut Window, cx: &mut App| {
            let msg_id = message_id.to_string();
            let target_msg = app_for_reply.read(cx)
                .messages_by_channel
                .get(&cid_for_reply)
                .and_then(|msgs| msgs.iter().find(|m| m.id == msg_id).cloned());
            if let Some(msg) = target_msg {
                self_for_reply.update(cx, |this, cx| {
                    this.replying_to = Some(msg);
                    cx.notify();
                });
            }
        }));

        // 3. Reaction Click: toggle/add specific emoji
        let cid_for_rx_click = channel_id.clone();
        let app_for_rx_click = app_entity.clone();
        let on_reaction_click: Option<ReactionActionCallback> = Some(Arc::new(move |message_id: &str, emoji: &str, _window: &mut Window, cx: &mut App| {
            let msg_id = message_id.to_string();
            let em = emoji.to_string();
            let cid = cid_for_rx_click.clone();
            app_for_rx_click.update(cx, |state, cx| {
                state.add_reaction_to_message(&cid, &msg_id, &em);
                cx.notify();
            });
        }));

        // 4. Add Reaction (+) button
        let cid_for_add_rx = channel_id.clone();
        let app_for_add_rx = app_entity.clone();
        let on_add_reaction: Option<AddReactionCallback> = Some(Arc::new(move |message_id: &str, _window: &mut Window, cx: &mut App| {
            let msg_id = message_id.to_string();
            let cid = cid_for_add_rx.clone();
            app_for_add_rx.update(cx, |state, cx| {
                state.add_reaction_to_message(&cid, &msg_id, "👍");
                cx.notify();
            });
        }));

        // 5. Send message
        let cid_for_send = channel_id.clone();
        let app_for_send = app_entity.clone();
        let self_for_send = self_entity.clone();

        let on_send: Option<EditorSendCallback> = Some(Arc::new(move |content: &str, _reply_id: Option<&str>, _window: &mut Window, cx: &mut App| {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return;
            }

            let text = trimmed.to_string();
            let cid = cid_for_send.clone();
            app_for_send.update(cx, |state, cx| {
                state.send_message(&cid, &text);
                cx.notify();
            });

            self_for_send.update(cx, |this, cx| {
                this.typed_text.clear();
                this.replying_to = None;
                this.scroll_handle.scroll_to_bottom();
                let cnt = this.list_state.item_count();
                if cnt > 0 {
                    this.list_state.scroll_to_reveal_item(cnt.saturating_sub(1));
                }
                cx.notify();
            });
        }));

        // 6. Dismiss / Cancel Reply
        let self_for_cancel = self_entity.clone();
        let on_cancel_reply: Option<EditorActionCallback> = Some(Arc::new(move |_window: &mut Window, cx: &mut App| {
            self_for_cancel.update(cx, |this, cx| {
                this.replying_to = None;
                cx.notify();
            });
        }));

        // 7. Text change (live typing synchronization)
        let self_for_text_change = self_entity.clone();
        let on_text_change: Option<EditorTextChangeCallback> = Some(Arc::new(move |new_text: &str, _window: &mut Window, cx: &mut App| {
            let t = new_text.to_string();
            self_for_text_change.update(cx, |this, cx| {
                this.typed_text = t;
                cx.notify();
            });
        }));

        // Virtualized list items rendering
        let items_arc = Arc::new(chat_items.clone());
        let ch_name = channel_name.clone();
        let ch_topic = channel_topic.clone();
        let app_entity_list = app_entity.clone();
        let on_react_item = on_react.clone();
        let on_reply_item = on_reply.clone();
        let on_rx_click_item = on_reaction_click.clone();
        let on_add_rx_item = on_add_reaction.clone();

        let virtual_list = list(self.list_state.clone(), move |ix, _window, cx| {
            if ix >= items_arc.len() {
                return div().into_any_element();
            }

            match &items_arc[ix] {
                ChatListItem::WelcomeHeader => {
                    render_welcome_banner(&ch_name, &ch_topic).into_any_element()
                }
                ChatListItem::DateSeparator(date) => {
                    render_date_separator_element(*date).into_any_element()
                }
                ChatListItem::Message { message_idx, is_grouped } => {
                    let state = app_entity_list.read(cx);
                    let msgs = state.get_active_messages();
                    if let Some(msg) = msgs.get(*message_idx) {
                        render_message_row(
                            msg,
                            MessageRowConfig {
                                is_grouped: *is_grouped,
                                role_color: None,
                                on_react: on_react_item.clone(),
                                on_reply: on_reply_item.clone(),
                                on_reaction_click: on_rx_click_item.clone(),
                                on_add_reaction: on_add_rx_item.clone(),
                            },
                        )
                        .into_any_element()
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

        // Handle target message scroll/focus from Search results
        if let Some(ref target_id) = app_state.target_message_id {
            if let Some(target_idx) = chat_items.iter().position(|item| {
                if let ChatListItem::Message { message_idx, .. } = item {
                    all_messages.get(*message_idx).map_or(false, |m| &m.id == target_id)
                } else {
                    false
                }
            }) {
                self.list_state.scroll_to_reveal_item(target_idx);
            }
        }

        // Query active typing users directly from AppState's coalescing tracker
        let active_typing = app_state.get_active_typing_users(&channel_id);
        let typing_users = if !active_typing.is_empty() {
            active_typing
        } else {
            self.typing_users.clone()
        };

        // Render search flyout if open (418px width, #2b2d31 bg, #1e1f22 border)
        let is_search_open = app_state.is_search_open;
        let search_panel = if is_search_open {
            Some(crate::chat::search::render_search_flyout(
                app_entity.clone(),
                app_state,
                &self.search_focus_handle,
            ))
        } else {
            None
        };

        // 8. Rich Media Picker Callbacks (GIF, Sticker, Emoji)
        let self_for_gif = self_entity.clone();
        let on_gif: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_for_gif.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Gifs {
                    this.media_picker_open = false;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Gifs;
                }
                cx.notify();
            });
        }));

        let self_for_sticker = self_entity.clone();
        let on_sticker: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_for_sticker.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Stickers {
                    this.media_picker_open = false;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Stickers;
                }
                cx.notify();
            });
        }));

        let self_for_emoji = self_entity.clone();
        let on_emoji: Option<EditorActionCallback> = Some(Arc::new(move |_window, cx| {
            self_for_emoji.update(cx, |this, cx| {
                if this.media_picker_open && this.media_picker_tab == MediaPickerTab::Emojis {
                    this.media_picker_open = false;
                } else {
                    this.media_picker_open = true;
                    this.media_picker_tab = MediaPickerTab::Emojis;
                }
                cx.notify();
            });
        }));

        let media_picker_overlay = if self.media_picker_open {
            let self_close = self_entity.clone();
            let on_close: Option<OnCloseMediaPickerCallback> = Some(Arc::new(move |_win, cx| {
                self_close.update(cx, |this, cx| {
                    this.media_picker_open = false;
                    cx.notify();
                });
            }));

            let self_tab = self_entity.clone();
            let on_tab_change: Option<OnTabChangeCallback> = Some(Arc::new(move |tab, _win, cx| {
                self_tab.update(cx, |this, cx| {
                    this.media_picker_tab = tab;
                    cx.notify();
                });
            }));

            let self_emoji = self_entity.clone();
            let app_emoji = app_entity.clone();
            let on_select_emoji: Option<OnSelectEmojiCallback> = Some(Arc::new(move |emoji, _win, cx| {
                let em = emoji.to_string();
                self_emoji.update(cx, |this, cx| {
                    this.typed_text.push_str(&em);
                    cx.notify();
                });
                app_emoji.update(cx, |state, _cx| {
                    state.record_emoji_usage(&em);
                });
            }));

            let self_sticker = self_entity.clone();
            let app_sticker = app_entity.clone();
            let cid_sticker = channel_id.clone();
            let on_select_sticker: Option<OnSelectStickerCallback> = Some(Arc::new(move |sticker, _win, cx| {
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
            let app_gif = app_entity.clone();
            let cid_gif = channel_id.clone();
            let on_select_gif: Option<OnSelectGifCallback> = Some(Arc::new(move |gif_url, _win, cx| {
                let url = gif_url.to_string();
                let cid = cid_gif.clone();
                app_gif.update(cx, |state, cx| {
                    state.send_message(&cid, &url);
                    cx.notify();
                });
                self_gif.update(cx, |this, cx| {
                    this.media_picker_open = false;
                    cx.notify();
                });
            }));

            let on_play_sound: Option<OnPlaySoundCallback> = Some(Arc::new(move |sound, _win, _cx| {
                log::info!("Soundboard sound preview: {}", sound.name);
            }));

            let self_sound = self_entity.clone();
            let app_sound = app_entity.clone();
            let cid_sound = channel_id.clone();
            let on_send_sound: Option<OnPlaySoundCallback> = Some(Arc::new(move |sound, _win, cx| {
                let s_name = sound.name.clone();
                let sound_text = if let Some(ref em) = sound.emoji_name {
                    format!("[Som da Soundboard: {} {}]", em, s_name)
                } else {
                    format!("[Som da Soundboard: {}]", s_name)
                };
                let cid = cid_sound.clone();
                app_sound.update(cx, |state, cx| {
                    state.send_message(&cid, &sound_text);
                    cx.notify();
                });
                self_sound.update(cx, |this, cx| {
                    this.media_picker_open = false;
                    cx.notify();
                });
            }));

            let self_search = self_entity.clone();
            let on_search_query: Option<OnSearchQueryCallback> = Some(Arc::new(move |query, _win, cx| {
                let q = query.to_string();
                self_search.update(cx, |this, cx| {
                    this.media_picker_search = q;
                    cx.notify();
                });
            }));

            let self_cat = self_entity.clone();
            let on_select_gif_category: Option<OnSelectGifCategoryCallback> = Some(Arc::new(move |cat, _win, cx| {
                let c = cat.to_string();
                self_cat.update(cx, |this, cx| {
                    this.media_picker_search = c;
                    cx.notify();
                });
            }));

            Some(
                div()
                    .absolute()
                    .bottom(px(68.0))
                    .right(px(16.0))
                    .child(MediaPickerModal::render(MediaPickerConfig {
                        active_tab: self.media_picker_tab,
                        search_query: self.media_picker_search.clone(),
                        recent_emojis: Vec::new(),
                        custom_emojis: Vec::new(),
                        stickers: Vec::new(),
                        gifs: Vec::new(),
                        sounds: Vec::new(),
                        hovered_info: None,
                        on_select_emoji,
                        on_select_sticker,
                        on_select_gif,
                        on_play_sound,
                        on_send_sound,
                        on_close,
                        on_tab_change,
                        on_search_query,
                        on_select_gif_category,
                    }))
            )
        } else {
            None
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
            // 1. Fully Virtualized Message Stream Area
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
            // 2. Chat Input Editor Bar
            .child(render_chat_editor(ChatEditorConfig {
                channel_name,
                text: self.typed_text.clone(),
                replying_to: self.replying_to.clone(),
                mention_author: true,
                typing_users,
                focus_handle: Some(self.focus_handle.clone()),
                is_focused: self.focus_handle.is_focused(window),
                on_text_change,
                on_send,
                on_attachment: None,
                on_gif,
                on_sticker,
                on_emoji,
                on_toggle_mention: None,
                on_cancel_reply,
                active_media_tab: if self.media_picker_open { Some(self.media_picker_tab) } else { None },
            }))
            .children(media_picker_overlay);

        let app_entity_shortcut = app_entity.clone();

        div()
            .size_full()
            .min_h(px(0.0))
            .min_w(px(0.0))
            .flex()
            .flex_row()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
                if is_ctrl && ev.keystroke.key.to_lowercase() == "f" {
                    app_entity_shortcut.update(cx, |state, cx| {
                        state.toggle_search();
                        cx.notify();
                    });
                }
            })
            .child(main_chat_col)
            .children(search_panel)
    }
}
