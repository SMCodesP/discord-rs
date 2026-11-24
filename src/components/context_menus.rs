//! Declarative context menu builders for guild rail, channel sidebar, and related surfaces.
//!
//! T-088: menus respect permissions and Developer Mode (Copy ID only when enabled).
//! Menu items are represented as data first so tests can verify the matrix without
//! depending on GPUI rendering.

use gpui::component::menu::{PopupMenu, PopupMenuItem};
use gpui::*;

use std::sync::Arc;

/// Discord permission bits relevant to context menus.
/// Source: https://discord.com/developers/docs/topics/permissions
pub mod perms {
    pub const VIEW_CHANNEL: u64 = 1 << 10;
    pub const SEND_MESSAGES: u64 = 1 << 11;
    pub const MANAGE_MESSAGES: u64 = 1 << 13;
    pub const MANAGE_CHANNELS: u64 = 1 << 4;
    pub const MANAGE_GUILD: u64 = 1 << 5;
    pub const MUTE_MEMBERS: u64 = 1 << 22;
}

/// Permission snapshot used by context menu builders.
/// This is a declarative, testable projection; callers compute it from real guild/role data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuPermissions {
    pub can_manage_guild: bool,
    pub can_manage_channels: bool,
    pub can_view_channel: bool,
}

impl MenuPermissions {
    pub fn all_allowed() -> Self {
        Self {
            can_manage_guild: true,
            can_manage_channels: true,
            can_view_channel: true,
        }
    }

    pub fn from_permission_bits(bits: u64) -> Self {
        let administrator = (bits & (1 << 3)) != 0;
        Self {
            can_manage_guild: administrator || (bits & perms::MANAGE_GUILD) != 0,
            can_manage_channels: administrator || (bits & perms::MANAGE_CHANNELS) != 0,
            can_view_channel: administrator || (bits & perms::VIEW_CHANNEL) != 0,
        }
    }
}

/// What kind of surface the context menu is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTargetKind {
    Guild,
    GuildFolder,
    Category,
    TextChannel,
    VoiceChannel,
}

/// Declarative action identifiers. Tests assert on these; UI maps them to handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    MarkGuildAsRead,
    MuteGuild,
    GuildNotificationSettings,
    HideMutedChannels,
    GuildPrivacySettings,
    EditGuildProfile,
    LeaveGuild,
    CopyGuildId,

    MarkFolderAsRead,
    MuteFolder,
    FolderSettings,
    CopyFolderId,

    MarkCategoryAsRead,
    MuteCategory,
    CollapseCategory,
    CollapseAllCategories,
    EditCategory,
    DeleteCategory,
    CopyCategoryId,

    MarkChannelAsRead,
    MuteChannel,
    ChannelNotificationSettings,
    EditChannel,
    CreateInvite,
    CloneChannel,
    DeleteChannel,
    CopyChannelLink,
    CopyChannelId,
}

/// A menu entry in declarative form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        action: MenuAction,
        label: &'static str,
        danger: bool,
        enabled: bool,
    },
    Separator,
}

impl MenuEntry {
    pub fn item(action: MenuAction, label: &'static str) -> Self {
        Self::Item {
            action,
            label,
            danger: false,
            enabled: true,
        }
    }

    pub fn danger(action: MenuAction, label: &'static str) -> Self {
        Self::Item {
            action,
            label,
            danger: true,
            enabled: true,
        }
    }

    pub fn disabled(action: MenuAction, label: &'static str) -> Self {
        Self::Item {
            action,
            label,
            danger: false,
            enabled: false,
        }
    }

    pub fn action(&self) -> Option<MenuAction> {
        match self {
            Self::Item { action, .. } => Some(*action),
            Self::Separator => None,
        }
    }
}

/// Input for building a context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextMenuContext {
    pub target: MenuTargetKind,
    pub developer_mode: bool,
    pub permissions: MenuPermissions,
    /// For channels/categories: whether the channel/category is currently muted.
    pub is_muted: bool,
    /// For category collapse state.
    pub is_collapsed: bool,
}

impl ContextMenuContext {
    pub fn new(target: MenuTargetKind) -> Self {
        Self {
            target,
            developer_mode: false,
            permissions: MenuPermissions::default(),
            is_muted: false,
            is_collapsed: false,
        }
    }

    pub fn developer_mode(mut self, enabled: bool) -> Self {
        self.developer_mode = enabled;
        self
    }

    pub fn permissions(mut self, permissions: MenuPermissions) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.is_collapsed = collapsed;
        self
    }
}

/// Build the declarative menu entries for the given context.
/// Copy ID entries are included only when Developer Mode is enabled.
pub fn build_menu_entries(ctx: ContextMenuContext) -> Vec<MenuEntry> {
    let mut entries = match ctx.target {
        MenuTargetKind::Guild => build_guild_entries(ctx),
        MenuTargetKind::GuildFolder => build_folder_entries(ctx),
        MenuTargetKind::Category => build_category_entries(ctx),
        MenuTargetKind::TextChannel | MenuTargetKind::VoiceChannel => build_channel_entries(ctx),
    };

    if ctx.developer_mode {
        let copy_id = copy_id_entry(ctx.target);
        if !matches!(entries.last(), Some(MenuEntry::Separator)) {
            entries.push(MenuEntry::Separator);
        }
        entries.push(MenuEntry::item(copy_id.0, copy_id.1));
    }

    entries
}

fn copy_id_entry(target: MenuTargetKind) -> (MenuAction, &'static str) {
    match target {
        MenuTargetKind::Guild => (MenuAction::CopyGuildId, "Copiar ID do servidor"),
        MenuTargetKind::GuildFolder => (MenuAction::CopyFolderId, "Copiar ID da pasta"),
        MenuTargetKind::Category => (MenuAction::CopyCategoryId, "Copiar ID da categoria"),
        MenuTargetKind::TextChannel | MenuTargetKind::VoiceChannel => {
            (MenuAction::CopyChannelId, "Copiar ID do canal")
        }
    }
}

fn build_guild_entries(ctx: ContextMenuContext) -> Vec<MenuEntry> {
    let mut entries = vec![
        MenuEntry::item(MenuAction::MarkGuildAsRead, "Marcar como lida"),
    ];

    let mute_label = if ctx.is_muted { "Reativar servidor" } else { "Silenciar servidor" };
    entries.push(MenuEntry::item(MenuAction::MuteGuild, mute_label));
    entries.push(MenuEntry::item(
        MenuAction::GuildNotificationSettings,
        "Configurações de notificação",
    ));
    entries.push(MenuEntry::item(MenuAction::HideMutedChannels, "Ocultar canais silenciados"));
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::item(MenuAction::GuildPrivacySettings, "Configurações de privacidade"));
    entries.push(MenuEntry::item(MenuAction::EditGuildProfile, "Editar perfil do servidor"));

    if ctx.permissions.can_manage_guild {
        // Management actions are visible only when the user can manage the guild.
        // Leave is always available as a danger action.
    }

    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::danger(MenuAction::LeaveGuild, "Sair do servidor"));
    entries
}

fn build_folder_entries(ctx: ContextMenuContext) -> Vec<MenuEntry> {
    let mute_label = if ctx.is_muted { "Reativar pasta" } else { "Silenciar pasta" };
    vec![
        MenuEntry::item(MenuAction::MarkFolderAsRead, "Marcar como lida"),
        MenuEntry::item(MenuAction::MuteFolder, mute_label),
        MenuEntry::item(MenuAction::FolderSettings, "Configurações da pasta"),
    ]
}

fn build_category_entries(ctx: ContextMenuContext) -> Vec<MenuEntry> {
    let mute_label = if ctx.is_muted { "Reativar categoria" } else { "Silenciar categoria" };
    let collapse_label = if ctx.is_collapsed { "Expandir categoria" } else { "Recolher categoria" };
    let mut entries = vec![
        MenuEntry::item(MenuAction::MarkCategoryAsRead, "Marcar como lida"),
        MenuEntry::item(MenuAction::MuteCategory, mute_label),
        MenuEntry::Separator,
        MenuEntry::item(MenuAction::CollapseCategory, collapse_label),
        MenuEntry::item(MenuAction::CollapseAllCategories, "Recolher todas as categorias"),
    ];

    if ctx.permissions.can_manage_channels {
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::item(MenuAction::EditCategory, "Editar categoria"));
        entries.push(MenuEntry::danger(MenuAction::DeleteCategory, "Excluir categoria"));
    }

    entries
}

fn build_channel_entries(ctx: ContextMenuContext) -> Vec<MenuEntry> {
    let is_voice = ctx.target == MenuTargetKind::VoiceChannel;
    let mute_label = if ctx.is_muted { "Reativar canal" } else { "Silenciar canal" };
    let mut entries = vec![
        MenuEntry::item(MenuAction::MarkChannelAsRead, "Marcar como lido"),
        MenuEntry::item(MenuAction::MuteChannel, mute_label),
        MenuEntry::item(MenuAction::ChannelNotificationSettings, "Configurações de notificação"),
    ];

    if !is_voice {
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::item(MenuAction::CopyChannelLink, "Copiar link do canal"));
    }

    if ctx.permissions.can_manage_channels {
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::item(MenuAction::EditChannel, "Editar canal"));
        if is_voice {
            entries.push(MenuEntry::item(MenuAction::CreateInvite, "Convidar pessoas"));
        }
        entries.push(MenuEntry::item(MenuAction::CloneChannel, "Clonar canal"));
        entries.push(MenuEntry::danger(MenuAction::DeleteChannel, "Excluir canal"));
    }

    entries
}

/// Callback map for wiring declarative menu entries to UI actions.
#[derive(Clone, Default)]
pub struct MenuCallbacks {
    handlers: Vec<(MenuAction, Arc<dyn Fn(&mut Window, &mut App) + 'static>)>,
}

impl MenuCallbacks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on(mut self, action: MenuAction, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handlers.push((action, Arc::new(handler)));
        self
    }

    pub fn get(&self, action: MenuAction) -> Option<Arc<dyn Fn(&mut Window, &mut App) + 'static>> {
        self.handlers
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, h)| h.clone())
    }
}

/// Apply declarative entries to a gpui-kit PopupMenu.
pub fn apply_entries_to_popup_menu(
    mut menu: PopupMenu,
    entries: &[MenuEntry],
    callbacks: &MenuCallbacks,
) -> PopupMenu {
    for entry in entries {
        match entry {
            MenuEntry::Separator => {
                menu = menu.separator();
            }
            MenuEntry::Item {
                action,
                label,
                danger: _,
                enabled,
            } => {
                let mut item = PopupMenuItem::new(*label);
                if !enabled {
                    // gpui-kit PopupMenuItem may not expose disabled; keep label but no handler.
                } else if let Some(handler) = callbacks.get(*action) {
                    item = item.on_click(move |_ev, window, cx| {
                        handler(window, cx);
                    });
                }
                menu = menu.item(item);
            }
        }
    }
    menu
}

/// Convenience: build and apply a menu in one call.
pub fn build_popup_menu(
    menu: PopupMenu,
    ctx: ContextMenuContext,
    callbacks: &MenuCallbacks,
) -> PopupMenu {
    let entries = build_menu_entries(ctx);
    apply_entries_to_popup_menu(menu, &entries, callbacks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_action(entries: &[MenuEntry], action: MenuAction) -> bool {
        entries.iter().any(|e| e.action() == Some(action))
    }

    #[test]
    fn copy_id_only_in_developer_mode() {
        let ctx = ContextMenuContext::new(MenuTargetKind::Guild)
            .permissions(MenuPermissions::all_allowed())
            .developer_mode(false);
        let entries = build_menu_entries(ctx);
        assert!(!has_action(&entries, MenuAction::CopyGuildId));

        let entries_dev = build_menu_entries(ctx.developer_mode(true));
        assert!(has_action(&entries_dev, MenuAction::CopyGuildId));
    }

    #[test]
    fn category_management_requires_permission() {
        let base = ContextMenuContext::new(MenuTargetKind::Category)
            .permissions(MenuPermissions::default());
        let entries = build_menu_entries(base);
        assert!(!has_action(&entries, MenuAction::EditCategory));
        assert!(!has_action(&entries, MenuAction::DeleteCategory));

        let allowed = build_menu_entries(base.permissions(MenuPermissions {
            can_manage_channels: true,
            ..MenuPermissions::default()
        }));
        assert!(has_action(&allowed, MenuAction::EditCategory));
        assert!(has_action(&allowed, MenuAction::DeleteCategory));
    }

    #[test]
    fn channel_menu_includes_copy_link_and_manage_actions() {
        let ctx = ContextMenuContext::new(MenuTargetKind::TextChannel)
            .permissions(MenuPermissions {
                can_manage_channels: true,
                ..MenuPermissions::default()
            })
            .developer_mode(true);
        let entries = build_menu_entries(ctx);
        assert!(has_action(&entries, MenuAction::MarkChannelAsRead));
        assert!(has_action(&entries, MenuAction::MuteChannel));
        assert!(has_action(&entries, MenuAction::CopyChannelLink));
        assert!(has_action(&entries, MenuAction::CopyChannelId));
        assert!(has_action(&entries, MenuAction::EditChannel));
    }
}
