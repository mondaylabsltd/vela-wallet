//! Screenshots for a bug report (078 round 3) — the web's
//! `services/screenshot-prep.ts` and iOS's `ScreenshotPrep.swift`, for the
//! desktop. The founder ruled screenshots PUBLIC: they are shown inline in
//! the GitHub issue, and the page says so beside the tiles before Send.
//!
//! ## What is sent is never the file that was picked
//!
//! Every image is decoded, drawn upright and opaque, then encoded again as a
//! JPEG. That round trip is the privacy guarantee, not an optimisation: a
//! photo's EXIF carries where and when it was taken, and the encoder here
//! writes none — and any APP1 segment (EXIF, XMP) that did appear is cut out
//! anyway, so the guarantee does not rest on one encoder's defaults. Not
//! skipped for a PNG that is already small.
//!
//! ## The steps, in order (the same ladder the web, iOS and Android run)
//!
//! 1. decode (PNG, JPEG, WebP, GIF, BMP, TIFF — whatever the `image` crate
//!    reads; HEIC is not among them, and is refused rather than sent raw),
//!    applying the EXIF orientation so a portrait photo goes the way it was
//!    seen, and laying any transparency over white (JPEG has no "clear", and
//!    black would hide dark-mode text);
//! 2. scale so the longest edge is at most [`MAX_EDGE`] — never up;
//! 3. encode JPEG at [`QUALITY`];
//! 4. still over the endpoint's [`MAX_SCREENSHOT_BYTES`]: again at
//!    [`QUALITY_SMALLER`], then at a [`SMALLER_EDGE`] edge.
//!
//! All of it is blocking and CPU-bound: call it off the frame.
//!
//! ## The tray
//!
//! [`Tray`] is the rules of the row of tiles, without the drawing: at most
//! [`MAX_SCREENSHOTS`], the first that fit are taken in the order given, a
//! refusal is shown until the next change, and a tile still being prepared
//! is waited for by Send rather than left behind.

use std::borrow::Cow;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use gpui::RenderImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, Frame, ImageDecoder as _, ImageReader, Rgb, RgbImage, RgbaImage};

/// At most this many per report (the endpoint's `MAX_SCREENSHOTS`).
pub const MAX_SCREENSHOTS: usize = 5;
/// Each screenshot, decoded, at most this many bytes (the endpoint's cap).
pub const MAX_SCREENSHOT_BYTES: usize = 2_000_000;
/// The longest edge a screenshot is sent at, then the fallback edge.
pub const MAX_EDGE: u32 = 1920;
pub const SMALLER_EDGE: u32 = 1440;
/// JPEG quality, then the fallback quality (the web's 0.85 / 0.7).
pub const QUALITY: u8 = 85;
pub const QUALITY_SMALLER: u8 = 70;
/// A tile's picture, in pixels: the 72-point tile at the raster scale the
/// icons use, so it is sharp on a Retina screen.
pub const THUMB_PX: u32 = 72 * crate::icons::RASTER_SCALE;

/// One image, ready to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// A baseline JPEG with no metadata segment of its own.
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The picked bytes as a sendable JPEG, or `None` when they are not an image
/// this machine can decode — the caller says `screenshotUnsupported`, and
/// the file is never sent as it was.
#[must_use]
pub fn prepare(bytes: &[u8]) -> Option<Prepared> {
    prepare_capped(bytes, MAX_SCREENSHOT_BYTES)
}

/// [`prepare`] against any cap — the seam that lets a test walk the whole
/// ladder without building a 2 MB picture.
pub(crate) fn prepare_capped(bytes: &[u8], cap: usize) -> Option<Prepared> {
    let upright = decode_upright(bytes)?;
    let opaque = DynamicImage::ImageRgb8(over_white(&upright));
    drop(upright);
    let mut last = None;
    // The 1920 picture is scaled once and encoded at both qualities.
    let mut scaled: Option<(u32, Cow<'_, DynamicImage>)> = None;
    for (edge, quality) in [
        (MAX_EDGE, QUALITY),
        (MAX_EDGE, QUALITY_SMALLER),
        (SMALLER_EDGE, QUALITY_SMALLER),
    ] {
        if scaled.as_ref().is_none_or(|(at, _)| *at != edge) {
            scaled = Some((edge, scale_within(&opaque, edge)));
        }
        let (_, picture) = scaled.as_ref()?;
        let jpeg = encode(picture, quality)?;
        let fits = jpeg.len() <= cap;
        last = Some(Prepared {
            jpeg,
            width: picture.width(),
            height: picture.height(),
        });
        if fits {
            break;
        }
    }
    // Over the cap even at the last rung: sent anyway, as every shell does —
    // the endpoint's 413 falls back to the form, which loses nothing.
    last
}

/// Decoded, with the EXIF orientation applied.
fn decode_upright(bytes: &[u8]) -> Option<DynamicImage> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    // A missing or unreadable orientation is "as stored", never a refusal.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    image.apply_orientation(orientation);
    (image.width() > 0 && image.height() > 0).then_some(image)
}

/// Opaque RGB: transparency laid over white, as iOS draws it.
fn over_white(image: &DynamicImage) -> RgbImage {
    // A screenshot is RGBA with every pixel opaque, almost always: nothing
    // to lay over, and the codec crate's own conversion does it.
    let opaque = match image {
        DynamicImage::ImageRgba8(rgba) => rgba.as_raw().chunks_exact(4).all(|px| px[3] == 255),
        other => !other.color().has_alpha(),
    };
    if opaque {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut out = RgbImage::new(width, height);
    for (src, dst) in rgba.pixels().zip(out.pixels_mut()) {
        let alpha = u16::from(src[3]);
        let over = |c: u8| {
            let mixed = (u16::from(c) * alpha + 255 * (255 - alpha) + 127) / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        *dst = Rgb([over(src[0]), over(src[1]), over(src[2])]);
    }
    out
}

/// The size an image is drawn at: its longest edge at most `edge`, never
/// larger — the web's `fitWithin`, rounding and all.
#[must_use]
pub fn fit_within(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= edge {
        return (width, height);
    }
    let scale = f64::from(edge) / f64::from(longest);
    let side = |length: u32| {
        // Bounded by `edge` (≤ u32) and at least 1, so the cast is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let scaled = (f64::from(length) * scale).round().max(1.) as u32;
        scaled
    };
    (side(width), side(height))
}

/// Scaled through `DynamicImage`'s own (non-generic) methods: the generic
/// `imageops` functions would be compiled into this crate, at its
/// opt-level, and a debug build spent seconds per screenshot in them.
fn scale_within(image: &DynamicImage, edge: u32) -> Cow<'_, DynamicImage> {
    let (width, height) = fit_within(image.width(), image.height(), edge);
    if (width, height) == (image.width(), image.height()) {
        Cow::Borrowed(image)
    } else {
        Cow::Owned(image.resize_exact(width, height, FilterType::CatmullRom))
    }
}

/// JPEG with no metadata of its own, and no APP1 segment whatever the
/// encoder decided to write.
fn encode(image: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    image
        .write_with_encoder(JpegEncoder::new_with_quality(&mut out, quality))
        .ok()?;
    strip_app1(&out)
}

/// The JPEG with every APP1 segment (EXIF, XMP) removed — iOS's
/// `stripAPP1`. Walks the marker segments up to the start of the scan;
/// `None` for anything it cannot walk, so an unparsable file is never passed
/// through on trust.
#[must_use]
pub fn strip_app1(jpeg: &[u8]) -> Option<Vec<u8>> {
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return None;
    }
    let mut out = Vec::with_capacity(jpeg.len());
    out.extend_from_slice(&jpeg[..2]);
    let mut at = 2;
    while at + 4 <= jpeg.len() {
        if jpeg[at] != 0xFF {
            return None;
        }
        let marker = jpeg[at + 1];
        // Start of scan: the rest is image data, copied as it is.
        if marker == 0xDA {
            out.extend_from_slice(&jpeg[at..]);
            return Some(out);
        }
        let length = usize::from(jpeg[at + 2]) << 8 | usize::from(jpeg[at + 3]);
        if length < 2 || at + 2 + length > jpeg.len() {
            return None;
        }
        if marker != 0xE1 {
            out.extend_from_slice(&jpeg[at..at + 2 + length]);
        }
        at += 2 + length;
    }
    None
}

/// Whether `jpeg` carries an APP1 segment before its scan — the test seam
/// for the privacy guarantee.
#[cfg(test)]
#[must_use]
pub fn has_app1(jpeg: &[u8]) -> bool {
    let mut at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF {
        match jpeg[at + 1] {
            0xDA => return false,
            0xE1 => return true,
            _ => at += 2 + (usize::from(jpeg[at + 2]) << 8 | usize::from(jpeg[at + 3])),
        }
    }
    false
}

/// Standard base64 (`+/`), padded, no line breaks, no `data:` prefix — what
/// the endpoint decodes. Spelled out: this crate names no base64 dependency.
#[must_use]
pub fn to_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        let sextet = |shift: u32| char::from(ALPHABET[((n >> shift) & 0x3F) as usize]);
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

/// The two pictures a ready tile draws, both decoded from the PREPARED
/// bytes — the ones that will be sent, never the original: the tile's
/// square (centre-cropped and scaled to `side`, as the web's
/// `object-fit: cover` shows it) and the whole picture the viewer opens.
pub struct Pictures {
    pub thumb: Arc<RenderImage>,
    pub full: Arc<RenderImage>,
}

/// [`Pictures`] from a prepared JPEG; `None` if it will not decode.
#[must_use]
pub fn pictures(jpeg: &[u8], side: u32) -> Option<Pictures> {
    let image = image::load_from_memory(jpeg).ok()?;
    let (width, height) = (image.width(), image.height());
    let square = width.min(height);
    if square == 0 || side == 0 {
        return None;
    }
    let thumb = image
        .crop_imm((width - square) / 2, (height - square) / 2, square, square)
        .resize_exact(side, side, FilterType::Triangle);
    Some(Pictures {
        thumb: render_image(thumb)?,
        full: render_image(image)?,
    })
}

/// gpui draws premultiplied BGRA (see `raster.rs`); opaque pixels need only
/// the swizzle.
fn render_image(image: DynamicImage) -> Option<Arc<RenderImage>> {
    let (width, height) = (image.width(), image.height());
    let mut data = image.into_rgba8().into_raw();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let frame = RgbaImage::from_raw(width, height, data)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(frame)])))
}

/// Whether a dropped, pasted or picked path is worth decoding.
///
/// Only images are candidates — a PDF or a text file is refused at once,
/// so it never takes one of the five places. The extension is the web's
/// MIME type here; a file with none is tried (the web lets an empty type
/// through too), and the decoder is still the judge of every candidate.
#[must_use]
pub fn is_candidate_path(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        None => true,
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "jpe"
                | "jfif"
                | "webp"
                | "gif"
                | "bmp"
                | "tif"
                | "tiff"
                | "heic"
                | "heif"
                | "avif"
        ),
    }
}

/// Why a file was not taken — shown in its own line above the public line,
/// until the next change (v2 A1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// More were offered than fit: the first that fit were taken.
    Limit,
    /// Not an image, or one this machine cannot decode.
    Unsupported,
}

/// One tile: still being prepared (`ready` is `None`), or ready.
#[derive(Clone, Debug)]
pub struct Shot<T> {
    pub id: u64,
    pub ready: Option<T>,
}

/// The row of tiles, as rules. `T` is whatever a ready tile holds.
#[derive(Clone, Debug)]
pub struct Tray<T> {
    shots: Vec<Shot<T>>,
    next_id: u64,
    refusal: Option<Refusal>,
}

impl<T> Default for Tray<T> {
    fn default() -> Self {
        Self {
            shots: Vec::new(),
            next_id: 0,
            refusal: None,
        }
    }
}

impl<T> Tray<T> {
    /// The tiles, in the order they were added — the order they are sent in.
    #[must_use]
    pub fn shots(&self) -> &[Shot<T>] {
        &self.shots
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.shots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shots.is_empty()
    }

    #[must_use]
    pub fn refusal(&self) -> Option<Refusal> {
        self.refusal
    }

    /// A send is the next change: the refusal has been read.
    pub fn clear_refusal(&mut self) {
        self.refusal = None;
    }

    /// Take `images` new candidates (and note `others`, offered files that
    /// are not images): the web's `chooseScreenshots`. The first that fit
    /// become tiles waiting for their picture, and their ids come back in
    /// order. `Unsupported` wins the notice when both happened — it is the
    /// one the person can act on with the same files.
    pub fn add(&mut self, images: usize, others: usize) -> Vec<u64> {
        if images == 0 && others == 0 {
            return Vec::new();
        }
        let room = MAX_SCREENSHOTS.saturating_sub(self.shots.len());
        let take = images.min(room);
        self.refusal = if others > 0 {
            Some(Refusal::Unsupported)
        } else if images > room {
            Some(Refusal::Limit)
        } else {
            None
        };
        (0..take)
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                self.shots.push(Shot { id, ready: None });
                id
            })
            .collect()
    }

    /// A tile's picture is ready (`Some`), or it could not be prepared
    /// (`None`): that tile goes, with the unsupported line, and the others
    /// stay. `false` when the tile was removed while it was being prepared.
    pub fn finish(&mut self, id: u64, prepared: Option<T>) -> bool {
        let Some(at) = self.shots.iter().position(|shot| shot.id == id) else {
            return false;
        };
        match prepared {
            Some(ready) => self.shots[at].ready = Some(ready),
            None => {
                self.shots.remove(at);
                self.refusal = Some(Refusal::Unsupported);
            }
        }
        true
    }

    /// The person removed a tile: the next change, so the refusal goes too.
    pub fn remove(&mut self, id: u64) {
        let before = self.shots.len();
        self.shots.retain(|shot| shot.id != id);
        if self.shots.len() != before {
            self.refusal = None;
        }
    }

    /// Any tile still being prepared.
    #[must_use]
    pub fn is_processing(&self) -> bool {
        self.shots.iter().any(|shot| shot.ready.is_none())
    }

    /// What a send carries, in tile order — or `None` while a tile is still
    /// being prepared: the person attached it and expects it to go, so the
    /// send waits for it instead of leaving it behind.
    #[must_use]
    pub fn ready_for_send(&self) -> Option<Vec<&T>> {
        if self.is_processing() {
            return None;
        }
        Some(self.shots.iter().filter_map(|s| s.ready.as_ref()).collect())
    }

    /// Done: an empty tray for the next report.
    pub fn clear(&mut self) {
        self.shots.clear();
        self.refusal = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        // A gradient with a transparent band, so the alpha path is walked.
        let picture = RgbaImage::from_fn(width, height, |x, y| {
            let alpha = if y < height / 8 { 0 } else { 255 };
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, alpha])
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(picture)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap_or_else(|error| unreachable!("png: {error}"));
        out
    }

    /// A JPEG carrying an EXIF APP1 segment with an orientation of 6 (turn
    /// 90° clockwise) and a GPS latitude — the photo whose location must
    /// never ship.
    fn gps_tagged_jpeg(width: u32, height: u32) -> Vec<u8> {
        let picture = RgbImage::from_fn(width, height, |x, _| Rgb([(x * 6) as u8, 40, 200]));
        let mut plain = Vec::new();
        JpegEncoder::new_with_quality(&mut plain, 90)
            .encode_image(&picture)
            .unwrap_or_else(|error| unreachable!("jpeg: {error}"));

        // TIFF, big-endian: IFD0 {Orientation = 6, GPS IFD pointer},
        // GPS IFD {LatitudeRef "N", Latitude 37/1 46/1 30/1}.
        let mut tiff: Vec<u8> = b"MM\0\x2a".to_vec();
        tiff.extend_from_slice(&8u32.to_be_bytes());
        let entry = |tag: u16, kind: u16, count: u32, value: [u8; 4]| {
            let mut e = Vec::with_capacity(12);
            e.extend_from_slice(&tag.to_be_bytes());
            e.extend_from_slice(&kind.to_be_bytes());
            e.extend_from_slice(&count.to_be_bytes());
            e.extend_from_slice(&value);
            e
        };
        let gps_ifd = 8 + 2 + 2 * 12 + 4;
        tiff.extend_from_slice(&2u16.to_be_bytes());
        tiff.extend(entry(0x0112, 3, 1, [0, 6, 0, 0]));
        tiff.extend(entry(0x8825, 4, 1, (gps_ifd as u32).to_be_bytes()));
        tiff.extend_from_slice(&0u32.to_be_bytes());
        let rationals = gps_ifd + 2 + 2 * 12 + 4;
        tiff.extend_from_slice(&2u16.to_be_bytes());
        tiff.extend(entry(0x0001, 2, 2, *b"N\0\0\0"));
        tiff.extend(entry(0x0002, 5, 3, (rationals as u32).to_be_bytes()));
        tiff.extend_from_slice(&0u32.to_be_bytes());
        for value in [37u32, 46, 30] {
            tiff.extend_from_slice(&value.to_be_bytes());
            tiff.extend_from_slice(&1u32.to_be_bytes());
        }
        let mut app1 = vec![0xFF, 0xE1];
        let length = (2 + 6 + tiff.len()) as u16;
        app1.extend_from_slice(&length.to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend(tiff);

        let mut tagged = plain[..2].to_vec();
        tagged.extend(app1);
        tagged.extend_from_slice(&plain[2..]);
        tagged
    }

    /// What leaves is a JPEG with no APP1 at all — even when what came in
    /// carried a location — and it was turned the way the photo was taken.
    #[test]
    fn a_gps_tagged_photo_leaves_as_a_bare_upright_jpeg() {
        let input = gps_tagged_jpeg(40, 20);
        assert!(has_app1(&input), "the fixture must carry EXIF");
        assert!(input.windows(4).any(|w| w == b"Exif"));

        let prepared = prepare(&input).unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!(&prepared.jpeg[..3], &[0xFF, 0xD8, 0xFF]);
        assert!(!has_app1(&prepared.jpeg));
        assert!(!prepared.jpeg.windows(4).any(|w| w == b"Exif"));
        // Orientation 6: the 40×20 stored picture is 20×40 as seen.
        assert_eq!((prepared.width, prepared.height), (20, 40));
        let decoded = image::load_from_memory(&prepared.jpeg)
            .unwrap_or_else(|error| unreachable!("re-decode: {error}"));
        assert_eq!((decoded.width(), decoded.height()), (20, 40));
    }

    /// A PNG is re-encoded too, even when small: the round trip IS the
    /// guarantee. Longest edge 1920, the aspect kept, never upscaled.
    #[test]
    fn the_longest_edge_is_1920_and_the_aspect_is_kept() {
        let prepared = prepare(&png(2400, 1200)).unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!((prepared.width, prepared.height), (1920, 960));
        assert_eq!(&prepared.jpeg[..3], &[0xFF, 0xD8, 0xFF]);
        assert!(prepared.jpeg.len() <= MAX_SCREENSHOT_BYTES);

        let tall = prepare(&png(300, 2500)).unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!((tall.width, tall.height), (230, 1920));

        let small = prepare(&png(64, 48)).unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!((small.width, small.height), (64, 48));
        assert_eq!(&small.jpeg[..3], &[0xFF, 0xD8, 0xFF]);
    }

    /// Transparency is laid over white, not black.
    #[test]
    fn transparency_turns_white() {
        let prepared = prepare(&png(64, 64)).unwrap_or_else(|| unreachable!("decodable"));
        let decoded = image::load_from_memory(&prepared.jpeg)
            .unwrap_or_else(|error| unreachable!("re-decode: {error}"))
            .to_rgb8();
        let top = decoded.get_pixel(32, 1);
        assert!(top.0.iter().all(|c| *c > 235), "{top:?}");
    }

    /// Nothing fits the cap: 0.85, then 0.7, then the 1440 edge — and the
    /// last rung is what goes.
    #[test]
    fn the_ladder_ends_at_the_1440_edge() {
        let prepared =
            prepare_capped(&png(2400, 1200), 1).unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!((prepared.width, prepared.height), (1440, 720));
        let roomy = prepare_capped(&png(2400, 1200), usize::MAX)
            .unwrap_or_else(|| unreachable!("decodable"));
        assert_eq!((roomy.width, roomy.height), (1920, 960));
    }

    /// A file that is not an image is refused, never sent as it was.
    #[test]
    fn what_cannot_be_decoded_is_refused() {
        assert_eq!(prepare(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"), None);
        assert_eq!(prepare(b"just some words"), None);
        assert_eq!(prepare(&[]), None);
        // A JPEG cut off after its header is not an image either.
        let truncated = &gps_tagged_jpeg(40, 20)[..40];
        assert_eq!(prepare(truncated), None);
    }

    #[test]
    fn fit_within_matches_the_web() {
        assert_eq!(fit_within(3024, 1964, 1920), (1920, 1247));
        assert_eq!(fit_within(1920, 1080, 1920), (1920, 1080));
        assert_eq!(fit_within(100, 5000, 1440), (29, 1440));
        assert_eq!(fit_within(1, 10_000, 1920), (1, 1920));
    }

    #[test]
    fn base64_is_standard_and_padded() {
        assert_eq!(to_base64(b""), "");
        assert_eq!(to_base64(b"f"), "Zg==");
        assert_eq!(to_base64(b"fo"), "Zm8=");
        assert_eq!(to_base64(b"foo"), "Zm9v");
        assert_eq!(to_base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(to_base64(&[0xFB, 0xFF, 0xBF]), "+/+/");
        assert_eq!(to_base64(&[0xFF, 0xD8, 0xFF, 0xE0]), "/9j/4A==");
    }

    #[test]
    fn strip_app1_keeps_everything_else() {
        let tagged = gps_tagged_jpeg(16, 16);
        let stripped = strip_app1(&tagged).unwrap_or_else(|| unreachable!("walkable"));
        assert!(!has_app1(&stripped));
        assert!(image::load_from_memory(&stripped).is_ok());
        assert_eq!(strip_app1(b"not a jpeg"), None);
    }

    #[test]
    fn only_image_paths_are_candidates() {
        assert!(is_candidate_path(Path::new(
            "Screenshot 2026-09-27 at 10.00.00.png"
        )));
        assert!(is_candidate_path(Path::new("IMG_0001.HEIC")));
        assert!(is_candidate_path(Path::new("photo.JPG")));
        assert!(is_candidate_path(Path::new("no-extension")));
        assert!(!is_candidate_path(Path::new("notes.txt")));
        assert!(!is_candidate_path(Path::new("statement.pdf")));
    }

    #[test]
    fn the_tile_is_a_square_and_the_viewer_the_whole_picture() {
        let prepared = prepare(&png(300, 120)).unwrap_or_else(|| unreachable!("decodable"));
        let pictures =
            pictures(&prepared.jpeg, THUMB_PX).unwrap_or_else(|| unreachable!("pictures"));
        let side = |image: &RenderImage| {
            let size = image.size(0);
            (
                u32::try_from(size.width.0).ok(),
                u32::try_from(size.height.0).ok(),
            )
        };
        assert_eq!(side(&pictures.thumb), (Some(THUMB_PX), Some(THUMB_PX)));
        assert_eq!(side(&pictures.full), (Some(300), Some(120)));
    }

    /// Five fit; the sixth is dropped with the limit line, and the first
    /// that fit are the ones taken.
    #[test]
    fn the_tray_holds_five() {
        let mut tray: Tray<&str> = Tray::default();
        let first = tray.add(3, 0);
        assert_eq!(first, vec![0, 1, 2]);
        assert_eq!(tray.refusal(), None);
        let second = tray.add(4, 0);
        assert_eq!(second, vec![3, 4], "only two places were left");
        assert_eq!(tray.len(), MAX_SCREENSHOTS);
        assert_eq!(tray.refusal(), Some(Refusal::Limit));
        // Full: nothing more is taken, and it says so again.
        assert!(tray.add(1, 0).is_empty());
        assert_eq!(tray.refusal(), Some(Refusal::Limit));
        // Removing one is the next change: the line goes, a place opens.
        tray.remove(2);
        assert_eq!(tray.refusal(), None);
        assert_eq!(tray.add(1, 0), vec![5]);
    }

    /// A file that is not an image takes no place and says so; it wins the
    /// notice over the limit.
    #[test]
    fn a_non_image_is_refused_as_unsupported() {
        let mut tray: Tray<&str> = Tray::default();
        assert_eq!(tray.add(1, 1), vec![0]);
        assert_eq!(tray.refusal(), Some(Refusal::Unsupported));
        tray.add(9, 1);
        assert_eq!(tray.len(), MAX_SCREENSHOTS);
        assert_eq!(tray.refusal(), Some(Refusal::Unsupported));
        // A candidate the decoder refuses goes, with the same line.
        let mut tray: Tray<&str> = Tray::default();
        let ids = tray.add(2, 0);
        assert!(tray.finish(ids[0], None));
        assert_eq!(tray.len(), 1);
        assert_eq!(tray.refusal(), Some(Refusal::Unsupported));
    }

    /// Tiles are sent in the order they were added, whatever order their
    /// pictures were ready in.
    #[test]
    fn the_tile_order_is_the_send_order() {
        let mut tray: Tray<&str> = Tray::default();
        let ids = tray.add(3, 0);
        tray.finish(ids[2], Some("third"));
        tray.finish(ids[0], Some("first"));
        tray.finish(ids[1], Some("second"));
        assert_eq!(
            tray.ready_for_send(),
            Some(vec![&"first", &"second", &"third"])
        );
    }

    /// Send waits while a tile is being prepared, and a tile that fails
    /// while it waits goes without holding the rest back.
    #[test]
    fn send_waits_for_tiles_still_being_prepared() {
        let mut tray: Tray<&str> = Tray::default();
        assert_eq!(tray.ready_for_send(), Some(Vec::new()), "no tiles, no wait");
        let ids = tray.add(3, 0);
        tray.finish(ids[0], Some("a"));
        assert!(tray.is_processing());
        assert_eq!(tray.ready_for_send(), None);
        tray.finish(ids[1], None);
        assert_eq!(tray.ready_for_send(), None);
        tray.finish(ids[2], Some("c"));
        assert!(!tray.is_processing());
        assert_eq!(tray.ready_for_send(), Some(vec![&"a", &"c"]));
        assert_eq!(tray.refusal(), Some(Refusal::Unsupported));
    }

    /// A tile removed while it was being prepared stays removed.
    #[test]
    fn a_removed_tile_is_not_brought_back() {
        let mut tray: Tray<&str> = Tray::default();
        let ids = tray.add(2, 0);
        tray.remove(ids[0]);
        assert!(!tray.finish(ids[0], Some("late")));
        assert_eq!(tray.len(), 1);
        tray.clear();
        assert!(tray.is_empty());
    }
}
