//! PNG/pixmap → gpui `RenderImage` conversion (spec 015).
//!
//! gpui's renderer consumes **premultiplied BGRA** wrapped in an `RgbaImage` —
//! the old launch-animation frame pump (until 2026-11-14) fed ThorVG's
//! premultiplied BGRA through `RgbaImage::from_raw` with no swizzle and it
//! rendered correctly, which is how the expected layout was pinned. Everything rasterized here (identicon PNGs, tinted icons)
//! arrives as straight or premultiplied RGBA and must be converted once.

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};

/// Decode PNG bytes (straight-alpha RGBA) into a gpui image.
pub fn render_image_from_png(png: &[u8]) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory(png).ok()?.into_rgba8();
    let (w, h) = decoded.dimensions();
    let mut data = decoded.into_raw();
    for px in data.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        let premul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
        let (r, g, b) = (premul(px[0]), premul(px[1]), premul(px[2]));
        px[0] = b;
        px[1] = g;
        px[2] = r;
    }
    let rgba = RgbaImage::from_raw(w, h, data)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(rgba)])))
}

/// Wrap a tiny-skia pixmap (premultiplied RGBA) into a gpui image.
pub fn render_image_from_pixmap(pixmap: &resvg::tiny_skia::Pixmap) -> Option<Arc<RenderImage>> {
    let (w, h) = (pixmap.width(), pixmap.height());
    let mut data = pixmap.data().to_vec();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2); // premultiplied RGBA → premultiplied BGRA
    }
    let rgba = RgbaImage::from_raw(w, h, data)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(rgba)])))
}

/// A 1×1 transparent fallback so a conversion failure renders as nothing
/// instead of panicking mid-frame.
pub fn empty_render_image() -> Arc<RenderImage> {
    let rgba = RgbaImage::from_raw(1, 1, vec![0, 0, 0, 0]).expect("1x1 buffer");
    Arc::new(RenderImage::new(vec![Frame::new(rgba)]))
}
