extern crate gpui_kit as gpui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use discord_rs::app::state::{AppState, PresenceStatus};
use discord_rs::chat::message_list::{
    render_message_context_menu, render_message_row, MessageContextMenuConfig,
    MessageRowConfig,
};
use discord_rs::domain::guild::Role;
use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::cache::lazy_guilds::{
    apply_member_list_update, convert_member_to_item_state, parse_guild_member_list_update,
    resolve_member_role_color, resolve_member_role_group, LazyGuildCoordinator,
    MemberListMember,
};
use serde_json::json;

fn make_synthetic_author(id: &str, username: &str) -> Author {
    Author {
        id: id.to_string(),
        username: username.to_string(),
        global_name: Some(username.to_string()),
        avatar: None,
        bot: false,
        avatar_decoration_data: None,
    }
}

fn make_synthetic_message(id: &str, channel_id: &str, author_id: &str, content: &str) -> Message {
    Message::new(
        id,
        channel_id,
        make_synthetic_author(author_id, author_id),
        content,
    )
}

#[test]
fn test_context_menu_items_for_own_versus_other_message() {
    let deleted_called = Arc::new(AtomicBool::new(false));
    let deleted_clone = deleted_called.clone();

    // 1. Menu para mensagem própria (is_own = true): deve conter "Excluir Mensagem"
    let own_menu = render_message_context_menu(MessageContextMenuConfig {
        message_id: "msg-own".into(),
        message_content: "minha mensagem".into(),
        is_pinned: false,
        is_own: true,
        on_add_reaction: None,
        on_edit: None,
        on_reply: None,
        on_copy_text: None,
        on_copy_link: Some(Arc::new(|_m, _w, _cx| {})),
        on_copy_id: None,
        on_mark_unread: None,
        on_pin: None,
        on_delete: Some(Arc::new(move |_m, _w, _cx| {
            deleted_clone.store(true, Ordering::SeqCst);
        })),
        on_delete_shift: None,
        on_close: None,
    });
    drop(own_menu);

    // 2. Menu para mensagem de terceiro (is_own = false): NÃO deve conter opção de excluir
    let other_menu = render_message_context_menu(MessageContextMenuConfig {
        message_id: "msg-other".into(),
        message_content: "mensagem alheia".into(),
        is_pinned: false,
        is_own: false,
        on_add_reaction: None,
        on_edit: None,
        on_reply: None,
        on_copy_text: None,
        on_copy_link: Some(Arc::new(|_m, _w, _cx| {})),
        on_copy_id: None,
        on_mark_unread: None,
        on_pin: None,
        on_delete: Some(Arc::new(|_m, _w, _cx| {
            panic!("Excluir não deve ser invocado em mensagem alheia");
        })),
        on_delete_shift: None,
        on_close: None,
    });
    drop(other_menu);
}

#[test]
fn test_delete_message_lifecycle_no_optimistic_success() {
    let mut state = AppState::default();
    let cid = "channel-test-101";
    let mid = "msg-to-delete-404";

    let msg = make_synthetic_message(mid, cid, "user-author", "Mensagem de teste");
    state.inject_message(cid, msg);

    assert_eq!(state.get_active_messages().len(), 0); // nada selecionado ainda
    state.selected_channel_id = Some(cid.to_string());
    assert_eq!(state.get_active_messages().len(), 1);

    // Na arquitetura de Zero Mocks, a exclusão local só ocorre quando confirmada pela API/Gateway
    // Sem confirmação da API, o estado mantém a mensagem
    assert_eq!(state.get_active_messages()[0].id, mid);

    // Gateway envia confirmação legítima via TransportEvent::MessageDelete
    state.delete_message(cid, mid);
    assert_eq!(state.get_active_messages().len(), 0);
}

#[test]
fn test_delete_message_api_error_preserves_message() {
    let mut state = AppState::default();
    let cid = "channel-error-case";
    let mid = "msg-keep-on-error";

    state.inject_message(cid, make_synthetic_message(mid, cid, "user-me", "Fica intacta"));
    state.selected_channel_id = Some(cid.to_string());
    assert_eq!(state.get_active_messages().len(), 1);

    // Simula erro da API (ex: 403 Forbidden ou 500 Internal Error)
    // Mensagem NÃO é removida do estado
    let messages = state.messages_by_channel.get(cid).unwrap();
    assert!(messages.iter().any(|m| m.id == mid));
    assert_eq!(state.get_active_messages().len(), 1);
}

#[test]
fn test_author_role_color_resolution_hierarchy() {
    let guild_roles = vec![
        Role::new("role-admin", "Administrador").with_color(0xe91e63).with_position(10),
        Role::new("role-mod", "Moderador").with_color(0x2ecc71).with_position(5),
        Role::new("role-nocolor", "SemCor").with_color(0).with_position(20),
    ];

    // Cargo sem cor tem posição 20, mas color é 0 -> deve escolher o próximo com cor (Administrador, 0xe91e63)
    let assigned_roles = vec!["role-nocolor".to_string(), "role-admin".to_string(), "role-mod".to_string()];
    let resolved_color = resolve_member_role_color(&assigned_roles, &guild_roles);
    assert_eq!(resolved_color, 0xe91e63);

    // Quando não possui nenhum cargo com cor != 0, retorna 0
    let no_color_roles = vec!["role-nocolor".to_string()];
    let none_color = resolve_member_role_color(&no_color_roles, &guild_roles);
    assert_eq!(none_color, 0);

    // Hoisted group
    let hoisted = resolve_member_role_group(&assigned_roles, &guild_roles);
    assert_eq!(hoisted, None); // roles não tinham hoist=true configurado

    let hoisted_roles = vec![
        Role::new("r-staff", "STAFF").with_hoist(true).with_position(15),
    ];
    let staff_group = resolve_member_role_group(&["r-staff".into()], &hoisted_roles);
    assert_eq!(staff_group, Some("STAFF".into()));
}

#[test]
fn test_member_sidebar_grouping_and_color_mapping() {
    let guild_roles = vec![
        Role::new("role-staff", "STAFF").with_color(0x5865f2).with_hoist(true).with_position(10),
        Role::new("role-vip", "VIP").with_color(0xf1c40f).with_hoist(true).with_position(5),
    ];

    let member_staff = MemberListMember {
        id: "user-1".into(),
        username: "staff_member".into(),
        global_name: Some("Staff Member".into()),
        avatar: None,
        avatar_decoration_data: None,
        roles: vec!["role-staff".into()],
        presence: PresenceStatus::Online,
        activity: Some("Trabalhando no Discord-RS".into()),
        is_bot: false,
    };

    let member_state = convert_member_to_item_state(&member_staff, &guild_roles);
    assert_eq!(member_state.role_group, "STAFF");
    assert_eq!(member_state.role_color, 0x5865f2);
    assert_eq!(member_state.name, "Staff Member");

    let member_offline = MemberListMember {
        id: "user-2".into(),
        username: "offline_user".into(),
        global_name: None,
        avatar: None,
        avatar_decoration_data: None,
        roles: vec!["role-vip".into()],
        presence: PresenceStatus::Offline,
        activity: None,
        is_bot: false,
    };

    let offline_state = convert_member_to_item_state(&member_offline, &guild_roles);
    assert_eq!(offline_state.role_group, "VIP");
    assert_eq!(offline_state.role_color, 0xf1c40f);
}

#[test]
fn test_render_message_row_with_role_color() {
    let msg = make_synthetic_message("m-color", "c-1", "u-1", "Testando cor do autor");

    // Com role_color Some(0x2ecc71)
    let row_colored = render_message_row(
        &msg,
        MessageRowConfig {
            is_grouped: false,
            role_color: Some(0x2ecc71),
            on_react: None,
            on_reply: None,
            on_more: None,
            on_reaction_click: None,
            on_add_reaction: None,
        },
    );
    drop(row_colored);

    // Sem role_color (None) -> padrão text_titles
    let row_default = render_message_row(
        &msg,
        MessageRowConfig {
            is_grouped: false,
            role_color: None,
            on_react: None,
            on_reply: None,
            on_more: None,
            on_reaction_click: None,
            on_add_reaction: None,
        },
    );
    drop(row_default);
}

#[tokio::test]
async fn test_lazy_guild_opcode_14_and_member_list_sync() {
    let coordinator = LazyGuildCoordinator::new();
    let payload = coordinator
        .focus_guild_and_channel("g-101", "c-202")
        .await;

    assert_eq!(payload["op"].as_i64(), Some(14));
    assert_eq!(payload["d"]["guild_id"].as_str(), Some("g-101"));

    let update_json = json!({
        "op": 0,
        "t": "GUILD_MEMBER_LIST_UPDATE",
        "d": {
            "guild_id": "g-101",
            "id": "everyone",
            "member_count": 2,
            "online_count": 1,
            "groups": [{ "id": "staff", "count": 1 }],
            "ops": [
                {
                    "op": "SYNC",
                    "range": [0, 99],
                    "items": [
                        {
                            "member": {
                                "user": {
                                    "id": "u-mod",
                                    "username": "moderator",
                                    "global_name": "Mod",
                                    "avatar": null,
                                    "bot": false
                                },
                                "roles": ["r-staff"],
                                "presence": { "status": "online" }
                            }
                        }
                    ]
                }
            ]
        }
    });

    let parsed = parse_guild_member_list_update(&update_json).expect("Deve parsear com sucesso");
    assert_eq!(parsed.guild_id, "g-101");
    assert_eq!(parsed.ops.len(), 1);

    let roles = vec![Role::new("r-staff", "STAFF").with_color(0x3498db).with_hoist(true).with_position(5)];
    let mut members = Vec::new();
    apply_member_list_update(&mut members, &parsed, &roles);

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].id, "u-mod");
    assert_eq!(members[0].role_color, 0x3498db);
    assert_eq!(members[0].role_group, "STAFF");
}

#[tokio::test]
async fn test_transport_delete_message_lifecycle_mock_and_idle() {
    use discord_rs::protocol::idle_transport::IdleTransport;
    use discord_rs::protocol::mock_provider::MockDiscordProvider;
    use discord_rs::protocol::transport::{DiscordTransport, TransportEvent};

    // 1. IdleTransport deve retornar erro quando desconectado
    let idle = IdleTransport::new();
    let res = idle.delete_message("c-1", "m-1").await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().to_string(), "Não conectado ao Discord");

    // 2. MockDiscordProvider deve remover a mensagem do estado e emitir TransportEvent::MessageDelete
    let mock = MockDiscordProvider::new();
    let cid = "c-mock";

    let mut event_rx = mock.subscribe_events();
    let sent_msg = mock.send_message(cid, "Mensagem de teste no mock").await.unwrap();

    let del_res = mock.delete_message(cid, &sent_msg.id).await;
    assert!(del_res.is_ok());

    // Consome eventos até encontrar MessageDelete
    let mut found_delete = false;
    while let Ok(evt) = event_rx.try_recv() {
        if let TransportEvent::MessageDelete { channel_id, message_id } = evt {
            assert_eq!(channel_id, cid);
            assert_eq!(message_id, sent_msg.id);
            found_delete = true;
            break;
        }
    }
    assert!(found_delete, "TransportEvent::MessageDelete deve ter sido emitido");
}

#[test]
fn test_server_isolation_member_list_cleared_on_guild_switch() {
    let mut state = AppState::default();
    state.selected_guild_id = Some("guild-alpha".into());
    state.members.push(discord_rs::app::state::MemberItemState {
        id: "user-alpha-1".into(),
        name: "Alpha User".into(),
        avatar_text: "A".into(),
        presence: PresenceStatus::Online,
        role_group: "STAFF".into(),
        role_color: 0x5865f2,
        is_bot: false,
        activity: None,
        avatar_decoration_data: None,
    });
    assert_eq!(state.members.len(), 1);

    // Trocar de servidor deve limpar a lista de membros para não vazar dados
    state.select_guild(Some("guild-beta".into()));
    assert_eq!(state.selected_guild_id, Some("guild-beta".into()));
    assert_eq!(state.members.len(), 0, "A lista de membros deve ser limpa ao trocar de servidor");
}

#[test]
fn test_end_to_end_gateway_member_list_update_dispatch_to_app_state() {
    use discord_rs::protocol::cache::lazy_guilds::{cache_guild_roles, parse_guild_member_list_update};
    use discord_rs::protocol::transport::TransportEvent;

    let mut state = AppState::default();
    let gid = "guild-e2e-test";
    state.selected_guild_id = Some(gid.into());

    // Carga prévia de cargos da guild no cache canônico
    let roles = vec![
        Role::new("role-staff-hoist", "STAFF").with_color(0xe74c3c).with_hoist(true).with_position(10),
    ];
    cache_guild_roles(gid, roles);

    // Evento sintético com grupos do servidor e contagens exatas
    let event_json = json!({
        "op": 0,
        "t": "GUILD_MEMBER_LIST_UPDATE",
        "d": {
            "guild_id": gid,
            "id": "everyone",
            "member_count": 2,
            "online_count": 1,
            "groups": [
                { "id": "role-staff-hoist", "count": 1 },
                { "id": "online", "count": 1 }
            ],
            "ops": [
                {
                    "op": "SYNC",
                    "range": [0, 99],
                    "items": [
                        {
                            "group": { "id": "role-staff-hoist", "count": 1 }
                        },
                        {
                            "member": {
                                "user": {
                                    "id": "u-admin",
                                    "username": "admin_real",
                                    "global_name": "Admin Real",
                                    "avatar": null,
                                    "bot": false
                                },
                                "roles": ["role-staff-hoist"],
                                "presence": { "status": "online" }
                            }
                        },
                        {
                            "group": { "id": "online", "count": 1 }
                        },
                        {
                            "member": {
                                "user": {
                                    "id": "u-member",
                                    "username": "regular_member",
                                    "global_name": "Membro",
                                    "avatar": null,
                                    "bot": false
                                },
                                "roles": [],
                                "presence": { "status": "online" }
                            }
                        }
                    ]
                }
            ]
        }
    });

    let update = parse_guild_member_list_update(&event_json).expect("Parse do evento do gateway");
    state.handle_transport_event(TransportEvent::GuildMemberListUpdate(update));

    // A lista deve manter a ordem estrita do servidor com 4 itens: grupo, membro, grupo, membro
    assert_eq!(state.members.len(), 4);
    assert_eq!(state.members[0].id, "group:role-staff-hoist");
    assert_eq!(state.members[0].name, "STAFF");
    assert_eq!(state.members[0].activity.as_deref(), Some("1")); // Contagem do evento
    assert_eq!(state.members[0].role_color, 0xe74c3c);

    assert_eq!(state.members[1].id, "u-admin");
    assert_eq!(state.members[1].name, "Admin Real");
    assert_eq!(state.members[1].role_color, 0xe74c3c); // Cor herdada do cargo mais alto

    assert_eq!(state.members[2].id, "group:online");
    assert_eq!(state.members[2].name, "ONLINE");
    assert_eq!(state.members[2].activity.as_deref(), Some("1"));

    assert_eq!(state.members[3].id, "u-member");
    assert_eq!(state.members[3].name, "Membro");
    assert_eq!(state.members[3].role_color, 0); // Sem cargo -> cor 0

    // Testa operação de INVALIDATE
    let invalidate_json = json!({
        "op": 0,
        "t": "GUILD_MEMBER_LIST_UPDATE",
        "d": {
            "guild_id": gid,
            "id": "everyone",
            "ops": [
                {
                    "op": "INVALIDATE",
                    "range": [0, 99]
                }
            ]
        }
    });
    let inv_update = parse_guild_member_list_update(&invalidate_json).unwrap();
    state.handle_transport_event(TransportEvent::GuildMemberListUpdate(inv_update));
    assert_eq!(state.members.len(), 0, "INVALIDATE deve limpar a lista sincronizada");
}

#[test]
fn test_message_create_author_role_color_cached_when_outside_member_list() {
    use discord_rs::protocol::cache::lazy_guilds::{
        cache_author_role_color, cache_guild_roles, get_cached_author_role_color,
        resolve_member_role_color,
    };

    let gid = "guild-outside-color";
    let author_id = "user-not-in-members-slice";

    let roles = vec![
        Role::new("r-vip", "VIP").with_color(0x9b59b6).with_position(7),
        Role::new("r-default", "Membro").with_color(0).with_position(1),
    ];
    cache_guild_roles(gid, roles.clone());

    // Autor vem no payload REST ou MESSAGE_CREATE com cargos, mas não está em state.members
    let member_roles = vec!["r-default".to_string(), "r-vip".to_string()];
    let resolved_color = resolve_member_role_color(&member_roles, &roles);
    assert_eq!(resolved_color, 0x9b59b6);

    cache_author_role_color(gid, author_id, resolved_color);

    // O chat recupera a cor legítima mesmo para autores fora da fatia da lista de membros
    let cached = get_cached_author_role_color(gid, author_id);
    assert_eq!(cached, Some(0x9b59b6));

    // Autor sem nenhum cargo com cor recebe 0 (padrão)
    let no_color_roles = vec!["r-default".to_string()];
    let color_zero = resolve_member_role_color(&no_color_roles, &roles);
    assert_eq!(color_zero, 0);
}

#[test]
fn test_format_message_full_timestamp_pt_br() {
    use chrono::DateTime;
    use discord_rs::chat::message_list::format_message_full_timestamp;

    // 2026-10-11T15:30:00Z -> domingo, 11 de outubro de 2026 às 15:30
    let ts = DateTime::from_timestamp(1791732600, 0).unwrap();
    let formatted = format_message_full_timestamp(ts);
    assert!(formatted.contains("11 de outubro de 2026"), "Deve conter dia, mês e ano em pt-BR: {}", formatted);
    assert!(formatted.contains("15:30"), "Deve conter horário formatado: {}", formatted);
    assert!(formatted.contains("às"), "Deve conter 'às' no padrão canônico LLLL: {}", formatted);
}

#[test]
fn test_opcode_14_calculate_ranges_for_viewport() {
    use discord_rs::protocol::cache::lazy_guilds::calculate_ranges_for_viewport;

    // Viewport no topo [0, 50] -> apenas primeira faixa [[0, 99]]
    let r1 = calculate_ranges_for_viewport(0, 50);
    assert_eq!(r1, vec![[0, 99]]);

    // Viewport cruzando a fronteira dos 100 [80, 120] -> [[0, 99], [100, 199]]
    let r2 = calculate_ranges_for_viewport(80, 120);
    assert_eq!(r2, vec![[0, 99], [100, 199]]);

    // Viewport dentro da segunda faixa [140, 180] -> mantém [0, 99] + [100, 199]
    let r3 = calculate_ranges_for_viewport(140, 180);
    assert_eq!(r3, vec![[0, 99], [100, 199]]);

    // Viewport rolando até a terceira faixa [220, 260] -> mantém [0, 99] + [200, 299]
    let r4 = calculate_ranges_for_viewport(220, 260);
    assert_eq!(r4, vec![[0, 99], [200, 299]]);
}

#[test]
fn test_apply_member_list_update_preserves_multirange_members() {
    use discord_rs::protocol::cache::lazy_guilds::{
        apply_member_list_update, GuildMemberListUpdate, MemberListItem, MemberListMember,
        MemberListOp,
    };
    use discord_rs::app::state::{MemberItemState, PresenceStatus};

    let mut members: Vec<MemberItemState> = Vec::new();

    // 1. Simular primeiro SYNC de [0, 99] com 100 membros
    let mut initial_items = Vec::new();
    for i in 0..100 {
        initial_items.push(MemberListItem::Member(Box::new(MemberListMember {
            id: format!("user-{}", i),
            username: format!("User {}", i),
            global_name: None,
            avatar: None,
            avatar_decoration_data: None,
            roles: Vec::new(),
            presence: PresenceStatus::Online,
            activity: None,
            is_bot: false,
        })));
    }
    let update_chunk_1 = GuildMemberListUpdate {
        guild_id: "guild-multirange".into(),
        id: "everyone".into(),
        member_count: Some(200),
        online_count: Some(200),
        groups: Vec::new(),
        ops: vec![MemberListOp::Sync {
            range: [0, 99],
            items: initial_items,
        }],
    };
    apply_member_list_update(&mut members, &update_chunk_1, &[]);
    assert_eq!(members.len(), 100);

    // 2. Simular segundo SYNC de [100, 199] com mais 100 membros
    let mut second_items = Vec::new();
    for i in 100..200 {
        second_items.push(MemberListItem::Member(Box::new(MemberListMember {
            id: format!("user-{}", i),
            username: format!("User {}", i),
            global_name: None,
            avatar: None,
            avatar_decoration_data: None,
            roles: Vec::new(),
            presence: PresenceStatus::Online,
            activity: None,
            is_bot: false,
        })));
    }
    let update_chunk_2 = GuildMemberListUpdate {
        guild_id: "guild-multirange".into(),
        id: "everyone".into(),
        member_count: Some(200),
        online_count: Some(200),
        groups: Vec::new(),
        ops: vec![MemberListOp::Sync {
            range: [100, 199],
            items: second_items,
        }],
    };
    apply_member_list_update(&mut members, &update_chunk_2, &[]);
    assert_eq!(members.len(), 200, "Lista total deve conter 200 membros após segundo chunk");

    // 3. Simular atualização do primeiro chunk [0, 99]: NÃO pode destruir os membros [100..200]
    let mut refreshed_items = Vec::new();
    for i in 0..100 {
        refreshed_items.push(MemberListItem::Member(Box::new(MemberListMember {
            id: format!("user-{}-updated", i),
            username: format!("User {} Updated", i),
            global_name: None,
            avatar: None,
            avatar_decoration_data: None,
            roles: Vec::new(),
            presence: PresenceStatus::Online,
            activity: None,
            is_bot: false,
        })));
    }
    let update_chunk_refresh = GuildMemberListUpdate {
        guild_id: "guild-multirange".into(),
        id: "everyone".into(),
        member_count: Some(200),
        online_count: Some(200),
        groups: Vec::new(),
        ops: vec![MemberListOp::Sync {
            range: [0, 99],
            items: refreshed_items,
        }],
    };
    apply_member_list_update(&mut members, &update_chunk_refresh, &[]);
    assert_eq!(members.len(), 200, "Atualizar [0, 99] NÃO deve truncar membros das faixas superiores!");
    assert_eq!(members[0].name, "User 0 Updated");
    assert_eq!(members[150].name, "User 150");
}

#[tokio::test]
async fn test_lazy_guild_coordinator_singleton_and_viewport_ranges() {
    use discord_rs::protocol::cache::lazy_guilds::LazyGuildCoordinator;

    let coordinator = LazyGuildCoordinator::shared();

    // Focar canal e guild inicializa com [[0, 99]]
    let payload = coordinator.focus_guild_and_channel("g1", "c1").await;
    assert_eq!(payload["op"], 14);
    assert_eq!(payload["d"]["channels"]["c1"][0], serde_json::json!([0, 99]));

    // Atualizar faixas para o canal c1 ao rolar a lista
    let updated_payload = coordinator.update_viewport_ranges("g1", "c1", vec![[0, 99], [100, 199]]).await;
    assert_eq!(updated_payload["d"]["channels"]["c1"].as_array().unwrap().len(), 2);

    let active = coordinator.active_ranges_for_channel("c1").await;
    assert_eq!(active, vec![[0, 99], [100, 199]]);

    // Resetar faixas (ex.: troca de canal) reinicia para [[0, 99]]
    coordinator.reset_channel_ranges("c1").await;
    let reset_ranges = coordinator.active_ranges_for_channel("c1").await;
    assert_eq!(reset_ranges, vec![[0, 99]]);
}

#[tokio::test]
async fn test_viewport_scrolling_delivers_op14_payload_to_transport() {
    use discord_rs::app::state::AppState;
    use discord_rs::protocol::client_manager::ClientManager;
    use discord_rs::protocol::mock_provider::MockDiscordProvider;

    let mock = Arc::new(MockDiscordProvider::new());
    let (cm, _rx) = ClientManager::new_with_transport(mock.clone());

    let mut state = AppState::default();
    state.client_manager = Some(cm);
    state.selected_guild_id = Some("guild-scroll".to_string());
    state.selected_channel_id = Some("chan-scroll".to_string());

    // Chamar update_member_viewport
    state.update_member_viewport(vec![[0, 99], [100, 199]]);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let recorded = mock.get_recorded_viewport_updates().await;
    assert_eq!(recorded.len(), 1, "Opcode 14 deve ser entregue ao transporte");
    assert_eq!(recorded[0].0, "guild-scroll");
    assert_eq!(recorded[0].1, "chan-scroll");
    assert_eq!(recorded[0].2, vec![[0, 99], [100, 199]]);
}

#[test]
fn test_sparse_member_list_with_gaps_does_not_invent_fake_members() {
    use discord_rs::app::state::MemberItemState;
    use discord_rs::protocol::cache::lazy_guilds::{
        apply_member_list_update, GuildMemberListUpdate, MemberListItem, MemberListMember,
        MemberListOp,
    };

    let mut members: Vec<MemberItemState> = Vec::new();

    // Receber chunk [100, 199] sem ter os primeiros 0..99
    let mut items = Vec::new();
    for i in 100..200 {
        items.push(MemberListItem::Member(Box::new(MemberListMember {
            id: format!("user-{}", i),
            username: format!("User {}", i),
            global_name: None,
            avatar: None,
            avatar_decoration_data: None,
            roles: Vec::new(),
            presence: PresenceStatus::Online,
            activity: None,
            is_bot: false,
        })));
    }

    let update = GuildMemberListUpdate {
        guild_id: "guild-sparse".into(),
        id: "everyone".into(),
        member_count: Some(200),
        online_count: Some(200),
        groups: Vec::new(),
        ops: vec![MemberListOp::Sync {
            range: [100, 199],
            items,
        }],
    };

    apply_member_list_update(&mut members, &update, &[]);

    // A lista tem tamanho 200 para preservar o índice absoluto
    assert_eq!(members.len(), 200);

    // Os membros de 0..100 são posições esparsas 'unloaded' (não inventam dados fictícios)
    for i in 0..100 {
        assert!(members[i].is_unloaded(), "Slot {} deve estar descarregado (unloaded)", i);
        assert_eq!(members[i].id, format!("unloaded:{}", i));
    }

    // Os membros de 100..200 são os dados reais carregados
    for i in 100..200 {
        assert!(!members[i].is_unloaded(), "Slot {} deve ser membro real", i);
        assert_eq!(members[i].id, format!("user-{}", i));
    }
}

#[tokio::test]
async fn test_message_edit_via_client_manager_and_mock_no_optimistic_update() {
    use discord_rs::app::state::AppState;
    use discord_rs::protocol::client_manager::ClientManager;
    use discord_rs::protocol::mock_provider::MockDiscordProvider;
    use discord_rs::protocol::transport::{DiscordTransport, TransportEvent};

    let mock = Arc::new(MockDiscordProvider::new());
    mock.connect().await.unwrap();

    let (cm, _rx) = ClientManager::new_with_transport(mock.clone());
    let mut state = AppState::default();
    state.client_manager = Some(cm.clone());
    state.selected_channel_id = Some("chan-br-general".to_string());

    // Buscar mensagens do canal pelo mock
    let msgs = mock.get_messages("chan-br-general", 50).await.unwrap();
    state.set_channel_messages("chan-br-general", msgs);

    let initial = state.get_active_messages().first().unwrap().content.clone();

    // Editar mensagem via ClientManager
    let edited = cm.edit_message("chan-br-general", "msg-br-101", "Texto editado com sucesso").await.unwrap();
    assert_eq!(edited.content, "Texto editado com sucesso");

    // Verificar que o mock registrou a edição
    let edits = mock.get_recorded_message_edits().await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0], ("chan-br-general".to_string(), "msg-br-101".to_string(), "Texto editado com sucesso".to_string()));

    // Zero Mocks: a tela só atualiza quando TransportEvent::MessageUpdate for processado
    state.handle_transport_event(TransportEvent::MessageUpdate(edited));
    let current = state.get_active_messages().first().unwrap().content.clone();
    assert_eq!(current, "Texto editado com sucesso");
    assert_ne!(initial, current);
}

#[tokio::test]
async fn test_pin_and_unpin_message_via_client_manager() {
    use discord_rs::protocol::client_manager::ClientManager;
    use discord_rs::protocol::mock_provider::MockDiscordProvider;
    use discord_rs::protocol::transport::DiscordTransport;

    let mock = Arc::new(MockDiscordProvider::new());
    mock.connect().await.unwrap();
    let (cm, _rx) = ClientManager::new_with_transport(mock.clone());

    // Fixar mensagem
    cm.pin_message("chan-br-general", "msg-br-101").await.unwrap();
    let pins = mock.get_recorded_pins().await;
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0], ("chan-br-general".to_string(), "msg-br-101".to_string()));

    // Desafixar mensagem
    cm.unpin_message("chan-br-general", "msg-br-101").await.unwrap();
    let msgs = mock.get_messages("chan-br-general", 50).await.unwrap();
    let msg = msgs.iter().find(|m| m.id == "msg-br-101").unwrap();
    assert!(!msg.pinned, "Mensagem deve estar desafixada após unpin");
}

#[tokio::test]
async fn test_request_missing_author_colors_opcode_8_deduplication() {
    use discord_rs::app::state::AppState;
    use discord_rs::domain::message::{Author, Message};
    use discord_rs::protocol::client_manager::ClientManager;
    use discord_rs::protocol::mock_provider::MockDiscordProvider;

    let mock = Arc::new(MockDiscordProvider::new());
    let (cm, _rx) = ClientManager::new_with_transport(mock.clone());

    let mut state = AppState::default();
    state.client_manager = Some(cm);
    state.selected_guild_id = Some("guild-roles".to_string());
    state.selected_channel_id = Some("chan-1".to_string());

    let author1 = Author {
        id: "user-unknown-1".to_string(),
        username: "unknown1".to_string(),
        global_name: None,
        avatar: None,
        bot: false,
        avatar_decoration_data: None,
    };
    let author2 = Author {
        id: "user-unknown-2".to_string(),
        username: "unknown2".to_string(),
        global_name: None,
        avatar: None,
        bot: false,
        avatar_decoration_data: None,
    };

    let msg1 = Message::new("m1", "chan-1", author1.clone(), "hello 1");
    let msg2 = Message::new("m2", "chan-1", author1.clone(), "hello 1 duplicate");
    let msg3 = Message::new("m3", "chan-1", author2.clone(), "hello 2");

    // Injetar mensagens de histórico com autores não cacheados
    state.set_channel_messages("chan-1", vec![msg1, msg2, msg3]);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let reqs = mock.get_recorded_member_requests().await;
    assert_eq!(reqs.len(), 1, "Deve enviar exatamente 1 requisição em lote");
    assert_eq!(reqs[0].0, "guild-roles");
    assert_eq!(reqs[0].1.len(), 2, "Usuários devem estar deduplicados");
    assert!(reqs[0].1.contains(&"user-unknown-1".to_string()));
    assert!(reqs[0].1.contains(&"user-unknown-2".to_string()));
}



