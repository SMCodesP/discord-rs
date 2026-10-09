use std::collections::HashMap;
use discord_rs::telemetry::{
    ComponentScope, DivergenceScoper, InteractionTracker, ParityCategory,
    ScreenKind, ScreenTracker, TelemetryConfig, TelemetryReporter, TelemetryService,
};

#[test]
fn test_telemetry_service_initialization_and_dispatch() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let config = TelemetryConfig {
            enabled: true,
            channel_capacity: 512,
            history_capacity: 128,
            auto_export_interval_secs: 3600,
            export_json_path: None,
            export_markdown_path: None,
        };

        let (_service, handle) = TelemetryService::start(config);
        assert!(handle.is_active());

        // 1. Test screen tracking
        {
            let mut meta = HashMap::new();
            meta.insert("guild_id".into(), "guild-123".into());
            let guard = handle.start_screen_transition(
                ScreenKind::Guild {
                    id: "guild-123".into(),
                    name: "Rust Devs".into(),
                },
                meta,
            );
            guard.complete(None);
        }

        assert_eq!(handle.screen().total_transitions(), 1);

        // 2. Test interaction tracking
        handle.track_button_click("btn-settings", "User Settings", 1500);
        handle.track_channel_select("chan-general", "geral", "text", 2200);
        handle.track_message_send("chan-general", 50, true, 4500);
        handle.track_reaction("👍", "msg-10", true, 800);
        handle.track_tab_change("Aparência", 1200);
        handle.track_scroll(-10.0, 150.0, 300);

        let throughput = handle.interaction().get_throughput_metrics();
        assert_eq!(throughput.total_interactions, 6);
        assert_eq!(throughput.total_button_clicks, 1);
        assert_eq!(throughput.total_channel_selects, 1);
        assert_eq!(throughput.total_messages_sent, 1);
        assert_eq!(throughput.total_reactions_toggled, 1);
        assert_eq!(throughput.total_scroll_events, 1);
        assert_eq!(throughput.dropped_telemetry_events, 0);

        // 3. Test divergence evaluation
        let score = handle.current_parity_score();
        assert!(score > 0.0 && score <= 100.0);

        // 4. Test report generation
        let report = TelemetryReporter::build_report(handle.screen(), handle.interaction());
        assert_eq!(report.total_features_mapped, ComponentScope::all().len());
        assert!(!report.component_audits.is_empty());
        assert!(!report.prioritized_corrections.is_empty());

        let markdown = TelemetryReporter::render_markdown(&report);
        assert!(markdown.contains("Discord RS — Relatório Oficial de Auditoria e Paridade em Rust"));
        assert!(markdown.contains("Score de Paridade Geral"));
    });
}

#[test]
fn test_canonical_matrix_and_sla_thresholds() {
    let screen_tracker = ScreenTracker::new(64, None);
    let interaction_tracker = InteractionTracker::new(64, None);

    // Mapeamento de componentes
    let all = ComponentScope::all();
    assert!(all.len() >= 15);

    let chat_comps = ComponentScope::by_category(ParityCategory::ChatMessaging);
    assert!(chat_comps.iter().any(|c| c.id == "message_list"));
    assert!(chat_comps.iter().any(|c| c.id == "markdown_parser"));

    let eval = DivergenceScoper::evaluate(&screen_tracker, &interaction_tracker);
    assert!(eval.counts.full_parity >= 8);
    assert!(eval.counts.performance_bottleneck >= 2);
    assert!(eval.counts.missing_feature >= 2);
}

#[test]
fn test_export_official_reports() {
    let screen_tracker = ScreenTracker::new(128, None);
    let interaction_tracker = InteractionTracker::new(128, None);

    // Simula transições e interações representativas
    let guard1 = screen_tracker.start_transition(
        ScreenKind::Guild {
            id: "guild-official".into(),
            name: "Comunidade Discord RS".into(),
        },
        HashMap::new(),
    );
    guard1.complete(None);

    let guard2 = screen_tracker.start_transition(
        ScreenKind::TextChannel {
            id: "chan-general".into(),
            name: "geral".into(),
            guild_id: Some("guild-official".into()),
        },
        HashMap::new(),
    );
    guard2.complete(None);

    interaction_tracker.track_button_click("btn-settings", "Configurações", 1200);
    interaction_tracker.track_channel_select("chan-general", "geral", "text", 2100);
    interaction_tracker.track_message_send("chan-general", 64, true, 4200);
    interaction_tracker.track_reaction("🚀", "msg-1", true, 950);
    interaction_tracker.track_scroll(-20.0, 400.0, 350);

    let (json_path, md_path) = TelemetryReporter::export_all(&screen_tracker, &interaction_tracker)
        .expect("Falha ao exportar relatórios de telemetria");

    assert!(json_path.exists());
    assert!(md_path.exists());
}

#[test]
fn test_visual_comparator_and_reports() {
    let report = discord_rs::telemetry::VisualComparator::run_comparison();
    assert!(report.summary.overall_visual_parity_score > 90.0);
    assert_eq!(report.summary.dimensions_match_pct, 100.0);
    assert_eq!(report.summary.colors_match_pct, 100.0);

    // Exporta os relatórios de diff visual 1:1
    let (json_path, md_path) = discord_rs::telemetry::VisualComparator::export_all()
        .expect("Falha ao exportar relatórios de diff visual");

    assert!(json_path.exists());
    assert!(md_path.exists());

    let md_content = std::fs::read_to_string(&md_path).unwrap();
    assert!(md_content.contains("Discord RS — Auditoria de Paridade Visual e Comparação 1:1"));
    assert!(md_content.contains("72.0px"));
    assert!(md_content.contains("240.0px"));
    assert!(md_content.contains("360.0px"));
    assert!(md_content.contains("48.0px"));
    assert!(md_content.contains("68.0px"));
    assert!(md_content.contains("#1e1f22"));
    assert!(md_content.contains("#2b2d31"));
    assert!(md_content.contains("#5865f2"));
    assert!(md_content.contains("#23a55a"));
    assert!(md_content.contains("Active Now Panel (Painel Ativo Agora)"));
    assert!(md_content.contains("Message Requests Row (Linha de Solicitação de Mensagem)"));
    assert!(md_content.contains("spotify_green"));
    assert!(md_content.contains("bg_active_now"));
}
