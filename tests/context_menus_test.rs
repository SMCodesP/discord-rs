use discord_rs::components::context_menus::{
    build_menu_entries, ContextMenuContext, MenuAction, MenuEntry, MenuPermissions, MenuTargetKind,
};

fn actions(entries: &[MenuEntry]) -> Vec<MenuAction> {
    entries.iter().filter_map(|e| e.action()).collect()
}

fn has(entries: &[MenuEntry], action: MenuAction) -> bool {
    actions(entries).contains(&action)
}

#[test]
fn guild_menu_has_expected_actions_and_copy_id_gated_by_developer_mode() {
    let base = ContextMenuContext::new(MenuTargetKind::Guild)
        .permissions(MenuPermissions::all_allowed());

    let entries = build_menu_entries(base.developer_mode(false));
    assert!(has(&entries, MenuAction::MarkGuildAsRead));
    assert!(has(&entries, MenuAction::MuteGuild));
    assert!(has(&entries, MenuAction::GuildNotificationSettings));
    assert!(has(&entries, MenuAction::GuildPrivacySettings));
    assert!(has(&entries, MenuAction::EditGuildProfile));
    assert!(has(&entries, MenuAction::LeaveGuild));
    assert!(!has(&entries, MenuAction::CopyGuildId), "Copy ID must be hidden when developer mode is off");

    let entries_dev = build_menu_entries(base.developer_mode(true));
    assert!(has(&entries_dev, MenuAction::CopyGuildId), "Copy ID must be visible in developer mode");
}

#[test]
fn guild_folder_menu_actions_and_copy_id_gate() {
    let ctx = ContextMenuContext::new(MenuTargetKind::GuildFolder)
        .permissions(MenuPermissions::all_allowed());

    let entries = build_menu_entries(ctx.developer_mode(false));
    assert!(has(&entries, MenuAction::MarkFolderAsRead));
    assert!(has(&entries, MenuAction::MuteFolder));
    assert!(has(&entries, MenuAction::FolderSettings));
    assert!(!has(&entries, MenuAction::CopyFolderId));

    let dev = build_menu_entries(ctx.developer_mode(true));
    assert!(has(&dev, MenuAction::CopyFolderId));
}

#[test]
fn category_menu_permission_gating_for_management_actions() {
    let no_perm = ContextMenuContext::new(MenuTargetKind::Category)
        .permissions(MenuPermissions::default())
        .developer_mode(true);
    let entries = build_menu_entries(no_perm);
    assert!(has(&entries, MenuAction::MarkCategoryAsRead));
    assert!(has(&entries, MenuAction::MuteCategory));
    assert!(has(&entries, MenuAction::CollapseCategory));
    assert!(!has(&entries, MenuAction::EditCategory));
    assert!(!has(&entries, MenuAction::DeleteCategory));
    assert!(has(&entries, MenuAction::CopyCategoryId));

    let with_perm = ContextMenuContext::new(MenuTargetKind::Category)
        .permissions(MenuPermissions {
            can_manage_channels: true,
            ..MenuPermissions::default()
        })
        .developer_mode(false);
    let entries_perm = build_menu_entries(with_perm);
    assert!(has(&entries_perm, MenuAction::EditCategory));
    assert!(has(&entries_perm, MenuAction::DeleteCategory));
    assert!(!has(&entries_perm, MenuAction::CopyCategoryId));
}

#[test]
fn text_channel_menu_has_full_matrix_and_permission_gating() {
    let base = ContextMenuContext::new(MenuTargetKind::TextChannel)
        .permissions(MenuPermissions::default())
        .developer_mode(false);
    let entries = build_menu_entries(base);
    assert!(has(&entries, MenuAction::MarkChannelAsRead));
    assert!(has(&entries, MenuAction::MuteChannel));
    assert!(has(&entries, MenuAction::ChannelNotificationSettings));
    assert!(has(&entries, MenuAction::CopyChannelLink));
    assert!(!has(&entries, MenuAction::EditChannel));
    assert!(!has(&entries, MenuAction::DeleteChannel));
    assert!(!has(&entries, MenuAction::CopyChannelId));

    let managed = ContextMenuContext::new(MenuTargetKind::TextChannel)
        .permissions(MenuPermissions {
            can_manage_channels: true,
            ..MenuPermissions::default()
        })
        .developer_mode(true);
    let managed_entries = build_menu_entries(managed);
    assert!(has(&managed_entries, MenuAction::EditChannel));
    assert!(has(&managed_entries, MenuAction::CloneChannel));
    assert!(has(&managed_entries, MenuAction::DeleteChannel));
    assert!(has(&managed_entries, MenuAction::CopyChannelLink));
    assert!(has(&managed_entries, MenuAction::CopyChannelId));
}

#[test]
fn voice_channel_menu_includes_invite_and_omits_copy_link() {
    let ctx = ContextMenuContext::new(MenuTargetKind::VoiceChannel)
        .permissions(MenuPermissions {
            can_manage_channels: true,
            ..MenuPermissions::default()
        })
        .developer_mode(true);
    let entries = build_menu_entries(ctx);
    assert!(has(&entries, MenuAction::MarkChannelAsRead));
    assert!(has(&entries, MenuAction::MuteChannel));
    assert!(has(&entries, MenuAction::CreateInvite));
    assert!(has(&entries, MenuAction::EditChannel));
    assert!(!has(&entries, MenuAction::CopyChannelLink), "voice channel menu should not include copy channel link");
    assert!(has(&entries, MenuAction::CopyChannelId));
}

#[test]
fn mute_and_collapse_labels_reflect_state() {
    let muted_guild = build_menu_entries(
        ContextMenuContext::new(MenuTargetKind::Guild)
            .permissions(MenuPermissions::all_allowed())
            .muted(true),
    );
    assert!(muted_guild.iter().any(|e| matches!(e, MenuEntry::Item { action: MenuAction::MuteGuild, label, .. } if *label == "Reativar servidor")));

    let collapsed_cat = build_menu_entries(
        ContextMenuContext::new(MenuTargetKind::Category)
            .permissions(MenuPermissions::default())
            .collapsed(true),
    );
    assert!(collapsed_cat.iter().any(|e| matches!(e, MenuEntry::Item { action: MenuAction::CollapseCategory, label, .. } if *label == "Expandir categoria")));
}

#[test]
fn permission_bits_map_to_menu_permissions() {
    let none = MenuPermissions::from_permission_bits(0);
    assert!(!none.can_manage_guild);
    assert!(!none.can_manage_channels);
    assert!(!none.can_view_channel);

    let manage_channels = MenuPermissions::from_permission_bits(1 << 4);
    assert!(!manage_channels.can_manage_guild);
    assert!(manage_channels.can_manage_channels);

    let admin = MenuPermissions::from_permission_bits(1 << 3);
    assert!(admin.can_manage_guild);
    assert!(admin.can_manage_channels);
    assert!(admin.can_view_channel);
}

#[test]
fn no_real_ids_are_embedded_in_labels() {
    // Guardrail: declarative labels must not contain numeric snowflakes.
    let ctx = ContextMenuContext::new(MenuTargetKind::TextChannel)
        .permissions(MenuPermissions::all_allowed())
        .developer_mode(true);
    for entry in build_menu_entries(ctx) {
        if let MenuEntry::Item { label, .. } = entry {
            assert!(
                !label.chars().any(|c| c.is_ascii_digit()),
                "label must not embed IDs/numbers: {label}"
            );
        }
    }
}
