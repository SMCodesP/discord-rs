use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use super::types::{
    sla, InteractionEvent, InteractionKind, ThroughputMetrics,
};

/// Monitor de cliques e interações com o usuário sem sobrecarga de CPU
#[derive(Clone)]
pub struct InteractionTracker {
    inner: Arc<InteractionTrackerInner>,
}

struct InteractionTrackerInner {
    // Contadores atômicos lock-free para o hot path de renderização e eventos de ponteiro
    total_interactions: AtomicU64,
    total_button_clicks: AtomicU64,
    total_channel_selects: AtomicU64,
    total_guild_selects: AtomicU64,
    total_messages_sent: AtomicU64,
    total_reactions_toggled: AtomicU64,
    total_scroll_events: AtomicU64,
    total_sla_violations: AtomicU64,
    total_latency_us: AtomicU64,
    dropped_events: AtomicU64,

    // Histórico circular recente protegido por RwLock
    recent_events: RwLock<VecDeque<InteractionEvent>>,
    max_history_capacity: usize,

    // Canal assíncrono opcional
    event_tx: Option<mpsc::Sender<InteractionEvent>>,
}

impl InteractionTracker {
    pub fn new(capacity: usize, event_tx: Option<mpsc::Sender<InteractionEvent>>) -> Self {
        Self {
            inner: Arc::new(InteractionTrackerInner {
                total_interactions: AtomicU64::new(0),
                total_button_clicks: AtomicU64::new(0),
                total_channel_selects: AtomicU64::new(0),
                total_guild_selects: AtomicU64::new(0),
                total_messages_sent: AtomicU64::new(0),
                total_reactions_toggled: AtomicU64::new(0),
                total_scroll_events: AtomicU64::new(0),
                total_sla_violations: AtomicU64::new(0),
                total_latency_us: AtomicU64::new(0),
                dropped_events: AtomicU64::new(0),
                recent_events: RwLock::new(VecDeque::with_capacity(capacity)),
                max_history_capacity: capacity,
                event_tx,
            }),
        }
    }

    /// Rastreia uma interação genérica com duração em microssegundos
    pub fn track_interaction(&self, kind: InteractionKind, duration_us: u64) {
        let id = self.inner.total_interactions.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner.total_latency_us.fetch_add(duration_us, Ordering::Relaxed);

        let is_sla_violated = duration_us > sla::INTERACTION_RESPONSE_SLA_US;
        if is_sla_violated {
            self.inner.total_sla_violations.fetch_add(1, Ordering::Relaxed);
        }

        // Incremento atômico por tipo específico
        match &kind {
            InteractionKind::ButtonClick { .. } => {
                self.inner.total_button_clicks.fetch_add(1, Ordering::Relaxed);
            }
            InteractionKind::ChannelSelect { .. } => {
                self.inner.total_channel_selects.fetch_add(1, Ordering::Relaxed);
            }
            InteractionKind::GuildSelect { .. } => {
                self.inner.total_guild_selects.fetch_add(1, Ordering::Relaxed);
            }
            InteractionKind::MessageSend { .. } => {
                self.inner.total_messages_sent.fetch_add(1, Ordering::Relaxed);
            }
            InteractionKind::ReactionToggle { .. } => {
                self.inner.total_reactions_toggled.fetch_add(1, Ordering::Relaxed);
            }
            InteractionKind::ChatScroll { .. } => {
                self.inner.total_scroll_events.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let event = InteractionEvent {
            id,
            kind,
            timestamp_us,
            duration_us,
            is_sla_violated,
        };

        // Salva no buffer circular de forma rápida
        if let Ok(mut buffer) = self.inner.recent_events.write() {
            if buffer.len() >= self.inner.max_history_capacity {
                buffer.pop_front();
            }
            buffer.push_back(event.clone());
        }

        // Despacha no canal mpsc com try_send para nunca bloquear a thread de UI
        if let Some(ref tx) = self.inner.event_tx {
            if tx.try_send(event).is_err() {
                self.inner.dropped_events.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Rastreia clique de botão
    pub fn track_button_click(&self, button_id: impl Into<String>, label: impl Into<String>, duration_us: u64) {
        self.track_interaction(
            InteractionKind::ButtonClick {
                button_id: button_id.into(),
                label: label.into(),
            },
            duration_us,
        );
    }

    /// Rastreia seleção de canal
    pub fn track_channel_select(
        &self,
        channel_id: impl Into<String>,
        channel_name: impl Into<String>,
        kind: impl Into<String>,
        duration_us: u64,
    ) {
        self.track_interaction(
            InteractionKind::ChannelSelect {
                channel_id: channel_id.into(),
                channel_name: channel_name.into(),
                kind: kind.into(),
            },
            duration_us,
        );
    }

    /// Rastreia seleção de servidor/guild
    pub fn track_guild_select(&self, guild_id: impl Into<String>, guild_name: impl Into<String>, duration_us: u64) {
        self.track_interaction(
            InteractionKind::GuildSelect {
                guild_id: guild_id.into(),
                guild_name: guild_name.into(),
            },
            duration_us,
        );
    }

    /// Rastreia alternância de reação em mensagem
    pub fn track_reaction(&self, emoji: impl Into<String>, message_id: impl Into<String>, added: bool, duration_us: u64) {
        self.track_interaction(
            InteractionKind::ReactionToggle {
                emoji: emoji.into(),
                message_id: message_id.into(),
                added,
            },
            duration_us,
        );
    }

    /// Rastreia troca de aba nas configurações
    pub fn track_tab_change(&self, tab_name: impl Into<String>, duration_us: u64) {
        self.track_interaction(
            InteractionKind::TabChange {
                tab_name: tab_name.into(),
            },
            duration_us,
        );
    }

    /// Rastreia envio de mensagem no chat
    pub fn track_message_send(&self, channel_id: impl Into<String>, byte_len: usize, is_optimistic: bool, duration_us: u64) {
        self.track_interaction(
            InteractionKind::MessageSend {
                channel_id: channel_id.into(),
                byte_len,
                is_optimistic,
            },
            duration_us,
        );
    }

    /// Rastreia evento de scroll na lista de mensagens
    pub fn track_scroll(&self, delta_y: f32, scroll_top: f32, duration_us: u64) {
        self.track_interaction(
            InteractionKind::ChatScroll { delta_y, scroll_top },
            duration_us,
        );
    }

    /// Retorna as métricas agregadas de throughput
    pub fn get_throughput_metrics(&self) -> ThroughputMetrics {
        ThroughputMetrics {
            total_interactions: self.inner.total_interactions.load(Ordering::Relaxed),
            total_button_clicks: self.inner.total_button_clicks.load(Ordering::Relaxed),
            total_channel_selects: self.inner.total_channel_selects.load(Ordering::Relaxed),
            total_guild_selects: self.inner.total_guild_selects.load(Ordering::Relaxed),
            total_messages_sent: self.inner.total_messages_sent.load(Ordering::Relaxed),
            total_reactions_toggled: self.inner.total_reactions_toggled.load(Ordering::Relaxed),
            total_scroll_events: self.inner.total_scroll_events.load(Ordering::Relaxed),
            total_interaction_sla_violations: self.inner.total_sla_violations.load(Ordering::Relaxed),
            dropped_telemetry_events: self.inner.dropped_events.load(Ordering::Relaxed),
        }
    }

    /// Latência média de resposta a interações em microssegundos
    pub fn average_latency_us(&self) -> u64 {
        let total = self.inner.total_interactions.load(Ordering::Relaxed);
        if total == 0 {
            0
        } else {
            self.inner.total_latency_us.load(Ordering::Relaxed) / total
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interaction_tracker_counters() {
        let tracker = InteractionTracker::new(128, None);

        tracker.track_button_click("btn-mute", "Mute Audio", 1200);
        tracker.track_channel_select("chan-1", "geral", "text", 2500);
        tracker.track_message_send("chan-1", 45, true, 8900); // violates 8.33ms SLA
        tracker.track_reaction("🔥", "msg-1", true, 900);
        tracker.track_scroll(-15.0, 350.0, 500);

        let metrics = tracker.get_throughput_metrics();
        assert_eq!(metrics.total_interactions, 5);
        assert_eq!(metrics.total_button_clicks, 1);
        assert_eq!(metrics.total_channel_selects, 1);
        assert_eq!(metrics.total_messages_sent, 1);
        assert_eq!(metrics.total_reactions_toggled, 1);
        assert_eq!(metrics.total_scroll_events, 1);
        assert_eq!(metrics.total_interaction_sla_violations, 1);
        assert_eq!(metrics.dropped_telemetry_events, 0);

        assert!(tracker.average_latency_us() > 0);
    }
}
