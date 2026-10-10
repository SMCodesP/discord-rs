use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::error::CaptureError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelFormat {
    Rgba8,
    Bgra8,
    Rgb8,
    Bgr8,
    Yuyv,
    Nv12,
    I420,
    Mjpeg,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> Option<usize> {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Bgra8 => Some(4),
            PixelFormat::Rgb8 | PixelFormat::Bgr8 => Some(3),
            PixelFormat::Yuyv => Some(2),
            PixelFormat::Nv12 | PixelFormat::I420 => None,
            PixelFormat::Mjpeg => None,
        }
    }

    pub fn from_fourcc(fourcc: &[u8; 4]) -> Option<Self> {
        match fourcc {
            b"RGBA" | b"RGB4" => Some(PixelFormat::Rgba8),
            b"BGRA" | b"BGR4" => Some(PixelFormat::Bgra8),
            b"RGB3" | b"RGBP" => Some(PixelFormat::Rgb8),
            b"BGR3" => Some(PixelFormat::Bgr8),
            b"YUYV" | b"YUY2" => Some(PixelFormat::Yuyv),
            b"NV12" => Some(PixelFormat::Nv12),
            b"YU12" | b"I420" => Some(PixelFormat::I420),
            b"MJPG" | b"JPEG" => Some(PixelFormat::Mjpeg),
            _ => None,
        }
    }

    pub fn to_fourcc(&self) -> Option<[u8; 4]> {
        match self {
            PixelFormat::Rgba8 => Some(*b"RGBA"),
            PixelFormat::Bgra8 => Some(*b"BGRA"),
            PixelFormat::Rgb8 => Some(*b"RGB3"),
            PixelFormat::Bgr8 => Some(*b"BGR3"),
            PixelFormat::Yuyv => Some(*b"YUYV"),
            PixelFormat::Nv12 => Some(*b"NV12"),
            PixelFormat::I420 => Some(*b"YU12"),
            PixelFormat::Mjpeg => Some(*b"MJPG"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    pub timestamp: Duration,
    pub sequence: u64,
}

impl VideoFrame {
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        data: Vec<u8>,
        timestamp: Duration,
        sequence: u64,
    ) -> Self {
        Self {
            width,
            height,
            format,
            data,
            timestamp,
            sequence,
        }
    }

    /// Converts this frame to RGBA8 format suitable for rendering in GPUI.
    pub fn to_rgba8(&self) -> Result<VideoFrame, CaptureError> {
        match self.format {
            PixelFormat::Rgba8 => Ok(self.clone()),
            PixelFormat::Bgra8 => {
                let converted = bgra_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::Rgb8 => {
                let converted = rgb_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::Bgr8 => {
                let converted = bgr_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::Yuyv => {
                let converted = yuyv_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::Nv12 => {
                let converted = nv12_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::I420 => {
                let converted = i420_to_rgba(&self.data, self.width, self.height)?;
                Ok(VideoFrame {
                    width: self.width,
                    height: self.height,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
            PixelFormat::Mjpeg => {
                let (w, h, converted) = mjpeg_to_rgba(&self.data)?;
                Ok(VideoFrame {
                    width: w,
                    height: h,
                    format: PixelFormat::Rgba8,
                    data: converted,
                    timestamp: self.timestamp,
                    sequence: self.sequence,
                })
            }
        }
    }

    /// Provides raw RGBA8 image data tuple (width, height, bytes) ready for GPUI.
    pub fn as_gpui_image_data(&self) -> Result<(u32, u32, Vec<u8>), CaptureError> {
        let rgba_frame = self.to_rgba8()?;
        Ok((rgba_frame.width, rgba_frame.height, rgba_frame.data))
    }
}

/// Converts YUYV (YUY2 4:2:2 interleaved) bytes to RGBA8.
/// Each 4 bytes in YUYV represent 2 pixels: [Y0, U, Y1, V].
pub fn yuyv_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient YUYV data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let num_pixels = (width as usize) * (height as usize);
    let mut output = vec![0u8; num_pixels * 4];

    for (chunk, out_pair) in data[..expected_len]
        .chunks_exact(4)
        .zip(output.chunks_exact_mut(8))
    {
        let y0 = chunk[0] as i32;
        let u = chunk[1] as i32 - 128;
        let y1 = chunk[2] as i32;
        let v = chunk[3] as i32 - 128;

        // Pixel 0 (Y0, U, V)
        let r0 = (y0 + ((1402 * v) >> 10)).clamp(0, 255) as u8;
        let g0 = (y0 - ((344 * u + 714 * v) >> 10)).clamp(0, 255) as u8;
        let b0 = (y0 + ((1772 * u) >> 10)).clamp(0, 255) as u8;

        out_pair[0] = r0;
        out_pair[1] = g0;
        out_pair[2] = b0;
        out_pair[3] = 255;

        // Pixel 1 (Y1, U, V)
        let r1 = (y1 + ((1402 * v) >> 10)).clamp(0, 255) as u8;
        let g1 = (y1 - ((344 * u + 714 * v) >> 10)).clamp(0, 255) as u8;
        let b1 = (y1 + ((1772 * u) >> 10)).clamp(0, 255) as u8;

        out_pair[4] = r1;
        out_pair[5] = g1;
        out_pair[6] = b1;
        out_pair[7] = 255;
    }

    Ok(output)
}

/// Converts BGRA8 bytes (common from X11 GetImage) to RGBA8.
pub fn bgra_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient BGRA data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let mut output = vec![0u8; expected_len];
    for (src, dst) in data[..expected_len]
        .chunks_exact(4)
        .zip(output.chunks_exact_mut(4))
    {
        dst[0] = src[2]; // R
        dst[1] = src[1]; // G
        dst[2] = src[0]; // B
        dst[3] = src[3]; // A
    }

    Ok(output)
}

/// Converts BGR8 bytes to RGBA8.
pub fn bgr_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient BGR data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let num_pixels = (width as usize) * (height as usize);
    let mut output = vec![0u8; num_pixels * 4];

    for (src, dst) in data[..expected_len]
        .chunks_exact(3)
        .zip(output.chunks_exact_mut(4))
    {
        dst[0] = src[2]; // R
        dst[1] = src[1]; // G
        dst[2] = src[0]; // B
        dst[3] = 255;    // A
    }

    Ok(output)
}

/// Converts RGB8 bytes to RGBA8.
pub fn rgb_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient RGB data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let num_pixels = (width as usize) * (height as usize);
    let mut output = vec![0u8; num_pixels * 4];

    for (src, dst) in data[..expected_len]
        .chunks_exact(3)
        .zip(output.chunks_exact_mut(4))
    {
        dst[0] = src[0]; // R
        dst[1] = src[1]; // G
        dst[2] = src[2]; // B
        dst[3] = 255;    // A
    }

    Ok(output)
}

/// Converts NV12 (Y plane followed by interleaved UV plane) to RGBA8.
pub fn nv12_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let num_pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;
    let expected_len = num_pixels + (num_pixels / 2);

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient NV12 data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let y_plane = &data[..num_pixels];
    let uv_plane = &data[num_pixels..expected_len];
    let mut output = vec![0u8; num_pixels * 4];

    for y in 0..height as usize {
        for x in 0..width as usize {
            let y_val = y_plane[y * (width as usize) + x] as i32;
            let uv_idx = (y / 2) * (width as usize) + (x / 2) * 2;
            let u = uv_plane[uv_idx] as i32 - 128;
            let v = uv_plane[uv_idx + 1] as i32 - 128;

            let r = (y_val + ((1402 * v) >> 10)).clamp(0, 255) as u8;
            let g = (y_val - ((344 * u + 714 * v) >> 10)).clamp(0, 255) as u8;
            let b = (y_val + ((1772 * u) >> 10)).clamp(0, 255) as u8;

            let out_idx = (y * (width as usize) + x) * 4;
            output[out_idx] = r;
            output[out_idx + 1] = g;
            output[out_idx + 2] = b;
            output[out_idx + 3] = 255;
        }
    }

    Ok(output)
}

/// Converts I420 (Y plane followed by planar U then planar V) to RGBA8.
pub fn i420_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureError> {
    let num_pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| CaptureError::ConversionError("Dimensions overflow".into()))?;
    let chroma_size = num_pixels / 4;
    let expected_len = num_pixels + chroma_size * 2;

    if data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Insufficient I420 data: expected at least {} bytes, got {}",
            expected_len,
            data.len()
        )));
    }

    let y_plane = &data[..num_pixels];
    let u_plane = &data[num_pixels..num_pixels + chroma_size];
    let v_plane = &data[num_pixels + chroma_size..expected_len];
    let mut output = vec![0u8; num_pixels * 4];

    let half_width = (width as usize) / 2;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let y_val = y_plane[y * (width as usize) + x] as i32;
            let chroma_idx = (y / 2) * half_width + (x / 2);
            let u = u_plane[chroma_idx] as i32 - 128;
            let v = v_plane[chroma_idx] as i32 - 128;

            let r = (y_val + ((1402 * v) >> 10)).clamp(0, 255) as u8;
            let g = (y_val - ((344 * u + 714 * v) >> 10)).clamp(0, 255) as u8;
            let b = (y_val + ((1772 * u) >> 10)).clamp(0, 255) as u8;

            let out_idx = (y * (width as usize) + x) * 4;
            output[out_idx] = r;
            output[out_idx + 1] = g;
            output[out_idx + 2] = b;
            output[out_idx + 3] = 255;
        }
    }

    Ok(output)
}

/// Decodes MJPEG compressed byte stream into raw RGBA8.
pub fn mjpeg_to_rgba(data: &[u8]) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)
        .map_err(|e| CaptureError::ConversionError(format!("Failed to decode MJPEG: {}", e)))?;
    let rgba = img.to_rgba8();
    Ok((rgba.width(), rgba.height(), rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_fourcc_and_bytes() {
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), Some(4));
        assert_eq!(PixelFormat::Bgra8.bytes_per_pixel(), Some(4));
        assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), Some(3));
        assert_eq!(PixelFormat::Bgr8.bytes_per_pixel(), Some(3));
        assert_eq!(PixelFormat::Yuyv.bytes_per_pixel(), Some(2));
        assert_eq!(PixelFormat::Nv12.bytes_per_pixel(), None);

        assert_eq!(PixelFormat::from_fourcc(b"YUYV"), Some(PixelFormat::Yuyv));
        assert_eq!(PixelFormat::from_fourcc(b"BGRA"), Some(PixelFormat::Bgra8));
        assert_eq!(PixelFormat::from_fourcc(b"RGBA"), Some(PixelFormat::Rgba8));
        assert_eq!(PixelFormat::from_fourcc(b"MJPG"), Some(PixelFormat::Mjpeg));
        assert_eq!(PixelFormat::from_fourcc(b"UNKN"), None);

        assert_eq!(PixelFormat::Yuyv.to_fourcc(), Some(*b"YUYV"));
        assert_eq!(PixelFormat::Bgra8.to_fourcc(), Some(*b"BGRA"));
    }

    #[test]
    fn test_bgra_to_rgba_conversion() {
        // 2x1 image (2 pixels): Blue and Red in BGRA
        // Blue pixel: B=255, G=0, R=0, A=255
        // Red pixel:  B=0, G=0, R=255, A=128
        let bgra_data = vec![255, 0, 0, 255, 0, 0, 255, 128];
        let rgba = bgra_to_rgba(&bgra_data, 2, 1).expect("Conversion should succeed");

        // Expected RGBA:
        // Pixel 0: R=0, G=0, B=255, A=255
        // Pixel 1: R=255, G=0, B=0, A=128
        assert_eq!(rgba, vec![0, 0, 255, 255, 255, 0, 0, 128]);

        // Truncated buffer check
        let truncated = vec![255, 0, 0];
        assert!(bgra_to_rgba(&truncated, 2, 1).is_err());
    }

    #[test]
    fn test_bgr_and_rgb_to_rgba_conversion() {
        // 1 pixel BGR: B=10, G=20, R=30
        let bgr_data = vec![10, 20, 30];
        let rgba_from_bgr = bgr_to_rgba(&bgr_data, 1, 1).unwrap();
        assert_eq!(rgba_from_bgr, vec![30, 20, 10, 255]);

        // 1 pixel RGB: R=30, G=20, B=10
        let rgb_data = vec![30, 20, 10];
        let rgba_from_rgb = rgb_to_rgba(&rgb_data, 1, 1).unwrap();
        assert_eq!(rgba_from_rgb, vec![30, 20, 10, 255]);

        assert!(bgr_to_rgba(&[10, 20], 1, 1).is_err());
        assert!(rgb_to_rgba(&[10, 20], 1, 1).is_err());
    }

    #[test]
    fn test_yuyv_to_rgba_conversion() {
        // 2 pixels YUYV: [Y0, U, Y1, V]
        // Neutral gray: Y0=128, U=128, Y1=128, V=128
        let yuyv_gray = vec![128, 128, 128, 128];
        let rgba = yuyv_to_rgba(&yuyv_gray, 2, 1).unwrap();
        assert_eq!(rgba.len(), 8);

        // U=128 and V=128 means no chroma delta: R=G=B=128
        assert_eq!(rgba[0], 128); // R0
        assert_eq!(rgba[1], 128); // G0
        assert_eq!(rgba[2], 128); // B0
        assert_eq!(rgba[3], 255); // A0

        assert_eq!(rgba[4], 128); // R1
        assert_eq!(rgba[5], 128); // G1
        assert_eq!(rgba[6], 128); // B1
        assert_eq!(rgba[7], 255); // A1

        // Truncated buffer check
        assert!(yuyv_to_rgba(&[128, 128, 128], 2, 1).is_err());
    }

    #[test]
    fn test_nv12_to_rgba_conversion() {
        // 2x2 image (4 pixels)
        // Y plane: 4 bytes
        // UV plane: 2 bytes (1 U, 1 V)
        let nv12_data = vec![
            128, 128, // Y row 0
            128, 128, // Y row 1
            128, 128, // UV
        ];
        let rgba = nv12_to_rgba(&nv12_data, 2, 2).unwrap();
        assert_eq!(rgba.len(), 16);
        for i in 0..4 {
            let offset = i * 4;
            assert_eq!(rgba[offset], 128);
            assert_eq!(rgba[offset + 1], 128);
            assert_eq!(rgba[offset + 2], 128);
            assert_eq!(rgba[offset + 3], 255);
        }

        assert!(nv12_to_rgba(&[128; 5], 2, 2).is_err());
    }

    #[test]
    fn test_i420_to_rgba_conversion() {
        // 2x2 image (4 pixels)
        // Y: 4 bytes, U: 1 byte, V: 1 byte
        let i420_data = vec![
            128, 128, 128, 128, // Y
            128,                // U
            128,                // V
        ];
        let rgba = i420_to_rgba(&i420_data, 2, 2).unwrap();
        assert_eq!(rgba.len(), 16);
        assert_eq!(rgba[0], 128);
        assert_eq!(rgba[3], 255);

        assert!(i420_to_rgba(&[128; 5], 2, 2).is_err());
    }

    #[test]
    fn test_video_frame_gpui_image_pipeline() {
        let frame = VideoFrame::new(
            2,
            1,
            PixelFormat::Bgra8,
            vec![255, 0, 0, 255, 0, 255, 0, 255],
            Duration::from_millis(16),
            1,
        );

        let (w, h, data) = frame.as_gpui_image_data().expect("Should produce GPUI data");
        assert_eq!(w, 2);
        assert_eq!(h, 1);
        assert_eq!(data.len(), 8);
        // Pixel 0 BGRA [255, 0, 0, 255] -> RGBA [0, 0, 255, 255]
        assert_eq!(&data[0..4], &[0, 0, 255, 255]);
        // Pixel 1 BGRA [0, 255, 0, 255] -> RGBA [0, 255, 0, 255]
        assert_eq!(&data[4..8], &[0, 255, 0, 255]);
    }
}
