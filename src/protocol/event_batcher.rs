use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::transport::TransportEvent;

pub const COALESCE_WINDOW: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BatchKey {
    Presence {
        user_id: String,
    },
    Typing {
        channel_id: String,
        user_id: String,
    },
}

impl BatchKey {
    pub fn from_event(event: &TransportEvent) -> Option<Self> {
        match event {
            TransportEvent::PresenceUpdate { user_id, .. } => Some(Self::Presence {
                user_id: user_id.clone(),
            }),
            TransportEvent::TypingStart {
                channel_id,
                user_id,
                ..
            } => Some(Self::Typing {
                channel_id: channel_id.clone(),
                user_id: user_id.clone(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingEvent {
    event: TransportEvent,
    deadline: Instant,
}

#[derive(Debug, Clone)]
pub struct EventBatcher {
    window: Duration,
    pending: HashMap<BatchKey, PendingEvent>,
    order: Vec<BatchKey>,
    coalesced: u64,
}

impl Default for EventBatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBatcher {
    pub fn new() -> Self {
        Self::with_window(COALESCE_WINDOW)
    }

    pub fn with_window(window: Duration) -> Self {
        Self {
            window,
            pending: HashMap::new(),
            order: Vec::new(),
            coalesced: 0,
        }
    }

    pub fn window(&self) -> Duration {
        self.window
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn coalesced_count(&self) -> u64 {
        self.coalesced
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending
            .values()
            .map(|pending| pending.deadline)
            .min()
    }

    pub fn push(&mut self, event: TransportEvent, now: Instant) -> Option<TransportEvent> {
        let Some(key) = BatchKey::from_event(&event) else {
            return Some(event);
        };

        if let Some(pending) = self.pending.get_mut(&key) {
            pending.event = event;
            self.coalesced = self.coalesced.saturating_add(1);
            return None;
        }

        self.pending.insert(
            key.clone(),
            PendingEvent {
                event,
                deadline: now + self.window,
            },
        );
        self.order.push(key);
        None
    }

    pub fn push_now(&mut self, event: TransportEvent) -> Option<TransportEvent> {
        self.push(event, Instant::now())
    }

    pub fn flush(&mut self, now: Instant) -> Vec<TransportEvent> {
        let mut ready = Vec::new();
        let mut remaining = Vec::with_capacity(self.order.len());

        for key in self.order.drain(..) {
            match self.pending.get(&key) {
                Some(pending) if pending.deadline <= now => {
                    if let Some(pending) = self.pending.remove(&key) {
                        ready.push(pending.event);
                    }
                }
                Some(_) => remaining.push(key),
                None => {}
            }
        }

        self.order = remaining;
        ready
    }

    pub fn flush_due(&mut self) -> Vec<TransportEvent> {
        self.flush(Instant::now())
    }
}
