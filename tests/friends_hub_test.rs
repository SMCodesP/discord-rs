use discord_rs::app::state::{
    AppState, FriendItemState, FriendsTab, MessageRequestItem, MessageRequestsSubTab,
    PresenceStatus,
};
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
        avatar_decoration_data: None,
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
        avatar_decoration_data: None,
    };
    assert!(blocked.is_blocked());
    assert!(!blocked.is_friend());

    let incoming = Relationship {
        id: "user-3".into(),
        username: "bob".into(),
        avatar: None,
        global_name: Some("Bob".into()),
        rel_type: Relationship::TYPE_INCOMING_REQUEST,
        avatar_decoration_data: None,
    };
    assert!(incoming.is_incoming_request());

    let outgoing = Relationship {
        id: "user-4".into(),
        username: "carol".into(),
        avatar: None,
        global_name: Some("Carol".into()),
        rel_type: Relationship::TYPE_OUTGOING_REQUEST,
        avatar_decoration_data: None,
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
            avatar_decoration_data: None,
        },
        Relationship {
            id: "102".into(),
            username: "pending_incoming".into(),
            avatar: None,
            global_name: Some("Pending User".into()),
            rel_type: 3,
            avatar_decoration_data: None,
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
        avatar_decoration_data: None,
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
        avatar_decoration_data: None,
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
        avatar_decoration_data: None,
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
        avatar_decoration_data: None,
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
        avatar_decoration_data: None,
    }));
    assert!(!state.pending_relationship_actions.contains("target-user"));
    assert_eq!(state.friends[0].relationship_type, 1);
}

#[test]
fn test_friends_tab_selection_covers_all_tabs() {
    let mut state = AppState::new();
    assert_eq!(state.friends_tab, FriendsTab::Online);
    assert_eq!(
        state.message_requests_subtab,
        MessageRequestsSubTab::YouMayKnow
    );

    for tab in [
        FriendsTab::All,
        FriendsTab::Pending,
        FriendsTab::Blocked,
        FriendsTab::AddFriend,
        FriendsTab::MessageRequests,
        FriendsTab::Online,
    ] {
        state.select_friends_tab(tab);
        assert_eq!(state.friends_tab, tab);
    }

    state.select_message_requests_subtab(MessageRequestsSubTab::Spam);
    assert_eq!(
        state.message_requests_subtab,
        MessageRequestsSubTab::Spam
    );
    state.select_message_requests_subtab(MessageRequestsSubTab::YouMayKnow);
    assert_eq!(
        state.message_requests_subtab,
        MessageRequestsSubTab::YouMayKnow
    );
}

#[test]
fn test_tab_badge_counts_match_view_filters() {
    let mut state = AppState::new();
    state.friends.clear();
    state.message_requests.clear();

    for (id, rel_type) in [
        ("u-1", 1),
        ("u-2", 1),
        ("u-3", 3),
        ("u-4", 4),
        ("u-5", 2),
    ] {
        state.friends.push(FriendItemState {
            id: id.into(),
            username: format!("user_{}", id),
            display_name: format!("User {}", id),
            avatar_url: None,
            avatar_text: "U".into(),
            presence: PresenceStatus::Online,
            custom_status: None,
            avatar_decoration_data: None,
            relationship_type: rel_type,
        });
    }

    // Same predicate the hub header uses for the "Pendente" badge.
    let pending_friends = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 3)
        .count();
    assert_eq!(pending_friends, 1);

    for (id, is_spam) in [("req-1", false), ("req-2", false), ("req-3", true)] {
        state.message_requests.push(MessageRequestItem {
            id: id.into(),
            user_id: format!("user_{}", id),
            username: format!("user_{}", id),
            display_name: format!("User {}", id),
            avatar_url: None,
            avatar_text: "U".into(),
            presence: PresenceStatus::Online,
            mutual_guilds: 0,
            mutual_guild_name: None,
            preview_message: "Olá!".into(),
            timestamp: "Hoje".into(),
            is_spam,
        });
    }

    // Same predicates the hub header/tabs use for badges and filtering.
    let pending_req = state
        .message_requests
        .iter()
        .filter(|r| !r.is_spam)
        .count();
    assert_eq!(pending_req, 2);
    let spam = state
        .message_requests
        .iter()
        .filter(|r| r.is_spam)
        .count();
    assert_eq!(spam, 1);

    // Spam clear keeps only real (non-spam) requests.
    state.clear_spam_requests();
    assert_eq!(state.message_requests.len(), 2);
    assert!(state.message_requests.iter().all(|r| !r.is_spam));
}

#[test]
fn test_friend_item_avatar_decoration_propagation() {
    let mut state = AppState::new();
    let deco = discord_rs::domain::user::AvatarDecorationData::new("a_decor_asset_123");
    state.handle_transport_event(TransportEvent::RelationshipsUpdated(vec![
        Relationship {
            id: "u-decor".into(),
            username: "DecoratedUser".into(),
            avatar: None,
            global_name: Some("Decorated".into()),
            rel_type: 1,
            avatar_decoration_data: Some(deco.clone()),
        },
    ]));

    assert_eq!(state.friends.len(), 1);
    assert_eq!(
        state.friends[0].avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
        Some("a_decor_asset_123")
    );
}

#[test]
fn test_friends_view_filter_predicates() {
    let mut state = AppState::new();
    state.friends.clear();

    state.friends.push(FriendItemState {
        id: "u-online".into(),
        username: "user_online".into(),
        display_name: "Online Friend".into(),
        avatar_url: None,
        avatar_text: "O".into(),
        presence: PresenceStatus::Online,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 1,
    });
    state.friends.push(FriendItemState {
        id: "u-offline".into(),
        username: "user_offline".into(),
        display_name: "Offline Friend".into(),
        avatar_url: None,
        avatar_text: "F".into(),
        presence: PresenceStatus::Offline,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 1,
    });
    state.friends.push(FriendItemState {
        id: "u-incoming".into(),
        username: "user_incoming".into(),
        display_name: "Incoming Request".into(),
        avatar_url: None,
        avatar_text: "I".into(),
        presence: PresenceStatus::Online,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 3,
    });
    state.friends.push(FriendItemState {
        id: "u-outgoing".into(),
        username: "user_outgoing".into(),
        display_name: "Outgoing Request".into(),
        avatar_url: None,
        avatar_text: "G".into(),
        presence: PresenceStatus::Online,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 4,
    });
    state.friends.push(FriendItemState {
        id: "u-blocked".into(),
        username: "user_blocked".into(),
        display_name: "Blocked User".into(),
        avatar_url: None,
        avatar_text: "B".into(),
        presence: PresenceStatus::Online,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 2,
    });

    // Online tab: rel_type == 1 && presence != Offline
    let online_count = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 1 && f.presence != PresenceStatus::Offline)
        .count();
    assert_eq!(online_count, 1);

    // All tab: rel_type == 1
    let all_count = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 1)
        .count();
    assert_eq!(all_count, 2);

    // Pending tab: rel_type == 3 || rel_type == 4
    let pending_count = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 3 || f.relationship_type == 4)
        .count();
    assert_eq!(pending_count, 2);

    // Blocked tab: rel_type == 2
    let blocked_count = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 2)
        .count();
    assert_eq!(blocked_count, 1);
}

#[test]
fn test_friends_search_filter_in_memory() {
    let mut state = AppState::new();
    state.friends.clear();
    state.friends_tab = FriendsTab::All;
    state.friends.push(FriendItemState {
        id: "u-1".into(),
        username: "alice_wonderland".into(),
        display_name: "Alice Liddell".into(),
        avatar_url: None,
        avatar_text: "A".into(),
        presence: PresenceStatus::Online,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 1,
    });
    state.friends.push(FriendItemState {
        id: "u-2".into(),
        username: "bob_builder".into(),
        display_name: "Robert Bob".into(),
        avatar_url: None,
        avatar_text: "B".into(),
        presence: PresenceStatus::Idle,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 1,
    });
    state.friends.push(FriendItemState {
        id: "u-3".into(),
        username: "charlie_chaplin".into(),
        display_name: "The Tramp".into(),
        avatar_url: None,
        avatar_text: "C".into(),
        presence: PresenceStatus::Offline,
        custom_status: None,
        avatar_decoration_data: None,
        relationship_type: 1,
    });

    // 1. Empty query returns all friends in the active tab (All -> 3)
    let rows_empty = discord_rs::app::friends_hub::collect_friend_rows(&state, "");
    assert_eq!(rows_empty.len(), 3);

    // 2. Filter matching display name ("alice")
    let rows_alice = discord_rs::app::friends_hub::collect_friend_rows(&state, "alice");
    assert_eq!(rows_alice.len(), 1);
    assert_eq!(rows_alice[0].user_id, "u-1");
    assert_eq!(rows_alice[0].display_name, "Alice Liddell");

    // 3. Filter matching username only ("chaplin" -> Charlie with display_name "The Tramp")
    let rows_chaplin = discord_rs::app::friends_hub::collect_friend_rows(&state, "chaplin");
    assert_eq!(rows_chaplin.len(), 1);
    assert_eq!(rows_chaplin[0].user_id, "u-3");

    // 4. Case-insensitivity ("BOB" matches "bob_builder" and "Robert Bob")
    let rows_bob = discord_rs::app::friends_hub::collect_friend_rows(&state, "BOB");
    assert_eq!(rows_bob.len(), 1);
    assert_eq!(rows_bob[0].user_id, "u-2");

    // 5. Query matching nothing returns empty list
    let rows_none = discord_rs::app::friends_hub::collect_friend_rows(&state, "nonexistent");
    assert!(rows_none.is_empty());

    // 6. Tab filtering combined with search query:
    // Charlie is offline, so searching "chaplin" in Online tab yields 0 rows
    state.friends_tab = FriendsTab::Online;
    let rows_online_c = discord_rs::app::friends_hub::collect_friend_rows(&state, "chaplin");
    assert!(rows_online_c.is_empty());

    // Alice is online, so searching "alice" in Online tab yields 1 row
    let rows_online_alice = discord_rs::app::friends_hub::collect_friend_rows(&state, "alice");
    assert_eq!(rows_online_alice.len(), 1);
    assert_eq!(rows_online_alice[0].user_id, "u-1");
}


