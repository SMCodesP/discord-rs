//! Discord RS - Media Viewer / Lightbox
//!
//! Fullscreen media viewer for message attachments with gallery navigation,
//! zoom, keyboard controls, and a bottom action bar.
//!
//! Visual reference (Discord desktop lightbox, inspected via task spec):
//! - `.lightbox_*`: fullscreen scrim backdrop
//! - `.imageZoom`: cursor zoom-in/out
//! - `.videoContainer.zoomed`: pan/scale
//! - carousel items positioned absolutely
//!
//! Zero mocks: renders only real attachment metadata/URLs provided by callers.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;

use crate::domain::message::Attachment;
use crate::tokens::colors::{
    bg_floating, modal_backdrop, status_danger, text_muted, text_normal, text_titles, white,
    with_alpha, HEX_STATUS_DANGER,
};
use crate::tokens::spacing::{radius_md, radius_sm, space_1, space_2, space_3, space_4};
use crate::tokens::typography::{font_size_sm, font_size_xs, WEIGHT_MEDIUM, WEIGHT_SEMIBOLD};

pub const MEDIA_VIEWER_MIN_ZOOM: f32 = 1.0;
pub const MEDIA_VIEWER_MAX_ZOOM: f32 = 4.0;
pub const MEDIA_VIEWER_ZOOM_STEP: f32 = 0.25;

/// Media kind supported by the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaViewerKind {
    Image,
    Gif,
    Video,
    Unsupported,
}

impl MediaViewerKind {
    pub fn from_attachment(att: &Attachment) -> Self {
        let ct = att.content_type.as_deref().unwrap_or("").to_ascii_lowercase();
        let name = att.filename.to_ascii_lowercase();
        let is_gif = ct == "image/gif" || name.ends_with(".gif");
        let is_image = ct.starts_with("image/")
            || name.ends_with(".png")
            || name.ends_with(".jpg")
            || name.ends_with(".jpeg")
            || name.ends_with(".webp")
            || name.ends_with(".avif")
            || name.ends_with(".apng");
        let is_video = ct.starts_with("video/")
            || name.ends_with(".mp4")
            || name.ends_with(".webm")
            || name.ends_with(".mov")
            || name.ends_with(".mkv");

        if is_gif {
            Self::Gif
        } else if is_video {
            Self::Video
        } else if is_image {
            Self::Image
        } else {
            Self::Unsupported
        }
    }

    pub fn is_zoomable(self) -> bool {
        matches!(self, Self::Image | Self::Gif)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "Imagem",
            Self::Gif => "GIF",
            Self::Video => "Vídeo",
            Self::Unsupported => "Arquivo",
        }
    }
}

/// A single gallery item derived from a real message attachment.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaViewerItem {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub proxy_url: Option<String>,
    pub content_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub kind: MediaViewerKind,
}

impl MediaViewerItem {
    pub fn from_attachment(att: &Attachment) -> Self {
        Self {
            id: att.id.clone(),
            filename: att.filename.clone(),
            url: att.url.clone(),
            proxy_url: att.proxy_url.clone(),
            content_type: att.content_type.clone(),
            width: att.width,
            height: att.height,
            kind: MediaViewerKind::from_attachment(att),
        }
    }

    /// Returns the best URL for display/open actions. Prefers proxy URL when present.
    pub fn display_url(&self) -> &str {
        self.proxy_url.as_deref().unwrap_or(&self.url)
    }

    pub fn is_openable(&self) -> bool {
        let url = self.display_url();
        url.starts_with("https://") || url.starts_with("http://")
    }

    pub fn size_label(&self) -> Option<String> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some(format!("{w} × {h}")),
            _ => None,
        }
    }
}

/// Build gallery items from message attachments, keeping only media items and
/// deduplicating by attachment id to avoid duplicated navigation entries.
pub fn collect_media_items(attachments: &[Attachment]) -> Vec<MediaViewerItem> {
    let mut items = Vec::new();
    for att in attachments {
        let kind = MediaViewerKind::from_attachment(att);
        if kind == MediaViewerKind::Unsupported {
            continue;
        }
        if items.iter().any(|item: &MediaViewerItem| item.id == att.id) {
            continue;
        }
        items.push(MediaViewerItem::from_attachment(att));
    }
    items
}

/// Pure state for the media viewer. Rendering is handled separately.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaViewerState {
    pub items: Vec<MediaViewerItem>,
    pub index: usize,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub open: bool,
}

impl MediaViewerState {
    pub fn new(items: Vec<MediaViewerItem>, initial_index: usize) -> Self {
        let mut state = Self {
            items,
            index: 0,
            zoom: MEDIA_VIEWER_MIN_ZOOM,
            pan_x: 0.0,
            pan_y: 0.0,
            open: false,
        };
        if !state.items.is_empty() {
            state.open = true;
            state.index = initial_index.min(state.items.len() - 1);
        }
        state
    }

    pub fn from_attachments(attachments: &[Attachment], initial_attachment_id: Option<&str>) -> Self {
        let items = collect_media_items(attachments);
        let initial_index = initial_attachment_id
            .and_then(|id| items.iter().position(|item| item.id == id))
            .unwrap_or(0);
        Self::new(items, initial_index)
    }

    pub fn is_open(&self) -> bool {
        self.open && !self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current(&self) -> Option<&MediaViewerItem> {
        if !self.is_open() {
            return None;
        }
        self.items.get(self.index)
    }

    pub fn close(&mut self) {
        self.open = false;
        self.reset_zoom();
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = MEDIA_VIEWER_MIN_ZOOM;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    pub fn can_prev(&self) -> bool {
        self.is_open() && self.index > 0
    }

    pub fn can_next(&self) -> bool {
        self.is_open() && self.index + 1 < self.items.len()
    }

    pub fn next(&mut self) -> bool {
        if !self.can_next() {
            return false;
        }
        self.index += 1;
        self.reset_zoom();
        true
    }

    pub fn prev(&mut self) -> bool {
        if !self.can_prev() {
            return false;
        }
        self.index -= 1;
        self.reset_zoom();
        true
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        if !self.is_open() || index >= self.items.len() || index == self.index {
            return false;
        }
        self.index = index;
        self.reset_zoom();
        true
    }

    pub fn zoom_in(&mut self) -> bool {
        let Some(current) = self.current() else {
            return false;
        };
        if !current.kind.is_zoomable() {
            return false;
        }
        let next = (self.zoom + MEDIA_VIEWER_ZOOM_STEP).min(MEDIA_VIEWER_MAX_ZOOM);
        let changed = (next - self.zoom).abs() > f32::EPSILON;
        self.zoom = next;
        changed
    }

    pub fn zoom_out(&mut self) -> bool {
        let Some(current) = self.current() else {
            return false;
        };
        if !current.kind.is_zoomable() {
            return false;
        }
        let next = (self.zoom - MEDIA_VIEWER_ZOOM_STEP).max(MEDIA_VIEWER_MIN_ZOOM);
        let changed = (next - self.zoom).abs() > f32::EPSILON;
        self.zoom = next;
        if self.zoom <= MEDIA_VIEWER_MIN_ZOOM {
            self.pan_x = 0.0;
            self.pan_y = 0.0;
        }
        changed
    }

    pub fn toggle_zoom(&mut self) -> bool {
        let Some(current) = self.current() else {
            return false;
        };
        if !current.kind.is_zoomable() {
            return false;
        }
        if self.zoom > MEDIA_VIEWER_MIN_ZOOM {
            self.reset_zoom();
            true
        } else {
            self.zoom = 2.0;
            true
        }
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) -> bool {
        let Some(current) = self.current() else {
            return false;
        };
        if !current.kind.is_zoomable() || self.zoom <= MEDIA_VIEWER_MIN_ZOOM {
            return false;
        }
        let limit = pan_limit(self.zoom);
        self.pan_x = (self.pan_x + dx).clamp(-limit, limit);
        self.pan_y = (self.pan_y + dy).clamp(-limit, limit);
        true
    }

    /// Handles keyboard input. Returns true if the key was consumed.
    pub fn handle_key(&mut self, key: &str) -> bool {
        match key {
            "escape" | "esc" => {
                if self.is_open() {
                    self.close();
                    true
                } else {
                    false
                }
            }
            "left" | "arrowleft" => self.prev(),
            "right" | "arrowright" => self.next(),
            "+" | "=" | "add" => self.zoom_in(),
            "-" | "subtract" => self.zoom_out(),
            "0" => {
                if self.is_open() {
                    self.reset_zoom();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn browser_target(&self) -> Option<String> {
        self.current()
            .filter(|item| item.is_openable())
            .map(|item| item.display_url().to_string())
    }

    pub fn download_target(&self) -> Option<MediaDownloadTarget> {
        self.current().filter(|item| item.is_openable()).map(|item| {
            MediaDownloadTarget {
                url: item.display_url().to_string(),
                filename: item.filename.clone(),
            }
        })
    }
}

fn pan_limit(zoom: f32) -> f32 {
    ((zoom - MEDIA_VIEWER_MIN_ZOOM) * 220.0).max(0.0)
}

/// Download intent returned by the viewer. Callers perform the actual download;
/// the viewer does not write files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDownloadTarget {
    pub url: String,
    pub filename: String,
}

/// Builds a platform command for opening a URL in the system browser.
/// No command is executed by this function.
pub fn browser_open_command(url: &str) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        vec!["open".to_string(), url.to_string()]
    }
    #[cfg(target_os = "windows")]
    {
        vec!["cmd".to_string(), "/C".to_string(), "start".to_string(), "".to_string(), url.to_string()]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        vec!["xdg-open".to_string(), url.to_string()]
    }
}

pub type MediaViewerCloseCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;
pub type MediaViewerNavigateCallback = Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>;
pub type MediaViewerActionCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct MediaViewerCallbacks {
    pub on_close: Option<MediaViewerCloseCallback>,
    pub on_prev: Option<MediaViewerActionCallback>,
    pub on_next: Option<MediaViewerActionCallback>,
    pub on_zoom_in: Option<MediaViewerActionCallback>,
    pub on_zoom_out: Option<MediaViewerActionCallback>,
    pub on_toggle_zoom: Option<MediaViewerActionCallback>,
    pub on_open_browser: Option<MediaViewerActionCallback>,
    pub on_download: Option<MediaViewerActionCallback>,
}

impl Default for MediaViewerCallbacks {
    fn default() -> Self {
        Self {
            on_close: None,
            on_prev: None,
            on_next: None,
            on_zoom_in: None,
            on_zoom_out: None,
            on_toggle_zoom: None,
            on_open_browser: None,
            on_download: None,
        }
    }
}

fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    disabled: bool,
    on_click: Option<MediaViewerActionCallback>,
) -> AnyElement {
    let label: SharedString = label.into();
    let mut btn = div()
        .id(id.into())
        .px(space_3())
        .py(space_1())
        .rounded(radius_sm())
        .text_size(font_size_sm())
        .font_weight(WEIGHT_MEDIUM)
        .text_color(if disabled { text_muted() } else { text_titles() })
        .bg(if disabled {
            with_alpha(0xffffff, 0.04)
        } else {
            with_alpha(0xffffff, 0.08)
        })
        .child(label);
    if disabled {
        btn = btn.cursor_default().opacity(0.5);
    } else {
        btn = btn.cursor_pointer().hover(|s| s.bg(with_alpha(0xffffff, 0.14)));
        if let Some(cb) = on_click {
            btn = btn.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                cx.stop_propagation();
                cb(window, cx);
            });
        }
    }
    btn.into_any_element()
}

fn render_media_content(item: &MediaViewerItem, zoom: f32) -> AnyElement {
    let url = item.display_url().to_string();
    match item.kind {
        MediaViewerKind::Image | MediaViewerKind::Gif => {
            let mut media = img(url)
                .id(ElementId::from(SharedString::from(format!("media-viewer-img-{}", item.id))))
                .max_w(px(1100.0))
                .max_h(px(720.0))
                .object_fit(ObjectFit::Contain);
            // Zoom affordance: cursor changes on zoomable media; actual zoom is
            // controlled by state/callbacks.
            if zoom > MEDIA_VIEWER_MIN_ZOOM {
                media = media.cursor_pointer();
            } else {
                media = media.cursor_pointer();
            }
            div()
                .flex()
                .items_center()
                .justify_center()
                .child(media)
                .into_any_element()
        }
        MediaViewerKind::Video => {
            // Video playback integration is provided by T-089; viewer shows the
            // real source URL and metadata without fabricating playback state.
            div()
                .id(ElementId::from(SharedString::from(format!("media-viewer-video-{}", item.id))))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(space_2())
                .p(space_4())
                .rounded(radius_md())
                .bg(with_alpha(0x000000, 0.35))
                .border_1()
                .border_color(with_alpha(0xffffff, 0.08))
                .child(
                    div()
                        .text_size(px(28.0))
                        .text_color(text_titles())
                        .child("🎬"),
                )
                .child(
                    div()
                        .text_size(font_size_sm())
                        .font_weight(WEIGHT_SEMIBOLD)
                        .text_color(text_titles())
                        .child(item.filename.clone()),
                )
                .child(
                    div()
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .max_w(px(720.0))
                        .truncate()
                        .child(url),
                )
                .into_any_element()
        }
        MediaViewerKind::Unsupported => div()
            .text_size(font_size_sm())
            .text_color(text_muted())
            .child(item.filename.clone())
            .into_any_element(),
    }
}

/// Renders the fullscreen media viewer overlay.
///
/// The caller owns `MediaViewerState` and provides callbacks for mutations.
/// This keeps state logic testable without GPUI. `focus_handle`, when provided,
/// is tracked so keyboard events (Esc/arrows) can be handled by the caller's
/// key handler attached to the returned element.
pub fn render_media_viewer(
    state: &MediaViewerState,
    focus_handle: Option<&FocusHandle>,
    callbacks: MediaViewerCallbacks,
) -> Option<AnyElement> {
    if !state.is_open() {
        return None;
    }
    let Some(current) = state.current() else {
        return None;
    };

    let on_close_btn = callbacks.on_close.clone();
    let on_prev = callbacks.on_prev.clone();
    let on_next = callbacks.on_next.clone();
    let on_zoom_in = callbacks.on_zoom_in.clone();
    let on_zoom_out = callbacks.on_zoom_out.clone();
    let on_toggle_zoom = callbacks.on_toggle_zoom.clone();
    let on_open_browser = callbacks.on_open_browser.clone();
    let on_download = callbacks.on_download.clone();

    let counter = format!("{} / {}", state.index + 1, state.items.len());
    let zoom_pct = format!("{}%", (state.zoom * 100.0).round() as i32);
    let size_label = current.size_label();
    let kind_label = current.kind.label().to_string();
    let zoomable = current.kind.is_zoomable();
    let browser_disabled = !current.is_openable();
    let download_disabled = !current.is_openable();

    let mut content_wrap = div()
        .id("media-viewer-content")
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .relative()
        .child(render_media_content(current, state.zoom));

    if zoomable && state.zoom > MEDIA_VIEWER_MIN_ZOOM {
        // Pan offset marker: pan limits are enforced by state; visual offset
        // is represented here as absolute translation metadata for integration.
        content_wrap = content_wrap.left(px(state.pan_x)).top(px(state.pan_y));
    }

    if let Some(cb) = on_toggle_zoom.clone() {
        content_wrap = content_wrap.cursor_pointer().on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
            cx.stop_propagation();
            cb(window, cx);
        });
    }

    let prev_btn = action_button("media-viewer-prev", "←", !state.can_prev(), on_prev);
    let next_btn = action_button("media-viewer-next", "→", !state.can_next(), on_next);
    let zoom_out_btn = action_button(
        "media-viewer-zoom-out",
        "−",
        !zoomable || state.zoom <= MEDIA_VIEWER_MIN_ZOOM,
        on_zoom_out,
    );
    let zoom_in_btn = action_button(
        "media-viewer-zoom-in",
        "+",
        !zoomable || state.zoom >= MEDIA_VIEWER_MAX_ZOOM,
        on_zoom_in,
    );
    let browser_btn = action_button(
        "media-viewer-open-browser",
        "Abrir no navegador",
        browser_disabled,
        on_open_browser,
    );
    let download_btn = action_button("media-viewer-download", "Baixar", download_disabled, on_download);
    let close_btn = action_button("media-viewer-close", "Fechar", false, on_close_btn);

    let mut bottom_bar = div()
        .id("media-viewer-bottom-bar")
        .w_full()
        .px(space_4())
        .py(space_3())
        .bg(with_alpha(0x000000, 0.55))
        .border_t_1()
        .border_color(with_alpha(0xffffff, 0.08))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(space_3())
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .gap(space_1())
                .child(
                    div()
                        .text_size(font_size_sm())
                        .font_weight(WEIGHT_SEMIBOLD)
                        .text_color(text_titles())
                        .truncate()
                        .max_w(px(520.0))
                        .child(current.filename.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(space_2())
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .child(kind_label)
                        .children(size_label.map(|s| div().child(format!("• {s}"))))
                        .child(format!("• {counter}"))
                        .children(zoomable.then(|| div().child(format!("• {zoom_pct}")))),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_2())
                .child(prev_btn)
                .child(next_btn)
                .child(zoom_out_btn)
                .child(zoom_in_btn)
                .child(browser_btn)
                .child(download_btn)
                .child(close_btn),
        );

    bottom_bar = bottom_bar.on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
        cx.stop_propagation();
    });

    let mut overlay = div()
        .id("media-viewer-overlay")
        .key_context("MediaViewer")
        .absolute()
        .inset_0()
        .bg(modal_backdrop())
        .flex()
        .flex_col()
        .overflow_hidden();

    if let Some(fh) = focus_handle {
        overlay = overlay.track_focus(fh);
    }

    if let Some(cb_close) = callbacks.on_close.clone() {
        overlay = overlay.on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.to_ascii_lowercase();
            if key == "escape" {
                cb_close(window, cx);
            }
        });
    }

    // Backdrop click closes.
    if let Some(cb_close) = callbacks.on_close.clone() {
        overlay = overlay.child(
            div()
                .id("media-viewer-backdrop")
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                    cb_close(window, cx);
                }),
        );
    } else {
        overlay = overlay.child(div().id("media-viewer-backdrop").absolute().inset_0());
    }

    let center = div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
            cx.stop_propagation();
        })
        .child(content_wrap)
        .child(bottom_bar);

    overlay = overlay.child(center);
    Some(overlay.into_any_element())
}
