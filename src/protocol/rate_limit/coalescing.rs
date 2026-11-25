//! Coalescing e Debouncing de Ações do Usuário e Requisições REST
//!
//! - Typing Indicator: no máximo uma vez a cada 10 s por canal.
//! - Read Receipts: debounce de 2 s com coalescing do maior Snowflake.
//! - GETs REST idênticos e simultâneos: single-flight, compartilhando uma resposta.

use anyhow::Context;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;

/// Janela oficial de coalescing do `POST /channels/{id}/typing`.
///
/// O indicador remoto dura até 10 s; enviar novamente antes disso apenas
/// consome cota REST sem efeito visual.
pub const TYPING_COOLDOWN: Duration = Duration::from_secs(10);

/// Gerenciador de Coalescing e Debounce de Ações da Interface.
#[derive(Clone)]
pub struct UserActionDebouncer {
    typing_last_sent: Arc<StdMutex<HashMap<String, Instant>>>,
    ack_pending: Arc<Mutex<HashMap<String, (String, JoinHandle<()>)>>>,
}

impl std::fmt::Debug for UserActionDebouncer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserActionDebouncer").finish_non_exhaustive()
    }
}

impl Default for UserActionDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

impl UserActionDebouncer {
    pub fn new() -> Self {
        Self {
            typing_last_sent: Arc::new(StdMutex::new(HashMap::new())),
            ack_pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Avalia se o typing indicator pode ser disparado para o canal.
    pub fn should_send_typing(&self, channel_id: &str) -> bool {
        self.should_send_typing_at(channel_id, Instant::now())
    }

    /// Variante determinística com relógio injetado, usada pelos testes e pelo
    /// editor para não depender do relógio real.
    pub fn should_send_typing_at(&self, channel_id: &str, now: Instant) -> bool {
        let mut map = self
            .typing_last_sent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Evita crescimento ilimitado quando o usuário visita muitos canais.
        map.retain(|_, last_sent| now.saturating_duration_since(*last_sent) < TYPING_COOLDOWN);

        if let Some(last_time) = map.get(channel_id) {
            if now.saturating_duration_since(*last_time) < TYPING_COOLDOWN {
                return false;
            }
        }

        map.insert(channel_id.to_string(), now);
        true
    }

    /// Limpa o estado de coalescing do canal ao enviar a mensagem, permitindo
    /// que a próxima digitação dispare um novo indicador imediatamente.
    pub fn reset_typing(&self, channel_id: &str) {
        let mut map = self
            .typing_last_sent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.remove(channel_id);
    }

    /// Agenda ou atualiza o envio coalescido de confirmação de leitura.
    /// Apenas o maior Snowflake ID visto no intervalo será despachado.
    pub async fn schedule_ack<F, Fut>(&self, channel_id: &str, message_id: &str, dispatch_fn: F)
    where
        F: FnOnce(String, String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut map = self.ack_pending.lock().await;
        // Tasks concluídas já enviaram seu ACK e não precisam ficar retidas.
        map.retain(|_, (_, task)| !task.is_finished());

        let channel = channel_id.to_string();
        let message = message_id.to_string();

        if let Some((existing_max_id, existing_task)) = map.remove(&channel) {
            existing_task.abort();

            let chosen_id = match (existing_max_id.parse::<u64>(), message.parse::<u64>()) {
                (Ok(existing), Ok(new)) if new > existing => message,
                _ => existing_max_id,
            };

            let dispatch_channel = channel.clone();
            let dispatch_message = chosen_id.clone();
            let task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                dispatch_fn(dispatch_channel, dispatch_message).await;
            });
            map.insert(channel, (chosen_id, task));
        } else {
            let dispatch_channel = channel.clone();
            let dispatch_message = message.clone();
            let task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                dispatch_fn(dispatch_channel, dispatch_message).await;
            });
            map.insert(channel, (message, task));
        }
    }
}

/// Resposta HTTP capturada para ser compartilhada por requisções coalescidas.
#[derive(Debug, Clone)]
pub struct CoalescedResponse {
    pub status: reqwest::StatusCode,
    pub headers: reqwest::header::HeaderMap,
    pub body: Vec<u8>,
}

impl CoalescedResponse {
    pub(crate) async fn capture(response: reqwest::Response) -> anyhow::Result<Self> {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await?.to_vec();
        Ok(Self {
            status,
            headers,
            body,
        })
    }

    pub(crate) fn into_response(self) -> reqwest::Response {
        let mut builder =
            tokio_tungstenite::tungstenite::http::Response::builder().status(self.status);
        for (name, value) in self.headers {
            if let Some(name) = name {
                builder = builder.header(name, value);
            }
        }
        builder
            .body(self.body)
            .expect("captured HTTP response must have valid headers")
            .into()
    }
}

struct InFlightRequest {
    receiver: watch::Receiver<Option<anyhow::Result<CoalescedResponse, String>>>,
    task: JoinHandle<()>,
}

/// Single-flight de GETs REST idênticos.
///
/// A chave deve ser um hash da URL e dos cabeçalhos condicionais, nunca um token
/// ou payload cru. O primeiro caller executa a rede; os demais aguardam e recebem
/// o mesmo resultado. Falhas também são compartilhadas.
#[derive(Clone, Default)]
pub struct RequestCoalescer {
    in_flight: Arc<Mutex<HashMap<String, InFlightRequest>>>,
}

impl RequestCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn coalesce<F, Fut>(
        &self,
        key: String,
        request: F,
    ) -> anyhow::Result<CoalescedResponse>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<CoalescedResponse>> + Send + 'static,
    {
        let receiver = loop {
            let mut in_flight = self.in_flight.lock().await;
            if let Some(existing) = in_flight.get(&key) {
                if !existing.task.is_finished() {
                    break existing.receiver.clone();
                }
                in_flight.remove(&key);
            } else {
                let (sender, receiver) = watch::channel(None);
                let task = tokio::spawn({
                    let this = self.clone();
                    let task_key = key.clone();
                    async move {
                        let result = request().await.map_err(|error| format!("{error:#}"));
                        // watch permite que todos os waiters leiam o mesmo
                        // resultado; nenhum Receiver é consumido duas vezes.
                        let _ = sender.send(Some(result));
                        this.in_flight.lock().await.remove(&task_key);
                    }
                });
                in_flight.insert(
                    key,
                    InFlightRequest {
                        receiver: receiver.clone(),
                        task,
                    },
                );
                break receiver;
            }
        };

        let mut receiver = receiver;
        receiver
            .changed()
            .await
            .context("Requisição coalescida foi cancelada")?;
        receiver
            .borrow()
            .clone()
            .context("Requisição coalescida não produziu resultado")?
            .map_err(|error| anyhow::anyhow!("{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_typing_throttling_10s() {
        let debouncer = UserActionDebouncer::new();
        assert!(debouncer.should_send_typing("chan_general"));
        assert!(!debouncer.should_send_typing("chan_general"));
        assert!(debouncer.should_send_typing("chan_random"));
    }

    #[test]
    fn typing_request_is_sent_once_per_window_and_stops_after_send() {
        let debouncer = UserActionDebouncer::new();
        let t0 = Instant::now();
        let mut requests = 0;

        // Simula digitação contínua por 30 s em passos de 1 s.
        for second in 0..30 {
            if debouncer.should_send_typing_at("chan_synthetic", t0 + Duration::from_secs(second))
            {
                requests += 1;
            }
        }
        assert_eq!(requests, 3, "uma requisição a cada janela de 10 s");

        // Envio da mensagem limpa o coalescing. A próxima digitação (nova
        // mensagem) pode notificar imediatamente, mas continua limitada à janela.
        debouncer.reset_typing("chan_synthetic");
        assert!(debouncer.should_send_typing_at(
            "chan_synthetic",
            t0 + Duration::from_secs(30)
        ));
        assert!(!debouncer.should_send_typing_at(
            "chan_synthetic",
            t0 + Duration::from_secs(31)
        ));

        // Canal diferente tem janela própria.
        assert!(debouncer.should_send_typing_at("chan_other", t0 + Duration::from_secs(30)));
    }

    #[tokio::test]
    async fn test_ack_coalescing_highest_snowflake() {
        let debouncer = UserActionDebouncer::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, String)>(10);

        let tx1 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "100", move |channel, message| async move {
                let _ = tx1.send((channel, message)).await;
            })
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let tx2 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "150", move |channel, message| async move {
                let _ = tx2.send((channel, message)).await;
            })
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let tx3 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "120", move |channel, message| async move {
                let _ = tx3.send((channel, message)).await;
            })
            .await;
        tokio::time::sleep(Duration::from_millis(2100)).await;

        let received = rx.recv().await.expect("Expected single coalesced ACK");
        assert_eq!(received.0, "chan_1");
        assert_eq!(received.1, "150");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_request_coalescer_executes_once_and_shares_result() {
        let coalescer = RequestCoalescer::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let futures = (0..5).map(|_| {
            let coalescer = coalescer.clone();
            let calls = calls.clone();
            async move {
                coalescer
                    .coalesce("synthetic-request-key".to_string(), move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Ok(CoalescedResponse {
                            status: reqwest::StatusCode::OK,
                            headers: reqwest::header::HeaderMap::new(),
                            body: b"shared".to_vec(),
                        })
                    })
                    .await
            }
        });

        let results = futures::future::join_all(futures).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        for result in results {
            assert_eq!(result.unwrap().body, b"shared");
        }
    }
}
