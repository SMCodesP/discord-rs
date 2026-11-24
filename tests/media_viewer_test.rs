extern crate gpui_kit as gpui;

use discord_rs::components::media_viewer::{
    browser_open_command, collect_media_items, MediaViewerKind, MediaViewerState,
    MEDIA_VIEWER_MAX_ZOOM, MEDIA_VIEWER_MIN_ZOOM,
};
use discord_rs::domain::message::Attachment;

fn att(id: &str, name: &str, content_type: &str, url: &str) -> Attachment {
    Attachment {
        id: id.to_string(),
        filename: name.to_string(),
        size: 1024,
        url: url.to_string(),
        proxy_url: Some(format!("https://media.discordapp.net/{id}/{name}")),
        content_type: Some(content_type.to_string()),
        width: Some(1280),
        height: Some(720),
    }
}

#[test]
fn gallery_collects_media_dedups_and_ignores_non_media() {
    let attachments = vec![
        att("a1", "photo.png", "image/png", "https://cdn.discordapp.com/a1.png"),
        att("a2", "clip.mp4", "video/mp4", "https://cdn.discordapp.com/a2.mp4"),
        att("a1", "photo-dup.png", "image/png", "https://cdn.discordapp.com/a1-dup.png"),
        Attachment {
            id: "a3".into(),
            filename: "notes.txt".into(),
            size: 10,
            url: "https://cdn.discordapp.com/notes.txt".into(),
            proxy_url: None,
            content_type: Some("text/plain".into()),
            width: None,
            height: None,
        },
    ];

    let items = collect_media_items(&attachments);
    assert_eq!(items.len(), 2, "deve manter apenas mídia e deduplicar por id");
    assert_eq!(items[0].id, "a1");
    assert_eq!(items[1].id, "a2");
    assert_eq!(items[0].kind, MediaViewerKind::Image);
    assert_eq!(items[1].kind, MediaViewerKind::Video);
}

#[test]
fn single_attachment_opens_and_closes() {
    let attachments = vec![att("a1", "single.jpg", "image/jpeg", "https://cdn.discordapp.com/a1.jpg")];
    let mut state = MediaViewerState::from_attachments(&attachments, Some("a1"));
    assert!(state.is_open());
    assert_eq!(state.len(), 1);
    assert_eq!(state.current().unwrap().id, "a1");
    assert!(!state.can_next());
    assert!(!state.can_prev());

    assert!(state.handle_key("escape"));
    assert!(!state.is_open());
    assert!(state.current().is_none());
}

#[test]
fn three_attachments_navigation_and_keyboard() {
    let attachments = vec![
        att("a1", "one.png", "image/png", "https://cdn.discordapp.com/1.png"),
        att("a2", "two.gif", "image/gif", "https://cdn.discordapp.com/2.gif"),
        att("a3", "three.mp4", "video/mp4", "https://cdn.discordapp.com/3.mp4"),
    ];
    let mut state = MediaViewerState::from_attachments(&attachments, Some("a2"));
    assert_eq!(state.index, 1);
    assert_eq!(state.current().unwrap().kind, MediaViewerKind::Gif);

    assert!(state.handle_key("arrowright"));
    assert_eq!(state.index, 2);
    assert_eq!(state.current().unwrap().kind, MediaViewerKind::Video);
    assert!(!state.handle_key("arrowright"), "fim da galeria não deve avançar");

    assert!(state.handle_key("arrowleft"));
    assert_eq!(state.index, 1);
    assert!(state.prev());
    assert_eq!(state.index, 0);
    assert!(!state.prev(), "início da galeria não deve voltar");
}

#[test]
fn zoom_is_limited_and_pan_clamped() {
    let attachments = vec![att("a1", "zoom.png", "image/png", "https://cdn.discordapp.com/z.png")];
    let mut state = MediaViewerState::from_attachments(&attachments, None);
    assert_eq!(state.zoom, MEDIA_VIEWER_MIN_ZOOM);

    for _ in 0..50 {
        state.zoom_in();
    }
    assert!(state.zoom <= MEDIA_VIEWER_MAX_ZOOM);
    assert_eq!(state.zoom, MEDIA_VIEWER_MAX_ZOOM);

    assert!(state.pan_by(10_000.0, -10_000.0));
    let limit = (MEDIA_VIEWER_MAX_ZOOM - MEDIA_VIEWER_MIN_ZOOM) * 220.0;
    assert!(state.pan_x <= limit && state.pan_x >= -limit);
    assert!(state.pan_y <= limit && state.pan_y >= -limit);

    for _ in 0..50 {
        state.zoom_out();
    }
    assert_eq!(state.zoom, MEDIA_VIEWER_MIN_ZOOM);
    assert_eq!(state.pan_x, 0.0);
    assert_eq!(state.pan_y, 0.0);
}

#[test]
fn video_is_not_zoomable_but_is_navigable() {
    let attachments = vec![
        att("v1", "movie.mp4", "video/mp4", "https://cdn.discordapp.com/movie.mp4"),
        att("i1", "img.png", "image/png", "https://cdn.discordapp.com/img.png"),
    ];
    let mut state = MediaViewerState::from_attachments(&attachments, Some("v1"));
    assert_eq!(state.current().unwrap().kind, MediaViewerKind::Video);
    assert!(!state.zoom_in());
    assert!(!state.toggle_zoom());
    assert_eq!(state.zoom, MEDIA_VIEWER_MIN_ZOOM);

    assert!(state.next());
    assert!(state.toggle_zoom());
    assert!(state.zoom > MEDIA_VIEWER_MIN_ZOOM);
}

#[test]
fn open_and_download_targets_use_real_urls() {
    let attachments = vec![att("a1", "real.png", "image/png", "https://cdn.discordapp.com/real.png")];
    let state = MediaViewerState::from_attachments(&attachments, Some("a1"));
    let open = state.browser_target().expect("deve ter alvo de navegador");
    assert!(open.starts_with("https://"));
    let dl = state.download_target().expect("deve ter alvo de download");
    assert_eq!(dl.filename, "real.png");
    assert!(dl.url.starts_with("https://"));

    let cmd = browser_open_command(&open);
    assert!(!cmd.is_empty());
    assert!(cmd.iter().any(|part| part.contains("https://")));
}
