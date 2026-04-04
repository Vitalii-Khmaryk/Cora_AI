//! Screen capture — captures the primary monitor as a JPEG-encoded
//! Base64 string for Ollama vision requests.

use anyhow::{Context, Result};
use base64::Engine;

/// Captures the primary monitor, encodes as JPEG, and returns
/// the image as a Base64 string.
pub async fn capture_screen_base64() -> Result<String> {
    tokio::task::spawn_blocking(|| -> Result<String> {
        let monitors = xcap::Monitor::all().context("Failed to enumerate monitors")?;
        let monitor = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .or_else(|| xcap::Monitor::all().ok()?.into_iter().next())
            .context("No monitor found")?;

        let rgba_image = monitor.capture_image().context("Capture failed")?;

        let dynamic_img = image::DynamicImage::ImageRgba8(rgba_image);
        let mut jpeg_buf = std::io::Cursor::new(Vec::new());

        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 80);
        encoder
            .encode_image(&dynamic_img)
            .context("JPEG Encoding failed")?;

        let jpeg_bytes = jpeg_buf.into_inner();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
        Ok(b64)
    })
    .await?
}
