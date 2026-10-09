//! Discord RS - Seletor Unificado de Mídia (Media Picker Popover)
//!
//! Implementa o popup flutuante de seleção de:
//! 1. Emojis (Frequentemente Usados, 8 Categorias Unicode Oficiais, Emojis de Servidores)
//! 2. Figurinhas / Stickers (Pacotes padrão Wumpus e stickers de guildas)
//! 3. GIFs (Busca e Trending integrados ao Tenor)
//! 4. Soundboard (Sons padrão e sons de servidores com preview de áudio)
//! 5. Barra de busca adaptável com filtragem em tempo real
//! 6. Faixa inferior de pré-visualização (Hover Preview Strip)

use gpui::*;
use std::sync::Arc;

use crate::protocol::media_cache::{
    DiscordEmoji, DiscordSticker, MediaCacheManager, RecentEmojiItem, SoundboardSound, TenorGif,
};
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

pub struct MediaPickerConfig {
    pub active_tab: MediaPickerTab,
    pub search_query: String,
    pub recent_emojis: Vec<RecentEmojiItem>,
    pub custom_emojis: Vec<DiscordEmoji>,
    pub stickers: Vec<DiscordSticker>,
    pub gifs: Vec<TenorGif>,
    pub sounds: Vec<SoundboardSound>,
    pub hovered_info: Option<String>,
    pub on_select_emoji: Option<OnSelectEmojiCallback>,
    pub on_select_sticker: Option<OnSelectStickerCallback>,
    pub on_select_gif: Option<OnSelectGifCallback>,
    pub on_play_sound: Option<OnPlaySoundCallback>,
    pub on_send_sound: Option<OnPlaySoundCallback>,
    pub on_close: Option<OnCloseMediaPickerCallback>,
    pub on_tab_change: Option<OnTabChangeCallback>,
    pub on_search_query: Option<OnSearchQueryCallback>,
    pub on_select_gif_category: Option<OnSelectGifCategoryCallback>,
}

// ============================================================================
// Componente de Renderização
// ============================================================================

/// Renderiza o seletor popup flutuante do Discord
pub fn render_media_picker(config: MediaPickerConfig) -> Div {
    let active_tab = config.active_tab;
    let query = config.search_query.trim().to_lowercase();
    let is_searching = !query.is_empty();

    div()
        .flex()
        .flex_col()
        .w(px(432.0))
        .h(px(440.0))
        .bg(rgb(HEX_BG_CHANNELS))
        .border_1()
        .border_color(rgb(0x1e1f22))
        .rounded(radius_lg())
        .shadow_xl()
        .overflow_hidden()
        // 1. Barra de Abas Superiores
        .child(render_top_tabs(&config))
        // 2. Barra de Busca
        .child(render_search_bar(&config))
        // 3. Área Central (Grade do Conteúdo Selecionado)
        .child(
            div()
                .id("media-picker-scroll-content")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .p(px(8.0))
                .child(match active_tab {
                    MediaPickerTab::Emojis => {
                        render_emojis_tab(&config, &query, is_searching)
                    }
                    MediaPickerTab::Stickers => {
                        render_stickers_tab(&config, &query)
                    }
                    MediaPickerTab::Gifs => {
                        render_gifs_tab(&config, &query)
                    }
                    MediaPickerTab::Soundboard => {
                        render_soundboard_tab(&config, &query)
                    }
                }),
        )
        // 4. Faixa Inferior de Preview (Hover Preview Strip)
        .child(render_preview_strip(config.hovered_info.as_deref()))
}

// ============================================================================
// 1. Barra de Abas Superiores
// ============================================================================

fn render_top_tabs(config: &MediaPickerConfig) -> Div {
    let tabs = [
        MediaPickerTab::Emojis,
        MediaPickerTab::Stickers,
        MediaPickerTab::Gifs,
        MediaPickerTab::Soundboard,
    ];

    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(40.0))
        .px(px(8.0))
        .bg(rgb(HEX_BG_USER_BAR))
        .border_b_1()
        .border_color(rgb(0x1e1f22));

    let mut tabs_container = div().flex().flex_row().items_center().gap(px(4.0));

    for tab in tabs {
        let is_active = tab == config.active_tab;
        let bg_col = if is_active {
            rgb(HEX_CHANNEL_ACTIVE)
        } else {
            rgba(0x00000000)
        };
        let text_col = if is_active {
            rgb(HEX_WHITE)
        } else {
            text_muted()
        };

        let mut tab_btn = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(28.0))
            .px(px(10.0))
            .rounded(radius_sm())
            .bg(bg_col)
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
            .child(tab.render_icon(px(16.0), text_col))
            .child(
                div()
                    .text_size(font_size_xs())
                    .font_weight(if is_active { WEIGHT_BOLD } else { WEIGHT_MEDIUM })
                    .text_color(text_col)
                    .child(tab.label()),
            );

        if let Some(on_tab) = config.on_tab_change.clone() {
            tab_btn = tab_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_tab(tab, window, cx);
            });
        }

        tabs_container = tabs_container.child(tab_btn);
    }

    // Botão Fechar (X)
    let mut close_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .text_color(text_muted())
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .child(icon_close(px(14.0), text_muted()));

    if let Some(on_close) = config.on_close.clone() {
        close_btn = close_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
            on_close(window, cx);
        });
    }

    row.child(tabs_container).child(close_btn)
}

// ============================================================================
// 2. Barra de Busca
// ============================================================================

fn render_search_bar(config: &MediaPickerConfig) -> Div {
    let placeholder = config.active_tab.search_placeholder();
    let query_display = if config.search_query.is_empty() {
        placeholder.to_string()
    } else {
        config.search_query.clone()
    };
    let is_empty = config.search_query.is_empty();

    let mut search_inner = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .px(px(8.0))
        .bg(rgb(HEX_BG_SERVER))
        .rounded(radius_sm())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(icon_search(px(14.0), text_muted()))
                .child(
                    div()
                        .text_size(font_size_sm())
                        .text_color(if is_empty { text_muted() } else { text_normal() })
                        .child(query_display),
                ),
        );

    if !is_empty {
        let mut clear_btn = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded_full()
            .text_size(px(12.0))
            .text_color(text_muted())
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
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
        .bg(rgb(HEX_BG_CHANNELS))
        .border_b_1()
        .border_color(rgb(0x1e1f22))
        .child(search_inner)
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
        "😀" | "😃" | "😄" | "😁" | "😆" | "😅" => &["smile", "sorriso", "feliz", "happy", "joy", "alegria"],
        "😂" | "🤣" => &["lol", "laugh", "risada", "haha", "crying laughing", "chorando de rir"],
        "🥲" | "🥹" | "🥺" | "😢" | "😭" => &["cry", "choro", "sad", "triste", "lagrimas", "tears", "pleading"],
        "😊" | "😇" | "🙂" | "🙃" | "😉" | "😌" => &["angel", "anjo", "smile", "piscadinha", "calm", "relax"],
        "😍" | "🥰" | "😘" | "😗" | "😙" | "😚" => &["love", "amor", "kiss", "beijo", "apaixonado", "heart"],
        "😋" | "😛" | "😝" | "😜" | "🤪" => &["tongue", "lingua", "yummy", "crazy", "maluco", "brincadeira"],
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
        "👏" | "🙌" | "🫶" | "🙏" => &["clap", "palmas", "obrigado", "thanks", "rezar", "pray", "heart hands"],
        "❤️" | "🧡" | "💛" | "💚" | "💙" | "💜" | "🖤" | "🤍" | "🤎" | "💔" => &["heart", "coracao", "love", "amor"],
        "🔥" => &["fire", "fogo", "hot", "chama", "lit", "quente"],
        "✨" | "⭐" | "🌟" => &["star", "sparkles", "estrela", "brilho", "shine"],
        "🎉" | "🎊" => &["party", "tada", "festa", "confete", "celebration"],
        "🐶" | "🐱" => &["dog", "cat", "cachorro", "gato", "pet", "animal", "auau", "miau"],
        "🐸" => &["frog", "sapo", "pepe"],
        "🦀" => &["crab", "caranguejo", "ferris", "rust"],
        "🍕" | "🍔" | "🍟" => &["pizza", "burger", "batata", "fastfood", "lanche", "food", "comida"],
        "☕" | "🍺" | "🍻" => &["coffee", "cafe", "beer", "cerveja", "drink", "bebida"],
        "🎮" => &["game", "jogo", "videogame", "gamer", "play"],
        "⚽" | "🏀" | "🏈" | "⚾" => &["futebol", "soccer", "football", "ball", "bola", "esporte", "sport"],
        "🚗" | "🏎️" => &["car", "carro", "speed", "corrida", "auto"],
        "🚀" => &["rocket", "foguete", "moon", "space", "espaco"],
        "🇧🇷" => &["brasil", "brazil", "br", "bandeira", "flag"],
        "🇺🇸" => &["eua", "usa", "america", "bandeira", "flag"],
        "💀" | "☠️" => &["skull", "caveira", "dead", "morto"],
        _ => &[],
    };

    tags.iter().any(|t| t.contains(&q) || q.contains(*t))
}

fn render_emojis_tab(config: &MediaPickerConfig, query: &str, is_searching: bool) -> Div {
    let mut container = div().flex().flex_col().gap(px(12.0));

    // A. Seção: Frequentemente Usados (apenas se não estiver filtrando por busca)
    let default_recents;
    let recents = if config.recent_emojis.is_empty() {
        default_recents = MediaCacheManager::default_recent_emojis();
        &default_recents
    } else {
        &config.recent_emojis
    };

    if !is_searching && !recents.is_empty() {
        let mut recents_section = div().flex().flex_col().gap(px(6.0));
        recents_section = recents_section.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_smile(px(14.0), text_muted()))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child("FREQUENTEMENTE USADOS"),
                ),
        );

        let mut grid = div().flex().flex_row().flex_wrap().gap(px(4.0));
        for item in recents {
            let em = item.emoji.clone();
            let mut btn = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(34.0))
                .rounded(radius_sm())
                .cursor_pointer()
                .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)))
                .child(div().text_size(px(20.0)).child(em.clone()));

            if let Some(on_sel) = config.on_select_emoji.clone() {
                let em_clone = em.clone();
                btn = btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                    on_sel(&em_clone, window, cx);
                });
            }
            grid = grid.child(btn);
        }
        recents_section = recents_section.child(grid);
        container = container.child(recents_section);
    }

    // B. Seção: Emojis Customizados do Servidor (se existirem)
    if !config.custom_emojis.is_empty() {
        let matching_custom: Vec<&DiscordEmoji> = if is_searching {
            config
                .custom_emojis
                .iter()
                .filter(|e| e.name.to_lowercase().contains(query))
                .collect()
        } else {
            config.custom_emojis.iter().collect()
        };

        if !matching_custom.is_empty() {
            let mut server_section = div().flex().flex_col().gap(px(6.0));
            server_section = server_section.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon_emoji(px(14.0), text_muted()))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(WEIGHT_BOLD)
                            .text_color(text_muted())
                            .child("EMOJIS DO SERVIDOR"),
                    ),
            );

            let mut grid = div().flex().flex_row().flex_wrap().gap(px(4.0));
            for emoji in matching_custom {
                let tag = emoji.to_tag();
                let name = emoji.name.clone();
                let mut btn = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(34.0))
                    .rounded(radius_sm())
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(blurple())
                            .child(format!(":{}", name)),
                    );

                if let Some(on_sel) = config.on_select_emoji.clone() {
                    let tag_clone = tag.clone();
                    btn = btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                        on_sel(&tag_clone, window, cx);
                    });
                }
                grid = grid.child(btn);
            }
            server_section = server_section.child(grid);
            container = container.child(server_section);
        }
    }

    // C. Categorias Canônicas Unicode do Discord (8 Categorias Oficiais)
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

        let cat_icon = match cat_key {
            "people" => icon_emoji(px(14.0), text_muted()),
            "nature" => icon_sparkles(px(14.0), text_muted()),
            "food" => icon_gift(px(14.0), text_muted()),
            "activity" => icon_gamepad(px(14.0), text_muted()),
            "travel" => icon_compass(px(14.0), text_muted()),
            "objects" => icon_gear(px(14.0), text_muted()),
            "symbols" => icon_hash(px(14.0), text_muted()),
            "flags" => icon_pin(px(14.0), text_muted()),
            _ => icon_emoji(px(14.0), text_muted()),
        };

        let mut cat_section = div().flex().flex_col().gap(px(6.0));
        cat_section = cat_section.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(cat_icon)
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child(cat_title),
                ),
        );

        let mut grid = div().flex().flex_row().flex_wrap().gap(px(4.0));
        for &em in &matching {
            let em_str = em.to_string();
            let mut btn = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(34.0))
                .rounded(radius_sm())
                .cursor_pointer()
                .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)))
                .child(div().text_size(px(20.0)).child(em_str.clone()));

            if let Some(on_sel) = config.on_select_emoji.clone() {
                let em_clone = em_str.clone();
                btn = btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                    on_sel(&em_clone, window, cx);
                });
            }
            grid = grid.child(btn);
        }
        cat_section = cat_section.child(grid);
        container = container.child(cat_section);
    }

    container
}

// ============================================================================
// 4. Conteúdo da Aba Figurinhas (Stickers)
// ============================================================================

fn render_stickers_tab(config: &MediaPickerConfig, query: &str) -> Div {
    let mut container = div().flex().flex_col().gap(px(12.0));
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

        let mut pack_section = div().flex().flex_col().gap(px(6.0));
        pack_section = pack_section.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_sticker(px(14.0), text_muted()))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child(pack.name.to_uppercase()),
                ),
        );

        let mut grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
        for sticker in matching {
            let sticker_clone = sticker.clone();
            let name = sticker.name.clone();

            let mut card = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .size(px(84.0))
                .p(px(6.0))
                .bg(rgb(0x232428))
                .rounded(radius_md())
                .border_1()
                .border_color(rgb(0x383a40))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).border_color(rgb(HEX_BLURPLE)))
                .child(icon_sticker(px(32.0), blurple()))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(WEIGHT_MEDIUM)
                        .text_color(text_titles())
                        .child(name),
                );

            if let Some(on_sel) = config.on_select_sticker.clone() {
                card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                    on_sel(&sticker_clone, window, cx);
                });
            }

            grid = grid.child(card);
        }
        pack_section = pack_section.child(grid);
        container = container.child(pack_section);
    }

    container
}

// ============================================================================
// 5. Conteúdo da Aba GIFs (Tenor)
// ============================================================================

fn render_gifs_tab(config: &MediaPickerConfig, query: &str) -> Div {
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

    // Categorias em alta canônicas com ícones vetoriais SVG autênticos
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
        let icon_color = if is_active { rgb(HEX_WHITE) } else { text_muted() };
        let mut chip = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(26.0))
            .px(px(10.0))
            .rounded(radius_pill())
            .bg(if is_active { rgb(HEX_BLURPLE) } else { rgb(0x232428) })
            .border_1()
            .border_color(if is_active { rgb(HEX_BLURPLE) } else { rgb(0x383a40) })
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
            .child(icon_builder(px(12.0), icon_color.into()))
            .child(
                div()
                    .text_size(font_size_xs())
                    .font_weight(WEIGHT_BOLD)
                    .text_color(if is_active { rgb(HEX_WHITE) } else { text_muted() })
                    .child(cat_label),
            );

        if let Some(on_cat) = config.on_select_gif_category.clone() {
            let cat_str = cat_id.to_string();
            chip = chip.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_cat(&cat_str, window, cx);
            });
        } else if let Some(on_search) = config.on_search_query.clone() {
            let cat_str = cat_id.to_string();
            chip = chip.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_search(&cat_str, window, cx);
            });
        }
        cat_row = cat_row.child(chip);
    }
    container = container.child(cat_row);

    // Fallback para biblioteca curada caso config.gifs esteja vazio
    let default_gifs;
    let gifs_to_render = if config.gifs.is_empty() {
        default_gifs = MediaCacheManager::get_curated_tenor_gifs_for_category(query);
        &default_gifs
    } else {
        &config.gifs
    };

    let mut grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
    for gif in gifs_to_render {
        let gif_url = gif.url.clone();
        let title = gif.title.clone();

        let mut card = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .w(px(190.0))
            .h(px(96.0))
            .p(px(8.0))
            .bg(rgb(0x232428))
            .rounded(radius_md())
            .border_1()
            .border_color(rgb(0x383a40))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).border_color(rgb(HEX_BLURPLE)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon_gif(px(20.0), text_muted()))
                    .child(
                        div()
                            .text_size(font_size_xs())
                            .font_weight(WEIGHT_BOLD)
                            .text_color(text_titles())
                            .child(title),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(text_muted())
                    .child("Clique para enviar no chat"),
            );

        if let Some(on_sel) = config.on_select_gif.clone() {
            let url_clone = gif_url.clone();
            card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_sel(&url_clone, window, cx);
            });
        }

        grid = grid.child(card);
    }

    container.child(grid)
}

// ============================================================================
// 6. Conteúdo da Aba Soundboard
// ============================================================================

fn render_soundboard_tab(config: &MediaPickerConfig, query: &str) -> Div {
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

    let mut grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
    for sound in matching_sounds {
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
            .bg(rgb(0x232428))
            .rounded(radius_md())
            .border_1()
            .border_color(rgb(0x383a40))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).border_color(rgb(HEX_BLURPLE)))
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
            .bg(rgb(0x2b2d31))
            .text_size(px(10.0))
            .text_color(text_muted())
            .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
            .child("▶");

        if let Some(on_play) = config.on_play_sound.clone() {
            let s_clone = sound_clone.clone();
            play_btn = play_btn.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_play(&s_clone, window, cx);
            });
        }
        card = card.child(play_btn);

        // Clique no card envia ou reproduz o som
        if let Some(on_send) = config.on_send_sound.clone() {
            let s_clone = sound_clone.clone();
            card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_send(&s_clone, window, cx);
            });
        } else if let Some(on_play) = config.on_play_sound.clone() {
            let s_clone = sound_clone.clone();
            card = card.on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_play(&s_clone, window, cx);
            });
        }

        grid = grid.child(card);
    }

    container.child(grid)
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
        .bg(rgb(HEX_BG_USER_BAR))
        .border_t_1()
        .border_color(rgb(0x1e1f22))
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
        assert!(packs.len() >= 3);
        let names: Vec<&str> = packs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Wumpus Adventures"));
        assert!(names.contains(&"Clyde Classic"));
        assert!(names.contains(&"Nelly the Hamster"));

        for pack in &packs {
            assert!(!pack.stickers.is_empty());
            for sticker in &pack.stickers {
                assert!(!sticker.name.is_empty());
                assert_eq!(sticker.format_type, StickerFormatType::Png);
            }
        }
    }

    #[::core::prelude::v1::test]
    fn test_soundboard_sounds_and_icons() {
        let sounds = MediaCacheManager::get_default_soundboard_sounds();
        assert!(!sounds.is_empty());
        assert!(sounds.iter().any(|s| s.name == "Quack"));
        assert!(sounds.iter().any(|s| s.name == "Airhorn"));
    }
}
