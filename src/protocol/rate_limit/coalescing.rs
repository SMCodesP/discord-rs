//! Coalescing e Debouncing de Ações do Usuário
//!
//! Previne o bombardeio da API Discord por eventos gerados na interface do usuário:
//! - Typing Indicator (`POST /channels/{id}/typing`): Throttled no máx 1x a cada 10s por canal.
//! - Read Receipts (`POST /channels/{id}/messages/{id}/ack`): Debounced por 2s com coalescing do maior Snowflake ID.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Gerenciador de Coalescing e Debounce de Ações da Interface
#[derive(Clone)]
pub struct UserActionDebouncer {
    typing_last_sent: Arc<Mutex<HashMap<String, Instant>>>,
    ack_pending: Arc<Mutex<HashMap<String, (String, tokio::task::JoinHandle<()>)>>>,
}

impl Default for UserActionDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

impl UserActionDebouncer {
    pub fn new() -> Self {
        Self {
            typing_last_sent: Arc::new(Mutex::new(HashMap::new())),
            ack_pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Avalia se o typing indicator pode ser disparado para o canal.
    /// Retorna `true` se 10s se passaram desde o último envio; `false` se deve ser ignorado (coalesced).
    pub async fn should_send_typing(&self, channel_id: &str) -> bool {
        let mut map = self.typing_last_sent.lock().await;
        let now = Instant::now();
        const TYPING_COOLDOWN: Duration = Duration::from_secs(10);

        if let Some(last_time) = map.get(channel_id) {
            if now.duration_since(*last_time) < TYPING_COOLDOWN {
                return false; // Ignora digitação intermediária (coalesced)
            }
        }

        map.insert(channel_id.to_string(), now);
        true
    }

    /// Agenda ou atualiza o envio coalescido de confirmação de leitura (`/ack`) com debounce de 2 segundos.
    /// Apenas o maior Snowflake ID visto no intervalo será efetivamente despachado para a API.
    pub async fn schedule_ack<F, Fut>(
        &self,
        channel_id: &str,
        message_id: &str,
        dispatch_fn: F,
    ) where
        F: FnOnce(String, String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut map = self.ack_pending.lock().await;
        let c_id = channel_id.to_string();
        let m_id = message_id.to_string();

        if let Some((existing_max_id, existing_task)) = map.remove(&c_id) {
            // Cancela o debounce anterior
            existing_task.abort();

            // Compara Snowflakes numericamente (ID maior = mensagem mais recente)
            let chosen_id = match (existing_max_id.parse::<u64>(), m_id.parse::<u64>()) {
                (Ok(e), Ok(n)) if n > e => m_id,
                _ => existing_max_id,
            };

            let c_clone = c_id.clone();
            let id_clone = chosen_id.clone();
            let task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                dispatch_fn(c_clone, id_clone).await;
            });

            map.insert(c_id, (chosen_id, task));
        } else {
            let c_clone = c_id.clone();
            let id_clone = m_id.clone();
            let task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                dispatch_fn(c_clone, id_clone).await;
            });

            map.insert(c_id, (m_id, task));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typing_throttling_10s() {
        let debouncer = UserActionDebouncer::new();

        // Primeiro evento: permitido
        assert!(debouncer.should_send_typing("chan_general").await);

        // Segundo evento imediato: bloqueado/coalesced
        assert!(!debouncer.should_send_typing("chan_general").await);

        // Outro canal: permitido independentemente
        assert!(debouncer.should_send_typing("chan_random").await);
    }

    #[tokio::test]
    async fn test_ack_coalescing_highest_snowflake() {
        let debouncer = UserActionDebouncer::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, String)>(10);

        let tx_clone1 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "100", move |c, m| async move {
                let _ = tx_clone1.send((c, m)).await;
            })
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Nova mensagem lida com Snowflake maior (150 > 100)
        let tx_clone2 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "150", move |c, m| async move {
                let _ = tx_clone2.send((c, m)).await;
            })
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Mensagem intermediária com Snowflake menor (120 < 150)
        let tx_clone3 = tx.clone();
        debouncer
            .schedule_ack("chan_1", "120", move |c, m| async move {
                let _ = tx_clone3.send((c, m)).await;
            })
            .await;

        // Aguarda expiração do debounce de 2s
        tokio::time::sleep(Duration::from_millis(2100)).await;

        // Deve ter recebido apenas UMA única mensagem com o ID 150!
        let received = rx.recv().await.expect("Expected single coalesced ACK");
        assert_eq!(received.0, "chan_1");
        assert_eq!(received.1, "150");

        // Fila deve estar vazia (sem duplicatas)
        assert!(rx.try_recv().is_err());
    }
}
