use std::time::{Duration, Instant};

use discord_rs::domain::user::PresenceStatus;
use discord_rs::protocol::event_batcher::{
    BatchKey, EventBatcher, COALESCE_WINDOW,
};
use discord_rs::protocol::transport::TransportEvent;

fn typing(channel_id: &str, user_id: &str, timestamp: u64) -> TransportEvent {
    TransportEvent::TypingStart {
        channel_id: channel_id.to_string(),
        user_id: user_id.to_string(),
        timestamp,
    }
}

fn presence(user_id: &str, status: PresenceStatus) -> TransportEvent {
    TransportEvent::PresenceUpdate {
        user_id: user_id.to_string(),
        status,
        activity: None,
        custom_status: None,
    }
}

#[test]
fn batch_key_only_matches_presence_and_typing() {
    assert_eq!(
        BatchKey::from_event(&presence("user-1", PresenceStatus::Online)),
        Some(BatchKey::Presence {
            user_id: "user-1".to_string()
        })
    );
    assert_eq!(
        BatchKey::from_event(&typing("channel-1", "user-1", 1)),
        Some(BatchKey::Typing {
            channel_id: "channel-1".to_string(),
            user_id: "user-1".to_string(),
        })
    );
    assert_eq!(
        BatchKey::from_event(&TransportEvent::Connected),
        None
    );
}

#[test]
fn coalesces_repeated_typing_from_same_user_within_window() {
    let mut batcher = EventBatcher::new();
    assert_eq!(batcher.window(), COALESCE_WINDOW);
    assert_eq!(batcher.window(), Duration::from_millis(8));

    let t0 = Instant::now();
    assert!(batcher.push(typing("channel-1", "user-1", 100), t0).is_none());
    assert!(batcher
        .push(typing("channel-1", "user-1", 101), t0 + Duration::from_millis(2))
        .is_none());
    assert!(batcher
        .push(typing("channel-1", "user-1", 102), t0 + Duration::from_millis(5))
        .is_none());

    assert_eq!(batcher.pending_len(), 1);
    assert_eq!(batcher.coalesced_count(), 2);
    assert_eq!(batcher.next_deadline(), Some(t0 + Duration::from_millis(8)));

    assert!(batcher.flush(t0 + Duration::from_millis(7)).is_empty());

    let ready = batcher.flush(t0 + Duration::from_millis(8));
    assert_eq!(ready.len(), 1);
    match &ready[0] {
        TransportEvent::TypingStart {
            channel_id,
            user_id,
            timestamp,
        } => {
            assert_eq!(channel_id, "channel-1");
            assert_eq!(user_id, "user-1");
            assert_eq!(*timestamp, 102);
        }
        other => panic!("expected latest TypingStart, got {other:?}"),
    }
    assert!(batcher.is_empty());
}

#[test]
fn coalesces_presence_updates_keeping_latest_status() {
    let mut batcher = EventBatcher::new();
    let t0 = Instant::now();

    batcher.push(presence("user-7", PresenceStatus::Online), t0);
    batcher.push(
        presence("user-7", PresenceStatus::Idle),
        t0 + Duration::from_millis(3),
    );
    batcher.push(
        presence("user-7", PresenceStatus::DoNotDisturb),
        t0 + Duration::from_millis(6),
    );

    assert_eq!(batcher.pending_len(), 1);

    let ready = batcher.flush(t0 + Duration::from_millis(8));
    assert_eq!(ready.len(), 1);
    match &ready[0] {
        TransportEvent::PresenceUpdate { user_id, status, .. } => {
            assert_eq!(user_id, "user-7");
            assert_eq!(*status, PresenceStatus::DoNotDisturb);
        }
        other => panic!("expected latest PresenceUpdate, got {other:?}"),
    }
}

#[test]
fn non_coalescing_events_pass_through_immediately() {
    let mut batcher = EventBatcher::new();
    let now = Instant::now();

    let output = batcher.push(TransportEvent::Connected, now);
    assert!(matches!(output, Some(TransportEvent::Connected)));
    assert!(batcher.is_empty());
    assert_eq!(batcher.coalesced_count(), 0);
    assert_eq!(batcher.next_deadline(), None);
}

#[test]
fn distinct_users_and_channels_are_not_merged() {
    let mut batcher = EventBatcher::new();
    let t0 = Instant::now();

    batcher.push(presence("user-b", PresenceStatus::Online), t0);
    batcher.push(presence("user-a", PresenceStatus::Idle), t0);
    batcher.push(typing("channel-1", "user-a", 10), t0);
    batcher.push(typing("channel-2", "user-a", 11), t0);

    assert_eq!(batcher.pending_len(), 4);

    let ready = batcher.flush(t0 + Duration::from_millis(8));
    assert_eq!(ready.len(), 4);
    assert!(matches!(
        &ready[0],
        TransportEvent::PresenceUpdate { user_id, .. } if user_id == "user-b"
    ));
    assert!(matches!(
        &ready[1],
        TransportEvent::PresenceUpdate { user_id, .. } if user_id == "user-a"
    ));
    assert!(matches!(
        &ready[2],
        TransportEvent::TypingStart { channel_id, user_id, timestamp }
            if channel_id == "channel-1" && user_id == "user-a" && *timestamp == 10
    ));
    assert!(matches!(
        &ready[3],
        TransportEvent::TypingStart { channel_id, user_id, timestamp }
            if channel_id == "channel-2" && user_id == "user-a" && *timestamp == 11
    ));
    assert!(batcher.is_empty());
}

#[test]
fn coalescing_does_not_extend_the_window_deadline() {
    let mut batcher = EventBatcher::new();
    let t0 = Instant::now();

    batcher.push(presence("user-1", PresenceStatus::Online), t0);
    let first_deadline = batcher.next_deadline();
    assert_eq!(first_deadline, Some(t0 + Duration::from_millis(8)));

    batcher.push(
        presence("user-1", PresenceStatus::Idle),
        t0 + Duration::from_millis(7),
    );
    assert_eq!(batcher.next_deadline(), first_deadline);

    let ready = batcher.flush(t0 + Duration::from_millis(8));
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        &ready[0],
        TransportEvent::PresenceUpdate { status, .. } if *status == PresenceStatus::Idle
    ));
}

#[test]
fn a_new_window_starts_after_flush() {
    let mut batcher = EventBatcher::new();
    let t0 = Instant::now();

    batcher.push(typing("channel-1", "user-1", 1), t0);
    assert_eq!(batcher.flush(t0 + Duration::from_millis(8)).len(), 1);

    let t1 = t0 + Duration::from_millis(9);
    assert!(batcher.push(typing("channel-1", "user-1", 2), t1).is_none());
    assert_eq!(batcher.pending_len(), 1);
    assert_eq!(batcher.next_deadline(), Some(t1 + Duration::from_millis(8)));

    let ready = batcher.flush(t1 + Duration::from_millis(8));
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        &ready[0],
        TransportEvent::TypingStart { timestamp, .. } if *timestamp == 2
    ));
}

#[test]
fn flush_only_releases_expired_keys() {
    let mut batcher = EventBatcher::new();
    let t0 = Instant::now();

    batcher.push(presence("early", PresenceStatus::Online), t0);
    batcher.push(
        presence("late", PresenceStatus::Idle),
        t0 + Duration::from_millis(4),
    );

    let ready = batcher.flush(t0 + Duration::from_millis(8));
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        &ready[0],
        TransportEvent::PresenceUpdate { user_id, .. } if user_id == "early"
    ));
    assert_eq!(batcher.pending_len(), 1);
    assert_eq!(
        batcher.next_deadline(),
        Some(t0 + Duration::from_millis(12))
    );

    let ready = batcher.flush(t0 + Duration::from_millis(12));
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        &ready[0],
        TransportEvent::PresenceUpdate { user_id, .. } if user_id == "late"
    ));
    assert!(batcher.is_empty());
}
