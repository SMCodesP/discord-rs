use std::fmt;

#[derive(Debug)]
pub enum CaptureError {
    DeviceNotFound(String),
    PermissionDenied(String),
    DeviceBusy(String),
    FormatUnsupported(String),
    InitializationFailed(String),
    StreamError(String),
    PortalError(String),
    PortalCancelled,
    X11Error(String),
    ConversionError(String),
    SessionUnavailable(String),
    Io(std::io::Error),
    Other(String),
}

impl std::error::Error for CaptureError {}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::DeviceNotFound(msg) => write!(f, "Capture device not found: {}", msg),
            CaptureError::PermissionDenied(msg) => write!(f, "Capture permission denied: {}", msg),
            CaptureError::DeviceBusy(msg) => write!(f, "Capture device busy: {}", msg),
            CaptureError::FormatUnsupported(msg) => write!(f, "Format unsupported: {}", msg),
            CaptureError::InitializationFailed(msg) => write!(f, "Capture initialization failed: {}", msg),
            CaptureError::StreamError(msg) => write!(f, "Stream error: {}", msg),
            CaptureError::PortalError(msg) => write!(f, "Wayland Portal screencast error: {}", msg),
            CaptureError::PortalCancelled => write!(f, "Wayland Portal screencast cancelled by user"),
            CaptureError::X11Error(msg) => write!(f, "X11 screencast error: {}", msg),
            CaptureError::ConversionError(msg) => write!(f, "Pixel format conversion error: {}", msg),
            CaptureError::SessionUnavailable(msg) => write!(f, "Session unavailable: {}", msg),
            CaptureError::Io(err) => write!(f, "IO error: {}", err),
            CaptureError::Other(msg) => write!(f, "Capture error: {}", msg),
        }
    }
}

impl From<std::io::Error> for CaptureError {
    fn from(err: std::io::Error) -> Self {
        CaptureError::Io(err)
    }
}
