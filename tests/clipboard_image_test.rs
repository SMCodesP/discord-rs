use std::cell::RefCell;

use discord_rs::platform::clipboard::{
    classify_clipboard_backend, ClipboardBackend, ClipboardBackendKind, ClipboardContent,
    ClipboardError, ClipboardImage, ClipboardImageFormat, MAX_CLIPBOARD_IMAGE_BYTES,
};

#[derive(Default)]
struct MemoryClipboard {
    content: RefCell<Option<ClipboardContent>>,
    fail_write: RefCell<Option<ClipboardError>>,
}

impl ClipboardBackend for MemoryClipboard {
    fn kind(&self) -> ClipboardBackendKind {
        ClipboardBackendKind::Unknown
    }

    fn read(&self) -> Result<Option<ClipboardContent>, ClipboardError> {
        Ok(self.content.borrow().clone())
    }

    fn write(&self, content: &ClipboardContent) -> Result<(), ClipboardError> {
        if let Some(error) = self.fail_write.borrow().clone() {
            return Err(error);
        }
        *self.content.borrow_mut() = Some(content.clone());
        Ok(())
    }
}

fn transparent_png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 0]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode synthetic transparent PNG");
    bytes
}

#[test]
fn text_roundtrip_through_injected_backend() {
    let backend = MemoryClipboard::default();
    backend
        .write(&ClipboardContent::Text("texto sintético".into()))
        .expect("write text");

    assert_eq!(
        backend.read_text().expect("read text"),
        Some("texto sintético".to_string())
    );
    assert_eq!(backend.read_image().expect("read image"), None);
}

#[test]
fn png_transparency_and_bytes_are_preserved() {
    let png = transparent_png();
    let image =
        ClipboardImage::from_bytes(ClipboardImageFormat::Png, png.clone()).expect("clipboard image");
    let backend = MemoryClipboard::default();

    backend
        .write(&ClipboardContent::Image(image))
        .expect("write image");
    let read = backend
        .read_image()
        .expect("read image")
        .expect("image present");

    assert_eq!(read.format, ClipboardImageFormat::Png);
    assert_eq!(read.bytes, png, "encoded bytes must not be re-encoded");

    let decoded = image::load_from_memory(&read.bytes).expect("decode PNG");
    assert_eq!(
        decoded.to_rgba8().get_pixel(0, 0)[3],
        0,
        "transparent alpha must survive the clipboard roundtrip"
    );
}

#[test]
fn size_limit_is_explicit_and_configurable() {
    let error =
        ClipboardImage::from_bytes_with_limit(ClipboardImageFormat::Png, vec![1, 2, 3], 2)
            .expect_err("oversized image must fail");
    assert_eq!(
        error,
        ClipboardError::ImageTooLarge { size: 3, limit: 2 }
    );
    assert!(error.to_string().contains("3 bytes"));
    assert!(MAX_CLIPBOARD_IMAGE_BYTES >= 1024 * 1024);
}

#[test]
fn empty_image_is_rejected() {
    assert_eq!(
        ClipboardImage::from_bytes(ClipboardImageFormat::Png, Vec::new()),
        Err(ClipboardError::EmptyImage)
    );
    assert_eq!(
        ClipboardImage::from_bytes(ClipboardImageFormat::Png, vec![0u8; 4])
            .expect("small image")
            .len(),
        4
    );
}

#[test]
fn backend_write_failure_is_explicit() {
    let backend = MemoryClipboard::default();
    *backend.fail_write.borrow_mut() = Some(ClipboardError::WriteUnavailable {
        backend: ClipboardBackendKind::X11,
    });

    let error = backend
        .write(&ClipboardContent::Text("nada".into()))
        .expect_err("write must fail");
    assert!(matches!(
        error,
        ClipboardError::WriteUnavailable {
            backend: ClipboardBackendKind::X11
        }
    ));
    assert!(error.to_string().contains("X11"));
}

#[test]
fn backend_classification_covers_wayland_x11_and_headless() {
    assert_eq!(
        classify_clipboard_backend(Some("wayland-0"), Some(":0"), Some("wayland")),
        ClipboardBackendKind::Wayland
    );
    assert_eq!(
        classify_clipboard_backend(None, Some(":0"), Some("x11")),
        ClipboardBackendKind::X11
    );
    assert_eq!(
        classify_clipboard_backend(None, None, None),
        ClipboardBackendKind::Unknown
    );
}

#[test]
fn png_mime_type_and_default_filename_are_stable() {
    let image = ClipboardImage::from_bytes(ClipboardImageFormat::Png, vec![0u8; 8])
        .expect("clipboard image");
    assert_eq!(image.mime_type(), "image/png");
    assert_eq!(image.default_filename(), "clipboard.png");
    assert_eq!(
        ClipboardImageFormat::from_mime_type("image/png; charset=binary"),
        Some(ClipboardImageFormat::Png)
    );
    assert_eq!(
        ClipboardImageFormat::from_mime_type("image/jpg"),
        Some(ClipboardImageFormat::Jpeg)
    );
}

#[test]
fn image_signatures_are_sniffed_without_mime_headers() {
    assert_eq!(
        ClipboardImageFormat::sniff(&transparent_png()),
        Some(ClipboardImageFormat::Png)
    );
    assert_eq!(
        ClipboardImageFormat::sniff(b"GIF89a-synthetic"),
        Some(ClipboardImageFormat::Gif)
    );
    assert_eq!(
        ClipboardImageFormat::sniff(b"\xff\xd8\xff-synthetic"),
        Some(ClipboardImageFormat::Jpeg)
    );
    assert_eq!(ClipboardImageFormat::sniff(b"not-an-image"), None);
}
