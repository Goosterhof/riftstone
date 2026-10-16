//! Screenshots as the window draws them: decoded from the package, scaled to
//! a card (≤ 360 px wide) or the lightbox (≤ 1600 px wide), re-encoded as
//! JPEG (quality 82) and handed over as a `data:image/jpeg;base64,…` URL.
//!
//! Each rendering is cached on disk in `thumbs/` under the app cache dir as
//! `<package id>-<fnv(entry)>-<width>.jpg`. A package id changes whenever the
//! archive's name or size does, so a new download never shows a stale picture.
//! A picture that cannot be shown (not an image, broken, too big) leaves an
//! empty `<package id>-<fnv(entry)>.fail` marker instead, so it is not decoded
//! again on every request; a new download (a new package id) is tried afresh.
//! The library itself is never written (RS-2).
//!
//! Decoding is bounded before a pixel is read: at most [`MAX_SIDE`] pixels a
//! side and [`MAX_DECODE_BYTES`] for the decoded picture, checked against the
//! header — a small file claiming a giant picture is refused at once.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageError, ImageReader, Limits, Rgb, RgbImage};

use rift_core::{fnv1a64, PackageId};

use crate::dto::ThumbnailSize;
use crate::persist;

/// The widest a card thumbnail gets.
pub const CARD_WIDTH: u32 = 360;
/// The widest a full-size preview gets.
pub const FULL_WIDTH: u32 = 1600;
/// JPEG quality for every rendering.
pub const JPEG_QUALITY: u8 = 82;
/// A screenshot larger than this is not a screenshot; it is not decoded.
pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
/// The widest and tallest picture decoded at all (an 8K screenshot fits).
pub const MAX_SIDE: u32 = 8192;
/// The most memory one decoded picture may take (8K at four bytes a pixel).
pub const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
/// Transparent pixels land on the cards' ground (`--rs-mist`, #111821).
const BACKDROP: [u8; 3] = [0x11, 0x18, 0x21];

pub fn width_for(size: ThumbnailSize) -> u32 {
    match size {
        ThumbnailSize::Card => CARD_WIDTH,
        ThumbnailSize::Full => FULL_WIDTH,
    }
}

/// The cache file name for one rendering.
pub fn cache_name(package: &PackageId, entry: &str, width: u32) -> String {
    format!(
        "{}-{:016x}-{width}.jpg",
        package.0,
        fnv1a64(entry.as_bytes())
    )
}

/// The cached rendering, when there is one.
pub fn cached(thumbs_dir: &Path, name: &str) -> Option<Vec<u8>> {
    fs::read(thumbs_dir.join(name))
        .ok()
        .filter(|b| !b.is_empty())
}

/// Keep a rendering for next time. A cache that cannot be written only costs
/// speed, so the failure is logged and swallowed.
pub fn remember(thumbs_dir: &Path, name: &str, jpeg: &[u8]) {
    if let Err(err) = persist::write_atomic(&thumbs_dir.join(name), jpeg) {
        log::warn!("could not cache a thumbnail: {err}");
    }
}

/// The marker name for a picture that could not be shown — one per picture,
/// whatever the size asked for.
pub fn failure_name(package: &PackageId, entry: &str) -> String {
    format!("{}-{:016x}.fail", package.0, fnv1a64(entry.as_bytes()))
}

/// Did this picture fail before?
pub fn failed_before(thumbs_dir: &Path, failure_name: &str) -> bool {
    thumbs_dir.join(failure_name).is_file()
}

/// Remember that this picture cannot be shown (best effort, like the cache).
pub fn remember_failure(thumbs_dir: &Path, failure_name: &str) {
    if let Err(err) = persist::write_atomic(&thumbs_dir.join(failure_name), b"") {
        log::warn!("could not note a picture that cannot be shown: {err}");
    }
}

/// Decode `bytes` within the limits, scale it to at most `max_width` pixels
/// wide (never up), flatten any transparency onto the card ground and encode
/// it as JPEG. `Err` carries a plain reason (an unknown format, a broken or
/// oversized image).
pub fn render(bytes: &[u8], max_width: u32) -> Result<Vec<u8>, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let image = reader.decode().map_err(|err| match err {
        ImageError::Limits(_) => format!(
            "the picture is larger than the Riftstone shows (at most {MAX_SIDE}×{MAX_SIDE} pixels)"
        ),
        other => other.to_string(),
    })?;
    if image.width() == 0 || image.height() == 0 {
        return Err("the image has no pixels".into());
    }
    let flat = flatten(&scale(image, max_width));
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&flat)
        .map_err(|err| err.to_string())?;
    Ok(jpeg)
}

/// At most `max_width` wide, never up. A big reduction first takes the fast
/// box path down to twice the target, then the triangle filter finishes:
/// the full-size picture is never copied or filtered pixel by pixel.
fn scale(image: DynamicImage, max_width: u32) -> DynamicImage {
    let (width, height) = (image.width(), image.height());
    if width <= max_width {
        return image;
    }
    let new_height = (u64::from(height) * u64::from(max_width) / u64::from(width)).max(1);
    let new_height = u32::try_from(new_height).unwrap_or(u32::MAX);
    let near = if width > max_width.saturating_mul(2) {
        image.thumbnail(max_width.saturating_mul(2), u32::MAX)
    } else {
        image
    };
    near.resize_exact(max_width, new_height, FilterType::Triangle)
}

/// Composite onto [`BACKDROP`] (JPEG has no alpha channel).
fn flatten(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        let mix = |channel: usize| {
            let over = u32::from(pixel[channel]) * alpha;
            let under = u32::from(BACKDROP[channel]) * (255 - alpha);
            u8::try_from((over + under + 127) / 255).unwrap_or(u8::MAX)
        };
        out.put_pixel(x, y, Rgb([mix(0), mix(1), mix(2)]));
    }
    out
}

/// A `data:` URL the webview can show directly.
pub fn data_url(jpeg: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png(width: u32, height: u32, pixel: Rgba<u8>) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, pixel);
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn decoded(jpeg: &[u8]) -> DynamicImage {
        image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg).unwrap()
    }

    #[test]
    fn wide_pictures_shrink_to_the_card_and_keep_their_shape() {
        let jpeg = render(&png(1920, 1080, Rgba([200, 40, 40, 255])), CARD_WIDTH).unwrap();
        let image = decoded(&jpeg);
        assert_eq!((image.width(), image.height()), (360, 202));
        let full = decoded(&render(&png(1920, 1080, Rgba([0, 0, 0, 255])), FULL_WIDTH).unwrap());
        assert_eq!((full.width(), full.height()), (1600, 900));
    }

    #[test]
    fn small_pictures_are_never_blown_up() {
        let image = decoded(&render(&png(200, 100, Rgba([1, 2, 3, 255])), CARD_WIDTH).unwrap());
        assert_eq!((image.width(), image.height()), (200, 100));
    }

    #[test]
    fn transparency_lands_on_the_card_ground() {
        let image = decoded(&render(&png(8, 8, Rgba([255, 255, 255, 0])), CARD_WIDTH).unwrap());
        let pixel = image.to_rgb8().get_pixel(4, 4).0;
        for (got, want) in pixel.iter().zip(BACKDROP) {
            assert!(got.abs_diff(want) <= 6, "{pixel:?} vs {BACKDROP:?}");
        }
    }

    #[test]
    fn garbage_is_refused_with_a_reason() {
        assert!(render(b"not a picture at all", CARD_WIDTH).is_err());
    }

    /// A PNG that is all header: it claims `width`×`height` and holds no
    /// pixels at all (its one IDAT is an empty zlib stream) — a few dozen
    /// bytes standing in for a decompression bomb.
    fn png_claiming(width: u32, height: u32, color_type: u8) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = 0xFFFF_FFFFu32;
            for &byte in bytes {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xEDB8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
        }
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, color_type, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &header);
        chunk(
            &mut png,
            b"IDAT",
            &[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
        );
        chunk(&mut png, b"IEND", &[]);
        png
    }

    #[test]
    fn giant_pictures_are_refused_from_their_header_before_any_pixel() {
        for (width, height, color) in [
            (20_000, 20_000, 6), // RGBA bomb
            (20_000, 20_000, 0), // grayscale: a quarter of the bytes, as giant
            (300, 100_000, 6),   // a strip taller than JPEG can hold
            (9_000, 10, 2),      // wider than the Riftstone shows
        ] {
            let started = std::time::Instant::now();
            let why = render(&png_claiming(width, height, color), CARD_WIDTH).unwrap_err();
            assert!(
                why.contains("larger than the Riftstone shows"),
                "{width}×{height}: {why}"
            );
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "{width}×{height} was decoded before it was refused"
            );
        }
        // The largest picture allowed still renders (and shrinks).
        let jpeg = render(&png(MAX_SIDE, 8, Rgba([9, 9, 9, 255])), CARD_WIDTH).unwrap();
        let image = decoded(&jpeg);
        assert_eq!(image.width(), CARD_WIDTH);
    }

    #[test]
    fn failures_are_remembered_per_picture() {
        let dir = tempfile::tempdir().unwrap();
        let id = PackageId("00000000000000aa".into());
        let name = failure_name(&id, "Glow/screenshot.png");
        assert!(!failed_before(dir.path(), &name));
        remember_failure(dir.path(), &name);
        assert!(failed_before(dir.path(), &name));
        assert_ne!(name, failure_name(&id, "Other/screenshot.png"));
        assert_ne!(
            name,
            failure_name(&PackageId("00000000000000bb".into()), "Glow/screenshot.png"),
            "a new download is tried afresh"
        );
    }

    #[test]
    fn data_urls_and_cache_names_have_their_documented_shape() {
        assert!(data_url(&[0xFF, 0xD8, 0xFF]).starts_with("data:image/jpeg;base64,/9j/"));
        let id = PackageId("00000000000000aa".into());
        let name = cache_name(&id, "Glow/screenshot.png", CARD_WIDTH);
        assert!(name.starts_with("00000000000000aa-") && name.ends_with("-360.jpg"));
        assert_ne!(name, cache_name(&id, "Glow/screenshot.png", FULL_WIDTH));
        assert_ne!(name, cache_name(&id, "Other/screenshot.png", CARD_WIDTH));
        let dir = tempfile::tempdir().unwrap();
        assert!(cached(dir.path(), &name).is_none());
        remember(dir.path(), &name, b"jpeg");
        assert_eq!(cached(dir.path(), &name).unwrap(), b"jpeg");
    }
}
