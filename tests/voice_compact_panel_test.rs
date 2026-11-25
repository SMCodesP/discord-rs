use discord_rs::app::channel_sidebar::{focus_active_voice_stage, VoiceQualitySummary};
use discord_rs::app::state::{
    AppState, CategoryItemState, ChannelItemState, ChannelKind, GuildItemState, VoiceQuality,
};
use discord_rs::protocol::voice_session::{VoicePacketStats, VoiceSessionState};

fn channel(id: &str, name: &str, kind: ChannelKind) -> ChannelItemState {
    ChannelItemState {
        id: id.into(),
        name: name.into(),
        kind,
        topic: None,
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: false,
        last_message_id: None,
        last_read_message_id: None,
        is_group_dm: false,
        recipient_ids: Vec::new(),
        recipient_names: Vec::new(),
        user_limit: None,
        avatar_decoration_data: None,
        thread_metadata: None,
        default_tag_ids: Vec::new(),
        available_tags: Vec::new(),
        flags: 0,
        rate_limit_per_user: 0,
    }
}

fn guild() -> GuildItemState {
    GuildItemState {
        id: "100000000000000001".into(),
        name: "Servidor Sintético".into(),
        acronym: "SS".into(),
        icon_url: None,
        icon_hash: None,
        unread: false,
        mention_count: 0,
        categories: vec![CategoryItemState {
            id: "100000000000000010".into(),
            name: "Geral".into(),
            channels: vec![
                channel(
                    "200000000000000001",
                    "Voz Sintética",
                    ChannelKind::Voice,
                ),
                channel(
                    "300000000000000001",
                    "Texto Sintético",
                    ChannelKind::Text,
                ),
            ],
        }],
        emojis: Vec::new(),
        stickers: Vec::new(),
    }
}

fn connected_state() -> AppState {
    let mut state = AppState::new();
    state.current_user.id = "900000000000000001".into();
    state.guilds = vec![guild()];
    state.selected_guild_id = Some("100000000000000001".into());
    state.select_channel("200000000000000001".into());
    state
}

#[test]
fn quality_classes_use_rtt_and_loss_metrics() {
    assert_eq!(VoiceQuality::classify(None, None), VoiceQuality::Unknown);
    assert_eq!(
        VoiceQuality::classify(Some(40), Some(0.2)),
        VoiceQuality::Fine
    );
    assert_eq!(
        VoiceQuality::classify(None, Some(0.4)),
        VoiceQuality::Fine
    );
    assert_eq!(
        VoiceQuality::classify(Some(120), None),
        VoiceQuality::Average
    );
    assert_eq!(
        VoiceQuality::classify(None, Some(2.0)),
        VoiceQuality::Average
    );
    assert_eq!(
        VoiceQuality::classify(Some(300), Some(0.1)),
        VoiceQuality::Bad
    );
    assert_eq!(
        VoiceQuality::classify(Some(50), Some(8.0)),
        VoiceQuality::Bad
    );
}

#[test]
fn packet_stats_derive_real_loss_percentage() {
    let stats = VoicePacketStats {
        sent_packets: 12,
        recv_packets: 95,
        lost_packets: 5,
    };
    assert_eq!(stats.packet_loss_pct(), Some(5.0));

    let empty = VoicePacketStats::default();
    assert_eq!(empty.packet_loss_pct(), None);
}

#[test]
fn quality_summary_popout_only_shows_measured_metrics() {
    let mut state = connected_state();
    let call = state.active_voice_call.as_ref().unwrap();
    let summary = VoiceQualitySummary::from_call(call);
    assert_eq!(summary.quality, VoiceQuality::Unknown);
    assert_eq!(
        summary.rows(),
        vec![("Qualidade", "Aguardando".to_string())]
    );

    let call = state.active_voice_call.as_mut().unwrap();
    call.ping_ms = Some(48);
    call.packet_loss_pct = Some(0.4);
    call.codec = Some("Opus".into());
    call.sent_packets = 40;
    call.recv_packets = 996;
    call.lost_packets = 4;

    let summary = VoiceQualitySummary::from_call(call);
    assert_eq!(summary.quality, VoiceQuality::Fine);
    assert_eq!(
        summary.rows(),
        vec![
            ("Qualidade", "Boa".to_string()),
            ("RTT", "48 ms".to_string()),
            ("Perda de pacotes", "0.4%".to_string()),
            ("Codec", "Opus".to_string()),
        ]
    );
}

#[test]
fn navigating_to_another_channel_keeps_voice_bar_and_focus_returns_to_stage() {
    let mut state = connected_state();
    assert_eq!(
        state.active_voice_call.as_ref().unwrap().channel_id,
        "200000000000000001"
    );

    state.select_channel("300000000000000001".into());
    assert_eq!(
        state.selected_channel_id.as_deref(),
        Some("300000000000000001")
    );
    assert_eq!(
        state.active_voice_call.as_ref().unwrap().channel_id,
        "200000000000000001"
    );

    assert!(focus_active_voice_stage(&mut state));
    assert_eq!(
        state.selected_channel_id.as_deref(),
        Some("200000000000000001")
    );
    assert!(!focus_active_voice_stage(&mut state));
}

#[test]
fn disconnect_clears_voice_bar_state_without_ghosts() {
    let mut state = connected_state();
    state.update_voice_session_state(VoiceSessionState::Connected, Some(35));
    assert!(state.active_voice_call.is_some());

    state.disconnect_voice();
    assert!(state.active_voice_call.is_none());
    assert!(!focus_active_voice_stage(&mut state));
}
