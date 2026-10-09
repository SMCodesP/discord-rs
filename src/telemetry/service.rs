use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};

use super::events::{InteractionType, ScreenType, TelemetryEvent};
use super::scoper::ParityScoper;

/// Statistics collected by the telemetry monitor
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    pub total_screen_transitions: u64,
    pub total_interactions: u64,
    pub total_components_mounted: u64,
    pub total_frames_rendered: u64,
    pub slow_frames_detected: u64,
    pub network_requests: u64,
    pub cached_304_responses: u64,
    pub gateway_dispatches: u64,
}

/// Lightweight handle cloneable across GPUI views and async tasks
#[derive(Clone)]
pub struct TelemetryHandle {
    sender: mpsc::UnboundedSender<TelemetryEvent>,
    scoper: Arc<RwLock<ParityScoper>>,
    metrics: Arc<RwLock<TelemetryMetrics>>,
    active_screen: Arc<RwLock<Option<ScreenType>>>,
    last_render_time_us: Arc<AtomicU64>,
}

impl TelemetryHandle {
    /// Dispatches a telemetry event asynchronously without blocking UI
    pub fn emit(&self, event: TelemetryEvent) {
        let _ = self.sender.send(event);
    }

    /// Records a screen / navigation change
    pub fn track_screen(&self, to: ScreenType) {
        let scoper = self.scoper.clone();
        let active = self.active_screen.clone();
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let mut current = active.write().await;
            let from = current.clone();
            *current = Some(to.clone());

            let feature_id = match &to {
                ScreenType::GuildChannel { .. } => "guild.guild_rail_ui",
                ScreenType::UserSettings { .. } => "settings.my_account",
                ScreenType::VoiceStage { .. } => "voice.webrtc_dave_encryption",
                _ => "",
            };
            if !feature_id.is_empty() {
                scoper.write().await.record_observation(feature_id);
            }

            let _ = sender.send(TelemetryEvent::ScreenChange {
                from,
                to,
                timestamp: Utc::now(),
            });
        });
    }

    /// Records a user click or interaction
    pub fn track_click(&self, target: &str, component_type: &str, detail: Option<String>) {
        self.emit(TelemetryEvent::Interaction {
            interaction: InteractionType::Click {
                target: target.to_string(),
                component_type: component_type.to_string(),
                detail,
            },
            timestamp: Utc::now(),
        });
    }

    /// Records a component render duration
    pub fn track_render(&self, component_name: &str, duration_us: u64) {
        self.last_render_time_us.store(duration_us, Ordering::Relaxed);
        let is_bottleneck = duration_us > 8000; // >8ms is slower than 120fps budget
        self.emit(TelemetryEvent::PerformanceSample {
            metric_name: format!("render.{}", component_name),
            value_us: duration_us,
            is_bottleneck,
            context: Some(component_name.to_string()),
            timestamp: Utc::now(),
        });
    }

    /// Records a REST request with caching metrics
    pub fn track_network(&self, endpoint: &str, method: &str, status: u16, duration_ms: u64, is_cached_304: bool) {
        self.emit(TelemetryEvent::NetworkAudit {
            endpoint: endpoint.to_string(),
            method: method.to_string(),
            status,
            duration_ms,
            is_cached_304,
            rate_limit_remaining: None,
            timestamp: Utc::now(),
        });
    }

    /// Returns the live parity scoper snapshot
    pub async fn scoper_snapshot(&self) -> ParityScoper {
        self.scoper.read().await.clone()
    }

    /// Returns the live metrics snapshot
    pub async fn metrics_snapshot(&self) -> TelemetryMetrics {
        self.metrics.read().await.clone()
    }
}

/// The Parity & Telemetry Monitoring Service in Rust
pub struct TelemetryService {
    scoper: Arc<RwLock<ParityScoper>>,
    metrics: Arc<RwLock<TelemetryMetrics>>,
    #[allow(dead_code)]
    active_screen: Arc<RwLock<Option<ScreenType>>>,
    receiver: mpsc::UnboundedReceiver<TelemetryEvent>,
    #[allow(dead_code)]
    sender: mpsc::UnboundedSender<TelemetryEvent>,
    #[allow(dead_code)]
    last_render_time_us: Arc<AtomicU64>,
}

impl TelemetryService {
    /// Creates a new unstarted telemetry service instance
    pub fn new() -> (Self, TelemetryHandle) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let scoper = Arc::new(RwLock::new(ParityScoper::new()));
        let metrics = Arc::new(RwLock::new(TelemetryMetrics::default()));
        let active_screen = Arc::new(RwLock::new(None));
        let last_render_time_us = Arc::new(AtomicU64::new(0));

        let handle = TelemetryHandle {
            sender: sender.clone(),
            scoper: scoper.clone(),
            metrics: metrics.clone(),
            active_screen: active_screen.clone(),
            last_render_time_us: last_render_time_us.clone(),
        };

        let service = Self {
            scoper,
            metrics,
            active_screen,
            receiver,
            sender,
            last_render_time_us,
        };

        (service, handle)
    }

    /// Spawns the telemetry worker loop on the Tokio runtime
    pub fn start(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            log::info!("[TelemetryService] Serviço de monitoramento de paridade iniciado.");
            let mut report_interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    Some(event) = self.receiver.recv() => {
                        self.process_event(event).await;
                    }
                    _ = report_interval.tick() => {
                        self.flush_reports_to_disk().await;
                    }
                    else => break,
                }
            }
            log::info!("[TelemetryService] Serviço de monitoramento encerrado.");
        })
    }

    async fn process_event(&mut self, event: TelemetryEvent) {
        let mut m = self.metrics.write().await;
        match event {
            TelemetryEvent::ScreenChange { to, .. } => {
                m.total_screen_transitions += 1;
                log::debug!("[TelemetryService] Transição de tela para: {:?}", to);
            }
            TelemetryEvent::Interaction { interaction, .. } => {
                m.total_interactions += 1;
                log::debug!("[TelemetryService] Interação registrada: {:?}", interaction);
            }
            TelemetryEvent::ComponentMount { component_name, .. } => {
                m.total_components_mounted += 1;
                log::trace!("[TelemetryService] Componente montado: {}", component_name);
            }
            TelemetryEvent::ComponentRender { .. } => {
                m.total_frames_rendered += 1;
            }
            TelemetryEvent::PerformanceSample { is_bottleneck, metric_name, value_us, .. } => {
                m.total_frames_rendered += 1;
                if is_bottleneck {
                    m.slow_frames_detected += 1;
                    log::warn!("[TelemetryService] ⚡ Gargalo de performance detectado em '{}': {}µs", metric_name, value_us);
                }
            }
            TelemetryEvent::NetworkAudit { is_cached_304, status, endpoint, .. } => {
                m.network_requests += 1;
                if is_cached_304 || status == 304 {
                    m.cached_304_responses += 1;
                    log::info!("[TelemetryService] 🚀 Cache 304 Not Modified hit em: {}", endpoint);
                }
            }
            TelemetryEvent::GatewayDispatch { .. } => {
                m.gateway_dispatches += 1;
            }
        }
    }

    /// Persists audit reports to disk (~/.config/discord-rs/parity_report.json and docs/PARITY_LIVE_AUDIT.md)
    pub async fn flush_reports_to_disk(&self) {
        let scoper = self.scoper.read().await;
        let metrics = self.metrics.read().await;

        let json_report = serde_json::json!({
            "generated_at": Utc::now(),
            "parity_score_percent": scoper.calculate_parity_score(),
            "metrics": *metrics,
            "features": scoper.get_gaps_and_divergences(),
        });

        // 1. JSON Report in user config
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("discord-rs").join("parity_report.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(pretty) = serde_json::to_string_pretty(&json_report) {
                let _ = std::fs::write(&path, pretty);
            }
        }

        // 2. Markdown Report in docs/
        let md_report = scoper.generate_markdown_report();
        let docs_path = PathBuf::from("docs/PARITY_LIVE_AUDIT.md");
        let _ = std::fs::write(docs_path, md_report);
    }
}
