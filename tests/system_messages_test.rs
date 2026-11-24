extern crate gpui_kit as gpui;

use discord_rs::chat::system_messages::{
    render_forwarded_card, render_system_message_row, should_group_system_with_regular,
    ForwardedMessage, ForwardedRenderOptions, MessageReferenceType, SystemLocale, SystemMessage,
    SystemMessageIcon, SystemMessageKind, SystemMessageTone,
};
use serde_json::json;

fn forwarded_payload() -> serde_json::Value {
    json!({
        "id": "900000000000000001",
        "channel_id": "800000000000000001",
        "guild_id": "800000000000000009",
        "type": 0,
        "content": "",
        "message_reference": {
            "type": 1,
            "message_id": "700000000000000001",
            "channel_id": "700000000000000002",
            "guild_id": "700000000000000003",
            "fail_if_not_exists": false
        },
        "message_snapshots": [
            {
                "message": {
                    "id": "700000000000000001",
                    "channel_id": "700000000000000002",
                    "guild_id": "700000000000000003",
                    "type": 0,
                    "content": "conteúdo original intacto",
                    "timestamp": "2026-11-20T12:34:56.000000+00:00",
                    "edited_timestamp": "2026-11-20T12:40:00.000000+00:00",
                    "author": {
                        "id": "600000000000000001",
                        "username": "orig_author",
                        "global_name": "Autora Original",
                        "avatar": "snapshot-avatar-hash"
                    },
                    "attachments": [
                        {
                            "id": "500000000000000001",
                            "filename": "captura.png",
                            "size": 1234,
                            "url": "https://cdn.discordapp.com/attachments/700000000000000002/500000000000000001/captura.png",
                            "proxy_url": "https://media.discordapp.net/attachments/700000000000000002/500000000000000001/captura.png",
                            "content_type": "image/png",
                            "width": 640,
                            "height": 480
                        }
                    ],
                    "embeds": [
                        {
                            "title": "Título real",
                            "description": "Descrição real",
                            "color": 5814783,
                            "image": { "url": "https://cdn.discordapp.com/embed.png" }
                        }
                    ],
                    "sticker_items": [
                        { "id": "400000000000000001", "name": "Sintética", "format_type": 1 }
                    ]
                }
            }
        ]
    })
}

fn author(id: &str, username: &str, global_name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "username": username,
        "global_name": global_name
    })
}

#[test]
fn forwarded_snapshot_preserves_author_content_attachments_embeds_and_stickers() {
    let payload = forwarded_payload();
    let forwarded = ForwardedMessage::from_json(&payload).expect("snapshot deve ser reconhecido");

    assert!(forwarded.is_forwarded());
    assert_eq!(forwarded.snapshots.len(), 1);
    assert_eq!(
        forwarded.reference.as_ref().unwrap().reference_type,
        MessageReferenceType::Forward
    );

    let snapshot = &forwarded.snapshots[0];
    assert_eq!(snapshot.message_id.as_deref(), Some("700000000000000001"));
    assert_eq!(
        snapshot.author.as_ref().unwrap().display_name(),
        "Autora Original"
    );
    assert_eq!(snapshot.author.as_ref().unwrap().avatar.as_deref(), Some("snapshot-avatar-hash"));
    assert_eq!(snapshot.content, "conteúdo original intacto");
    assert!(snapshot.timestamp.is_some());
    assert!(snapshot.edited_timestamp.is_some());
    assert_eq!(snapshot.attachments.len(), 1);
    assert_eq!(snapshot.attachments[0].filename, "captura.png");
    assert_eq!(snapshot.attachments[0].width, Some(640));
    assert_eq!(snapshot.embeds[0].title.as_deref(), Some("Título real"));
    assert_eq!(snapshot.sticker_items[0].name, "Sintética");

    let card = forwarded.card(ForwardedRenderOptions {
        locale: SystemLocale::PtBr,
        origin_accessible: true,
        age_restricted: false,
    });
    assert_eq!(card.label, "Encaminhado");
    assert_eq!(card.snapshots[0].author_name.as_deref(), Some("Autora Original"));
    assert_eq!(card.snapshots[0].content, "conteúdo original intacto");
    assert_eq!(card.snapshots[0].attachments.len(), 1);
    assert_eq!(card.snapshots[0].embeds.len(), 1);
    assert_eq!(card.snapshots[0].sticker_items.len(), 1);
    assert_eq!(
        card.origin.as_ref().unwrap().url,
        "https://discord.com/channels/700000000000000003/700000000000000002/700000000000000001"
    );
    assert!(card.restriction_notice.is_none());
    assert!(card.has_visible_content());
}

#[test]
fn forwarded_snapshot_without_origin_does_not_invent_a_link_or_content() {
    let mut payload = forwarded_payload();
    payload["message_reference"] = json!({ "type": 1 });

    let forwarded = ForwardedMessage::from_json(&payload).expect("snapshot ainda deve existir");
    assert!(forwarded.is_forwarded());

    let card = forwarded.card(ForwardedRenderOptions {
        locale: SystemLocale::EnUs,
        origin_accessible: true,
        age_restricted: false,
    });
    assert_eq!(card.label, "Forwarded");
    assert!(card.origin.is_none());
    assert_eq!(card.snapshots[0].content, "conteúdo original intacto");
    assert!(card.has_visible_content());
}

#[test]
fn forwarded_snapshot_age_restricted_hides_content_but_keeps_raw_snapshot() {
    let payload = forwarded_payload();
    let forwarded = ForwardedMessage::from_json(&payload).unwrap();

    let card = forwarded.card(ForwardedRenderOptions {
        locale: SystemLocale::EnUs,
        origin_accessible: true,
        age_restricted: true,
    });
    assert_eq!(
        card.restriction_notice.as_deref(),
        Some("Forwarded from an age-restricted channel")
    );
    assert_eq!(card.snapshots[0].content, "");
    assert!(card.snapshots[0].attachments.is_empty());
    assert!(card.snapshots[0].embeds.is_empty());
    assert!(card.snapshots[0].sticker_items.is_empty());
    assert!(!card.has_visible_content());

    assert_eq!(forwarded.snapshots[0].content, "conteúdo original intacto");
    assert_eq!(forwarded.snapshots[0].attachments[0].filename, "captura.png");
}

#[test]
fn join_system_message_is_localized_and_has_its_own_semantics() {
    let payload = json!({
        "id": "300000000000000001",
        "channel_id": "200000000000000001",
        "guild_id": "100000000000000001",
        "type": 7,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000002", "ana", "Ana")
    });

    let message = SystemMessage::from_json(&payload).unwrap();
    assert_eq!(message.kind, SystemMessageKind::Join);
    assert_eq!(message.actor_id(), Some("600000000000000002"));

    let pt = message.view(SystemLocale::PtBr);
    assert_eq!(pt.text, "Ana entrou no servidor!");
    assert_eq!(pt.icon, SystemMessageIcon::MemberJoin);
    assert_eq!(pt.tone, SystemMessageTone::Positive);

    let en = message.view(SystemLocale::EnUs);
    assert_eq!(en.text, "Ana joined the server!");
    assert_eq!(en.icon, SystemMessageIcon::MemberJoin);
}

#[test]
fn join_without_author_uses_only_the_real_payload_content() {
    let payload = json!({
        "id": "300000000000000002",
        "channel_id": "200000000000000001",
        "type": 7,
        "content": "texto real vindo do payload",
        "timestamp": "2026-11-24T10:00:00.000000+00:00"
    });

    let message = SystemMessage::from_json(&payload).unwrap();
    let view = message.view(SystemLocale::EnUs);
    assert_eq!(view.text, "texto real vindo do payload");
}

#[test]
fn boost_tiers_and_counts_are_localized() {
    let tier_one = json!({
        "id": "300000000000000010",
        "channel_id": "200000000000000001",
        "type": 8,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000003", "bea", "Bea")
    });
    let tier_two = json!({
        "id": "300000000000000011",
        "channel_id": "200000000000000001",
        "type": 9,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000003", "bea", "Bea")
    });
    let tier_three = json!({
        "id": "300000000000000012",
        "channel_id": "200000000000000001",
        "type": 10,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000003", "bea", "Bea")
    });
    let multiple = json!({
        "id": "300000000000000013",
        "channel_id": "200000000000000001",
        "type": 8,
        "content": "just boosted the server 3 times!",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000003", "bea", "Bea")
    });

    let one = SystemMessage::from_json(&tier_one).unwrap();
    assert_eq!(
        one.kind,
        SystemMessageKind::Boost { tier: 1, count: 1 }
    );
    assert_eq!(one.view(SystemLocale::PtBr).text, "Bea impulsionou o servidor!");
    assert_eq!(
        one.view(SystemLocale::EnUs).text,
        "Bea just boosted the server!"
    );

    let two = SystemMessage::from_json(&tier_two).unwrap();
    assert_eq!(two.kind, SystemMessageKind::Boost { tier: 2, count: 1 });
    assert_eq!(
        two.view(SystemLocale::PtBr).text,
        "Bea impulsionou o servidor para o Nível 2!"
    );
    assert_eq!(
        two.view(SystemLocale::EnUs).text,
        "Bea just boosted the server to Level 2!"
    );

    let three = SystemMessage::from_json(&tier_three).unwrap();
    assert_eq!(three.kind, SystemMessageKind::Boost { tier: 3, count: 1 });
    assert_eq!(three.view(SystemLocale::PtBr).icon, SystemMessageIcon::Boost);
    assert_eq!(three.view(SystemLocale::PtBr).tone, SystemMessageTone::Boost);

    let many = SystemMessage::from_json(&multiple).unwrap();
    assert_eq!(many.kind, SystemMessageKind::Boost { tier: 1, count: 3 });
    assert_eq!(
        many.view(SystemLocale::EnUs).text,
        "Bea just boosted the server 3 times!"
    );
}

#[test]
fn call_started_ended_and_missed_are_distinct() {
    let started = json!({
        "id": "300000000000000020",
        "channel_id": "200000000000000001",
        "type": 3,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000004", "caio", "Caio"),
        "call": { "participants": ["600000000000000004"], "ended_timestamp": null }
    });
    let ended = json!({
        "id": "300000000000000021",
        "channel_id": "200000000000000001",
        "type": 3,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000004", "caio", "Caio"),
        "call": {
            "participants": ["600000000000000004", "600000000000000099"],
            "ended_timestamp": "2026-11-24T10:01:05.000000+00:00"
        }
    });
    let missed = json!({
        "id": "300000000000000022",
        "channel_id": "200000000000000001",
        "type": 3,
        "content": "",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "author": author("600000000000000004", "caio", "Caio"),
        "call": {
            "participants": ["600000000000000004"],
            "ended_timestamp": "2026-11-24T10:01:05.000000+00:00"
        }
    });

    let started = SystemMessage::from_json(&started).unwrap();
    assert_eq!(
        started.kind,
        SystemMessageKind::Call {
            ended: false,
            missed: false,
            duration_seconds: None
        }
    );
    let started_view = started.view(SystemLocale::PtBr);
    assert_eq!(started_view.text, "Caio iniciou uma chamada.");
    assert_eq!(started_view.icon, SystemMessageIcon::CallStarted);
    assert_eq!(started_view.tone, SystemMessageTone::Call);

    let ended = SystemMessage::from_json_for_viewer(&ended, Some("600000000000000099")).unwrap();
    assert_eq!(
        ended.kind,
        SystemMessageKind::Call {
            ended: true,
            missed: false,
            duration_seconds: Some(65)
        }
    );
    let ended_view = ended.view(SystemLocale::PtBr);
    assert_eq!(ended_view.text, "Chamada encerrada. 1:05");
    assert_eq!(ended_view.icon, SystemMessageIcon::CallEnded);
    assert_eq!(ended_view.tone, SystemMessageTone::Muted);
    assert_eq!(ended_view.call_duration_seconds, Some(65));

    let missed = SystemMessage::from_json_for_viewer(&missed, Some("600000000000000099")).unwrap();
    assert_eq!(
        missed.kind,
        SystemMessageKind::Call {
            ended: true,
            missed: true,
            duration_seconds: Some(65)
        }
    );
    let missed_view = missed.view(SystemLocale::EnUs);
    assert_eq!(missed_view.text, "Missed call.");
    assert_eq!(missed_view.icon, SystemMessageIcon::CallEnded);
    assert_eq!(missed_view.tone, SystemMessageTone::Alert);
}

#[test]
fn pin_and_unpin_are_localized_and_semantically_distinct() {
    let pinned = json!({
        "id": "300000000000000030",
        "channel_id": "200000000000000001",
        "type": 6,
        "content": "pinned a message to this channel.",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "message_reference": { "message_id": "300000000000000099" },
        "author": author("600000000000000005", "duda", "Duda")
    });
    let unpinned = json!({
        "id": "300000000000000031",
        "channel_id": "200000000000000001",
        "type": 6,
        "content": "desafixou uma mensagem deste canal.",
        "timestamp": "2026-11-24T10:00:00.000000+00:00",
        "message_reference": { "message_id": "300000000000000099" },
        "author": author("600000000000000005", "duda", "Duda")
    });

    let pinned = SystemMessage::from_json(&pinned).unwrap();
    assert_eq!(pinned.kind, SystemMessageKind::Pin { pinned: true });
    let pt = pinned.view(SystemLocale::PtBr);
    assert_eq!(pt.text, "Duda fixou uma mensagem neste canal.");
    assert_eq!(pt.icon, SystemMessageIcon::Pin);
    assert_eq!(pt.tone, SystemMessageTone::Boost);
    assert_eq!(pt.target_message_id.as_deref(), Some("300000000000000099"));

    let unpinned = SystemMessage::from_json(&unpinned).unwrap();
    assert_eq!(unpinned.kind, SystemMessageKind::Pin { pinned: false });
    let en = unpinned.view(SystemLocale::EnUs);
    assert_eq!(en.text, "Duda unpinned a message from this channel.");
    assert_eq!(en.icon, SystemMessageIcon::Unpin);
    assert_eq!(en.tone, SystemMessageTone::Muted);
}

#[test]
fn unknown_system_type_keeps_the_real_content_and_never_uses_a_fake_author() {
    let payload = json!({
        "id": "300000000000000040",
        "channel_id": "200000000000000001",
        "type": 11,
        "content": "conteúdo legítimo de um tipo não mapeado",
        "timestamp": "2026-11-24T10:00:00.000000+00:00"
    });

    let message = SystemMessage::from_json(&payload).unwrap();
    assert_eq!(message.kind, SystemMessageKind::Other(11));
    assert!(message.actor.is_none());
    assert_eq!(message.kind.type_code(), 11);

    let view = message.view(SystemLocale::PtBr);
    assert_eq!(view.text, "conteúdo legítimo de um tipo não mapeado");
    assert_eq!(view.icon, SystemMessageIcon::Info);
    assert_eq!(view.tone, SystemMessageTone::Muted);
    assert!(view.actor_name.is_none());
    assert!(view.actor_id.is_none());
}

#[test]
fn system_messages_never_group_with_regular_messages() {
    assert!(!should_group_system_with_regular());
}

#[test]
fn renderers_build_without_inventing_data() {
    let forwarded = ForwardedMessage::from_json(&forwarded_payload()).unwrap();
    let _system = render_system_message_row(
        &SystemMessage::from_json(&json!({
            "id": "300000000000000050",
            "channel_id": "200000000000000001",
            "type": 7,
            "content": "",
            "timestamp": "2026-11-24T10:00:00.000000+00:00",
            "author": author("600000000000000006", "eva", "Eva")
        }))
        .unwrap(),
        SystemLocale::PtBr,
    );
    let _card = render_forwarded_card(
        &forwarded,
        ForwardedRenderOptions {
            locale: SystemLocale::EnUs,
            origin_accessible: false,
            age_restricted: false,
        },
    );
}
