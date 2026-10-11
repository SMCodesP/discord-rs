use discord_rs::app::state::{AppState, FriendItemState, PresenceStatus};
use discord_rs::domain::user::{Activity, ActivityType, PresenceStatus as DomainPresenceStatus};
use discord_rs::protocol::transport::{Relationship, TransportEvent};

#[test]
fn test_relationship_model_types_and_helpers() {
    let friend = Relationship {
        id: "user-1".into(),
        username: "alice".into(),
        avatar: Some("avatar1".into()),
        global_name: Some("Alice".into()),
        rel_type: Relationship::TYPE_FRIEND,
    };
    assert!(friend.is_friend());
    assert!(!friend.is_blocked());
    assert!(!friend.is_incoming_request());
    assert!(!friend.is_outgoing_request());

    let blocked = Relationship {
        id: "user-2".into(),
        username: "spammer".into(),
        avatar: None,
        global_name: None,
        rel_type: Relationship::TYPE_BLOCKED,
    };
    assert!(blocked.is_blocked());
    assert!(!blocked.is_friend());

    let incoming = Relationship {
        id: "user-3".into(),
        username: "bob".into(),
        avatar: None,
        global_name: Some("Bob".into()),
        rel_type: Relationship::TYPE_INCOMING_REQUEST,
    };
    assert!(incoming.is_incoming_request());

    let outgoing = Relationship {
        id: "user-4".into(),
        username: "carol".into(),
        avatar: None,
        global_name: Some("Carol".into()),
        rel_type: Relationship::TYPE_OUTGOING_REQUEST,
    };
    assert!(outgoing.is_outgoing_request());
}

#[test]
fn test_app_state_relationship_events() {
    let mut state = AppState::new();

    // 1. Initial bulk update via RelationshipsUpdated
    let rels = vec![
        Relationship {
            id: "101".into(),
            username: "friend_one".into(),
            avatar: Some("av101".into()),
            global_name: Some("Friend One".into()),
            rel_type: 1,
        },
        Relationship {
            id: "102".into(),
            username: "pending_incoming".into(),
            avatar: None,
            global_name: Some("Pending User".into()),
            rel_type: 3,
        },
    ];
    state.handle_transport_event(TransportEvent::RelationshipsUpdated(rels));

    assert_eq!(state.friends.len(), 2);
    let f1 = state.friends.iter().find(|f| f.id == "101").expect("friend 101 missing");
    assert_eq!(f1.relationship_type, 1);
    assert_eq!(f1.display_name, "Friend One");

    let f2 = state.friends.iter().find(|f| f.id == "102").expect("friend 102 missing");
    assert_eq!(f2.relationship_type, 3);

    // 2. Add another relationship dynamically via RelationshipAdd
    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "103".into(),
        username: "new_blocked".into(),
        avatar: None,
        global_name: None,
        rel_type: 2,
    }));

    assert_eq!(state.friends.len(), 3);
    let f3 = state.friends.iter().find(|f| f.id == "103").expect("friend 103 missing");
    assert_eq!(f3.relationship_type, 2);

    // 3. Remove relationship via RelationshipRemove
    state.handle_transport_event(TransportEvent::RelationshipRemove { id: "103".into() });
    assert_eq!(state.friends.len(), 2);
    assert!(state.friends.iter().all(|f| f.id != "103"));
}

#[test]
fn test_active_now_aggregation_from_presence() {
    let mut state = AppState::new();
    state.friends.clear();
    state.active_now_items.clear();

    // Add friend 1
    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "user-game".into(),
        username: "gamer_friend".into(),
        avatar: None,
        global_name: Some("Gamer".into()),
        rel_type: 1,
    }));

    // Presence update with Playing game
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: "user-game".into(),
        status: DomainPresenceStatus::Online,
        activity: Some(Activity {
            name: "Rust".into(),
            activity_type: ActivityType::Playing,
            details: Some("Playing on Official Server".into()),
            state: Some("In Lobby".into()),
            created_at: Some(1700000000),
            emoji: None,
        }),
        custom_status: None,
    });

    assert_eq!(state.active_now_items.len(), 1);
    let act = &state.active_now_items[0];
    assert_eq!(act.user_id, "user-game");
    assert_eq!(act.app_name, "Rust");
    assert_eq!(act.app_icon_symbol, "");
    assert!(act.header_subtitle.starts_with("Rust"));

    // Presence update with Listening to Spotify
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: "user-game".into(),
        status: DomainPresenceStatus::Online,
        activity: Some(Activity {
            name: "Spotify".into(),
            activity_type: ActivityType::Listening,
            details: Some("Song Title".into()),
            state: Some("Artist Name".into()),
            created_at: None,
            emoji: None,
        }),
        custom_status: None,
    });

    assert_eq!(state.active_now_items.len(), 1);
    let spotify_act = &state.active_now_items[0];
    assert_eq!(spotify_act.app_icon_symbol, "");
    assert_eq!(spotify_act.header_subtitle, "Spotify");
    assert_eq!(spotify_act.spotify_track.as_deref(), Some("Song Title"));
    assert_eq!(spotify_act.spotify_artist.as_deref(), Some("Artist Name"));

    // User goes offline -> removed from active now
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: "user-game".into(),
        status: DomainPresenceStatus::Offline,
        activity: None,
        custom_status: None,
    });
    assert_eq!(state.active_now_items.len(), 0);
}

#[test]
fn test_relationship_default_presence_is_offline() {
    let mut state = AppState::new();
    state.friends.clear();

    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "user-new".into(),
        username: "newbie".into(),
        avatar: None,
        global_name: Some("Newbie".into()),
        rel_type: 1,
    }));

    let f = state.friends.iter().find(|f| f.id == "user-new").unwrap();
    // Must default to Offline, not Online (Orchestrator rule 3)
    assert_eq!(f.presence, PresenceStatus::Offline);

    // Update with real presence
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: "user-new".into(),
        status: DomainPresenceStatus::Online,
        activity: None,
        custom_status: None,
    });
    let f = state.friends.iter().find(|f| f.id == "user-new").unwrap();
    assert_eq!(f.presence, PresenceStatus::Online);
}

#[tokio::test]
async fn test_send_friend_request_no_fake_line_and_result_handling() {
    let mut state = AppState::new();
    state.friends.clear();

    // 1. Empty username validation
    state.send_friend_request("   ");
    assert_eq!(
        state.add_friend_feedback,
        Some((false, "Insira um nome de usuário válido.".into()))
    );
    assert!(!state.is_sending_friend_request);

    // 2. Disconnected check (client_manager is None)
    state.send_friend_request("valid_user#1234");
    assert!(!state.is_sending_friend_request);
    assert_eq!(
        state.add_friend_feedback,
        Some((false, "Não conectado ao Discord.".into()))
    );

    // 3. Connect client_manager (mock)
    let (cm, _rx) = discord_rs::protocol::ClientManager::new_mock();
    state.client_manager = Some(cm);

    // 4. Sending friend request sets is_sending_friend_request without fake line
    state.send_friend_request("valid_user#1234");
    assert!(state.is_sending_friend_request);
    assert!(state.add_friend_feedback.is_none());
    // Crucial check: zero fake lines inserted in friends
    assert_eq!(state.friends.len(), 0);

    // 5. Simulating error from API/Provider
    state.handle_transport_event(TransportEvent::FriendRequestResult {
        username: "valid_user".into(),
        success: false,
        message: "Usuário não encontrado.".into(),
    });
    assert!(!state.is_sending_friend_request);
    assert_eq!(
        state.add_friend_feedback,
        Some((false, "Usuário não encontrado.".into()))
    );
    assert_eq!(state.friends.len(), 0);

    // 6. Simulating success from API/Provider
    state.is_sending_friend_request = true;
    state.handle_transport_event(TransportEvent::FriendRequestResult {
        username: "valid_user".into(),
        success: true,
        message: "Solicitação enviada com sucesso!".into(),
    });
    assert!(!state.is_sending_friend_request);
    assert_eq!(
        state.add_friend_feedback,
        Some((true, "Solicitação enviada com sucesso!".into()))
    );
    // Real item arrives via RelationshipAdd gateway event
    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "real-snowflake-12345".into(),
        username: "valid_user".into(),
        avatar: None,
        global_name: None,
        rel_type: 4, // Outgoing pending
    }));
    assert_eq!(state.friends.len(), 1);
    assert_eq!(state.friends[0].id, "real-snowflake-12345");
}

#[tokio::test]
async fn test_pending_relationship_actions_state() {
    let mut state = AppState::new();
    state.friends.clear();

    let (cm, _rx) = discord_rs::protocol::ClientManager::new_mock();
    state.client_manager = Some(cm);

    state.friends.push(FriendItemState {
        id: "target-user".into(),
        username: "target".into(),
        display_name: "Target".into(),
        avatar_url: None,
        avatar_text: "T".into(),
        presence: PresenceStatus::Offline,
        custom_status: None,
        relationship_type: 3, // Incoming request
        avatar_decoration_data: None,
    });

    state.accept_friend_request("target-user");
    assert!(state.pending_relationship_actions.contains("target-user"));

    // If action fails, event removes it from pending actions
    state.handle_transport_event(TransportEvent::RelationshipActionFailed {
        id: "target-user".into(),
        error: "Erro de rede".into(),
    });
    assert!(!state.pending_relationship_actions.contains("target-user"));
    assert_eq!(state.friends.len(), 1);

    // If action succeeds, RelationshipAdd removes it and updates relationship_type
    state.accept_friend_request("target-user");
    assert!(state.pending_relationship_actions.contains("target-user"));
    state.handle_transport_event(TransportEvent::RelationshipAdd(Relationship {
        id: "target-user".into(),
        username: "target".into(),
        avatar: None,
        global_name: Some("Target".into()),
        rel_type: 1, // now Friend
    }));
    assert!(!state.pending_relationship_actions.contains("target-user"));
    assert_eq!(state.friends[0].relationship_type, 1);
}
