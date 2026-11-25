//! Tests for T-035: announced shortcuts, message selection, and keybind persistence.
//!
//! Covers:
//! - Default keybind config contains all announced shortcuts (CTRL+/, E, BACKSPACE)
//! - Keybind config round-trips through disk persistence
//! - Settings keybind list matches announced behavior
//! - Message row selection state is renderable
//! - Keyboard selection/delete/edit decision logic

use discord_rs::app::user_settings::keybinds::{
    load_keybinds_config_from, save_keybinds_config_to, KeybindConfig,
};
use discord_rs::chat::message_list::{render_message_row, MessageRowConfig};
use discord_rs::domain::message::{Author, Message};

fn synthetic_message(id: &str, author_id: &str, content: &str) -> Message {
    Message::new(id, "ch-1", Author::new(author_id, "User"), content)
}

#[test]
fn default_keybinds_include_announced_shortcuts() {
    let cfg = KeybindConfig::default();
    let actions: Vec<(&str, Vec<&str>)> = cfg
        .bindings
        .iter()
        .map(|b| (b.action.as_str(), b.keys.iter().map(|k| k.as_str()).collect()))
        .collect();

    assert!(
        actions.iter().any(|(a, k)| *a == "Mostrar Lista de Atalhos" && k == &vec!["CTRL", "/"]),
        "missing CTRL+/ shortcut: {actions:?}"
    );
    assert!(
        actions.iter().any(|(a, k)| *a == "Editar Mensagem Selecionada" && k == &vec!["E"]),
        "missing E shortcut: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|(a, k)| *a == "Excluir Mensagem Selecionada" && k == &vec!["BACKSPACE"]),
        "missing BACKSPACE shortcut: {actions:?}"
    );
}

#[test]
fn keybind_config_persists_round_trip() {
    let dir = std::env::temp_dir().join(format!("discord-rs-keybinds-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("keybinds.json");

    let mut cfg = KeybindConfig::default();
    cfg.bindings.push(discord_rs::app::user_settings::keybinds::KeybindBinding {
        action: "Ação Custom".into(),
        keys: vec!["CTRL".into(), "K".into()],
    });
    save_keybinds_config_to(&path, &cfg);

    let loaded = load_keybinds_config_from(&path);
    assert_eq!(loaded, cfg);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn keybind_config_missing_file_returns_default() {
    let path = std::env::temp_dir().join(format!("discord-rs-keybinds-missing-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let loaded = load_keybinds_config_from(&path);
    assert_eq!(loaded, KeybindConfig::default());
}

#[test]
fn keybind_config_invalid_json_returns_default() {
    let dir = std::env::temp_dir().join(format!("discord-rs-keybinds-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("keybinds.json");
    std::fs::write(&path, "{not json").expect("write bad json");
    let loaded = load_keybinds_config_from(&path);
    assert_eq!(loaded, KeybindConfig::default());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn message_row_renders_selected_and_unselected() {
    let msg = synthetic_message("m1", "u1", "hello");
    let selected = render_message_row(
        &msg,
        MessageRowConfig {
            is_grouped: false,
            role_color: None,
            on_react: None,
            on_reply: None,
            on_more: None,
            on_reaction_click: None,
            on_add_reaction: None,
            is_selected: true,
        },
    );
    drop(selected);

    let unselected = render_message_row(
        &msg,
        MessageRowConfig {
            is_grouped: false,
            role_color: None,
            on_react: None,
            on_reply: None,
            on_more: None,
            on_reaction_click: None,
            on_add_reaction: None,
            is_selected: false,
        },
    );
    drop(unselected);
}

/// Keyboard decision logic mirror for testability without GPUI context.
/// These rules match ChatView::on_key_down behavior implemented for T-035.
#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyAction {
    None,
    OpenShortcuts,
    MoveSelection(i32),
    EditSelected,
    DeleteSelected,
    ClearSelection,
}

fn decide_key_action(
    key_lower: &str,
    is_ctrl: bool,
    has_shift: bool,
    has_alt: bool,
    composer_empty: bool,
    is_editing: bool,
    has_modal: bool,
    has_selection: bool,
) -> KeyAction {
    if is_ctrl && !has_shift && !has_alt && (key_lower == "/" || key_lower == "?") {
        return KeyAction::OpenShortcuts;
    }
    if key_lower == "escape" && has_selection && !has_modal && !is_editing {
        return KeyAction::ClearSelection;
    }
    if !is_editing && !has_modal && (key_lower == "up" || key_lower == "arrowup") {
        if has_selection || composer_empty {
            return KeyAction::MoveSelection(-1);
        }
    }
    if !is_editing && !has_modal && (key_lower == "down" || key_lower == "arrowdown") && has_selection {
        return KeyAction::MoveSelection(1);
    }
    if !is_editing && !has_modal && !is_ctrl && !has_alt && composer_empty {
        if key_lower == "e" {
            return KeyAction::EditSelected;
        }
        if key_lower == "backspace" || key_lower == "delete" {
            return KeyAction::DeleteSelected;
        }
    }
    KeyAction::None
}

#[test]
fn key_event_ctrl_slash_opens_shortcuts() {
    assert_eq!(
        decide_key_action("/", true, false, false, true, false, false, false),
        KeyAction::OpenShortcuts
    );
}

#[test]
fn key_event_e_edits_selected_when_composer_empty() {
    assert_eq!(
        decide_key_action("e", false, false, false, true, false, false, true),
        KeyAction::EditSelected
    );
}

#[test]
fn key_event_e_does_not_edit_when_typing() {
    assert_eq!(
        decide_key_action("e", false, false, false, false, false, false, true),
        KeyAction::None
    );
}

#[test]
fn key_event_backspace_deletes_selected_when_composer_empty() {
    assert_eq!(
        decide_key_action("backspace", false, false, false, true, false, false, true),
        KeyAction::DeleteSelected
    );
}

#[test]
fn key_event_delete_also_deletes_selected() {
    assert_eq!(
        decide_key_action("delete", false, false, false, true, false, false, true),
        KeyAction::DeleteSelected
    );
}

#[test]
fn key_event_arrow_navigation_moves_selection() {
    assert_eq!(
        decide_key_action("arrowup", false, false, false, true, false, false, true),
        KeyAction::MoveSelection(-1)
    );
    assert_eq!(
        decide_key_action("arrowdown", false, false, false, true, false, false, true),
        KeyAction::MoveSelection(1)
    );
}

#[test]
fn key_event_ignored_when_modal_or_editing() {
    assert_eq!(
        decide_key_action("e", false, false, false, true, true, false, true),
        KeyAction::None
    );
    assert_eq!(
        decide_key_action("backspace", false, false, false, true, false, true, true),
        KeyAction::None
    );
}

#[test]
fn key_event_escape_clears_selection() {
    assert_eq!(
        decide_key_action("escape", false, false, false, true, false, false, true),
        KeyAction::ClearSelection
    );
}
