use std::fs;
use std::path::PathBuf;
use chrono::Utc;

use super::divergence_scoper::DivergenceScoper;
use super::interaction_tracker::InteractionTracker;
use super::screen_tracker::ScreenTracker;
use super::types::{
    LatencyMetrics, ParityAuditReport, PriorityLevel,
};

/// Gerador e exportador de relatórios de telemetria e paridade
pub struct TelemetryReporter;

impl TelemetryReporter {
    /// Caminho padrão para o relatório JSON em ~/.config/discord-rs/parity_report.json
    pub fn default_json_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("discord-rs")
            .join("parity_report.json")
    }

    /// Caminho padrão para o relatório Markdown em docs/PARITY_AUDIT.md
    pub fn default_markdown_path() -> PathBuf {
        PathBuf::from("docs").join("PARITY_AUDIT.md")
    }

    /// Constrói a estrutura completa de auditoria com métricas atuais
    pub fn build_report(
        screen_tracker: &ScreenTracker,
        interaction_tracker: &InteractionTracker,
    ) -> ParityAuditReport {
        let evaluation = DivergenceScoper::evaluate(screen_tracker, interaction_tracker);
        let latency_summary = screen_tracker.calculate_latency_summary();
        let throughput = interaction_tracker.get_throughput_metrics();

        let latency_metrics = LatencyMetrics {
            screen_mount_p50_us: latency_summary.p50_us,
            screen_mount_p95_us: latency_summary.p95_us,
            screen_mount_p99_us: latency_summary.p99_us,
            screen_mount_max_us: latency_summary.max_us,
            average_interaction_response_us: interaction_tracker.average_latency_us(),
            total_screen_transitions: screen_tracker.total_transitions(),
            total_mount_sla_violations: screen_tracker.total_sla_violations(),
        };

        ParityAuditReport {
            generated_at: Utc::now(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "Linux x86_64".into(),
            compositor: std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_else(|_| "Wayland / X11".into()),
            global_parity_score: evaluation.global_parity_score,
            total_features_mapped: evaluation.audits.len(),
            counts: evaluation.counts,
            latency_metrics,
            throughput_metrics: throughput,
            component_audits: evaluation.audits,
            prioritized_corrections: evaluation.prioritized_corrections,
        }
    }

    /// Exporta o relatório completo em formato JSON para o disco
    pub fn export_json(
        report: &ParityAuditReport,
        target_path: Option<PathBuf>,
    ) -> Result<PathBuf, std::io::Error> {
        let path = target_path.unwrap_or_else(Self::default_json_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(report)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, json)?;
        Ok(path)
    }

    /// Renderiza e exporta o relatório de auditoria em Markdown rico
    pub fn export_markdown(
        report: &ParityAuditReport,
        target_path: Option<PathBuf>,
    ) -> Result<PathBuf, std::io::Error> {
        let path = target_path.unwrap_or_else(Self::default_markdown_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let md = Self::render_markdown(report);
        fs::write(&path, md)?;
        Ok(path)
    }

    /// Exporta simultaneamente JSON e Markdown
    pub fn export_all(
        screen_tracker: &ScreenTracker,
        interaction_tracker: &InteractionTracker,
    ) -> Result<(PathBuf, PathBuf), std::io::Error> {
        let report = Self::build_report(screen_tracker, interaction_tracker);
        let json_path = Self::export_json(&report, None)?;
        let md_path = Self::export_markdown(&report, None)?;
        Ok((json_path, md_path))
    }

    /// Gera o conteúdo textual formatado em Markdown
    pub fn render_markdown(report: &ParityAuditReport) -> String {
        let mut out = String::with_capacity(16 * 1024);

        out.push_str("# Discord RS — Relatório Oficial de Auditoria e Paridade em Rust\n\n");
        out.push_str("> **Documento Gerado Automaticamente pelo Serviço Nativo de Telemetria e Observabilidade**  \n");
        out.push_str(&format!("> **Data e Hora:** {} UTC  \n", report.generated_at.format("%Y-%m-%d %H:%M:%S")));
        out.push_str(&format!("> **Versão do Cliente:** v{} | **Ambiente:** {} ({})\n\n", report.app_version, report.platform, report.compositor));
        out.push_str("---\n\n");

        // 0. ARQUITETURA DO SERVIÇO
        out.push_str("## 0. Arquitetura do Serviço Nativo de Telemetria\n\n");
        out.push_str("O subsistema de telemetria e auditoria de paridade opera de forma 100% desacoplada do loop de renderização do GPUI, garantindo zero sobrecarga de CPU no hot path gráfico:\n\n");
        out.push_str("```mermaid\n");
        out.push_str("flowchart TD\n");
        out.push_str("    subgraph UIThread[\"GPUI Render Thread (Hot Path 120 FPS)\"]\n");
        out.push_str("        UI[\"Componentes GPUI: GuildRail, ChatView, Modals, Buttons\"]\n");
        out.push_str("        TG[\"TransitionGuard (Medição RAII)\"]\n");
        out.push_str("        IT[\"InteractionTracker (Hot Path)\"]\n");
        out.push_str("        AtomicCounters[\"Atomics Lock-Free: AtomicU64, AtomicBool\"]\n");
        out.push_str("    end\n\n");
        out.push_str("    subgraph AsyncBridge[\"Ponte Não-Bloqueante (Zero-Overhead)\"]\n");
        out.push_str("        MpscScreen[\"tokio::sync::mpsc::channel ScreenEvent\"]\n");
        out.push_str("        MpscInteract[\"tokio::sync::mpsc::channel InteractionEvent\"]\n");
        out.push_str("        TrySend[\"try_send() com Contador Atômico de Drops\"]\n");
        out.push_str("    end\n\n");
        out.push_str("    subgraph BgWorker[\"Background Tokio Runtime Task\"]\n");
        out.push_str("        WorkerLoop[\"Worker Loop de Agregação em Lote\"]\n");
        out.push_str("        RingBuffers[\"Ring Buffers Circulares em Memória\"]\n");
        out.push_str("        Divergence[\"DivergenceScoper Engine\"]\n");
        out.push_str("        CompMatrix[\"ComponentScope Canonical Matrix\"]\n");
        out.push_str("    end\n\n");
        out.push_str("    subgraph Persistence[\"Exportação & Diagnóstico\"]\n");
        out.push_str("        JsonRep[\"~/.config/discord-rs/parity_report.json\"]\n");
        out.push_str("        MdRep[\"docs/PARITY_AUDIT.md\"]\n");
        out.push_str("    end\n\n");
        out.push_str("    UI -->|start_transition| TG\n");
        out.push_str("    UI -->|track_click, scroll, send| IT\n");
        out.push_str("    IT -->|fetch_add lock-free| AtomicCounters\n");
        out.push_str("    TG -->|complete RAII| TrySend\n");
        out.push_str("    IT -->|try_send| TrySend\n");
        out.push_str("    TrySend --> MpscScreen\n");
        out.push_str("    TrySend --> MpscInteract\n");
        out.push_str("    MpscScreen --> WorkerLoop\n");
        out.push_str("    MpscInteract --> WorkerLoop\n");
        out.push_str("    WorkerLoop --> RingBuffers\n");
        out.push_str("    WorkerLoop --> Divergence\n");
        out.push_str("    CompMatrix --> Divergence\n");
        out.push_str("    Divergence --> JsonRep\n");
        out.push_str("    Divergence --> MdRep\n");
        out.push_str("```\n\n");
        out.push_str("- **Zero Bloqueio**: Emissões no hot loop utilizam `try_send()`. Caso o buffer atinja a capacidade máxima, o evento é descartado com incremento de contador atômico (`dropped_telemetry_events`), impedindo que a thread de renderização pause.\n");
        out.push_str("- **SLA de Frame Rate**: Orçamento de 8.33 ms (120 FPS) e tolerância máxima de 16.66 ms (60 FPS).\n\n");
        out.push_str("---\n\n");

        // 1. SUMÁRIO EXECUTIVO & ÍNDICE DE PARIDADE
        out.push_str("## 1. Sumário Executivo & Índice de Paridade Global\n\n");
        out.push_str(&format!("### Score de Paridade Geral: **{:.1}%**\n\n", report.global_parity_score));

        let progress_bars = (report.global_parity_score / 5.0).round() as usize;
        let empty_bars = 20usize.saturating_sub(progress_bars);
        out.push_str(&format!("`[{}{}] {:.1}%`\n\n", "█".repeat(progress_bars), "░".repeat(empty_bars), report.global_parity_score));

        out.push_str("| Categoria de Status | Quantidade de Recursos | Percentual da Matriz | Impacto no Score |\n");
        out.push_str("| :--- | :---: | :---: | :---: |\n");
        out.push_str(&format!("| **FullParity (100% Idêntico)** | {} | {:.1}% | 1.0x (Total) |\n",
            report.counts.full_parity,
            (report.counts.full_parity as f64 / report.total_features_mapped as f64) * 100.0));
        out.push_str(&format!("| **Divergent (Parcial / Ajuste Necessário)** | {} | {:.1}% | 0.5x (Parcial) |\n",
            report.counts.divergent,
            (report.counts.divergent as f64 / report.total_features_mapped as f64) * 100.0));
        out.push_str(&format!("| **PerformanceBottleneck (SLA Violado)** | {} | {:.1}% | 0.35x (Degradado) |\n",
            report.counts.performance_bottleneck,
            (report.counts.performance_bottleneck as f64 / report.total_features_mapped as f64) * 100.0));
        out.push_str(&format!("| **MissingFeature (Não Implementado)** | {} | {:.1}% | 0.0x (Pendente) |\n",
            report.counts.missing_feature,
            (report.counts.missing_feature as f64 / report.total_features_mapped as f64) * 100.0));
        out.push_str(&format!("| **TOTAL DE RECURSOS CANÔNICOS** | **{}** | **100.0%** | — |\n\n", report.total_features_mapped));

        out.push_str("---\n\n");

        // 2. TELEMETRIA OPERACIONAL E LATÊNCIA EM TEMPO REAL
        out.push_str("## 2. Telemetria Operacional & Orçamento de Latência (SLA)\n\n");
        out.push_str("Monitoramento de latência e taxas de throughput capturados via atomics lock-free no hot path:\n\n");

        out.push_str("| Métrica de Desempenho | Valor Medido | Meta SLA (120 FPS) | Limite Tolerável (60 FPS) | Situação |\n");
        out.push_str("| :--- | :---: | :---: | :---: | :---: |\n");

        let p50_ms = report.latency_metrics.screen_mount_p50_us as f64 / 1000.0;
        let p95_ms = report.latency_metrics.screen_mount_p95_us as f64 / 1000.0;
        let p99_ms = report.latency_metrics.screen_mount_p99_us as f64 / 1000.0;
        let avg_interact_ms = report.latency_metrics.average_interaction_response_us as f64 / 1000.0;

        out.push_str(&format!("| **Montagem de Tela (p50)** | {:.2} ms | < 8.33 ms | < 16.66 ms | {} |\n",
            p50_ms, if p50_ms <= 8.33 { "✅ Ótimo" } else { "⚠️ Atenção" }));
        out.push_str(&format!("| **Montagem de Tela (p95)** | {:.2} ms | < 8.33 ms | < 16.66 ms | {} |\n",
            p95_ms, if p95_ms <= 16.66 { "✅ Conforme" } else { "❌ Violação" }));
        out.push_str(&format!("| **Montagem de Tela (p99)** | {:.2} ms | < 12.00 ms | < 25.00 ms | {} |\n",
            p99_ms, if p99_ms <= 25.00 { "✅ Conforme" } else { "❌ Pico Crítico" }));
        out.push_str(&format!("| **Resposta a Interações (Média)** | {:.2} ms | < 8.33 ms | < 16.66 ms | {} |\n",
            avg_interact_ms, if avg_interact_ms <= 8.33 { "✅ Excelente" } else { "⚠️ Lento" }));
        out.push_str(&format!("| **Transições Totais Registradas** | {} transições | — | — | ℹ️ Ativo |\n",
            report.latency_metrics.total_screen_transitions));
        out.push_str(&format!("| **Violações de SLA de Montagem** | {} eventos | 0 eventos | < 5% do total | {} |\n",
            report.latency_metrics.total_mount_sla_violations,
            if report.latency_metrics.total_mount_sla_violations == 0 { "✅ Zero Violações" } else { "⚠️ Ocorrências Detectadas" }));
        out.push_str(&format!("| **Interações de Usuário Monitoradas** | {} eventos | — | — | ℹ️ Ativo |\n",
            report.throughput_metrics.total_interactions));
        out.push_str(&format!("| **Eventos de Telemetria Descartados (Drops)** | {} eventos | 0 drops | 0 drops | {} |\n\n",
            report.throughput_metrics.dropped_telemetry_events,
            if report.throughput_metrics.dropped_telemetry_events == 0 { "✅ Zero Drops (Perfeito)" } else { "⚠️ Perda de Buffers" }));

        out.push_str("---\n\n");

        // 3. MATRIZ AUDITADA DE COMPONENTES CANÔNICOS
        out.push_str("## 3. Matriz Canônica de Componentes Auditados\n\n");
        out.push_str("| Componente / Recurso | Categoria | Status de Paridade | Comportamento Atual no discord-rs | Latência Medida | Arquivo Fonte |\n");
        out.push_str("| :--- | :--- | :---: | :--- | :---: | :--- |\n");

        for audit in &report.component_audits {
            let status_badge = match audit.status {
                super::types::ParityStatus::FullParity => "`FullParity` ✅",
                super::types::ParityStatus::Divergent => "`Divergent` ⚠️",
                super::types::ParityStatus::PerformanceBottleneck => "`Bottleneck` 🛑",
                super::types::ParityStatus::MissingFeature => "`Missing` ❌",
            };
            let lat_str = audit.measured_latency_us
                .map(|u| format!("{:.2} ms", u as f64 / 1000.0))
                .unwrap_or_else(|| "—".into());

            out.push_str(&format!("| **{}** | {} | {} | {} | {} | `{}` |\n",
                audit.name,
                audit.category.label(),
                status_badge,
                audit.current_behavior,
                lat_str,
                audit.source_location
            ));
        }

        out.push_str("\n---\n\n");

        // 4. LISTA PRIORIZADA DE CORREÇÕES (P0 A P3)
        out.push_str("## 4. Lista Priorizada de Correções e Plano de Ação\n\n");
        out.push_str("Backlog técnico priorizado pela equipe de arquitetura com cálculo do impacto de cada item no score global:\n\n");

        for (idx, item) in report.prioritized_corrections.iter().enumerate() {
            let priority_badge = match item.priority {
                PriorityLevel::P0Blocker | PriorityLevel::P0_Blocker => "🔴 **P0 (Bloqueador)**",
                PriorityLevel::P1Critical | PriorityLevel::P1_Critical => "🟠 **P1 (Crítico)**",
                PriorityLevel::P2Major | PriorityLevel::P2_Major => "🟡 **P2 (Maior)**",
                PriorityLevel::P3Minor | PriorityLevel::P3_Minor => "🟢 **P3 (Menor)**",
            };

            out.push_str(&format!("### {}. {} — {}\n", idx + 1, priority_badge, item.title));
            out.push_str(&format!("- **Arquivo Alvo:** `{}`\n", item.target_file));
            out.push_str(&format!("- **Impacto no Score de Paridade:** `+{:.2}%`\n", item.parity_score_delta));
            out.push_str(&format!("- **Diagnóstico:** {}\n", item.description));
            out.push_str(&format!("- **Plano de Mitigação Arquitetural:** {}\n\n", item.mitigation_plan));
        }

        out.push_str("---\n\n");

        // 5. RECOMENDAÇÕES ARQUITETURAIS PARA O TIME DE ENGENHARIA
        out.push_str("## 5. Recomendações Estruturais da Especialidade em Telemetria\n\n");
        out.push_str("1. **Desacoplamento do AppState na Renderização**: Adotar padrão de stores isolados (ex: `TextInputState` independente) para impedir re-renderização em cascata das 7 views filhas ao digitar.\n");
        out.push_str("2. **Virtualização Linear de Mensagens**: Substituir a iteração direta de até 100 mensagens por `uniform_list` nativo do GPUI, reduzindo alocações na GPU de 1.200 nós para apenas ~15 nós visíveis por quadro.\n");
        out.push_str("3. **Memoization do Parser de Markdown**: Implementar cache LRU com chave `(message_id, message_version)` para os nós AST do parser GPUI, eliminando parsing de regexes no hot loop.\n");
        out.push_str("4. **Subsistema Real de Áudio e Criptografia DAVE**: Planejar a implementação da pilha de mídia com `pipewire-rs`/`cpal` e handshake MLS para transformar os componentes mock em transmissão de voz real com latência inferior a 15ms.\n\n");

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporter_build_and_render() {
        let screen_tracker = ScreenTracker::new(64, None);
        let interaction_tracker = InteractionTracker::new(64, None);

        let report = TelemetryReporter::build_report(&screen_tracker, &interaction_tracker);
        assert!(report.global_parity_score > 0.0);

        let md = TelemetryReporter::render_markdown(&report);
        assert!(md.contains("Discord RS — Relatório Oficial de Auditoria e Paridade em Rust"));
        assert!(md.contains("Score de Paridade Geral"));
        assert!(md.contains("Lista Priorizada de Correções"));
    }
}
