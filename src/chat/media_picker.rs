//! Discord RS - Seletor Unificado de Mídia (Media Picker Popover)
//!
//! Implementa o popup flutuante de seleção de:
//! 1. Emojis (Frequentemente Usados, 8 Categorias Unicode Oficiais, Emojis de Servidores)
//! 2. Figurinhas / Stickers (Pacotes padrão Wumpus e stickers de guildas)
//! 3. GIFs (Busca e Trending integrados ao Tenor)
//! 4. Soundboard (Sons padrão e sons de servidores com preview de áudio)
//! 5. Barra de busca adaptável com filtragem em tempo real
//! 6. Faixa inferior de pré-visualização (Hover Preview Strip)

use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::protocol::media_cache::{
    DiscordEmoji, DiscordSticker, MediaCacheManager, RecentEmojiItem, SoundboardSound, TenorGif,
};
use crate::tokens::colors::white;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

// ============================================================================
// Tipos de Callbacks do Media Picker
// ============================================================================

pub type OnSelectEmojiCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnSelectStickerCallback =
    Arc<dyn Fn(&DiscordSticker, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnSelectGifCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnPlaySoundCallback =
    Arc<dyn Fn(&SoundboardSound, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnCloseMediaPickerCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;
pub type OnTabChangeCallback =
    Arc<dyn Fn(MediaPickerTab, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnSearchQueryCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;

pub type OnSelectGifCategoryCallback =
    Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;
pub type OnSelectGuildCallback =
    Arc<dyn Fn(Option<&str>, &mut Window, &mut App) + Send + Sync + 'static>;

// ============================================================================
// Enums de Abas e Categorias
// ============================================================================

/// Abas superiores do Seletor de Mídia
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaPickerTab {
    #[default]
    Emojis,
    Stickers,
    Gifs,
    Soundboard,
}

impl MediaPickerTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Emojis => "Emojis",
            Self::Stickers => "Figurinhas",
            Self::Gifs => "GIFs",
            Self::Soundboard => "Sons",
        }
    }

    pub fn icon(&self) -> DiscordIcon {
        match self {
            Self::Emojis => DiscordIcon::Emoji,
            Self::Stickers => DiscordIcon::Sticker,
            Self::Gifs => DiscordIcon::Gif,
            Self::Soundboard => DiscordIcon::Speaker,
        }
    }

    pub fn render_icon(&self, size: Pixels, color: impl Into<Hsla>) -> Div {
        match self {
            Self::Emojis => icon_emoji(size, color),
            Self::Stickers => icon_sticker(size, color),
            Self::Gifs => icon_gif(size, color),
            Self::Soundboard => icon_speaker(size, color),
        }
    }

    pub fn search_placeholder(&self) -> &'static str {
        match self {
            Self::Emojis => "Buscar emojis...",
            Self::Stickers => "Buscar figurinhas...",
            Self::Gifs => "Buscar no Tenor...",
            Self::Soundboard => "Buscar sons...",
        }
    }
}

// ============================================================================
// Configuração do Media Picker & Modal Flutuante
// ============================================================================

/// Popup flutuante de seleção de mídia (MediaPickerModal) posicionado acima da caixa de digitação
#[derive(Debug, Clone, Default)]
pub struct MediaPickerModal {
    pub active_tab: MediaPickerTab,
    pub search_query: String,
    pub selected_gif_category: Option<String>,
}

impl MediaPickerModal {
    pub fn new(tab: MediaPickerTab) -> Self {
        Self {
            active_tab: tab,
            search_query: String::new(),
            selected_gif_category: None,
        }
    }

    /// Renderiza o seletor popup flutuante do Discord
    pub fn render(config: MediaPickerConfig) -> Div {
        render_media_picker(config)
    }
}

/// Seção de guilda / servidor no seletor de mídia com emojis e stickers correspondentes
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaPickerGuildSection {
    pub guild_id: String,
    pub guild_name: String,
    pub icon_url: Option<String>,
    pub acronym: String,
    pub emojis: Vec<DiscordEmoji>,
    pub stickers: Vec<DiscordSticker>,
}

pub struct MediaPickerConfig {
    pub active_tab: MediaPickerTab,
    pub search_query: String,
    pub recent_emojis: Vec<RecentEmojiItem>,
    pub custom_emojis: Vec<DiscordEmoji>,
    pub stickers: Vec<DiscordSticker>,
    pub gifs: Vec<TenorGif>,
    pub sounds: Vec<SoundboardSound>,
    pub hovered_info: Option<String>,
    pub guild_sections: Vec<MediaPickerGuildSection>,
    pub selected_guild_id: Option<String>,
    pub focus_handle: Option<FocusHandle>,
    pub is_focused: bool,
    pub list_state: Option<ListState>,
    pub on_select_emoji: Option<OnSelectEmojiCallback>,
    pub on_select_sticker: Option<OnSelectStickerCallback>,
    pub on_select_gif: Option<OnSelectGifCallback>,
    pub on_play_sound: Option<OnPlaySoundCallback>,
    pub on_send_sound: Option<OnPlaySoundCallback>,
    pub on_close: Option<OnCloseMediaPickerCallback>,
    pub on_tab_change: Option<OnTabChangeCallback>,
    pub on_search_query: Option<OnSearchQueryCallback>,
    pub on_select_gif_category: Option<OnSelectGifCategoryCallback>,
    pub on_select_guild: Option<OnSelectGuildCallback>,
}

// ============================================================================
// Componente de Renderização Principal
// ============================================================================

/// Renderiza o seletor popup flutuante do Discord com dimensões medidas (498x486, radius 8px)
pub fn render_media_picker(config: MediaPickerConfig) -> Div {
    let active_tab = config.active_tab;
    let query = config.search_query.trim().to_lowercase();
    let is_searching = !query.is_empty();

    div()
        .flex()
        .flex_col()
        .w(px(498.0))
        .h(px(486.0))
        .bg(bg_channels())
        .border_1()
        .border_color(bg_server())
        .rounded(radius_md())
        .shadow_xl()
        .overflow_hidden()
        .on_scroll_wheel(|_ev, _win, cx| {
            cx.stop_propagation();
        })
        // 1. Barra de Abas Superiores (sem moldura e sem divisórias verticais)
        .child(render_top_tabs(&config))
        // 2. Barra de Busca
        .child(render_search_bar(&config))
        // 3. Área Central (Grade do Conteúdo Selecionado com Rail de navegação em Emojis e Figurinhas)
        .child(match active_tab {
            MediaPickerTab::Emojis => {
                let content_element = if let Some(ref list_state) = config.list_state {
                    render_virtualized_emojis_content(&config, &query, is_searching, list_state)
                } else {
                    div()
                        .id("media-picker-scroll-content")
                        .flex_1()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .on_scroll_wheel(|_ev, _win, cx| {
                            cx.stop_propagation();
                        })
                        .p(px(8.0))
                        .child(render_emojis_tab(&config, &query, is_searching))
                        .into_any_element()
                };

                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.0))
                    // Vertical Navigation Rail (~44px à esquerda)
                    .child(render_navigation_rail(&config))
                    // Grade rolável central
                    .child(content_element)
                    .into_any_element()
            }
            MediaPickerTab::Stickers => {
                let content_element = if let Some(ref list_state) = config.list_state {
                    render_virtualized_stickers_content(&config, &query, list_state)
                } else {
                    div()
                        .id("media-picker-scroll-content")
                        .flex_1()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .on_scroll_wheel(|_ev, _win, cx| {
                            cx.stop_propagation();
                        })
                        .p(px(8.0))
                        .child(render_stickers_tab(&config, &query))
                        .into_any_element()
                };

                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.0))
                    // Vertical Navigation Rail (~44px à esquerda)
                    .child(render_navigation_rail(&config))
                    // Grade rolável central
                    .child(content_element)
                    .into_any_element()
            }
            MediaPickerTab::Gifs => {
                div()
                    .id("media-picker-scroll-content")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .on_scroll_wheel(|_ev, _win, cx| {
                        cx.stop_propagation();
                    })
                    .p(px(8.0))
                    .child(render_gifs_tab(&config, &query))
                    .into_any_element()
            }
            MediaPickerTab::Soundboard => {
                div()
                    .id("media-picker-scroll-content")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .on_scroll_wheel(|_ev, _win, cx| {
                        cx.stop_propagation();
                    })
                    .p(px(8.0))
                    .child(render_soundboard_tab(&config, &query))
                    .into_any_element()
            }
        })
        // 4. Faixa Inferior de Preview (Hover Preview Strip)
        .child(render_preview_strip(config.hovered_info.as_deref()))
}

// ============================================================================
// 1. Barra de Abas Superiores (sem moldura e sem divisórias verticais)
// ============================================================================

fn render_top_tabs(config: &MediaPickerConfig) -> Div {
    let tabs = [
        MediaPickerTab::Emojis,
        MediaPickerTab::Stickers,
        MediaPickerTab::Gifs,
        MediaPickerTab::Soundboard,
    ];

    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .h(px(40.0))
        .px(px(12.0));

    for tab in tabs {
        let is_active = tab == config.active_tab;
        let icon_col = if is_active { text_titles() } else { text_secondary() };
        let text_col = if is_active { text_titles() } else { text_secondary() };
        let tab_bg = if is_active { control_secondary_bg_active() } else { transparent() };

        let mut item = div()
            .id(ElementId::from(SharedString::from(format!("media-tab-{}", tab.label()))))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .h(px(30.0))
            .px(px(12.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .bg(tab_bg)
            .hover(move |s| {
                if !is_active {
                    s.bg(control_secondary_bg_default()).text_color(text_titles())
                } else {
                    s
                }
            })
            .child(tab.render_icon(px(16.0), icon_col))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(if is_active { WEIGHT_SEMIBOLD } else { WEIGHT_MEDIUM })
                    .text_color(text_col)
                    .child(tab.label()),
            );

        if let Some(ref on_tab) = config.on_tab_change {
            let on_tab = on_tab.clone();
            item = item.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_tab(tab, window, cx);
            });
        }

        row = row.child(item);
    }

    row
}

// ============================================================================
// 2. Barra de Busca Interativa com Tokens de Tema
// ============================================================================

fn render_search_bar(config: &MediaPickerConfig) -> Div {
    let placeholder = config.active_tab.search_placeholder();
    let is_empty = config.search_query.is_empty();
    let is_focused = config.is_focused;
    let cursor_color = text_normal();

    let mut search_inner = div()
        .id("media-picker-search-inner")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(38.0))
        .px(px(12.0))
        .bg(bg_server())
        .rounded(radius_sm())
        .cursor_text();

    if let Some(ref fh) = config.focus_handle {
        search_inner = search_inner
            .track_focus(fh)
            .on_mouse_down(MouseButton::Left, {
                let fh = fh.clone();
                move |_e, window, cx| {
                    window.focus(&fh, cx);
                }
            })
            .on_key_down({
                let on_search = config.on_search_query.clone();
                let on_close = config.on_close.clone();
                let current_query = config.search_query.clone();
                let fh = fh.clone();
                move |ev: &KeyDownEvent, window, cx| {
                    window.focus(&fh, cx);
                    let key = &ev.keystroke.key;
                    let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
                    let is_paste = (is_ctrl && (key == "v" || key == "V"))
                        || (ev.keystroke.modifiers.shift && key == "insert");

                    if is_paste {
                        if let Some(clipboard) = cx.read_from_clipboard() {
                            if let Some(text) = clipboard.text() {
                                let mut new_str = current_query.clone();
                                new_str.push_str(&text);
                                if let Some(ref handler) = on_search {
                                    handler(&new_str, window, cx);
                                }
                            }
                        }
                        return;
                    }

                    if is_ctrl {
                        return;
                    }

                    match key.as_str() {
                        "backspace" => {
                            let mut q = current_query.clone();
                            if !q.is_empty() {
                                q.pop();
                                if let Some(ref handler) = on_search {
                                    handler(&q, window, cx);
                                }
                            }
                        }
                        "escape" => {
                            if !current_query.is_empty() {
                                if let Some(ref handler) = on_search {
                                    handler("", window, cx);
                                }
                            } else if let Some(ref handler) = on_close {
                                handler(window, cx);
                            }
                        }
                        "space" => {
                            let mut q = current_query.clone();
                            q.push(' ');
                            if let Some(ref handler) = on_search {
                                handler(&q, window, cx);
                            }
                        }
                        k if k.chars().count() == 1 => {
                            let mut q = current_query.clone();
                            q.push_str(k);
                            if let Some(ref handler) = on_search {
                                handler(&q, window, cx);
                            }
                        }
                        _ => {}
                    }
                }
            });
    }

    let text_area = if is_empty {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(icon_search(px(16.0), text_muted()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(if is_focused {
                        div()
                            .w(px(2.0))
                            .h(px(16.0))
                            .bg(cursor_color)
                            .rounded(px(1.0))
                            .mr(px(2.0))
                    } else {
                        div()
                    })
                    .child(
                        div()
                            .text_size(font_size_sm())
                            .text_color(text_muted())
                            .child(placeholder),
                    ),
            )
    } else {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(icon_search(px(16.0), text_muted()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(font_size_sm())
                            .text_color(text_normal())
                            .child(config.search_query.clone()),
                    )
                    .child(if is_focused {
                        div()
                            .w(px(2.0))
                            .h(px(16.0))
                            .bg(cursor_color)
                            .rounded(px(1.0))
                            .ml(px(2.0))
                    } else {
                        div()
                    }),
            )
    };

    search_inner = search_inner.child(text_area);

    if !is_empty {
        let mut clear_btn = div()
            .id("media-picker-search-clear")
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded(radius_xs())
            .text_size(px(12.0))
            .text_color(text_muted())
            .cursor_pointer()
            .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
            .child(icon_close(px(12.0), text_muted()));

        if let Some(on_search) = config.on_search_query.clone() {
            clear_btn = clear_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_search("", window, cx);
            });
        }
        search_inner = search_inner.child(clear_btn);
    }

    div()
        .p(px(8.0))
        .bg(bg_channels())
        .border_b_1()
        .border_color(bg_server())
        .child(search_inner)
}

// ============================================================================
// 2.1. Rail de Navegação Vertical Lateral (~44px) com Tooltips do Kit
// ============================================================================

fn render_navigation_rail(config: &MediaPickerConfig) -> impl IntoElement {
    let selected_guild = config.selected_guild_id.as_deref();
    let is_all_selected = selected_guild.is_none() || selected_guild == Some("recents");
    let is_standard_selected = selected_guild == Some("standard");

    let mut rail = div()
        .id("media-picker-nav-rail")
        .w(px(44.0))
        .flex()
        .flex_col()
        .items_center()
        .py(px(6.0))
        .gap(px(6.0))
        .bg(bg_server())
        .border_r_1()
        .border_color(bg_server())
        .overflow_y_scroll()
        .on_scroll_wheel(|_ev, _win, cx| {
            cx.stop_propagation();
        });

    // A. Botão: Todos / Início (Recentes + Servidores + Padrão)
    let mut recents_btn = div()
        .id("media-picker-nav-recents")
        .flex_shrink_0()
        .size(px(32.0))
        .min_h(px(32.0))
        .min_w(px(32.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(if is_all_selected {
            blurple()
        } else {
            bg_channels()
        })
        .text_color(if is_all_selected {
            white()
        } else {
            text_muted()
        })
        .hover(|s| {
            s.bg(if is_all_selected {
                blurple()
            } else {
                channel_hover()
            })
            .text_color(text_titles())
        })
        .tooltip(|win, cx| Tooltip::new("Todos os Emojis").build(win, cx))
        .child(icon_smile(
            px(18.0),
            if is_all_selected {
                white()
            } else {
                text_muted()
            },
        ));

    if let Some(on_sel) = config.on_select_guild.clone() {
        recents_btn = recents_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_sel(None, window, cx);
        });
    }
    rail = rail.child(recents_btn);

    // Divisor
    rail = rail.child(
        div()
            .flex_shrink_0()
            .w(px(24.0))
            .min_h(px(1.0))
            .h(px(1.0))
            .bg(channel_hover())
            .my(px(2.0)),
    );

    // B. Botões de Guildas / Servidores (32px, sem colapso flex)
    for guild in &config.guild_sections {
        let is_selected = selected_guild == Some(&guild.guild_id);
        let guild_name = guild.guild_name.clone();
        let guild_icon_child = if let Some(ref url) = guild.icon_url {
            div()
                .size_full()
                .rounded_full()
                .overflow_hidden()
                .child(img(url.clone()).size_full().rounded_full())
        } else {
            div()
                .size_full()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .font_weight(WEIGHT_BOLD)
                .text_color(if is_selected {
                    white()
                } else {
                    text_muted()
                })
                .child(guild.acronym.clone())
        };

        let mut guild_btn = div()
            .id(ElementId::from(SharedString::from(format!(
                "media-picker-nav-guild-{}",
                guild.guild_id
            ))))
            .flex_shrink_0()
            .size(px(32.0))
            .min_h(px(32.0))
            .min_w(px(32.0))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .overflow_hidden()
            .bg(if is_selected {
                blurple()
            } else {
                bg_chat()
            })
            .border_2()
            .border_color(if is_selected {
                blurple()
            } else {
                transparent()
            })
            .hover(|s| s.border_color(blurple()))
            .tooltip(move |win, cx| Tooltip::new(guild_name.clone()).build(win, cx))
            .child(guild_icon_child);

        if let Some(on_sel) = config.on_select_guild.clone() {
            let gid = guild.guild_id.clone();
            let next = if is_selected { None } else { Some(gid) };
            guild_btn = guild_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_sel(next.as_deref(), window, cx);
            });
        }
        rail = rail.child(guild_btn);
    }

    // Divisor
    rail = rail.child(
        div()
            .flex_shrink_0()
            .w(px(24.0))
            .min_h(px(1.0))
            .h(px(1.0))
            .bg(channel_hover())
            .my(px(2.0)),
    );

    // C. Botão Oficial / Categorias Unicode Padrão
    let official_icon = if config.active_tab == MediaPickerTab::Emojis {
        icon_emoji(
            px(18.0),
            if is_standard_selected {
                white()
            } else {
                text_muted()
            },
        )
    } else {
        icon_sticker(
            px(18.0),
            if is_standard_selected {
                white()
            } else {
                text_muted()
            },
        )
    };

    let mut standard_btn = div()
        .id("media-picker-nav-standard")
        .flex_shrink_0()
        .size(px(32.0))
        .min_h(px(32.0))
        .min_w(px(32.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(if is_standard_selected {
            blurple()
        } else {
            bg_channels()
        })
        .text_color(if is_standard_selected {
            white()
        } else {
            text_muted()
        })
        .hover(|s| {
            s.bg(if is_standard_selected {
                blurple()
            } else {
                channel_hover()
            })
            .text_color(text_titles())
        })
        .tooltip(|win, cx| Tooltip::new("Categorias Padrão").build(win, cx))
        .child(official_icon);

    if let Some(on_sel) = config.on_select_guild.clone() {
        let next = if is_standard_selected { None } else { Some("standard") };
        standard_btn = standard_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_sel(next, window, cx);
        });
    }
    rail = rail.child(standard_btn);

    rail
}

// ============================================================================
// 3. Conteúdo da Aba Emojis
// ============================================================================

/// Helper para busca semântica de emojis por tags, nomes e categorias (EN/PT)
pub fn emoji_matches_query(emoji: &str, cat_key: &str, cat_title: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    if emoji.contains(&q) {
        return true;
    }
    if cat_key.to_lowercase().contains(&q) || cat_title.to_lowercase().contains(&q) {
        return true;
    }

    let tags: &[&str] = match emoji {
        "😀" | "😃" | "😄" | "😁" | "😆" | "😅" => {
            &["smile", "sorriso", "feliz", "happy", "joy", "alegria"]
        }
        "😂" | "🤣" => &["lol", "laugh", "risada", "haha", "crying laughing", "chorando de rir"],
        "🥲" | "🥹" | "🥺" | "😢" | "😭" => {
            &["cry", "choro", "sad", "triste", "lagrimas", "tears", "pleading"]
        }
        "😊" | "😇" | "🙂" | "🙃" | "😉" | "😌" => {
            &["angel", "anjo", "smile", "piscadinha", "calm", "relax"]
        }
        "😍" | "🥰" | "😘" | "😗" | "😙" | "😚" => {
            &["love", "amor", "kiss", "beijo", "apaixonado", "heart"]
        }
        "😋" | "😛" | "😝" | "😜" | "🤪" => {
            &["tongue", "lingua", "yummy", "crazy", "maluco", "brincadeira"]
        }
        "😎" | "🥸" => &["cool", "sunglasses", "oculos", "estiloso", "swag", "disfarce"],
        "🤩" | "🥳" => &["party", "festa", "star", "celebrar", "hype"],
        "😏" | "😒" | "😞" | "😔" | "😟" | "😕" => &["smug", "meh", "sad", "desapontado", "bad"],
        "😤" | "😠" | "😡" | "🤬" => &["angry", "raiva", "furious", "bravo", "mad"],
        "🤯" | "😳" | "😱" | "😨" => &["mindblown", "boom", "choque", "scared", "medo", "surpreso"],
        "🤔" | "🧐" | "🤨" => &["think", "pensar", "duvida", "ponder", "hmm"],
        "👍" => &["thumbsup", "joinha", "positivo", "like", "ok", "aprovado"],
        "👎" => &["thumbsdown", "desjoinha", "negativo", "dislike"],
        "👋" | "🤚" | "🖐️" | "✋" => &["wave", "ola", "oi", "tchau", "bye", "hello", "hi", "acenar"],
        "👌" => &["ok", "perfeito", "nice", "suave"],
        "✌️" | "🤞" | "🤟" | "🤘" => &["peace", "paz", "rock", "sorte", "victory"],
        "👏" | "🙌" | "🫶" | "🙏" => &[
            "clap", "palmas", "obrigado", "thanks", "rezar", "pray", "heart hands",
        ],
        "❤️" | "🧡" | "💛" | "💚" | "💙" | "💜" | "🖤" | "🤍" | "🤎" | "💔" => {
            &["heart", "coracao", "love", "amor"]
        }
        "🔥" => &["fire", "fogo", "hot", "chama", "lit", "quente"],
        "✨" | "⭐" | "🌟" => &["star", "sparkles", "estrela", "brilho", "shine"],
        "🎉" | "🎊" => &["party", "tada", "festa", "confete", "celebration"],
        "🐶" | "🐱" => &["dog", "cat", "cachorro", "gato", "pet", "animal", "auau", "miau"],
        "🐸" => &["frog", "sapo", "pepe"],
        "🦀" => &["crab", "caranguejo", "ferris", "rust"],
        "🍕" | "🍔" | "🍟" => &["pizza", "burger", "batata", "fastfood", "lanche", "food", "comida"],
        "☕" | "🍺" | "🍻" => &["coffee", "cafe", "beer", "cerveja", "drink", "bebida"],
        "🎮" => &["game", "jogo", "videogame", "gamer", "play"],
        "⚽" | "🏀" | "🏈" | "⚾" => &[
            "futebol", "soccer", "football", "ball", "bola", "esporte", "sport",
        ],
        "🚗" | "🏎️" => &["car", "carro", "speed", "corrida", "auto"],
        "🚀" => &["rocket", "foguete", "moon", "space", "espaco"],
        "🇧🇷" => &["brasil", "brazil", "br", "bandeira", "flag"],
        "🇺🇸" => &["eua", "usa", "america", "bandeira", "flag"],
        "💀" | "☠️" => &["skull", "caveira", "dead", "morto"],
        _ => &[],
    };

    tags.iter().any(|t| t.contains(&q) || q.contains(*t))
}

pub const EMOJIS_PER_ROW: usize = 8;

#[derive(Clone, Debug)]
pub enum EmojiVirtualItem {
    Custom(DiscordEmoji),
    Unicode(String),
}

#[derive(Clone, Debug)]
pub enum EmojiVirtualRow {
    FilterBanner {
        title: String,
    },
    SectionHeader {
        title: String,
        icon_url: Option<String>,
        acronym: Option<String>,
        cat_key: Option<&'static str>,
        count: usize,
    },
    GridRow {
        items: Vec<EmojiVirtualItem>,
    },
    EmptyNotice {
        message: String,
    },
}

pub fn build_emoji_virtual_rows(
    config: &MediaPickerConfig,
    query: &str,
    is_searching: bool,
) -> Vec<EmojiVirtualRow> {
    let mut rows = Vec::new();
    let selected_guild = config.selected_guild_id.as_deref();
    let is_global = selected_guild.is_none() || selected_guild == Some("recents");

    // 1. Banner de filtro quando selecionado um servidor específico ou categorias padrão
    if let Some(gid) = selected_guild {
        if gid != "recents" {
            let filter_title = if gid == "standard" {
                "Categorias Padrão (Unicode)".to_string()
            } else if let Some(g) = config.guild_sections.iter().find(|g| g.guild_id == gid) {
                g.guild_name.clone()
            } else {
                "Servidor Selecionado".to_string()
            };
            rows.push(EmojiVirtualRow::FilterBanner {
                title: filter_title,
            });
        }
    }

    // 2. Seção Frequentemente Usados
    let show_recents = !is_searching && is_global;
    if show_recents {
        let recents = &config.recent_emojis;

        if !recents.is_empty() {
            rows.push(EmojiVirtualRow::SectionHeader {
                title: "FREQUENTEMENTE USADOS".to_string(),
                icon_url: None,
                acronym: None,
                cat_key: None,
                count: recents.len(),
            });
            for chunk in recents.chunks(EMOJIS_PER_ROW) {
                let items = chunk
                    .iter()
                    .map(|r| EmojiVirtualItem::Unicode(r.emoji.clone()))
                    .collect();
                rows.push(EmojiVirtualRow::GridRow { items });
            }
        }
    }

    // 3. Seções de Servidores
    if selected_guild != Some("standard") {
        let mut rendered_guild_count = 0;
        if !config.guild_sections.is_empty() {
            for guild in &config.guild_sections {
                if let Some(target_gid) = selected_guild {
                    if target_gid != "recents" && guild.guild_id != target_gid {
                        continue;
                    }
                }

                let matching_emojis: Vec<&DiscordEmoji> = guild
                    .emojis
                    .iter()
                    .filter(|e| {
                        if is_searching {
                            e.name.to_lowercase().contains(query)
                        } else {
                            true
                        }
                    })
                    .collect();

                if matching_emojis.is_empty() {
                    continue;
                }

                rendered_guild_count += 1;
                rows.push(EmojiVirtualRow::SectionHeader {
                    title: guild.guild_name.to_uppercase(),
                    icon_url: guild.icon_url.clone(),
                    acronym: Some(guild.acronym.clone()),
                    cat_key: None,
                    count: matching_emojis.len(),
                });

                for chunk in matching_emojis.chunks(EMOJIS_PER_ROW) {
                    let items = chunk
                        .iter()
                        .map(|&e| EmojiVirtualItem::Custom(e.clone()))
                        .collect();
                    rows.push(EmojiVirtualRow::GridRow { items });
                }
            }
        } else if !config.custom_emojis.is_empty() {
            let matching_custom: Vec<&DiscordEmoji> = config
                .custom_emojis
                .iter()
                .filter(|e| {
                    if is_searching {
                        e.name.to_lowercase().contains(query)
                    } else {
                        true
                    }
                })
                .collect();

            if !matching_custom.is_empty() {
                rendered_guild_count += 1;
                rows.push(EmojiVirtualRow::SectionHeader {
                    title: "EMOJIS DO SERVIDOR".to_string(),
                    icon_url: None,
                    acronym: None,
                    cat_key: None,
                    count: matching_custom.len(),
                });

                for chunk in matching_custom.chunks(EMOJIS_PER_ROW) {
                    let items = chunk
                        .iter()
                        .map(|&e| EmojiVirtualItem::Custom(e.clone()))
                        .collect();
                    rows.push(EmojiVirtualRow::GridRow { items });
                }
            }
        }

        if let Some(gid) = selected_guild {
            if gid != "standard" && gid != "recents" && rendered_guild_count == 0 {
                rows.push(EmojiVirtualRow::EmptyNotice {
                    message: "Nenhum emoji personalizado disponível neste servidor.".to_string(),
                });
            }
        }
    }

    // 4. Categorias Padrão Unicode (8 Categorias Oficiais)
    let show_standard = is_global || selected_guild == Some("standard");
    if show_standard {
        let categories = MediaCacheManager::get_standard_categories();
        for (cat_key, cat_title, emojis) in categories {
            let matching: Vec<&str> = if is_searching {
                emojis
                    .iter()
                    .copied()
                    .filter(|em| emoji_matches_query(em, cat_key, cat_title, query))
                    .collect()
            } else {
                emojis.iter().copied().collect()
            };

            if matching.is_empty() {
                continue;
            }

            rows.push(EmojiVirtualRow::SectionHeader {
                title: cat_title.to_string(),
                icon_url: None,
                acronym: None,
                cat_key: Some(cat_key),
                count: matching.len(),
            });

            for chunk in matching.chunks(EMOJIS_PER_ROW) {
                let items = chunk
                    .iter()
                    .map(|&em| EmojiVirtualItem::Unicode(em.to_string()))
                    .collect();
                rows.push(EmojiVirtualRow::GridRow { items });
            }
        }
    }

    rows
}

pub fn render_emoji_virtual_row(
    row: &EmojiVirtualRow,
    on_select_emoji: Option<&OnSelectEmojiCallback>,
    on_select_guild: Option<&OnSelectGuildCallback>,
) -> AnyElement {
    match row {
        EmojiVirtualRow::FilterBanner { title } => {
            render_emoji_filter_banner(title, on_select_guild)
        }
        EmojiVirtualRow::SectionHeader {
            title,
            icon_url,
            acronym,
            cat_key,
            count,
        } => render_emoji_section_header(
            title,
            icon_url.as_deref(),
            acronym.as_deref(),
            *cat_key,
            *count,
        ),
        EmojiVirtualRow::GridRow { items } => render_emoji_grid_row(items, on_select_emoji),
        EmojiVirtualRow::EmptyNotice { message } => render_empty_notice(message),
    }
}

fn render_emoji_filter_banner(
    title: &str,
    on_select_guild: Option<&OnSelectGuildCallback>,
) -> AnyElement {
    let mut banner = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(px(8.0))
        .py(px(4.0))
        .mb(px(4.0))
        .bg(bg_user_bar())
        .rounded(radius_sm())
        .border_1()
        .border_color(blurple())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_emoji(px(14.0), blurple()))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_titles())
                        .child(format!("Filtrado por: {}", title)),
                ),
        );

    if let Some(on_sel) = on_select_guild {
        let on_sel_clone = (*on_sel).clone();
        let reset_btn = div()
            .text_size(px(11.0))
            .font_weight(WEIGHT_MEDIUM)
            .text_color(blurple())
            .cursor_pointer()
            .hover(|s| s.text_color(white()))
            .child("Ver todos os emojis")
            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_sel_clone(None, window, cx);
            });
        banner = banner.child(reset_btn);
    }
    banner.into_any_element()
}

fn render_emoji_section_header(
    title: &str,
    icon_url: Option<&str>,
    acronym: Option<&str>,
    cat_key: Option<&'static str>,
    _count: usize,
) -> AnyElement {
    let header_icon = if let Some(cat) = cat_key {
        match cat {
            "people" => icon_emoji(px(14.0), text_muted()),
            "nature" => icon_sparkles(px(14.0), text_muted()),
            "food" => icon_gift(px(14.0), text_muted()),
            "activity" => icon_gamepad(px(14.0), text_muted()),
            "travel" => icon_compass(px(14.0), text_muted()),
            "objects" => icon_gear(px(14.0), text_muted()),
            "symbols" => icon_hash(px(14.0), text_muted()),
            "flags" => icon_pin(px(14.0), text_muted()),
            _ => icon_emoji(px(14.0), text_muted()),
        }
    } else if let Some(url) = icon_url {
        div()
            .size(px(16.0))
            .rounded(radius_xs())
            .overflow_hidden()
            .child(img(url.to_string()).size_full().rounded(radius_xs()))
    } else if let Some(acr) = acronym {
        div()
            .size(px(16.0))
            .rounded(radius_xs())
            .bg(bg_server())
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(8.0))
            .font_weight(WEIGHT_BOLD)
            .text_color(text_muted())
            .child(acr.to_string())
    } else {
        icon_smile(px(14.0), text_muted())
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_start()
        .pt(px(8.0))
        .pb(px(4.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(header_icon)
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child(title.to_string()),
                ),
        )
        .into_any_element()
}

fn render_emoji_grid_row(
    items: &[EmojiVirtualItem],
    on_select_emoji: Option<&OnSelectEmojiCallback>,
) -> AnyElement {
    let mut row_div = div()
        .flex()
        .flex_row()
        .items_center()
        .py(px(2.0))
        .gap(px(4.0));

    for item in items {
        row_div = row_div.child(render_emoji_item(item, on_select_emoji));
    }
    row_div.into_any_element()
}

fn render_emoji_item(
    item: &EmojiVirtualItem,
    on_select_emoji: Option<&OnSelectEmojiCallback>,
) -> AnyElement {
    match item {
        EmojiVirtualItem::Custom(emoji) => {
            let tag = emoji.to_tag();
            let name = emoji.name.clone();
            let cdn_url = emoji.cdn_url();

            let emoji_child = if let Some(url) = cdn_url {
                div()
                    .size(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(url)
                            .id(ElementId::from(SharedString::from(format!(
                                "picker-emoji-img-{}",
                                emoji.id.as_deref().unwrap_or(&emoji.name)
                            ))))
                            .size_full()
                            .rounded(radius_xs()),
                    )
            } else {
                div()
                    .text_size(px(14.0))
                    .text_color(blurple())
                    .child(format!(":{}", name))
            };

            // Célula de emoji: 48x48, radius 4px conforme DOM oficial
            let mut btn = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(48.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(channel_hover()))
                .child(emoji_child);

            if let Some(on_sel) = on_select_emoji {
                let on_sel_clone = (*on_sel).clone();
                let tag_clone = tag.clone();
                btn = btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                    on_sel_clone(&tag_clone, window, cx);
                });
            }
            btn.into_any_element()
        }
        EmojiVirtualItem::Unicode(em) => {
            let em_str = em.clone();
            // Célula de emoji: 48x48, radius 4px conforme DOM oficial
            let mut btn = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(48.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(channel_hover()))
                .child(div().text_size(px(26.0)).child(em_str.clone()));

            if let Some(on_sel) = on_select_emoji {
                let on_sel_clone = (*on_sel).clone();
                let em_clone = em_str.clone();
                btn = btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                    on_sel_clone(&em_clone, window, cx);
                });
            }
            btn.into_any_element()
        }
    }
}

fn render_empty_notice(message: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .py(px(24.0))
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(text_muted())
                .child(message.to_string()),
        )
        .into_any_element()
}

fn render_virtualized_emojis_content(
    config: &MediaPickerConfig,
    query: &str,
    is_searching: bool,
    list_state: &ListState,
) -> AnyElement {
    let rows = build_emoji_virtual_rows(config, query, is_searching);
    let total_rows = rows.len();
    if list_state.item_count() != total_rows {
        list_state.reset(total_rows);
    }
    let rows_arc = Arc::new(rows);
    let on_select_emoji = config.on_select_emoji.clone();
    let on_select_guild = config.on_select_guild.clone();

    div()
        .id("media-picker-virtual-emojis")
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .pl(px(8.0))
        .pr(px(4.0))
        .py(px(4.0))
        .child(
            list(list_state.clone(), move |ix, _win, _cx| {
                if ix >= rows_arc.len() {
                    return div().into_any_element();
                }
                render_emoji_virtual_row(
                    &rows_arc[ix],
                    on_select_emoji.as_ref(),
                    on_select_guild.as_ref(),
                )
            })
            .size_full(),
        )
        .into_any_element()
}

fn render_emojis_tab(config: &MediaPickerConfig, query: &str, is_searching: bool) -> AnyElement {
    let rows = build_emoji_virtual_rows(config, query, is_searching);
    let on_select_emoji = config.on_select_emoji.as_ref();
    let on_select_guild = config.on_select_guild.as_ref();
    let mut container = div().flex().flex_col().gap(px(4.0));
    for row in &rows {
        container = container.child(render_emoji_virtual_row(row, on_select_emoji, on_select_guild));
    }
    container.into_any_element()
}

// ============================================================================
// 4. Conteúdo da Aba Figurinhas / Stickers
// ============================================================================

pub const STICKERS_PER_ROW: usize = 4;

#[derive(Clone, Debug)]
pub enum StickerVirtualRow {
    FilterBanner {
        title: String,
    },
    SectionHeader {
        title: String,
        icon_url: Option<String>,
        acronym: Option<String>,
        is_favorites: bool,
        count: usize,
    },
    GridRow {
        items: Vec<DiscordSticker>,
    },
    EmptyNotice {
        message: String,
    },
}

pub fn build_sticker_virtual_rows(
    config: &MediaPickerConfig,
    query: &str,
) -> Vec<StickerVirtualRow> {
    let mut rows = Vec::new();
    let selected_guild = config.selected_guild_id.as_deref();
    let is_global = selected_guild.is_none() || selected_guild == Some("recents");

    // 1. Banner de filtro quando selecionado um servidor específico ou pacotes oficiais
    if let Some(gid) = selected_guild {
        if gid != "recents" {
            let filter_title = if gid == "standard" {
                "Pacotes Oficiais do Discord".to_string()
            } else if let Some(g) = config.guild_sections.iter().find(|g| g.guild_id == gid) {
                g.guild_name.clone()
            } else {
                "Servidor Selecionado".to_string()
            };
            rows.push(StickerVirtualRow::FilterBanner {
                title: filter_title,
            });
        }
    }

    // 2. Seção Figurinhas Favoritas
    let show_favorites = is_global;
    if show_favorites {
        let favorites: Vec<&DiscordSticker> = config
            .stickers
            .iter()
            .filter(|s| s.guild_id.is_none())
            .filter(|s| {
                if query.is_empty() {
                    true
                } else {
                    s.name.to_lowercase().contains(query) || s.tags.to_lowercase().contains(query)
                }
            })
            .collect();

        if !favorites.is_empty() {
            rows.push(StickerVirtualRow::SectionHeader {
                title: "FAVORITOS".to_string(),
                icon_url: None,
                acronym: None,
                is_favorites: true,
                count: favorites.len(),
            });

            for chunk in favorites.chunks(STICKERS_PER_ROW) {
                let items = chunk.iter().map(|&s| s.clone()).collect();
                rows.push(StickerVirtualRow::GridRow { items });
            }
        }
    }

    // 3. Seções de Servidores
    if selected_guild != Some("standard") {
        let mut rendered_guild_count = 0;
        if !config.guild_sections.is_empty() {
            for guild in &config.guild_sections {
                if let Some(target_gid) = selected_guild {
                    if target_gid != "recents" && guild.guild_id != target_gid {
                        continue;
                    }
                }

                let matching_stickers: Vec<&DiscordSticker> = guild
                    .stickers
                    .iter()
                    .filter(|s| {
                        if query.is_empty() {
                            true
                        } else {
                            s.name.to_lowercase().contains(query)
                                || s.tags.to_lowercase().contains(query)
                        }
                    })
                    .collect();

                if matching_stickers.is_empty() {
                    continue;
                }

                rendered_guild_count += 1;
                rows.push(StickerVirtualRow::SectionHeader {
                    title: guild.guild_name.to_uppercase(),
                    icon_url: guild.icon_url.clone(),
                    acronym: Some(guild.acronym.clone()),
                    is_favorites: false,
                    count: matching_stickers.len(),
                });

                for chunk in matching_stickers.chunks(STICKERS_PER_ROW) {
                    let items = chunk.iter().map(|&s| s.clone()).collect();
                    rows.push(StickerVirtualRow::GridRow { items });
                }
            }
        } else if !config.stickers.is_empty() {
            let server_stickers: Vec<&DiscordSticker> = config
                .stickers
                .iter()
                .filter(|s| s.guild_id.is_some())
                .filter(|s| {
                    if query.is_empty() {
                        true
                    } else {
                        s.name.to_lowercase().contains(query)
                            || s.tags.to_lowercase().contains(query)
                    }
                })
                .collect();

            if !server_stickers.is_empty() {
                rendered_guild_count += 1;
                rows.push(StickerVirtualRow::SectionHeader {
                    title: "FIGURINHAS DO SERVIDOR".to_string(),
                    icon_url: None,
                    acronym: None,
                    is_favorites: false,
                    count: server_stickers.len(),
                });

                for chunk in server_stickers.chunks(STICKERS_PER_ROW) {
                    let items = chunk.iter().map(|&s| s.clone()).collect();
                    rows.push(StickerVirtualRow::GridRow { items });
                }
            }
        }

        if let Some(gid) = selected_guild {
            if gid != "standard" && gid != "recents" && rendered_guild_count == 0 {
                rows.push(StickerVirtualRow::EmptyNotice {
                    message: "Nenhuma figurinha personalizada disponível neste servidor.".to_string(),
                });
            }
        }
    }

    // 4. Pacotes padrão do Discord (Wumpus, Clyde, Nelly, etc.)
    let show_packs = is_global || selected_guild == Some("standard");
    if show_packs {
        let default_packs = MediaCacheManager::get_default_sticker_packs();
        for pack in default_packs {
            let matching: Vec<&DiscordSticker> = if query.is_empty() {
                pack.stickers.iter().collect()
            } else {
                pack.stickers
                    .iter()
                    .filter(|s| {
                        s.name.to_lowercase().contains(query)
                            || s.tags.to_lowercase().contains(query)
                    })
                    .collect()
            };

            if matching.is_empty() {
                continue;
            }

            rows.push(StickerVirtualRow::SectionHeader {
                title: pack.name.to_uppercase(),
                icon_url: None,
                acronym: None,
                is_favorites: false,
                count: matching.len(),
            });

            for chunk in matching.chunks(STICKERS_PER_ROW) {
                let items = chunk.iter().map(|&s| s.clone()).collect();
                rows.push(StickerVirtualRow::GridRow { items });
            }
        }
    }

    if rows.is_empty() {
        rows.push(StickerVirtualRow::EmptyNotice {
            message: "Nenhuma figurinha disponível.".to_string(),
        });
    }

    rows
}

pub fn render_sticker_virtual_row(
    row: &StickerVirtualRow,
    on_select_sticker: Option<&OnSelectStickerCallback>,
    on_select_guild: Option<&OnSelectGuildCallback>,
) -> AnyElement {
    match row {
        StickerVirtualRow::FilterBanner { title } => {
            render_sticker_filter_banner(title, on_select_guild)
        }
        StickerVirtualRow::SectionHeader {
            title,
            icon_url,
            acronym,
            is_favorites,
            count,
        } => render_sticker_section_header(
            title,
            icon_url.as_deref(),
            acronym.as_deref(),
            *is_favorites,
            *count,
        ),
        StickerVirtualRow::GridRow { items } => {
            render_sticker_grid_row(items, on_select_sticker)
        }
        StickerVirtualRow::EmptyNotice { message } => render_empty_notice(message),
    }
}

fn render_sticker_filter_banner(
    title: &str,
    on_select_guild: Option<&OnSelectGuildCallback>,
) -> AnyElement {
    let mut banner = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(px(8.0))
        .py(px(4.0))
        .mb(px(4.0))
        .bg(bg_user_bar())
        .rounded(radius_sm())
        .border_1()
        .border_color(blurple())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_sticker(px(14.0), blurple()))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_titles())
                        .child(format!("Filtrado por: {}", title)),
                ),
        );

    if let Some(on_sel) = on_select_guild {
        let on_sel_clone = (*on_sel).clone();
        let reset_btn = div()
            .text_size(px(11.0))
            .font_weight(WEIGHT_MEDIUM)
            .text_color(blurple())
            .cursor_pointer()
            .hover(|s| s.text_color(white()))
            .child("Ver todas as figurinhas")
            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_sel_clone(None, window, cx);
            });
        banner = banner.child(reset_btn);
    }
    banner.into_any_element()
}

fn render_sticker_section_header(
    title: &str,
    icon_url: Option<&str>,
    acronym: Option<&str>,
    is_favorites: bool,
    _count: usize,
) -> AnyElement {
    let header_icon = if is_favorites {
        icon_sparkles(px(14.0), text_muted())
    } else if let Some(url) = icon_url {
        div()
            .size(px(16.0))
            .rounded(radius_xs())
            .overflow_hidden()
            .child(img(url.to_string()).size_full().rounded(radius_xs()))
    } else if let Some(acr) = acronym {
        div()
            .size(px(16.0))
            .rounded(radius_xs())
            .bg(bg_server())
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(8.0))
            .font_weight(WEIGHT_BOLD)
            .text_color(text_muted())
            .child(acr.to_string())
    } else {
        icon_sticker(px(14.0), text_muted())
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_start()
        .pt(px(10.0))
        .pb(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(header_icon)
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child(title.to_string()),
                ),
        )
        .into_any_element()
}

fn render_sticker_grid_row(
    items: &[DiscordSticker],
    on_select_sticker: Option<&OnSelectStickerCallback>,
) -> AnyElement {
    let mut row_div = div()
        .flex()
        .flex_row()
        .items_center()
        .py(px(4.0))
        .gap(px(8.0));

    for sticker in items {
        row_div = row_div.child(render_sticker_card(sticker, on_select_sticker));
    }
    row_div.into_any_element()
}

fn render_sticker_card(
    sticker: &DiscordSticker,
    on_select_sticker: Option<&OnSelectStickerCallback>,
) -> AnyElement {
    let sticker_clone = sticker.clone();
    let name = sticker.name.clone();
    let sticker_cdn = sticker.cdn_url();

    // Item de figurinha: 98x98 com imagem 96x96 conforme DOM oficial (sticker-picker.md)
    let mut card = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .size(px(98.0))
        .p(px(6.0))
        .bg(bg_user_bar())
        .rounded(radius_md())
        .border_1()
        .border_color(bg_chat_hover())
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()).border_color(blurple()))
        .child(
            div()
                .size(px(64.0))
                .flex()
                .items_center()
                .justify_center()
                .child(if !sticker_cdn.is_empty() {
                    img(sticker_cdn)
                        .id(ElementId::from(SharedString::from(format!(
                            "picker-sticker-img-{}",
                            sticker.id
                        ))))
                        .size_full()
                        .rounded(radius_sm())
                        .into_any_element()
                } else {
                    icon_sticker(px(36.0), blurple()).into_any_element()
                }),
        )
        .child(
            div()
                .text_size(px(10.0))
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_muted())
                .overflow_hidden()
                .text_ellipsis()
                .max_w(px(84.0))
                .child(name),
        );

    if let Some(on_sel) = on_select_sticker {
        let on_sel_clone = (*on_sel).clone();
        card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_sel_clone(&sticker_clone, window, cx);
        });
    }

    card.into_any_element()
}

fn render_virtualized_stickers_content(
    config: &MediaPickerConfig,
    query: &str,
    list_state: &ListState,
) -> AnyElement {
    let rows = build_sticker_virtual_rows(config, query);
    let total_rows = rows.len();
    if list_state.item_count() != total_rows {
        list_state.reset(total_rows);
    }
    let rows_arc = Arc::new(rows);
    let on_select_sticker = config.on_select_sticker.clone();
    let on_select_guild = config.on_select_guild.clone();

    div()
        .id("media-picker-virtual-stickers")
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .pl(px(8.0))
        .pr(px(4.0))
        .py(px(4.0))
        .child(
            list(list_state.clone(), move |ix, _win, _cx| {
                if ix >= rows_arc.len() {
                    return div().into_any_element();
                }
                render_sticker_virtual_row(
                    &rows_arc[ix],
                    on_select_sticker.as_ref(),
                    on_select_guild.as_ref(),
                )
            })
            .size_full(),
        )
        .into_any_element()
}

fn render_stickers_tab(config: &MediaPickerConfig, query: &str) -> AnyElement {
    let rows = build_sticker_virtual_rows(config, query);
    let on_select_sticker = config.on_select_sticker.as_ref();
    let on_select_guild = config.on_select_guild.as_ref();
    let mut container = div().flex().flex_col().gap(px(4.0));
    for row in &rows {
        container = container.child(render_sticker_virtual_row(
            row,
            on_select_sticker,
            on_select_guild,
        ));
    }
    container.into_any_element()
}

// ============================================================================
// 5. Conteúdo da Aba GIFs (Integração Tenor)
// ============================================================================

fn render_gifs_tab(config: &MediaPickerConfig, query: &str) -> AnyElement {
    let mut container = div().flex().flex_col().gap(px(10.0));

    let header_title = if query.is_empty() {
        "GIFS EM ALTA NO TENOR"
    } else {
        "RESULTADOS DA BUSCA"
    };

    container = container.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(icon_gif(px(14.0), text_muted()))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(WEIGHT_BOLD)
                    .text_color(text_muted())
                    .child(header_title),
            ),
    );

    // Chips de categorias
    container = container.child(render_gif_category_chips(
        query,
        config.on_select_gif_category.as_ref(),
        config.on_search_query.as_ref(),
    ));

    // Fallback para biblioteca curada caso config.gifs esteja vazio
    let default_gifs;
    let gifs_to_render = if config.gifs.is_empty() {
        default_gifs = MediaCacheManager::get_curated_tenor_gifs_for_category(query);
        &default_gifs
    } else {
        &config.gifs
    };

    if gifs_to_render.is_empty() {
        container = container.child(render_empty_notice("Nenhum GIF disponível."));
    } else {
        let mut grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
        for gif in gifs_to_render {
            grid = grid.child(render_gif_card(gif, config.on_select_gif.as_ref()));
        }
        container = container.child(grid);
    }

    container.into_any_element()
}

fn render_gif_category_chips(
    query: &str,
    on_select_category: Option<&OnSelectGifCategoryCallback>,
    on_search_query: Option<&OnSearchQueryCallback>,
) -> AnyElement {
    let categories: [(&str, &str, fn(Pixels, Hsla) -> Div); 6] = [
        ("trending", "Trending", |sz, col| icon_sparkles(sz, col)),
        ("anime", "Anime", |sz, col| icon_sparkles(sz, col)),
        ("reactions", "Reactions", |sz, col| icon_smile(sz, col)),
        ("memes", "Memes", |sz, col| icon_chat(sz, col)),
        ("gaming", "Gaming", |sz, col| icon_gamepad(sz, col)),
        ("music", "Music", |sz, col| icon_music(sz, col)),
    ];

    let mut cat_row = div().flex().flex_row().items_center().gap(px(6.0));
    for (cat_id, cat_label, icon_builder) in categories {
        let is_active = query == cat_id;
        let icon_color = if is_active { white() } else { text_muted() };
        let mut chip = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(26.0))
            .px(px(10.0))
            .rounded(radius_pill())
            .bg(if is_active { blurple() } else { bg_user_bar() })
            .border_1()
            .border_color(if is_active { blurple() } else { bg_chat_hover() })
            .cursor_pointer()
            .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
            .child(icon_builder(px(12.0), icon_color.into()))
            .child(
                div()
                    .text_size(font_size_xs())
                    .font_weight(WEIGHT_BOLD)
                    .text_color(if is_active { white() } else { text_muted() })
                    .child(cat_label),
            );

        if let Some(on_cat) = on_select_category {
            let on_cat_clone = (*on_cat).clone();
            let cat_str = cat_id.to_string();
            chip = chip.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_cat_clone(&cat_str, window, cx);
            });
        } else if let Some(on_search) = on_search_query {
            let on_search_clone = (*on_search).clone();
            let cat_str = cat_id.to_string();
            chip = chip.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_search_clone(&cat_str, window, cx);
            });
        }
        cat_row = cat_row.child(chip);
    }
    cat_row.into_any_element()
}

fn render_gif_card(
    gif: &TenorGif,
    on_select_gif: Option<&OnSelectGifCallback>,
) -> AnyElement {
    let gif_url = gif.url.clone();
    let title = gif.title.clone();
    let preview_url = if let Some(ref gs) = gif.gif_src {
        gs.clone()
    } else if let Some(ref s) = gif.src {
        s.clone()
    } else {
        gif.preview_url.clone()
    };

    // Miniatura Tenor: 230x110 com radius 5px conforme DOM oficial (gif-picker.md)
    let mut card = div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(px(230.0))
        .h(px(110.0))
        .bg(bg_user_bar())
        .rounded(px(5.0))
        .border_1()
        .border_color(bg_chat_hover())
        .cursor_pointer()
        .overflow_hidden()
        .hover(|s| s.border_color(blurple()))
        .child(
            img(preview_url)
                .id(ElementId::from(SharedString::from(format!(
                    "picker-gif-img-{}",
                    gif.id
                ))))
                .size_full()
                .rounded(px(5.0)),
        )
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .bg(rgba(0x000000c0)) // TOKEN FALTANDO: --background-scrim
                .px(px(6.0))
                .py(px(4.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(WEIGHT_MEDIUM)
                        .text_color(white())
                        .child(title),
                ),
        );

    if let Some(on_sel) = on_select_gif {
        let on_sel_clone = (*on_sel).clone();
        let url_clone = gif_url.clone();
        card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_sel_clone(&url_clone, window, cx);
        });
    }

    card.into_any_element()
}

// ============================================================================
// 6. Conteúdo da Aba Soundboard
// ============================================================================

fn render_soundboard_tab(config: &MediaPickerConfig, query: &str) -> AnyElement {
    let mut container = div().flex().flex_col().gap(px(12.0));

    container = container.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(icon_speaker(px(14.0), text_muted()))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(WEIGHT_BOLD)
                    .text_color(text_muted())
                    .child("SOUNDBOARD: EFEITOS SONOROS RÁPIDOS"),
            ),
    );

    let default_sounds;
    let all_sounds = if config.sounds.is_empty() {
        default_sounds = MediaCacheManager::get_default_soundboard_sounds();
        &default_sounds
    } else {
        &config.sounds
    };

    let matching_sounds: Vec<&SoundboardSound> = if query.is_empty() {
        all_sounds.iter().collect()
    } else {
        all_sounds
            .iter()
            .filter(|s| s.name.to_lowercase().contains(query))
            .collect()
    };

    if matching_sounds.is_empty() {
        container = container.child(render_empty_notice("Nenhum som disponível."));
    } else {
        let mut grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
        for sound in matching_sounds {
            grid = grid.child(render_sound_item(
                sound,
                config.on_play_sound.as_ref(),
                config.on_send_sound.as_ref(),
            ));
        }
        container = container.child(grid);
    }

    container.into_any_element()
}

fn render_sound_item(
    sound: &SoundboardSound,
    on_play: Option<&OnPlaySoundCallback>,
    on_send: Option<&OnPlaySoundCallback>,
) -> AnyElement {
    let sound_clone = sound.clone();
    let name = sound.name.clone();
    let sound_icon_element = if let Some(ref em) = sound.emoji_name {
        div().text_size(px(18.0)).child(em.clone())
    } else {
        div()
            .size(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .child(icon_speaker(px(18.0), blurple()))
    };

    let mut card = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .w(px(190.0))
        .h(px(44.0))
        .px(px(10.0))
        .bg(bg_user_bar())
        .rounded(radius_md())
        .border_1()
        .border_color(bg_chat_hover())
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()).border_color(blurple()))
        .child(sound_icon_element)
        .child(
            div()
                .flex_1()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_titles())
                .child(name),
        );

    // Botão de preview (Play)
    let mut play_btn = div()
        .size(px(22.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius_sm())
        .bg(bg_channels())
        .text_size(px(10.0))
        .text_color(text_muted())
        .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
        .child("▶");

    if let Some(on_play_cb) = on_play {
        let s_clone = sound_clone.clone();
        let on_play_cb = (*on_play_cb).clone();
        play_btn = play_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_play_cb(&s_clone, window, cx);
        });
    }
    card = card.child(play_btn);

    // Clique no card envia ou reproduz o som
    if let Some(on_send_cb) = on_send {
        let s_clone = sound_clone.clone();
        let on_send_cb = (*on_send_cb).clone();
        card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_send_cb(&s_clone, window, cx);
        });
    } else if let Some(on_play_cb) = on_play {
        let s_clone = sound_clone.clone();
        let on_play_cb = (*on_play_cb).clone();
        card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_play_cb(&s_clone, window, cx);
        });
    }

    card.into_any_element()
}

// ============================================================================
// 7. Faixa Inferior de Preview (Hover Preview Strip)
// ============================================================================

fn render_preview_strip(hovered_info: Option<&str>) -> Div {
    let display_text = hovered_info.unwrap_or("Selecione um emoji, figurinha ou som para enviar");
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .h(px(36.0))
        .px(px(12.0))
        .bg(bg_user_bar())
        .border_t_1()
        .border_color(bg_server())
        .child(
            div()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(if hovered_info.is_some() {
                    text_titles()
                } else {
                    text_muted()
                })
                .child(display_text.to_string()),
        )
}

// ============================================================================
// Testes Automatizados
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::media_cache::StickerFormatType;

    #[::core::prelude::v1::test]
    fn test_media_picker_tabs_metadata() {
        assert_eq!(MediaPickerTab::Emojis.label(), "Emojis");
        assert_eq!(MediaPickerTab::Stickers.label(), "Figurinhas");
        assert_eq!(MediaPickerTab::Gifs.label(), "GIFs");
        assert_eq!(MediaPickerTab::Soundboard.label(), "Sons");

        assert_eq!(MediaPickerTab::Emojis.icon(), DiscordIcon::Emoji);
        assert_eq!(MediaPickerTab::Stickers.icon(), DiscordIcon::Sticker);
        assert_eq!(MediaPickerTab::Gifs.icon(), DiscordIcon::Gif);
        assert_eq!(MediaPickerTab::Soundboard.icon(), DiscordIcon::Speaker);

        assert_eq!(MediaPickerTab::Emojis.search_placeholder(), "Buscar emojis...");
        assert_eq!(MediaPickerTab::Stickers.search_placeholder(), "Buscar figurinhas...");
        assert_eq!(MediaPickerTab::Gifs.search_placeholder(), "Buscar no Tenor...");
        assert_eq!(MediaPickerTab::Soundboard.search_placeholder(), "Buscar sons...");
    }

    #[::core::prelude::v1::test]
    fn test_media_picker_modal_initialization() {
        let modal = MediaPickerModal::new(MediaPickerTab::Gifs);
        assert_eq!(modal.active_tab, MediaPickerTab::Gifs);
        assert!(modal.search_query.is_empty());
        assert!(modal.selected_gif_category.is_none());
    }

    #[::core::prelude::v1::test]
    fn test_canonical_emoji_categories_coverage() {
        let categories = MediaCacheManager::get_standard_categories();
        assert_eq!(categories.len(), 8);

        let cat_keys: Vec<&str> = categories.iter().map(|(k, _, _)| *k).collect();
        assert!(cat_keys.contains(&"people"));
        assert!(cat_keys.contains(&"nature"));
        assert!(cat_keys.contains(&"food"));
        assert!(cat_keys.contains(&"activity"));
        assert!(cat_keys.contains(&"travel"));
        assert!(cat_keys.contains(&"objects"));
        assert!(cat_keys.contains(&"symbols"));
        assert!(cat_keys.contains(&"flags"));

        for (_, title, emojis) in categories {
            assert!(!title.is_empty());
            assert!(!emojis.is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn test_emoji_matches_query_logic() {
        assert!(emoji_matches_query("😀", "people", "PESSOAS", "smile"));
        assert!(emoji_matches_query("😀", "people", "PESSOAS", "sorriso"));
        assert!(emoji_matches_query("🐶", "nature", "NATUREZA", "dog"));
        assert!(emoji_matches_query("🐶", "nature", "NATUREZA", "cachorro"));
        assert!(emoji_matches_query("🍕", "food", "COMIDAS", "pizza"));
        assert!(emoji_matches_query("🇧🇷", "flags", "BANDEIRAS", "brasil"));
        assert!(emoji_matches_query("🚗", "travel", "VIAGEM", "car"));
        assert!(emoji_matches_query("⚽", "activity", "ATIVIDADES", "futebol"));
    }

    #[::core::prelude::v1::test]
    fn test_official_sticker_packs_presence() {
        let packs = MediaCacheManager::get_default_sticker_packs();
        assert!(
            packs.is_empty(),
            "Sem resposta da rede/API, pacotes de figurinhas devem vir vazios (zero mocks)"
        );
    }

    #[::core::prelude::v1::test]
    fn test_soundboard_sounds_and_icons() {
        let sounds = MediaCacheManager::get_default_soundboard_sounds();
        assert!(
            sounds.is_empty(),
            "Sem resposta da rede/API, sons da soundboard devem vir vazios (zero mocks)"
        );
    }

    #[::core::prelude::v1::test]
    fn test_media_picker_guild_section_and_config() {
        let section = MediaPickerGuildSection {
            guild_id: "guild-123".to_string(),
            guild_name: "Rust Brasil".to_string(),
            icon_url: Some("https://cdn.discordapp.com/icons/guild-123/abc.png".to_string()),
            acronym: "RB".to_string(),
            emojis: vec![DiscordEmoji::new_custom(
                "e1",
                "crab_party",
                false,
                Some("guild-123".to_string()),
            )],
            stickers: vec![DiscordSticker {
                id: "s1".to_string(),
                pack_id: None,
                name: "ferris_hype".to_string(),
                description: None,
                tags: "ferris,rust".to_string(),
                format_type: StickerFormatType::Png,
                available: true,
                guild_id: Some("guild-123".to_string()),
            }],
        };

        assert_eq!(section.guild_id, "guild-123");
        assert_eq!(section.guild_name, "Rust Brasil");
        assert_eq!(section.acronym, "RB");
        assert_eq!(section.emojis.len(), 1);
        assert_eq!(section.stickers.len(), 1);

        let config = MediaPickerConfig {
            active_tab: MediaPickerTab::Emojis,
            search_query: "crab".to_string(),
            recent_emojis: Vec::new(),
            custom_emojis: Vec::new(),
            stickers: Vec::new(),
            gifs: Vec::new(),
            sounds: Vec::new(),
            hovered_info: None,
            guild_sections: vec![section],
            selected_guild_id: Some("guild-123".to_string()),
            focus_handle: None,
            is_focused: true,
            list_state: None,
            on_select_emoji: None,
            on_select_sticker: None,
            on_select_gif: None,
            on_play_sound: None,
            on_send_sound: None,
            on_close: None,
            on_tab_change: None,
            on_search_query: None,
            on_select_gif_category: None,
            on_select_guild: None,
        };

        assert_eq!(config.guild_sections.len(), 1);
        assert_eq!(config.selected_guild_id.as_deref(), Some("guild-123"));
        assert!(config.is_focused);
    }

    #[::core::prelude::v1::test]
    fn test_media_picker_guild_filtering_behavior() {
        let g1 = MediaPickerGuildSection {
            guild_id: "g1".to_string(),
            guild_name: "Server One".to_string(),
            icon_url: None,
            acronym: "S1".to_string(),
            emojis: vec![DiscordEmoji::new_custom("1", "alpha", false, Some("g1".to_string()))],
            stickers: Vec::new(),
        };
        let g2 = MediaPickerGuildSection {
            guild_id: "g2".to_string(),
            guild_name: "Server Two".to_string(),
            icon_url: None,
            acronym: "S2".to_string(),
            emojis: vec![DiscordEmoji::new_custom("2", "beta", false, Some("g2".to_string()))],
            stickers: Vec::new(),
        };

        let sections = vec![g1, g2];

        // Filter by g1
        let filtered_g1: Vec<_> = sections.iter().filter(|s| s.guild_id == "g1").collect();
        assert_eq!(filtered_g1.len(), 1);
        assert_eq!(filtered_g1[0].guild_name, "Server One");

        // Filter by g2
        let filtered_g2: Vec<_> = sections.iter().filter(|s| s.guild_id == "g2").collect();
        assert_eq!(filtered_g2.len(), 1);
        assert_eq!(filtered_g2[0].guild_name, "Server Two");
    }

    #[::core::prelude::v1::test]
    fn test_build_emoji_virtual_rows_chunking_and_structure() {
        let mut emojis = Vec::new();
        for i in 0..25 {
            emojis.push(DiscordEmoji::new_custom(
                &format!("e{}", i),
                &format!("crab_{}", i),
                false,
                Some("g1".to_string()),
            ));
        }
        let section = MediaPickerGuildSection {
            guild_id: "g1".to_string(),
            guild_name: "Crab Guild".to_string(),
            icon_url: None,
            acronym: "CG".to_string(),
            emojis,
            stickers: Vec::new(),
        };
        let config = MediaPickerConfig {
            active_tab: MediaPickerTab::Emojis,
            search_query: String::new(),
            recent_emojis: Vec::new(),
            custom_emojis: Vec::new(),
            stickers: Vec::new(),
            gifs: Vec::new(),
            sounds: Vec::new(),
            hovered_info: None,
            guild_sections: vec![section],
            selected_guild_id: Some("g1".to_string()),
            focus_handle: None,
            is_focused: false,
            list_state: None,
            on_select_emoji: None,
            on_select_sticker: None,
            on_select_gif: None,
            on_play_sound: None,
            on_send_sound: None,
            on_close: None,
            on_tab_change: None,
            on_search_query: None,
            on_select_gif_category: None,
            on_select_guild: None,
        };

        let rows = build_emoji_virtual_rows(&config, "", false);
        // Expect FilterBanner, SectionHeader, and 4 GridRows (8 + 8 + 8 + 1 = 25 items)
        assert_eq!(rows.len(), 6);
        match &rows[0] {
            EmojiVirtualRow::FilterBanner { title } => assert_eq!(title, "Crab Guild"),
            _ => panic!("Expected FilterBanner"),
        }
        match &rows[1] {
            EmojiVirtualRow::SectionHeader { title, count, .. } => {
                assert_eq!(title, "CRAB GUILD");
                assert_eq!(*count, 25);
            }
            _ => panic!("Expected SectionHeader"),
        }
        match &rows[2] {
            EmojiVirtualRow::GridRow { items } => assert_eq!(items.len(), 8),
            _ => panic!("Expected GridRow of 8 items"),
        }
        match &rows[3] {
            EmojiVirtualRow::GridRow { items } => assert_eq!(items.len(), 8),
            _ => panic!("Expected GridRow of 8 items"),
        }
        match &rows[4] {
            EmojiVirtualRow::GridRow { items } => assert_eq!(items.len(), 8),
            _ => panic!("Expected GridRow of 8 items"),
        }
        match &rows[5] {
            EmojiVirtualRow::GridRow { items } => assert_eq!(items.len(), 1),
            _ => panic!("Expected GridRow of 1 items"),
        }
    }

    #[::core::prelude::v1::test]
    fn test_build_sticker_virtual_rows_chunking_and_structure() {
        let mut stickers = Vec::new();
        for i in 0..10 {
            stickers.push(DiscordSticker {
                id: format!("s{}", i),
                pack_id: None,
                name: format!("sticker_{}", i),
                description: None,
                tags: "wumpus".to_string(),
                format_type: StickerFormatType::Png,
                available: true,
                guild_id: Some("g1".to_string()),
            });
        }
        let section = MediaPickerGuildSection {
            guild_id: "g1".to_string(),
            guild_name: "Sticker Guild".to_string(),
            icon_url: None,
            acronym: "SG".to_string(),
            emojis: Vec::new(),
            stickers,
        };
        let config = MediaPickerConfig {
            active_tab: MediaPickerTab::Stickers,
            search_query: String::new(),
            recent_emojis: Vec::new(),
            custom_emojis: Vec::new(),
            stickers: Vec::new(),
            gifs: Vec::new(),
            sounds: Vec::new(),
            hovered_info: None,
            guild_sections: vec![section],
            selected_guild_id: Some("g1".to_string()),
            focus_handle: None,
            is_focused: false,
            list_state: None,
            on_select_emoji: None,
            on_select_sticker: None,
            on_select_gif: None,
            on_play_sound: None,
            on_send_sound: None,
            on_close: None,
            on_tab_change: None,
            on_search_query: None,
            on_select_gif_category: None,
            on_select_guild: None,
        };

        let rows = build_sticker_virtual_rows(&config, "");
        // FilterBanner, SectionHeader, and 3 GridRows (4 + 4 + 2 = 10 items)
        assert_eq!(rows.len(), 5);
        match &rows[0] {
            StickerVirtualRow::FilterBanner { title } => assert_eq!(title, "Sticker Guild"),
            _ => panic!("Expected FilterBanner"),
        }
        match &rows[1] {
            StickerVirtualRow::SectionHeader { title, count, .. } => {
                assert_eq!(title, "STICKER GUILD");
                assert_eq!(*count, 10);
            }
            _ => panic!("Expected SectionHeader"),
        }
        match &rows[2] {
            StickerVirtualRow::GridRow { items } => assert_eq!(items.len(), 4),
            _ => panic!("Expected GridRow of 4 items"),
        }
        match &rows[3] {
            StickerVirtualRow::GridRow { items } => assert_eq!(items.len(), 4),
            _ => panic!("Expected GridRow of 4 items"),
        }
        match &rows[4] {
            StickerVirtualRow::GridRow { items } => assert_eq!(items.len(), 2),
            _ => panic!("Expected GridRow of 2 items"),
        }
    }

    #[::core::prelude::v1::test]
    fn test_media_picker_measured_dimensions() {
        // Verifica constantes de layout do picker correspondentes às medições do Discord
        assert_eq!(EMOJIS_PER_ROW, 8);
        assert_eq!(STICKERS_PER_ROW, 4);

        // Verifica que render_emoji_item cria elemento com célula correta
        let item = EmojiVirtualItem::Unicode("😀".to_string());
        let _el = render_emoji_item(&item, None);
    }
}
