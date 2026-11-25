//! Clipboard abstraction for text and bitmap content on Linux.
//!
//! GPUI exposes a cross-platform [`ClipboardItem`] that can carry strings and
//! images. Reading image entries already works through the platform layer on
//! both X11 and Wayland, but writing an image through GPUI currently only
//! publishes text. This module keeps the GPUI read path and adds explicit
//! image write backends:
//!
//! * X11: a dedicated `x11rb` selection owner that serves `image/*` targets.
//! * Wayland: `wl-copy` (from wl-clipboard) when available.
//!
//! If the preferred backend cannot write the bitmap, the other Linux backend is
//! tried. When no backend can publish the image, the caller receives an
//! explicit [`ClipboardError`] instead of a silent no-op.

use std::fmt;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gpui::{ClipboardEntry, ClipboardItem, Image as GpuiImage, ImageFormat as GpuiImageFormat};

/// Maximum accepted clipboard image size.
///
/// Discord's non-Nitro upload ceiling is 25 MiB, so larger clipboard payloads
/// would be refused by the API anyway. The limit also bounds memory use before
/// any upload or preview allocation.
pub const MAX_CLIPBOARD_IMAGE_BYTES: usize = 25 * 1024 * 1024;

/// Timeout used when downloading a chat attachment to copy as an image.
pub const CLIPBOARD_FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// Linux clipboard backend selected from the session environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardBackendKind {
    Wayland,
    X11,
    Unknown,
}

impl ClipboardBackendKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
            Self::Unknown => "desconhecido",
        }
    }
}

/// Pure backend classification used by [`current_backend`].
///
/// A Wayland session wins over an XWayland `DISPLAY`, because the native
/// clipboard is the one users interact with. Without session hints, the
/// presence of `WAYLAND_DISPLAY` or `DISPLAY` decides.
pub fn classify_clipboard_backend(
    wayland_display: Option<&str>,
    display: Option<&str>,
    session_type: Option<&str>,
) -> ClipboardBackendKind {
    let has_wayland = wayland_display
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_x11 = display
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let session = session_type
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match session {
        Some(value) if value.eq_ignore_ascii_case("wayland") => {
            return ClipboardBackendKind::Wayland;
        }
        Some(value) if value.eq_ignore_ascii_case("x11") && has_x11 => {
            return ClipboardBackendKind::X11;
        }
        _ => {}
    }

    if has_wayland {
        ClipboardBackendKind::Wayland
    } else if has_x11 {
        ClipboardBackendKind::X11
    } else {
        ClipboardBackendKind::Unknown
    }
}

/// Returns the clipboard backend for the current process environment.
pub fn current_backend() -> ClipboardBackendKind {
    classify_clipboard_backend(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
    )
}

/// Image formats that can travel through the clipboard without re-encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Svg,
    Bmp,
    Tiff,
    Ico,
    Pnm,
}

impl ClipboardImageFormat {
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Svg => "image/svg+xml",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
            Self::Ico => "image/ico",
            Self::Pnm => "image/x-portable-anymap",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Svg => "svg",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Pnm => "pnm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Gif => "GIF",
            Self::Svg => "SVG",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Ico => "ICO",
            Self::Pnm => "PNM",
        }
    }

    /// Resolves a MIME type, including the aliases commonly used in the wild.
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        let base = mime_type
            .split(';')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())?
            .to_ascii_lowercase();
        match base.as_str() {
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/webp" => Some(Self::Webp),
            "image/gif" => Some(Self::Gif),
            "image/svg+xml" | "image/svg" => Some(Self::Svg),
            "image/bmp" => Some(Self::Bmp),
            "image/tiff" | "image/tif" => Some(Self::Tiff),
            "image/ico" | "image/x-icon" | "image/vnd.microsoft.icon" => Some(Self::Ico),
            "image/x-portable-anymap" | "image/x-portable-pixmap" | "image/x-portable-bitmap" => {
                Some(Self::Pnm)
            }
            _ => None,
        }
    }

    /// Detects the format from the file signature when the MIME type is absent.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else if bytes.starts_with(b"BM") {
            Some(Self::Bmp)
        } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            Some(Self::Tiff)
        } else if bytes.starts_with(b"\0\0\x01\0") {
            Some(Self::Ico)
        } else if bytes.starts_with(b"P1")
            || bytes.starts_with(b"P2")
            || bytes.starts_with(b"P3")
            || bytes.starts_with(b"P4")
            || bytes.starts_with(b"P5")
            || bytes.starts_with(b"P6")
        {
            Some(Self::Pnm)
        } else {
            None
        }
    }

    fn to_gpui(self) -> GpuiImageFormat {
        match self {
            Self::Png => GpuiImageFormat::Png,
            Self::Jpeg => GpuiImageFormat::Jpeg,
            Self::Webp => GpuiImageFormat::Webp,
            Self::Gif => GpuiImageFormat::Gif,
            Self::Svg => GpuiImageFormat::Svg,
            Self::Bmp => GpuiImageFormat::Bmp,
            Self::Tiff => GpuiImageFormat::Tiff,
            Self::Ico => GpuiImageFormat::Ico,
            Self::Pnm => GpuiImageFormat::Pnm,
        }
    }

    fn from_gpui(format: GpuiImageFormat) -> Self {
        match format {
            GpuiImageFormat::Png => Self::Png,
            GpuiImageFormat::Jpeg => Self::Jpeg,
            GpuiImageFormat::Webp => Self::Webp,
            GpuiImageFormat::Gif => Self::Gif,
            GpuiImageFormat::Svg => Self::Svg,
            GpuiImageFormat::Bmp => Self::Bmp,
            GpuiImageFormat::Tiff => Self::Tiff,
            GpuiImageFormat::Ico => Self::Ico,
            GpuiImageFormat::Pnm => Self::Pnm,
        }
    }
}

/// A bitmap kept in its original encoding so transparency and metadata are
/// preserved byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardImage {
    pub format: ClipboardImageFormat,
    pub bytes: Vec<u8>,
}

impl ClipboardImage {
    /// Builds an image using [`MAX_CLIPBOARD_IMAGE_BYTES`] as the limit.
    pub fn from_bytes(
        format: ClipboardImageFormat,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ClipboardError> {
        Self::from_bytes_with_limit(format, bytes, MAX_CLIPBOARD_IMAGE_BYTES)
    }

    /// Builds an image with an explicit size limit (used by tests and callers
    /// that need a tighter budget).
    pub fn from_bytes_with_limit(
        format: ClipboardImageFormat,
        bytes: impl Into<Vec<u8>>,
        limit: usize,
    ) -> Result<Self, ClipboardError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(ClipboardError::EmptyImage);
        }
        if bytes.len() > limit {
            return Err(ClipboardError::ImageTooLarge {
                size: bytes.len(),
                limit,
            });
        }
        Ok(Self { format, bytes })
    }

    pub fn from_gpui_image(image: &GpuiImage) -> Result<Self, ClipboardError> {
        Self::from_bytes(
            ClipboardImageFormat::from_gpui(image.format),
            image.bytes.clone(),
        )
    }

    pub fn to_gpui_image(&self) -> GpuiImage {
        GpuiImage::from_bytes(self.format.to_gpui(), self.bytes.clone())
    }

    pub fn mime_type(&self) -> &'static str {
        self.format.mime_type()
    }

    pub fn extension(&self) -> &'static str {
        self.format.extension()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Default filename used when the clipboard payload becomes an attachment.
    pub fn default_filename(&self) -> String {
        format!("clipboard.{}", self.extension())
    }
}

/// Content read from (or written to) the clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardContent {
    Text(String),
    Image(ClipboardImage),
}

/// Explicit clipboard errors shown to users instead of silent failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    ReadUnavailable,
    UnsupportedContent,
    EmptyImage,
    ImageTooLarge {
        size: usize,
        limit: usize,
    },
    WriteUnavailable {
        backend: ClipboardBackendKind,
    },
    Backend {
        backend: ClipboardBackendKind,
        message: String,
    },
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadUnavailable => {
                formatter.write_str("a área de transferência não está disponível")
            }
            Self::UnsupportedContent => {
                formatter.write_str("o conteúdo da área de transferência não é suportado")
            }
            Self::EmptyImage => formatter.write_str("a imagem da área de transferência está vazia"),
            Self::ImageTooLarge { size, limit } => write!(
                formatter,
                "a imagem tem {size} bytes e excede o limite de {limit} bytes"
            ),
            Self::WriteUnavailable { backend } => write!(
                formatter,
                "não foi possível gravar a imagem usando o backend {}",
                backend.label()
            ),
            Self::Backend { backend, message } => {
                write!(formatter, "falha no backend {}: {message}", backend.label())
            }
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Injectable clipboard contract used by the UI and tests.
pub trait ClipboardBackend {
    fn kind(&self) -> ClipboardBackendKind;
    fn read(&self) -> Result<Option<ClipboardContent>, ClipboardError>;
    fn write(&self, content: &ClipboardContent) -> Result<(), ClipboardError>;

    fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        Ok(match self.read()? {
            Some(ClipboardContent::Text(text)) => Some(text),
            _ => None,
        })
    }

    fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        Ok(match self.read()? {
            Some(ClipboardContent::Image(image)) => Some(image),
            _ => None,
        })
    }
}

/// Clipboard access backed by GPUI reads and the Linux image writers.
pub struct PlatformClipboard<'a> {
    app: &'a gpui::App,
}

impl<'a> PlatformClipboard<'a> {
    pub fn new(app: &'a gpui::App) -> Self {
        Self { app }
    }

    pub fn kind(&self) -> ClipboardBackendKind {
        current_backend()
    }

    pub fn read(&self) -> Result<Option<ClipboardContent>, ClipboardError> {
        let Some(item) = self.app.read_from_clipboard() else {
            return Ok(None);
        };
        item_to_content(&item)
    }

    pub fn read_text(&self) -> Result<Option<String>, ClipboardError> {
        let Some(item) = self.app.read_from_clipboard() else {
            return Ok(None);
        };
        Ok(item.text())
    }

    /// Reads only the bitmap entries, so an image payload wins over text when
    /// both representations are offered by the source application.
    pub fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        let Some(item) = self.app.read_from_clipboard() else {
            return Ok(None);
        };
        for entry in item.entries() {
            if let ClipboardEntry::Image(image) = entry {
                return Ok(Some(ClipboardImage::from_gpui_image(image)?));
            }
        }
        Ok(None)
    }

    pub fn write(&self, content: &ClipboardContent) -> Result<(), ClipboardError> {
        match content {
            ClipboardContent::Text(text) => self.write_text(text),
            ClipboardContent::Image(image) => self.write_image(image),
        }
    }

    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        self.app
            .write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        Ok(())
    }

    pub fn write_image(&self, image: &ClipboardImage) -> Result<(), ClipboardError> {
        write_image_to_platform(image)
    }
}

impl ClipboardBackend for PlatformClipboard<'_> {
    fn kind(&self) -> ClipboardBackendKind {
        self.kind()
    }

    fn read(&self) -> Result<Option<ClipboardContent>, ClipboardError> {
        self.read()
    }

    fn write(&self, content: &ClipboardContent) -> Result<(), ClipboardError> {
        self.write(content)
    }

    fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        self.read_image()
    }
}

fn item_to_content(item: &ClipboardItem) -> Result<Option<ClipboardContent>, ClipboardError> {
    if let Some(text) = item.text() {
        if !text.is_empty() {
            return Ok(Some(ClipboardContent::Text(text)));
        }
    }
    for entry in item.entries() {
        if let ClipboardEntry::Image(image) = entry {
            return Ok(Some(ClipboardContent::Image(
                ClipboardImage::from_gpui_image(image)?,
            )));
        }
    }
    Ok(None)
}

/// Writes a bitmap using the preferred Linux backend, falling back to the other
/// one when both environments are available.
pub fn write_image_to_platform(image: &ClipboardImage) -> Result<(), ClipboardError> {
    if image.is_empty() {
        return Err(ClipboardError::EmptyImage);
    }
    if image.bytes.len() > MAX_CLIPBOARD_IMAGE_BYTES {
        return Err(ClipboardError::ImageTooLarge {
            size: image.bytes.len(),
            limit: MAX_CLIPBOARD_IMAGE_BYTES,
        });
    }

    let has_wayland = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    let has_x11 = std::env::var("DISPLAY")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());

    match current_backend() {
        ClipboardBackendKind::Wayland => {
            match write_image_wayland(image) {
                Ok(()) => Ok(()),
                Err(wayland_error) => {
                    if has_x11 {
                        write_image_x11(image)
                    } else {
                        Err(wayland_error)
                    }
                }
            }
        }
        ClipboardBackendKind::X11 => match write_image_x11(image) {
            Ok(()) => Ok(()),
            Err(x11_error) => {
                if has_wayland {
                    write_image_wayland(image)
                } else {
                    Err(x11_error)
                }
            }
        },
        ClipboardBackendKind::Unknown => Err(ClipboardError::WriteUnavailable {
            backend: ClipboardBackendKind::Unknown,
        }),
    }
}

fn write_image_wayland(image: &ClipboardImage) -> Result<(), ClipboardError> {
    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg(image.mime_type())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ClipboardError::WriteUnavailable {
                    backend: ClipboardBackendKind::Wayland,
                }
            } else {
                ClipboardError::Backend {
                    backend: ClipboardBackendKind::Wayland,
                    message: error.to_string(),
                }
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&image.bytes).map_err(|error| {
            ClipboardError::Backend {
                backend: ClipboardBackendKind::Wayland,
                message: format!("falha ao enviar bytes para wl-copy: {error}"),
            }
        })?;
    }

    let stderr = child.stderr.take();
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let mut message = String::new();
                if let Some(mut stderr) = stderr {
                    let _ = stderr.read_to_string(&mut message);
                }
                let message = message.trim();
                return Err(ClipboardError::Backend {
                    backend: ClipboardBackendKind::Wayland,
                    message: if message.is_empty() {
                        format!("wl-copy terminou com {status}")
                    } else {
                        message.to_string()
                    },
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    // wl-copy keeps serving the selection in the background.
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(ClipboardError::Backend {
                    backend: ClipboardBackendKind::Wayland,
                    message: error.to_string(),
                });
            }
        }
    }
}

#[derive(Clone, Copy)]
struct X11Atoms {
    clipboard: u32,
    targets: u32,
}

fn write_image_x11(image: &ClipboardImage) -> Result<(), ClipboardError> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, CreateWindowAux, WindowClass};

    let (connection, screen_number) = x11rb::connect(None).map_err(|error| {
        ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        }
    })?;
    let screen = &connection.setup().roots[screen_number];
    let window = connection.generate_id().map_err(|error| ClipboardError::Backend {
        backend: ClipboardBackendKind::X11,
        message: error.to_string(),
    })?;

    connection
        .create_window(
            0,
            window,
            screen.root,
            -1,
            -1,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root,
            &CreateWindowAux::new(),
        )
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })?;

    let atoms = X11Atoms {
        clipboard: intern_x11_atom(&connection, "CLIPBOARD")?,
        targets: intern_x11_atom(&connection, "TARGETS")?,
    };
    let target = intern_x11_atom(&connection, image.mime_type())?;

    connection
        .set_selection_owner(window, atoms.clipboard, x11rb::CURRENT_TIME)
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })?;
    connection.flush().map_err(|error| ClipboardError::Backend {
        backend: ClipboardBackendKind::X11,
        message: error.to_string(),
    })?;

    let bytes = image.bytes.clone();
    std::thread::Builder::new()
        .name("discord-rs-x11-clipboard".to_string())
        .spawn(move || serve_x11_selection(connection, window, atoms, target, bytes))
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })?;

    Ok(())
}

fn intern_x11_atom(
    connection: &impl x11rb::connection::Connection,
    name: &str,
) -> Result<u32, ClipboardError> {
    use x11rb::protocol::xproto::ConnectionExt;

    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::X11,
            message: error.to_string(),
        })
}

fn serve_x11_selection(
    connection: impl x11rb::connection::Connection,
    _window: u32,
    atoms: X11Atoms,
    target: u32,
    bytes: Vec<u8>,
) {
    use x11rb::protocol::xproto::{
        AtomEnum, ConnectionExt, EventMask, PropMode, SelectionNotifyEvent, SELECTION_NOTIFY_EVENT,
    };
    use x11rb::protocol::Event;
    use x11rb::wrapper::ConnectionExt as _;

    loop {
        let event = match connection.wait_for_event() {
            Ok(event) => event,
            Err(error) => {
                log::warn!("X11 clipboard backend encerrado: {error}");
                return;
            }
        };

        match event {
            Event::SelectionRequest(request) => {
                let property = if request.property == 0 {
                    request.target
                } else {
                    request.property
                };
                let mut served = true;

                if request.target == atoms.targets {
                    let targets = [atoms.targets, target];
                    if connection
                        .change_property32(
                            PropMode::REPLACE,
                            request.requestor,
                            property,
                            AtomEnum::ATOM,
                            &targets,
                        )
                        .is_err()
                    {
                        served = false;
                    }
                } else if request.target == target {
                    if connection
                        .change_property8(
                            PropMode::REPLACE,
                            request.requestor,
                            property,
                            target,
                            &bytes,
                        )
                        .is_err()
                    {
                        served = false;
                    }
                } else {
                    served = false;
                }

                let notification = SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: request.time,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property: if served { property } else { 0 },
                };
                let _ = connection.send_event(
                    false,
                    request.requestor,
                    EventMask::NO_EVENT,
                    notification,
                );
                let _ = connection.flush();
            }
            Event::SelectionClear(_) => return,
            _ => {}
        }
    }
}

/// Downloads a chat attachment and prepares it for the clipboard without
/// re-encoding. The payload is bounded by [`MAX_CLIPBOARD_IMAGE_BYTES`].
pub async fn fetch_remote_image(url: &str) -> Result<ClipboardImage, ClipboardError> {
    fetch_remote_image_with_limit(url, MAX_CLIPBOARD_IMAGE_BYTES).await
}

/// Same as [`fetch_remote_image`] with a caller-provided size limit.
pub async fn fetch_remote_image_with_limit(
    url: &str,
    limit: usize,
) -> Result<ClipboardImage, ClipboardError> {
    use futures::StreamExt;

    let client = reqwest::Client::builder()
        .timeout(CLIPBOARD_FETCH_TIMEOUT)
        .build()
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::Unknown,
            message: error.to_string(),
        })?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::Unknown,
            message: format!("falha ao baixar a imagem: {error}"),
        })?;

    if !response.status().is_success() {
        return Err(ClipboardError::Backend {
            backend: ClipboardBackendKind::Unknown,
            message: format!("o servidor respondeu {}", response.status()),
        });
    }

    if let Some(length) = response.content_length() {
        if length > limit as u64 {
            return Err(ClipboardError::ImageTooLarge {
                size: length as usize,
                limit,
            });
        }
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| ClipboardError::Backend {
            backend: ClipboardBackendKind::Unknown,
            message: format!("falha ao ler a imagem: {error}"),
        })?;
        let next_len = bytes.len().saturating_add(chunk.len());
        if next_len > limit {
            return Err(ClipboardError::ImageTooLarge {
                size: next_len,
                limit,
            });
        }
        bytes.extend_from_slice(&chunk);
    }

    let format = content_type
        .as_deref()
        .and_then(ClipboardImageFormat::from_mime_type)
        .or_else(|| ClipboardImageFormat::sniff(&bytes))
        .ok_or(ClipboardError::UnsupportedContent)?;

    ClipboardImage::from_bytes_with_limit(format, bytes, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn classifies_wayland_x11_and_headless() {
        assert_eq!(
            classify_clipboard_backend(Some("wayland-0"), Some(":0"), Some("wayland")),
            ClipboardBackendKind::Wayland
        );
        assert_eq!(
            classify_clipboard_backend(None, Some(":0"), Some("x11")),
            ClipboardBackendKind::X11
        );
        assert_eq!(
            classify_clipboard_backend(Some("wayland-0"), None, None),
            ClipboardBackendKind::Wayland
        );
        assert_eq!(
            classify_clipboard_backend(None, Some(":0"), None),
            ClipboardBackendKind::X11
        );
        assert_eq!(
            classify_clipboard_backend(None, None, None),
            ClipboardBackendKind::Unknown
        );
    }

    #[::core::prelude::v1::test]
    fn image_format_roundtrips_mime_and_extension() {
        for format in [
            ClipboardImageFormat::Png,
            ClipboardImageFormat::Jpeg,
            ClipboardImageFormat::Webp,
            ClipboardImageFormat::Gif,
            ClipboardImageFormat::Svg,
            ClipboardImageFormat::Bmp,
            ClipboardImageFormat::Tiff,
            ClipboardImageFormat::Ico,
            ClipboardImageFormat::Pnm,
        ] {
            assert_eq!(
                ClipboardImageFormat::from_mime_type(format.mime_type()),
                Some(format)
            );
            assert!(!format.extension().is_empty());
            assert!(!format.label().is_empty());
        }
        assert_eq!(
            ClipboardImageFormat::from_mime_type("image/jpg"),
            Some(ClipboardImageFormat::Jpeg)
        );
    }
}
