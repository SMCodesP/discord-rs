pub mod comparator;
pub mod component_scope;
pub mod divergence_scoper;
pub mod interaction_tracker;
pub mod reporter;
pub mod screen_tracker;
pub mod types;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub use comparator::VisualComparator;
pub use component_scope::ComponentScope;
pub use divergence_scoper::DivergenceScoper;
pub use interaction_tracker::InteractionTracker;
pub use reporter::TelemetryReporter;
pub use screen_tracker::{ScreenTracker, TransitionGuard};
pub use types::*;

/// Configurações operacionais do serviço nativo de telemetria
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub channel_capacity: usize,
    pub history_capacity: usize,
    pub auto_export_interval_secs: u64,
    pub export_json_path: Option<PathBuf>,
    pub export_markdown_path: Option<PathBuf>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel_capacity: 2048,
            history_capacity: 256,
            auto_export_interval_secs: 180, // Exporta periodicamente a cada 3 minutos
            export_json_path: None,
            export_markdown_path: None,
        }
    }
}

/// Handle leve e clonável para emissão de métricas e rastreamento em qualquer ponto do app
#[derive(Clone)]
pub struct TelemetryHandle {
    screen_tracker: ScreenTracker,
    interaction_tracker: InteractionTracker,
    is_active: Arc<AtomicBool>,
}

impl std::fmt::Debug for TelemetryHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryHandle")
            .field("is_active", &self.is_active())
            .finish()
    }
}

impl TelemetryHandle {
    pub fn screen(&self) -> &ScreenTracker {
        &self.screen_tracker
    }

    pub fn interaction(&self) -> &InteractionTracker {
        &self.interaction_tracker
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Rastreia o início de uma transição de tela
    pub fn start_screen_transition(
        &self,
        to_screen: ScreenKind,
        metadata: std::collections::HashMap<String, String>,
    ) -> TransitionGuard<'_> {
        self.screen_tracker.start_transition(to_screen, metadata)
    }

    /// Rastreia clique de botão com medição de latência
    pub fn track_button_click(&self, button_id: impl Into<String>, label: impl Into<String>, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_button_click(button_id, label, duration_us);
        }
    }

    /// Rastreia seleção de canal
    pub fn track_channel_select(
        &self,
        channel_id: impl Into<String>,
        channel_name: impl Into<String>,
        kind: impl Into<String>,
        duration_us: u64,
    ) {
        if self.is_active() {
            self.interaction_tracker.track_channel_select(channel_id, channel_name, kind, duration_us);
        }
    }

    /// Rastreia seleção de servidor
    pub fn track_guild_select(&self, guild_id: impl Into<String>, guild_name: impl Into<String>, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_guild_select(guild_id, guild_name, duration_us);
        }
    }

    /// Rastreia envio de mensagem no chat
    pub fn track_message_send(&self, channel_id: impl Into<String>, byte_len: usize, is_optimistic: bool, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_message_send(channel_id, byte_len, is_optimistic, duration_us);
        }
    }

    /// Rastreia reação adicionada/removida
    pub fn track_reaction(&self, emoji: impl Into<String>, message_id: impl Into<String>, added: bool, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_reaction(emoji, message_id, added, duration_us);
        }
    }

    /// Rastreia alternância de abas nas configurações
    pub fn track_tab_change(&self, tab_name: impl Into<String>, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_tab_change(tab_name, duration_us);
        }
    }

    /// Rastreia rolagem no histórico de chat
    pub fn track_scroll(&self, delta_y: f32, scroll_top: f32, duration_us: u64) {
        if self.is_active() {
            self.interaction_tracker.track_scroll(delta_y, scroll_top, duration_us);
        }
    }

    /// Executa exportação imediata dos relatórios JSON e Markdown
    pub fn export_reports(&self) -> Result<(PathBuf, PathBuf), std::io::Error> {
        TelemetryReporter::export_all(&self.screen_tracker, &self.interaction_tracker)
    }

    /// Retorna o score global de paridade atual (0.0% - 100.0%)
    pub fn current_parity_score(&self) -> f64 {
        DivergenceScoper::evaluate(&self.screen_tracker, &self.interaction_tracker).global_parity_score
    }
}

/// Instância singleton global para telemetria acessível de qualquer camada
static GLOBAL_TELEMETRY: OnceLock<TelemetryHandle> = OnceLock::new();

/// Retorna o handle global de telemetria (inicializa com padrões caso ainda não tenha sido iniciado)
pub fn global() -> &'static TelemetryHandle {
    GLOBAL_TELEMETRY.get_or_init(|| {
        let (screen_tracker, interaction_tracker) = (
            ScreenTracker::new(256, None),
            InteractionTracker::new(256, None),
        );
        TelemetryHandle {
            screen_tracker,
            interaction_tracker,
            is_active: Arc::new(AtomicBool::new(true)),
        }
    })
}

/// Serviço de telemetria desacoplado em background
pub struct TelemetryService {
    handle: TelemetryHandle,
    _bg_worker: Option<JoinHandle<()>>,
}

impl TelemetryService {
    /// Inicia o serviço assíncrono de telemetria sem bloqueio de CPU
    pub fn start(config: TelemetryConfig) -> (Self, TelemetryHandle) {
        let (screen_tx, mut screen_rx) = mpsc::channel::<ScreenTransitionEvent>(config.channel_capacity);
        let (interaction_tx, mut interaction_rx) = mpsc::channel::<InteractionEvent>(config.channel_capacity);

        let screen_tracker = ScreenTracker::new(config.history_capacity, Some(screen_tx));
        let interaction_tracker = InteractionTracker::new(config.history_capacity, Some(interaction_tx));
        let is_active = Arc::new(AtomicBool::new(config.enabled));

        let handle = TelemetryHandle {
            screen_tracker: screen_tracker.clone(),
            interaction_tracker: interaction_tracker.clone(),
            is_active: is_active.clone(),
        };

        // Registra globalmente se for a primeira inicialização
        let _ = GLOBAL_TELEMETRY.set(handle.clone());

        // Tarefa Tokio em background para processar eventos e exportar relatórios periódicos
        let screen_clone = screen_tracker.clone();
        let interaction_clone = interaction_tracker.clone();
        let auto_interval = config.auto_export_interval_secs;
        let json_dest = config.export_json_path;
        let md_dest = config.export_markdown_path;

        let bg_worker = if config.enabled {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(auto_interval.max(10)));
                // Consome o primeiro tick imediato
                interval.tick().await;

                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let report = TelemetryReporter::build_report(&screen_clone, &interaction_clone);
                            let _ = TelemetryReporter::export_json(&report, json_dest.clone());
                            let _ = TelemetryReporter::export_markdown(&report, md_dest.clone());
                            log::debug!("Relatórios de paridade do Discord RS exportados periodicamente.");
                        }
                        Some(_evt) = screen_rx.recv() => {
                            // Evento de tela consumido em lote pelo worker assíncrono
                        }
                        Some(_evt) = interaction_rx.recv() => {
                            // Evento de interação consumido em lote pelo worker assíncrono
                        }
                        else => {
                            break;
                        }
                    }
                }
            }))
        } else {
            None
        };

        (
            Self {
                handle: handle.clone(),
                _bg_worker: bg_worker,
            },
            handle,
        )
    }

    pub fn handle(&self) -> &TelemetryHandle {
        &self.handle
    }
}
