use gpui::prelude::*;
use gpui::*;
use serde::{Deserialize, Serialize};

use crate::tokens::icons::icon_info;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_description;
use super::UserSettingsModal;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindBinding {
    pub action: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindConfig {
    pub bindings: Vec<KeybindBinding>,
}

impl Default for KeybindConfig {
    fn default() -> Self {
        Self {
            bindings: vec![
                KeybindBinding { action: "Buscar".into(), keys: vec!["CTRL".into(), "F".into()] },
                KeybindBinding { action: "Mostrar Lista de Atalhos".into(), keys: vec!["CTRL".into(), "/".into()] },
                KeybindBinding { action: "Editar Mensagem Selecionada".into(), keys: vec!["E".into()] },
                KeybindBinding { action: "Excluir Mensagem Selecionada".into(), keys: vec!["BACKSPACE".into()] },
                KeybindBinding { action: "Navegar Mensagens".into(), keys: vec!["↑".into(), "↓".into()] },
                KeybindBinding { action: "Fechar / Cancelar".into(), keys: vec!["ESC".into()] },
            ],
        }
    }
}

pub fn keybinds_config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("discord-rs")
        .join("keybinds.json")
}

pub fn load_keybinds_config_from(path: &std::path::Path) -> KeybindConfig {
    if !path.exists() {
        return KeybindConfig::default();
    }
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return KeybindConfig::default(),
    };
    serde_json::from_str::<KeybindConfig>(&data).unwrap_or_default()
}

pub fn save_keybinds_config_to(path: &std::path::Path, config: &KeybindConfig) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_keybinds_config() -> KeybindConfig {
    load_keybinds_config_from(&keybinds_config_path())
}

pub fn save_keybinds_config(config: &KeybindConfig) {
    save_keybinds_config_to(&keybinds_config_path(), config);
}

pub fn render_keybinds(
    _modal: Entity<UserSettingsModal>,
    _modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let config = load_keybinds_config();
    let (nav_bindings, msg_bindings): (Vec<_>, Vec<_>) = config.bindings.iter().partition(|b| {
        matches!(b.action.as_str(), "Buscar" | "Mostrar Lista de Atalhos" | "Fechar / Cancelar")
    });

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Atalhos de Teclado"),
                )
                .child(render_section_description(
                    "Atalhos padrão disponíveis no cliente. A personalização de atalhos é persistida localmente quando configurada.",
                    cx,
                ))
                .child(
                    div()
                        .w_full()
                        .p(px(12.0))
                        .rounded(px(6.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon_info(px(18.0), cx.discord().blurple))
                        .child(
                            div()
                                .w(px(580.0))
                                .text_color(cx.discord().text_primary)
                                .text_size(px(13.0))
                                .child("A reatribuição interativa de atalhos ainda não está disponível nesta versão; os atalhos padrão abaixo estão ativos e sua configuração é persistida em disco."),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Navegação"),
                )
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .children(nav_bindings.iter().map(|b| {
                            let keys: Vec<&str> = b.keys.iter().map(|k| k.as_str()).collect();
                            render_keybind_row(&b.action, &keys, cx)
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Mensagens"),
                )
                .child(render_section_description(
                    "Estes atalhos funcionam com uma mensagem selecionada ou no contexto do chat.",
                    cx,
                ))
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .children(msg_bindings.iter().map(|b| {
                            let keys: Vec<&str> = b.keys.iter().map(|k| k.as_str()).collect();
                            render_keybind_row(&b.action, &keys, cx)
                        })),
                ),
        )
}

fn render_keybind_row(
    action: &str,
    keys: &[&str],
    cx: &App,
) -> Div {
    let hover_bg = cx.discord().channel_hover;

    div()
        .h(px(44.0))
        .px(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .hover(move |s| s.bg(hover_bg))
        // Action Label
        .child(
            div()
                .text_color(cx.discord().text_titles)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                        .child(action.to_string()),
        )
        // Key Pills
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(keys.iter().map(|key| {
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .text_color(cx.discord().text_primary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child((*key).to_string())
                })),
        )
}
