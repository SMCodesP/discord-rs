//! Discord RS - Chat View Component
//! Complete reactive chat stream combining kit v_virtual_list message stream,
//! welcome header, hover actions, active replies, popover media picker, and message editor.

use gpui::component::input::{InputEvent, TextareaState};
use gpui::*;
use std::sync::Arc;

use crate::app::state::AppState;
use crate::chat::editor::{
    ChatComposer, EditorActionCallback, EditorSendCallback,
};
use crate::chat::media_picker::{
    MediaPickerConfig, MediaPickerGuildSection, MediaPickerModal, MediaPickerTab,
    OnCloseMediaPickerCallback, OnPlaySoundCallback, OnSearchQueryCallback,
    OnSelectEmojiCallback, OnSelectGifCallback, OnSelectGifCategoryCallback,
    OnSelectGuildCallback, OnSelectStickerCallback, OnTabChangeCallback,
};
use crate::chat::message_list::{
    build_chat_list_items, render_date_separator_element, render_message_context_menu,
    render_message_row, render_welcome_banner, ChatListItem, MessageActionCallback,
    MessageActionPosCallback, MessageContextMenuConfig, MessageRowConfig,
};
use crate::chat::reactions::{AddReactionCallback, ReactionActionCallback};
use crate::domain::message::Message;
use crate::protocol::media_cache::MediaCacheManager;
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
    pub media_picker_react_message_id: Option<String>,
    pub media_picker_focus_handle: FocusHandle,
    pub media_picker_selected_guild_id: Option<String>,
    pub media_picker_list_state: ListState,
    pub active_context_menu: Option<(String, Point<Pixels>)>,
    pub composer_state: Option<Entity<TextareaState>>,
    pub composer_sub: Option<Subscription>,
}

impl ChatView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let search_focus_handle = cx.focus_handle();
        let media_picker_focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        // Virtual ListState configured with bottom alignment (canonical chat stream)
        let list_state = ListState::new(0, ListAlignment::Bottom, px(250.0));
        // Top-aligned ListState for rich media picker popup with 150px overdraw windowing
        let media_picker_list_state = ListState::new(0, ListAlignment::Top, px(150.0));
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
            media_picker_focus_handle,
            scroll_handle,
            list_state,
            last_channel_id: None,
            last_message_count: 0,
            media_picker_open: false,
            media_picker_tab: MediaPickerTab::Emojis,
            media_picker_search: String::new(),
            media_picker_react_message_id: None,
            media_picker_selected_guild_id: None,
            media_picker_list_state,
            active_context_menu: None,
            composer_state: None,
            composer_sub: None,
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
                    let content = st_for_sub.read(cx).value().to_string();
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        let text = trimmed.to_string();
                        let app_state = this.state.read(cx);
                        let cid = if let Some((_, ch)) = app_state.get_selected_channel() {
                            ch.id.clone()
                        } else if let Some(ref cid) = app_state.selected_channel_id {
                            cid.clone()
                        } else {
                            "chan-general".to_string()
                        };

                        this.state.update(cx, |state, cx| {
                            state.send_message(&cid, &text);
                            cx.notify();
                        });

                        st_for_sub.update(cx, |s, cx| {
                            s.set_value("", window, cx);
                        });

                        this.typed_text.clear();
                        this.replying_to = None;
                        this.scroll_handle.scroll_to_bottom();
                        let cnt = this.list_state.item_count();
                        if cnt > 0 {
                            this.list_state.scroll_to_reveal_item(cnt.saturating_sub(1));
                        }
                        cx.notify();
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
        let composer_state = self.ensure_composer_state(window, cx);
        if !self.typed_text.is_empty() && composer_state.read(cx).value() != self.typed_text.as_str() {
            let t = self.typed_text.clone();
            composer_state.update(cx, |s, cx| {
                s.set_value(t, window, cx);
            });
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
        let chat_items = build_chat_list_items(all_messages);
        let total_items = chat_items.len();

        // Synchronize scroll state when channel switches or message count changes
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

        // 1. Message Action: React -> opens emoji picker for that specific message
        let self_for_react = self_entity.clone();
        let app_for_react = app_entity.clone();
        let on_react: Option<MessageActionCallback> = Some(Arc::new(move |message_id: &str, window: &mut Window, cx: &mut App| {
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
        }));

        // 2. Message Action: Reply
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

        // 4. Add Reaction (+) button -> opens emoji picker for that specific message
        let self_for_add_rx = self_entity.clone();
        let app_for_add_rx = app_entity.clone();
        let on_add_reaction: Option<AddReactionCallback> = Some(Arc::new(move |message_id: &str, window: &mut Window, cx: &mut App| {
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
        }));

        // 5. More Action: (•••) button -> opens message context menu
        let self_for_more = self_entity.clone();
        let on_more: Option<MessageActionPosCallback> = Some(Arc::new(move |message_id: &str, pos: Point<Pixels>, _window: &mut Window, cx: &mut App| {
            let msg_id = message_id.to_string();
            self_for_more.update(cx, |this, cx| {
                this.active_context_menu = Some((msg_id, pos));
                cx.notify();
            });
        }));

        // 6. Send message
        let cid_for_send = channel_id.clone();
        let app_for_send = app_entity.clone();
        let self_for_send = self_entity.clone();
        let st_for_send = composer_state.clone();

        let on_send: Option<EditorSendCallback> = Some(Arc::new(move |content: &str, _reply_id: Option<&str>, window: &mut Window, cx: &mut App| {
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

            st_for_send.update(cx, |s, cx| {
                s.set_value("", window, cx);
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

        // 7. Dismiss / Cancel Reply
        let self_for_cancel = self_entity.clone();
        let on_cancel_reply: Option<EditorActionCallback> = Some(Arc::new(move |_window: &mut Window, cx: &mut App| {
            self_for_cancel.update(cx, |this, cx| {
                this.replying_to = None;
                cx.notify();
            });
        }));

        // Virtualized list items rendering via GPUI list
        let items_arc = Arc::new(chat_items.clone());
        let ch_name = channel_name.clone();
        let ch_topic = channel_topic.clone();
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
                                on_more: on_more_item.clone(),
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

        // Render search flyout if open
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

        // 8. Rich Media Picker Callbacks & Popover
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
            let on_tab_change: Option<OnTabChangeCallback> = Some(Arc::new(move |tab, _win, cx| {
                self_tab.update(cx, |this, cx| {
                    this.media_picker_tab = tab;
                    this.media_picker_list_state.reset(0);
                    cx.notify();
                });
            }));

            let self_emoji = self_entity.clone();
            let app_emoji = app_entity.clone();
            let cid_emoji = channel_id.clone();
            let on_select_emoji: Option<OnSelectEmojiCallback> = Some(Arc::new(move |emoji, _win, cx| {
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
                    this.media_picker_list_state.reset(0);
                    cx.notify();
                });
            }));

            let self_cat = self_entity.clone();
            let on_select_gif_category: Option<OnSelectGifCategoryCallback> = Some(Arc::new(move |cat, _win, cx| {
                let c = cat.to_string();
                self_cat.update(cx, |this, cx| {
                    this.media_picker_search = c;
                    this.media_picker_list_state.reset(0);
                    cx.notify();
                });
            }));

            let self_guild = self_entity.clone();
            let on_select_guild: Option<OnSelectGuildCallback> = Some(Arc::new(move |gid_opt, _win, cx| {
                let next_gid = gid_opt.map(|s| s.to_string());
                self_guild.update(cx, |this, cx| {
                    this.media_picker_selected_guild_id = next_gid;
                    this.media_picker_list_state.reset(0);
                    cx.notify();
                });
            }));

            let guild_sections: Vec<MediaPickerGuildSection> = app_state
                .guilds
                .iter()
                .map(|g| MediaPickerGuildSection {
                    guild_id: g.id.clone(),
                    guild_name: g.name.clone(),
                    icon_url: g.icon_url(),
                    acronym: g.acronym.clone(),
                    emojis: g.emojis.clone(),
                    stickers: g.stickers.clone(),
                })
                .collect();

            let is_picker_focused = self.media_picker_focus_handle.is_focused(window);

            let modal_content = MediaPickerModal::render(MediaPickerConfig {
                active_tab: self.media_picker_tab,
                search_query: self.media_picker_search.clone(),
                recent_emojis: app_state.media_cache.get_recent_emojis_sync(),
                custom_emojis: app_state.get_current_custom_emojis(),
                stickers: app_state.get_current_stickers(),
                gifs: MediaCacheManager::get_curated_tenor_gifs_for_category(&self.media_picker_search),
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

        let context_menu_overlay = if let Some((ref target_id, click_pos)) = self.active_context_menu {
            let target_msg = all_messages.iter().find(|m| &m.id == target_id).cloned();
            if let Some(msg) = target_msg {
                let vp = window.viewport_size();
                let menu_width = px(222.0);
                let menu_height = px(240.0);
                let top_y = if click_pos.y + menu_height > vp.height {
                    (click_pos.y - menu_height).max(px(8.0))
                } else {
                    click_pos.y.max(px(8.0))
                };
                let left_x = if click_pos.x + menu_width > vp.width {
                    (vp.width - menu_width - px(16.0)).max(px(8.0))
                } else {
                    click_pos.x.max(px(8.0))
                };

                let msg_id = msg.id.clone();
                let msg_content = msg.content.clone();
                let is_pinned = msg.pinned;

                let self_close = self_entity.clone();
                let on_close: Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static> = Arc::new(move |_win, cx| {
                    self_close.update(cx, |this, cx| {
                        this.active_context_menu = None;
                        cx.notify();
                    });
                });

                let self_add_rx = self_entity.clone();
                let on_cm_add_rx: Option<MessageActionCallback> = Some(Arc::new(move |m_id: &str, _win, cx| {
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
                let on_cm_reply: Option<MessageActionCallback> = Some(Arc::new(move |_m_id: &str, _win, cx| {
                    let m = msg_clone.clone();
                    self_cm_reply.update(cx, |this, cx| {
                        this.replying_to = Some(m);
                        this.active_context_menu = None;
                        cx.notify();
                    });
                }));

                let content_for_copy = msg_content.clone();
                let on_cm_copy_text: Option<MessageActionCallback> = Some(Arc::new(move |_m_id: &str, _win, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(content_for_copy.clone()));
                }));

                let id_for_copy = msg_id.clone();
                let on_cm_copy_id: Option<MessageActionCallback> = Some(Arc::new(move |_m_id: &str, _win, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(id_for_copy.clone()));
                }));

                let app_cm_pin = app_entity.clone();
                let cid_pin = channel_id.clone();
                let on_cm_pin: Option<MessageActionCallback> = Some(Arc::new(move |m_id: &str, _win, cx| {
                    let mid = m_id.to_string();
                    let cid = cid_pin.clone();
                    app_cm_pin.update(cx, |state, cx| {
                        state.pin_message(&cid, &mid);
                        cx.notify();
                    });
                }));

                let app_cm_del = app_entity.clone();
                let cid_del = channel_id.clone();
                let on_cm_delete: Option<MessageActionCallback> = Some(Arc::new(move |m_id: &str, _win, cx| {
                    let mid = m_id.to_string();
                    let cid = cid_del.clone();
                    app_cm_del.update(cx, |state, cx| {
                        state.delete_message(&cid, &mid);
                        cx.notify();
                    });
                }));

                let self_dismiss = self_entity.clone();
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                            self_dismiss.update(cx, |this, cx| {
                                this.active_context_menu = None;
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .absolute()
                                .top(top_y)
                                .left(left_x)
                                .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                    cx.stop_propagation();
                                })
                                .child(render_message_context_menu(MessageContextMenuConfig {
                                    message_id: msg_id,
                                    message_content: msg_content,
                                    is_pinned,
                                    on_add_reaction: on_cm_add_rx,
                                    on_reply: on_cm_reply,
                                    on_copy_text: on_cm_copy_text,
                                    on_copy_id: on_cm_copy_id,
                                    on_pin: on_cm_pin,
                                    on_delete: on_cm_delete,
                                    on_close: Some(on_close),
                                })),
                        ),
                )
            } else {
                None
            }
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
            // 2. Chat Input Editor Bar
            .child(
                ChatComposer::new("chat-composer", &composer_state)
                    .channel_name(channel_name)
                    .replying_to(self.replying_to.clone())
                    .mention_author(true)
                    .typing_users(typing_users)
                    .active_media_tab(if self.media_picker_open { Some(self.media_picker_tab) } else { None })
                    .on_gif(on_gif)
                    .on_sticker(on_sticker)
                    .on_emoji(on_emoji)
                    .on_cancel_reply(on_cancel_reply)
                    .on_send(on_send),
            )
            .children(media_picker_overlay)
            .children(context_menu_overlay);

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
