//! vela-qr — reads QR codes out of 8-bit luminance images.
//!
//! QR model 2 only, no dependencies, no `unsafe`. The detector, sampler and
//! decoder are a port of zxing-cpp's (via rxing), Apache-2.0 — see NOTICE.
//!
//! What this crate decides is only *what text a symbol carries*; what that
//! text means is the caller's to judge. It never guesses: a symbol it cannot
//! read completely and correctly (error correction exhausted, an unsupported
//! character set, one part of a structured-append sequence) yields nothing.

#![forbid(unsafe_code)]

mod binarize;
mod bitmatrix;
mod concentric;
mod content;
mod cursor;
mod decode;
mod detect;
mod geometry;
mod rs;
mod version;
pub mod wire;

use bitmatrix::BitMatrix;
pub use content::{Charset, Code, Part, Segment};

/// Every QR code found in a `width`×`height` 8-bit luminance image
/// (row-major, one byte per pixel), each once, in detection order.
#[must_use]
pub fn read_all(luma: &[u8], width: usize, height: usize) -> Vec<Code> {
    scan(luma, width, height, usize::MAX)
}

/// The first QR code found, if any. [`Code::text`] is its text whenever the
/// crate can produce it alone (ASCII, UTF-8, …); otherwise its segments name
/// the charset for the platform's decoder.
///
/// Tried at the image's own size, dark-on-light and light-on-dark, then at
/// each halving of the size (a 2×2 average, which also smooths noise) until
/// the image is too small to hold a code. Halving is what reads a code whose
/// modules are wider than the binarizer's local window — a phone held close
/// to a screen — and it costs at most a third more than the first pass. A
/// small image is finally tried at double size, which is what reads modules
/// under two pixels wide.
#[must_use]
pub fn read(luma: &[u8], width: usize, height: usize) -> Option<Code> {
    scan(luma, width, height, 1).into_iter().next()
}

/// Below this, a level is not halved again: version 1 is 21 modules plus a
/// quiet zone, and the binarizer wants 40 pixels for its 5×5 blocks.
const MIN_SIDE: usize = 40;

/// Images whose longest side is under this are also tried at double size.
const DOUBLE_BELOW: usize = 640;

fn scan(luma: &[u8], width: usize, height: usize, max: usize) -> Vec<Code> {
    if width == 0 || height == 0 || width.checked_mul(height) != Some(luma.len()) {
        return Vec::new();
    }
    let mut found: Vec<Code> = Vec::new();
    let (mut w, mut h) = (width, height);
    let mut level: Option<Vec<u8>> = None;
    loop {
        let pixels = level.as_deref().unwrap_or(luma);
        if attempt(pixels, w, h, max, &mut found) {
            return found;
        }
        if w / 2 < MIN_SIDE || h / 2 < MIN_SIDE {
            break;
        }
        level = Some(halve(pixels, w, h));
        (w, h) = (w / 2, h / 2);
    }
    if width.max(height) < DOUBLE_BELOW {
        attempt(
            &double(luma, width, height),
            width * 2,
            height * 2,
            max,
            &mut found,
        );
    }
    found
}

/// One size, both polarities; true once `max` codes are found.
fn attempt(pixels: &[u8], w: usize, h: usize, max: usize, found: &mut Vec<Code>) -> bool {
    let Some(mut bits) = binarize::binarize(pixels, w, h) else {
        return false;
    };
    for pass in 0..2 {
        if pass == 1 {
            bits.invert();
        }
        for code in read_bits(&bits, max - found.len()) {
            if !found.contains(&code) {
                found.push(code);
            }
        }
        if found.len() >= max {
            return true;
        }
    }
    false
}

/// Half the size, each pixel the rounded mean of a 2×2 block.
fn halve(luma: &[u8], width: usize, height: usize) -> Vec<u8> {
    let (w, h) = (width / 2, height / 2);
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        let (r0, r1) = (&luma[2 * y * width..], &luma[(2 * y + 1) * width..]);
        for x in 0..w {
            let sum = u16::from(r0[2 * x])
                + u16::from(r0[2 * x + 1])
                + u16::from(r1[2 * x])
                + u16::from(r1[2 * x + 1]);
            out.push(((sum + 2) / 4) as u8);
        }
    }
    out
}

/// Double the size, bilinearly (each output pixel ¾ its nearest source pixel,
/// ¼ the next one, per axis).
fn double(luma: &[u8], width: usize, height: usize) -> Vec<u8> {
    let (w, h) = (width * 2, height * 2);
    // Source index pairs and weights (in quarters) for one output coordinate.
    let taps = |o: usize, n: usize| -> (usize, usize, u32) {
        let near = o / 2;
        let far = if o % 2 == 0 {
            near.saturating_sub(1)
        } else {
            (near + 1).min(n - 1)
        };
        (near, far, 3)
    };
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        let (ny, fy, wy) = taps(y, height);
        for x in 0..w {
            let (nx, fx, wx) = taps(x, width);
            let px = |xx: usize, yy: usize| u32::from(luma[yy * width + xx]);
            let v = wy * (wx * px(nx, ny) + (4 - wx) * px(fx, ny))
                + (4 - wy) * (wx * px(nx, fy) + (4 - wx) * px(fx, fy));
            out.push(((v + 8) / 16) as u8);
        }
    }
    out
}

/// One pass, one size, dark-on-light only — the lab's baseline.
#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn read_once(luma: &[u8], width: usize, height: usize) -> Option<Code> {
    if width == 0 || height == 0 || width.checked_mul(height) != Some(luma.len()) {
        return None;
    }
    read_bits(&binarize::binarize(luma, width, height)?, 1)
        .into_iter()
        .next()
}

fn read_bits(bits: &BitMatrix, max: usize) -> Vec<Code> {
    let mut patterns = detect::find_finder_patterns(bits);
    let sets = detect::generate_finder_sets(&mut patterns);
    let mut used: Vec<geometry::P> = Vec::new();
    let mut found = Vec::new();
    for set in sets {
        if [set.bl.p, set.tl.p, set.tr.p]
            .iter()
            .any(|c| used.contains(c))
        {
            continue;
        }
        let Some(grid) = detect::sample_qr(bits, &set) else {
            continue;
        };
        if let Some(code) = decode::decode(&grid) {
            used.extend([set.bl.p, set.tl.p, set.tr.p]);
            found.push(code);
            if found.len() >= max {
                break;
            }
        }
    }
    found
}

/// Lab diagnostics: how far each stage got. Not part of the API.
#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn probe(luma: &[u8], width: usize, height: usize) -> String {
    use core::fmt::Write;
    let mut out = String::new();
    let Some(bits) = binarize::binarize(luma, width, height) else {
        return "binarize failed".into();
    };
    let mut patterns = detect::find_finder_patterns(&bits);
    let _ = writeln!(out, "{} finder patterns", patterns.len());
    for c in &patterns {
        let _ = writeln!(out, "  fp ({:.1},{:.1}) size {}", c.p.x, c.p.y, c.size);
    }
    let sets = detect::generate_finder_sets(&mut patterns);
    let _ = writeln!(out, "{} sets", sets.len());
    for set in sets.iter().take(6) {
        let grid = detect::sample_qr(&bits, set);
        let text = grid.as_ref().and_then(decode::decode);
        let _ = writeln!(
            out,
            "  set tl({:.0},{:.0}) tr({:.0},{:.0}) bl({:.0},{:.0}): {}",
            set.tl.p.x,
            set.tl.p.y,
            set.tr.p.x,
            set.tr.p.y,
            set.bl.p.x,
            set.bl.p.y,
            match (&grid, &text) {
                (None, _) => "sample failed".to_owned(),
                (Some(g), None) => format!("{}x{} sampled, decode failed", g.width(), g.height()),
                (Some(_), Some(c)) => format!(
                    "OK {:?}",
                    c.text().map(|t| t.chars().take(30).collect::<String>())
                ),
            }
        );
    }
    out
}

#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn probe_binarize(luma: &[u8], width: usize, height: usize) -> Vec<u8> {
    binarize::binarize(luma, width, height)
        .map(|b| b.raw().to_vec())
        .unwrap_or_default()
}

#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn probe_finders(luma: &[u8], width: usize, height: usize) -> Vec<(f64, f64, i32)> {
    let Some(bits) = binarize::binarize(luma, width, height) else {
        return Vec::new();
    };
    detect::find_finder_patterns(&bits)
        .iter()
        .map(|c| (c.p.x, c.p.y, c.size))
        .collect()
}

#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn probe_row(
    luma: &[u8],
    width: usize,
    height: usize,
    y: usize,
    x0: usize,
    x1: usize,
) -> String {
    let Some(bits) = binarize::binarize(luma, width, height) else {
        return String::new();
    };
    detect::probe_row(&bits, y, x0, x1)
}

/// Fuzzing entry: a module grid straight into format, error-correction and
/// bitstream decoding, past the detector.
#[cfg(feature = "probe")]
#[doc(hidden)]
pub fn decode_grid(modules: &[u8], dimension: usize) -> Option<Code> {
    if modules.len() != dimension * dimension {
        return None;
    }
    decode::decode(&BitMatrix::from_bits(
        dimension,
        dimension,
        modules.iter().map(|&m| m & 1).collect(),
    ))
}
