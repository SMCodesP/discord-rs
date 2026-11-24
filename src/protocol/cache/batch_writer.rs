//! Debouncer de Escritas em Disco
//!
//! Agrupa gravações consecutivas do cache local em um único lote, evitando
//! múltiplos `fsync`/`rename` para o mesmo arquivo durante rajadas de eventos
//! do Gateway. Cada arquivo é gravado via arquivo temporário + `rename` atômico,
//! garantindo que leitores nunca observem conteúdo parcial. O lote pendente é
//! descarregado no shutdown.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio::task::JoinHandle;

use crate::protocol::cache::directory::CacheDirectory;

const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(250);
const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(2);

struct PendingState {
    pending: HashMap<PathBuf, Vec<u8>>,
    first_enqueued_at: Option<Instant>,
    last_enqueued_at: Option<Instant>,
    shutdown: bool,
}

struct Inner {
    cache_dir: CacheDirectory,
    debounce: Duration,
    max_delay: Duration,
    state: AsyncMutex<PendingState>,
    flush_lock: AsyncMutex<()>,
    notify: Arc<Notify>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Inner {
    async fn enqueue_bytes(&self, path: PathBuf, bytes: Vec<u8>) -> Result<()> {
        let mut state = self.state.lock().await;
        if state.shutdown {
            anyhow::bail!("cache batch writer is shut down");
        }

        let now = Instant::now();
        if state.pending.is_empty() {
            state.first_enqueued_at = Some(now);
        }
        state.last_enqueued_at = Some(now);
        state.pending.insert(path, bytes);
        drop(state);

        self.notify.notify_one();
        Ok(())
    }

    async fn flush_pending(&self) -> Result<()> {
        let _guard = self.flush_lock.lock().await;

        let mut batch: Vec<(PathBuf, Vec<u8>)> = {
            let mut state = self.state.lock().await;
            let pending = std::mem::take(&mut state.pending);
            state.first_enqueued_at = None;
            state.last_enqueued_at = None;
            pending.into_iter().collect()
        };

        if batch.is_empty() {
            return Ok(());
        }

        while let Some((path, bytes)) = batch.pop() {
            if let Err(error) = self.cache_dir.atomic_write_bytes(&path, &bytes) {
                let now = Instant::now();
                {
                    let mut state = self.state.lock().await;
                    if state.pending.is_empty() {
                        state.first_enqueued_at = Some(now);
                    }
                    state.last_enqueued_at = Some(now);
                    state.pending.insert(path, bytes);
                    for (pending_path, pending_bytes) in batch {
                        state.pending.insert(pending_path, pending_bytes);
                    }
                }
                self.notify.notify_one();
                return Err(error).context("failed to flush cache batch entry");
            }
        }

        Ok(())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.worker.lock() {
            if let Some(handle) = slot.take() {
                handle.abort();
            }
        }
    }
}

async fn worker_loop(weak: Weak<Inner>) {
    loop {
        let Some(inner) = weak.upgrade() else {
            return;
        };

        let (deadline, notify) = {
            let state = inner.state.lock().await;
            if state.shutdown {
                return;
            }

            let deadline = if state.pending.is_empty() {
                None
            } else {
                let last = state.last_enqueued_at.unwrap_or_else(Instant::now);
                let first = state.first_enqueued_at.unwrap_or(last);
                Some((last + inner.debounce).min(first + inner.max_delay))
            };

            (deadline, Arc::clone(&inner.notify))
        };

        match deadline {
            None => notify.notified().await,
            Some(deadline) => {
                let now = Instant::now();
                if deadline > now {
                    tokio::select! {
                        _ = notify.notified() => continue,
                        _ = tokio::time::sleep(deadline - now) => {}
                    }
                }
            }
        }

        let Some(inner) = weak.upgrade() else {
            return;
        };
        if let Err(error) = inner.flush_pending().await {
            log::error!("cache batch writer flush failed: {error:#}");
        }
    }
}

/// Agrupador de gravações do cache com debounce e descarga no shutdown.
#[derive(Clone)]
pub struct BatchWriter {
    inner: Arc<Inner>,
}

impl BatchWriter {
    /// Cria um writer com debounce de 250ms e atraso máximo de 2s.
    pub fn new(cache_dir: CacheDirectory) -> Self {
        Self::with_timing(cache_dir, DEFAULT_DEBOUNCE, DEFAULT_MAX_DELAY)
    }

    /// Cria um writer com debounce customizado. O atraso máximo é quatro vezes
    /// o debounce para que rajadas contínuas não adiem a gravação indefinidamente.
    pub fn with_debounce(cache_dir: CacheDirectory, debounce: Duration) -> Self {
        Self::with_timing(cache_dir, debounce, debounce.saturating_mul(4))
    }

    /// Cria um writer com debounce e atraso máximo explícitos.
    pub fn with_timing(
        cache_dir: CacheDirectory,
        debounce: Duration,
        max_delay: Duration,
    ) -> Self {
        let inner = Arc::new(Inner {
            cache_dir,
            debounce,
            max_delay,
            state: AsyncMutex::new(PendingState {
                pending: HashMap::new(),
                first_enqueued_at: None,
                last_enqueued_at: None,
                shutdown: false,
            }),
            flush_lock: AsyncMutex::new(()),
            notify: Arc::new(Notify::new()),
            worker: Mutex::new(None),
        });

        let weak = Arc::downgrade(&inner);
        let handle = tokio::spawn(worker_loop(weak));
        *inner
            .worker
            .lock()
            .expect("cache batch writer worker mutex poisoned") = Some(handle);

        Self { inner }
    }

    /// Agenda a gravação de bytes. Se o mesmo caminho já estiver pendente, o
    /// conteúdo anterior é substituído pelo mais recente.
    pub async fn enqueue_bytes(&self, path: impl Into<PathBuf>, bytes: Vec<u8>) -> Result<()> {
        self.inner.enqueue_bytes(path.into(), bytes).await
    }

    /// Serializa o valor em JSON e agenda sua gravação atômica.
    pub async fn enqueue_json<T: Serialize>(
        &self,
        path: impl Into<PathBuf>,
        value: &T,
    ) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value)
            .context("failed to serialize cache batch entry")?;
        self.enqueue_bytes(path, bytes).await
    }

    /// Retorna a quantidade de arquivos aguardando gravação.
    pub async fn pending_count(&self) -> usize {
        self.inner.state.lock().await.pending.len()
    }

    /// Descarrega imediatamente todos os arquivos pendentes, sem esperar o
    /// debounce.
    pub async fn flush(&self) -> Result<()> {
        self.inner.flush_pending().await
    }

    /// Para o worker, descarrega o lote pendente e impede novos enqueues.
    pub async fn shutdown(&self) -> Result<()> {
        {
            let mut state = self.inner.state.lock().await;
            state.shutdown = true;
        }
        self.inner.notify.notify_one();

        let handle = {
            let mut slot = self
                .inner
                .worker
                .lock()
                .expect("cache batch writer worker mutex poisoned");
            slot.take()
        };
        if let Some(handle) = handle {
            let _ = handle.await;
        }

        self.inner.flush_pending().await
    }
}
