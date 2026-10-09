use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use super::types::{
    sla, Breadcrumb, ScreenKind, ScreenTransitionEvent,
};

/// Estrutura para cálculo e armazenamento de percentis de latência
#[derive(Debug, Clone, Default)]
pub struct LatencySummary {
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
    pub count: usize,
}

/// Monitor de transição de telas e latência de montagem
#[derive(Clone)]
pub struct ScreenTracker {
    inner: Arc<ScreenTrackerInner>,
}

struct ScreenTrackerInner {
    current_screen: RwLock<Option<ScreenKind>>,
    recent_transitions: RwLock<VecDeque<ScreenTransitionEvent>>,
    breadcrumbs: RwLock<VecDeque<Breadcrumb>>,
    max_history_capacity: usize,

    // Contadores atômicos lock-free para acesso instantâneo sem contenção
    transition_counter: AtomicU64,
    sla_violations: AtomicU64,
    total_mount_time_us: AtomicU64,

    // Canal assíncrono opcional para streaming contínuo sem bloquear a UI
    event_tx: Option<mpsc::Sender<ScreenTransitionEvent>>,
}

/// Guardião RAII de medição de tempo de montagem de tela
pub struct TransitionGuard<'a> {
    tracker: &'a ScreenTracker,
    id: u64,
    from: Option<ScreenKind>,
    to: ScreenKind,
    start_instant: Instant,
    start_us: u64,
    metadata: HashMap<String, String>,
    completed: bool,
}

impl ScreenTracker {
    pub fn new(capacity: usize, event_tx: Option<mpsc::Sender<ScreenTransitionEvent>>) -> Self {
        Self {
            inner: Arc::new(ScreenTrackerInner {
                current_screen: RwLock::new(None),
                recent_transitions: RwLock::new(VecDeque::with_capacity(capacity)),
                breadcrumbs: RwLock::new(VecDeque::with_capacity(capacity)),
                max_history_capacity: capacity,
                transition_counter: AtomicU64::new(0),
                sla_violations: AtomicU64::new(0),
                total_mount_time_us: AtomicU64::new(0),
                event_tx,
            }),
        }
    }

    /// Retorna a tela atualmente ativa
    pub fn current_screen(&self) -> Option<ScreenKind> {
        self.inner.current_screen.read().ok()?.clone()
    }

    /// Inicia uma transição para uma nova tela, retornando um guardião RAII
    pub fn start_transition<'a>(
        &'a self,
        to: ScreenKind,
        metadata: HashMap<String, String>,
    ) -> TransitionGuard<'a> {
        let from = self.current_screen();
        let id = self.inner.transition_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let start_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        TransitionGuard {
            tracker: self,
            id,
            from,
            to,
            start_instant: Instant::now(),
            start_us,
            metadata,
            completed: false,
        }
    }

    /// Finaliza e registra o evento de transição medido
    pub fn record_transition(&self, event: ScreenTransitionEvent) {
        if event.is_sla_violated {
            self.inner.sla_violations.fetch_add(1, Ordering::Relaxed);
        }
        self.inner
            .total_mount_time_us
            .fetch_add(event.duration_us, Ordering::Relaxed);

        // Atualiza a tela corrente
        if let Ok(mut cur) = self.inner.current_screen.write() {
            *cur = Some(event.to_screen.clone());
        }

        // Adiciona breadcrumb para telemetria de navegação
        let breadcrumb = Breadcrumb {
            timestamp_us: event.completed_at_us,
            category: "navigation".into(),
            message: format!(
                "Screen transition: {:?} -> {} in {:.2}ms",
                event.from_screen.as_ref().map(|s| s.name()),
                event.to_screen.name(),
                event.duration_ms
            ),
            data: event.metadata.clone(),
        };

        if let Ok(mut crumbs) = self.inner.breadcrumbs.write() {
            if crumbs.len() >= self.inner.max_history_capacity {
                crumbs.pop_front();
            }
            crumbs.push_back(breadcrumb);
        }

        // Armazena no buffer circular de histórico recente
        if let Ok(mut transitions) = self.inner.recent_transitions.write() {
            if transitions.len() >= self.inner.max_history_capacity {
                transitions.pop_front();
            }
            transitions.push_back(event.clone());
        }

        // Despacha assincronamente sem bloquear a thread principal
        if let Some(ref tx) = self.inner.event_tx {
            let _ = tx.try_send(event);
        }
    }

    /// Calcula os percentis de tempo de montagem (p50, p95, p99, max)
    pub fn calculate_latency_summary(&self) -> LatencySummary {
        let Ok(transitions) = self.inner.recent_transitions.read() else {
            return LatencySummary::default();
        };

        if transitions.is_empty() {
            return LatencySummary::default();
        }

        let mut latencies: Vec<u64> = transitions.iter().map(|t| t.duration_us).collect();
        latencies.sort_unstable();

        let count = latencies.len();
        let p50_idx = (count as f64 * 0.50) as usize;
        let p95_idx = ((count as f64 * 0.95) as usize).min(count - 1);
        let p99_idx = ((count as f64 * 0.99) as usize).min(count - 1);
        let max_us = *latencies.last().unwrap_or(&0);

        LatencySummary {
            p50_us: latencies[p50_idx],
            p95_us: latencies[p95_idx],
            p99_us: latencies[p99_idx],
            max_us,
            count,
        }
    }

    /// Total de transições registradas
    pub fn total_transitions(&self) -> u64 {
        self.inner.transition_counter.load(Ordering::Relaxed)
    }

    /// Total de violações do SLA de montagem (> 16.6ms)
    pub fn total_sla_violations(&self) -> u64 {
        self.inner.sla_violations.load(Ordering::Relaxed)
    }

    /// Latência média acumulada de montagem em microssegundos
    pub fn average_mount_latency_us(&self) -> u64 {
        let total = self.total_transitions();
        if total == 0 {
            0
        } else {
            self.inner.total_mount_time_us.load(Ordering::Relaxed) / total
        }
    }

    /// Snapshot dos breadcrumbs recentes para depuração
    pub fn get_recent_breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.inner
            .breadcrumbs
            .read()
            .map(|b| b.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl<'a> TransitionGuard<'a> {
    /// Finaliza manualmente a medição e envia metadados adicionais
    pub fn complete(mut self, extra_metadata: Option<HashMap<String, String>>) {
        self.finish(extra_metadata);
    }

    fn finish(&mut self, extra_metadata: Option<HashMap<String, String>>) {
        if self.completed {
            return;
        }
        self.completed = true;

        let duration = self.start_instant.elapsed();
        let duration_us = duration.as_micros() as u64;
        let duration_ms = duration.as_secs_f64() * 1000.0;
        let completed_at_us = self.start_us + duration_us;
        let is_sla_violated = duration_us > sla::MAX_MOUNT_LATENCY_US;

        let mut meta = self.metadata.clone();
        if let Some(extra) = extra_metadata {
            meta.extend(extra);
        }

        let event = ScreenTransitionEvent {
            id: self.id,
            from_screen: self.from.clone(),
            to_screen: self.to.clone(),
            started_at_us: self.start_us,
            completed_at_us,
            duration_us,
            duration_ms,
            is_sla_violated,
            metadata: meta,
        };

        self.tracker.record_transition(event);
    }
}

impl<'a> Drop for TransitionGuard<'a> {
    fn drop(&mut self) {
        if !self.completed {
            self.finish(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_transition_tracking() {
        let tracker = ScreenTracker::new(64, None);

        assert!(tracker.current_screen().is_none());

        {
            let guard = tracker.start_transition(
                ScreenKind::Guild {
                    id: "guild-1".into(),
                    name: "Rust Brasil".into(),
                },
                HashMap::new(),
            );
            // Simula renderização
            std::thread::sleep(std::time::Duration::from_millis(2));
            guard.complete(None);
        }

        assert_eq!(tracker.total_transitions(), 1);
        let cur = tracker.current_screen().unwrap();
        assert_eq!(cur.name(), "Guild View");

        // Transição para canal de texto
        {
            let guard = tracker.start_transition(
                ScreenKind::TextChannel {
                    id: "chan-general".into(),
                    name: "geral".into(),
                    guild_id: Some("guild-1".into()),
                },
                HashMap::new(),
            );
            guard.complete(None);
        }

        assert_eq!(tracker.total_transitions(), 2);
        let summary = tracker.calculate_latency_summary();
        assert_eq!(summary.count, 2);
        assert!(summary.p50_us > 0);
    }
}
