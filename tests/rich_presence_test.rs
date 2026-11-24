use discord_rs::domain::rich_presence::{
    activity_details, activity_headline, activity_image_urls, activity_large_image_url,
    activity_small_image_url, activity_state, format_activity, format_activity_at,
    is_spotify_activity, resolve_asset_url, ActivityImageUrls, RichPresence, SpotifyProgress,
};
use discord_rs::domain::user::{
    Activity, ActivityAssets, ActivityTimestamps, ActivityType, CustomStatusEmoji,
};

fn spotify_activity() -> Activity {
    let mut activity = Activity::new("Spotify", ActivityType::Listening);
    activity.details = Some("Track Title".to_string());
    activity.state = Some("Artist One; Artist Two".to_string());
    activity.sync_id = Some("spotify-track-id".to_string());
    activity.timestamps = Some(ActivityTimestamps {
        start: Some(1_700_000_000_000),
        end: Some(1_700_000_180_000),
    });
    activity.assets = Some(ActivityAssets {
        large_image: Some("spotify:cover_hash".to_string()),
        large_text: Some("Album Name".to_string()),
        small_image: None,
        small_text: None,
    });
    activity
}

#[test]
fn headlines_match_discord_activity_verbs() {
    assert_eq!(
        activity_headline(&Activity::playing("Rust")).as_deref(),
        Some("Playing Rust")
    );
    assert_eq!(
        activity_headline(&Activity::new("Lo-fi Radio", ActivityType::Listening)).as_deref(),
        Some("Listening to Lo-fi Radio")
    );
    assert_eq!(
        activity_headline(&Activity::new("Live Coding", ActivityType::Streaming)).as_deref(),
        Some("Streaming Live Coding")
    );
    assert_eq!(
        activity_headline(&Activity::new("A Movie", ActivityType::Watching)).as_deref(),
        Some("Watching A Movie")
    );
    assert_eq!(
        activity_headline(&Activity::new("Rust Cup", ActivityType::Competing)).as_deref(),
        Some("Competing in Rust Cup")
    );
    assert_eq!(
        activity_headline(&Activity::custom("Working on Rust")).as_deref(),
        Some("Working on Rust")
    );
}

#[test]
fn absent_or_blank_text_never_becomes_a_placeholder() {
    let unnamed = Activity::new("   ", ActivityType::Playing);
    assert_eq!(activity_headline(&unnamed), None);

    let empty_custom = Activity::new("Custom Status", ActivityType::Custom);
    assert_eq!(activity_headline(&empty_custom), None);

    let mut activity = Activity::playing("Rust");
    activity.details = Some("   ".to_string());
    activity.state = Some(String::new());
    assert_eq!(activity_details(&activity), None);
    assert_eq!(activity_state(&activity), None);

    activity.details = Some("  Editing main.rs  ".to_string());
    activity.state = Some(" In a party ".to_string());
    assert_eq!(activity_details(&activity), Some("Editing main.rs"));
    assert_eq!(activity_state(&activity), Some("In a party"));
}

#[test]
fn spotify_detection_uses_real_payload_markers_only() {
    assert!(is_spotify_activity(&Activity::new("Spotify", ActivityType::Listening)));

    let mut via_sync_id = Activity::new("Music App", ActivityType::Listening);
    via_sync_id.sync_id = Some("track-id".to_string());
    assert!(is_spotify_activity(&via_sync_id));

    let mut via_asset = Activity::new("Music App", ActivityType::Listening);
    via_asset.assets = Some(ActivityAssets {
        large_image: Some("spotify:cover_hash".to_string()),
        ..ActivityAssets::default()
    });
    assert!(is_spotify_activity(&via_asset));

    let mut plain_listening = Activity::new("Radio", ActivityType::Listening);
    plain_listening.sync_id = Some("   ".to_string());
    assert!(!is_spotify_activity(&plain_listening));

    assert!(!is_spotify_activity(&Activity::new("Spotify", ActivityType::Playing)));
    assert!(!is_spotify_activity(&Activity::new("Rust", ActivityType::Playing)));
}

#[test]
fn spotify_progress_ratio_and_clock_text_are_derived_from_timestamps() {
    let activity = spotify_activity();
    let progress = SpotifyProgress::from_activity(&activity, 1_700_000_090_000).unwrap();
    assert_eq!(progress.start_ms, 1_700_000_000_000);
    assert_eq!(progress.end_ms, 1_700_000_180_000);
    assert_eq!(progress.total_ms, 180_000);
    assert_eq!(progress.elapsed_ms, 90_000);
    assert_eq!(progress.remaining_ms, 90_000);
    assert_eq!(progress.ratio, 0.5);
    assert_eq!(progress.ratio_percent(), 50.0);
    assert_eq!(progress.elapsed_text, "1:30");
    assert_eq!(progress.total_text, "3:00");
}

#[test]
fn spotify_progress_normalises_second_timestamps() {
    let progress = SpotifyProgress::from_timestamps(1_000, 1_300, 1_150).unwrap();
    assert_eq!(progress.start_ms, 1_000_000);
    assert_eq!(progress.end_ms, 1_300_000);
    assert_eq!(progress.now_ms, 1_150_000);
    assert_eq!(progress.ratio, 0.5);
    assert_eq!(progress.elapsed_text, "2:30");
    assert_eq!(progress.total_text, "5:00");
}

#[test]
fn spotify_progress_clamps_clock_to_the_real_interval() {
    let before =
        SpotifyProgress::from_timestamps(1_700_000_000_000, 1_700_000_300_000, 1_699_999_900_000)
            .unwrap();
    assert_eq!(before.elapsed_ms, 0);
    assert_eq!(before.ratio, 0.0);

    let after =
        SpotifyProgress::from_timestamps(1_700_000_000_000, 1_700_000_300_000, 1_700_001_000_000)
            .unwrap();
    assert_eq!(after.elapsed_ms, 300_000);
    assert_eq!(after.remaining_ms, 0);
    assert_eq!(after.ratio, 1.0);
}

#[test]
fn invalid_spotify_intervals_return_none_instead_of_fake_progress() {
    assert_eq!(SpotifyProgress::from_timestamps(1_300, 1_000, 1_150), None);
    assert_eq!(SpotifyProgress::from_timestamps(1_000, 1_000, 1_000), None);
    assert_eq!(SpotifyProgress::from_timestamps(-1, 1_300, 1_150), None);
    assert_eq!(SpotifyProgress::from_timestamps(1_000, 1_300, -1), None);
    assert_eq!(
        SpotifyProgress::from_timestamps(i64::MAX, 1_300, 1_150),
        None
    );

    let mut activity = Activity::new("Spotify", ActivityType::Listening);
    assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);

    activity.timestamps = Some(ActivityTimestamps {
        start: Some(1_000_000),
        end: None,
    });
    assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);

    activity.timestamps = Some(ActivityTimestamps {
        start: Some(1_300_000),
        end: Some(1_000_000),
    });
    assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);
}

#[test]
fn non_spotify_activity_never_exposes_spotify_metadata() {
    let mut activity = Activity::new("Radio", ActivityType::Listening);
    activity.details = Some("Show".to_string());
    activity.state = Some("Host".to_string());
    activity.timestamps = Some(ActivityTimestamps {
        start: Some(1_000_000),
        end: Some(1_300_000),
    });

    let presence = RichPresence::from_activity(&activity, 1_150_000);
    assert!(!presence.is_spotify());
    assert_eq!(presence.spotify_track, None);
    assert_eq!(presence.spotify_artist, None);
    assert_eq!(presence.spotify_album, None);
    assert_eq!(presence.spotify_progress, None);
}

#[test]
fn rich_presence_exposes_real_spotify_track_artist_album_and_progress() {
    let presence = RichPresence::from_activity(&spotify_activity(), 1_700_000_090_000);
    assert!(presence.is_spotify());
    assert_eq!(presence.activity_type, ActivityType::Listening);
    assert_eq!(presence.headline.as_deref(), Some("Listening to Spotify"));
    assert_eq!(presence.details.as_deref(), Some("Track Title"));
    assert_eq!(presence.state.as_deref(), Some("Artist One; Artist Two"));
    assert_eq!(presence.spotify_track.as_deref(), Some("Track Title"));
    assert_eq!(presence.spotify_artist.as_deref(), Some("Artist One; Artist Two"));
    assert_eq!(presence.spotify_album.as_deref(), Some("Album Name"));
    assert_eq!(presence.spotify_progress.as_ref().unwrap().ratio, 0.5);
    assert_eq!(
        presence.large_image_url.as_deref(),
        Some("https://i.scdn.co/image/cover_hash")
    );
}

#[test]
fn custom_status_keeps_real_text_and_emoji_without_fake_values() {
    let mut activity = Activity::custom_with_emoji(
        Some("Building".to_string()),
        Some(CustomStatusEmoji::custom("123456789", "ferris", false)),
    );
    activity.assets = None;

    let presence = RichPresence::from_activity(&activity, 1_150_000);
    assert_eq!(presence.headline.as_deref(), Some("Building"));
    assert_eq!(presence.state.as_deref(), Some("Building"));
    assert_eq!(presence.emoji.as_ref().unwrap().name, "ferris");
    assert_eq!(
        presence.emoji.as_ref().unwrap().image_url().as_deref(),
        Some("https://cdn.discordapp.com/emojis/123456789.png?size=48")
    );
    assert!(!presence.is_spotify());
    assert_eq!(presence.spotify_progress, None);

    let unicode = Activity::custom_with_emoji(
        None,
        Some(CustomStatusEmoji::unicode("🦀")),
    );
    let presence = RichPresence::from_activity(&unicode, 1_150_000);
    assert_eq!(presence.headline, None);
    assert_eq!(presence.emoji.as_ref().unwrap().image_url(), None);
}

#[test]
fn cdn_asset_urls_resolve_only_real_keys() {
    assert_eq!(
        resolve_asset_url("mp:external/abc123/https/example.com/art.png", None).as_deref(),
        Some("https://media.discordapp.net/external/abc123/https/example.com/art.png")
    );
    assert_eq!(
        resolve_asset_url("spotify:cover_hash", None).as_deref(),
        Some("https://i.scdn.co/image/cover_hash")
    );
    assert_eq!(
        resolve_asset_url("https://cdn.example.com/art.png", None).as_deref(),
        Some("https://cdn.example.com/art.png")
    );
    assert_eq!(
        resolve_asset_url("382910398472918273", Some("383508436472004608")).as_deref(),
        Some(
            "https://cdn.discordapp.com/app-assets/383508436472004608/382910398472918273.png"
        )
    );
    assert_eq!(resolve_asset_url("382910398472918273", None), None);
    assert_eq!(resolve_asset_url("   ", Some("383508436472004608")), None);
}

#[test]
fn activity_image_urls_use_application_assets_and_keep_missing_assets_empty() {
    let mut activity = Activity::playing("VS Code");
    activity.application_id = Some("383508436472004608".to_string());
    activity.assets = Some(ActivityAssets {
        large_image: Some("382910398472918273".to_string()),
        large_text: Some("Editing".to_string()),
        small_image: Some("mp:external/xyz/https/example.com/small.png".to_string()),
        small_text: Some("Status".to_string()),
    });

    let images = activity_image_urls(&activity);
    assert_eq!(
        images.large_url.as_deref(),
        Some(
            "https://cdn.discordapp.com/app-assets/383508436472004608/382910398472918273.png"
        )
    );
    assert_eq!(images.large_text.as_deref(), Some("Editing"));
    assert_eq!(
        images.small_url.as_deref(),
        Some("https://media.discordapp.net/external/xyz/https/example.com/small.png")
    );
    assert_eq!(images.small_text.as_deref(), Some("Status"));

    let mut bare = Activity::playing("Rust");
    assert_eq!(activity_large_image_url(&bare), None);
    assert_eq!(activity_small_image_url(&bare), None);
    assert_eq!(activity_image_urls(&bare), ActivityImageUrls::default());

    bare.application_id = Some("383508436472004608".to_string());
    bare.assets = Some(ActivityAssets {
        large_image: Some("   ".to_string()),
        ..ActivityAssets::default()
    });
    assert_eq!(activity_large_image_url(&bare), None);
}

#[test]
fn format_helpers_build_a_snapshot_at_a_concrete_instant() {
    let activity = spotify_activity();
    let presence = format_activity_at(&activity, 1_700_000_090_000);
    assert_eq!(presence.spotify_progress.as_ref().unwrap().ratio, 0.5);

    let now = format_activity(&Activity::playing("Rust"));
    assert_eq!(now.headline.as_deref(), Some("Playing Rust"));
    assert_eq!(now.activity_type, ActivityType::Playing);
    assert!(!now.is_spotify());
}

#[test]
fn formats_a_payload_shaped_like_a_real_discord_spotify_activity() {
    let mut activity = Activity::new("Spotify", ActivityType::Listening);
    activity.details = Some("Track Title".to_string());
    activity.state = Some("Artist One; Artist Two".to_string());
    activity.sync_id = Some("spotify-track-id".to_string());
    activity.created_at = Some(1_700_000_000_000);
    activity.application_id = Some("383508436472004608".to_string());
    activity.timestamps = Some(ActivityTimestamps {
        start: Some(1_700_000_000_000),
        end: Some(1_700_000_180_000),
    });
    activity.assets = Some(ActivityAssets {
        large_image: Some("spotify:cover_hash".to_string()),
        large_text: Some("Album Name".to_string()),
        small_image: None,
        small_text: None,
    });

    assert!(is_spotify_activity(&activity));
    assert_eq!(activity.activity_type, ActivityType::Listening);
    assert_eq!(
        activity_headline(&activity).as_deref(),
        Some("Listening to Spotify")
    );

    let presence = RichPresence::from_activity(&activity, 1_700_000_090_000);
    assert_eq!(presence.spotify_track.as_deref(), Some("Track Title"));
    assert_eq!(
        presence.spotify_artist.as_deref(),
        Some("Artist One; Artist Two")
    );
    assert_eq!(presence.spotify_album.as_deref(), Some("Album Name"));
    assert_eq!(
        presence.large_image_url.as_deref(),
        Some("https://i.scdn.co/image/cover_hash")
    );
    assert_eq!(presence.spotify_progress.unwrap().ratio, 0.5);
}

#[test]
fn formats_streaming_watching_competing_and_playing_activities() {
    let activities = [
        (ActivityType::Playing, "Rust", "Playing Rust"),
        (ActivityType::Streaming, "Live Coding", "Streaming Live Coding"),
        (ActivityType::Watching, "A Movie", "Watching A Movie"),
        (ActivityType::Competing, "Rust Cup", "Competing in Rust Cup"),
    ];

    for (kind, name, expected) in activities {
        let activity = Activity::new(name, kind);
        assert_eq!(activity_headline(&activity).as_deref(), Some(expected));
        assert!(!is_spotify_activity(&activity));
    }
}
