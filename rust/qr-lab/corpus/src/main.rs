//! qr-corpus — a reproducible QR decoding corpus for the decoder lab.
//!
//! Every sample is generated from a fixed seed, so a decoder written next month
//! is measured against exactly the images today's baseline was. Payloads are
//! the strings Vela actually scans (addresses, EIP-681 links, WalletConnect
//! URIs, `FIDO:/` pairing codes, long links), encoded by the same `qrcode`
//! crate and EC levels vela-core draws with (`M` on screen, `H` on the share
//! card), then degraded the ways a camera, a screenshot or a chat app degrade
//! them.
//!
//! Output (under `--out`, default `out/corpus`):
//!   manifest.json         every sample: expected text, files, parameters
//!   <id>.gray.gz          the image, 8-bit luma, row-major, gzip
//!   <id>@<w>.gray.gz      the web still-image ladder (widths 1200..400 < width)
//!   <id>@cam.gray.gz      one camera frame: the image drawn 1000 wide
//!   ../preview/<id>.png   the first few of each category, to look at
//!
//! Resizing is `image`'s Triangle filter, an antialiased resample — the same
//! low-pass a canvas `drawImage` downscale provides (spec 028's finding). It
//! is an approximation of the browser, stated rather than hidden.

use std::f64::consts::TAU;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use image::{imageops, GrayImage, Luma};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct Variant {
    w: u32,
    h: u32,
    file: String,
}

#[derive(Serialize)]
struct Sample {
    id: String,
    category: &'static str,
    payload_kind: &'static str,
    ec: &'static str,
    expected: String,
    width: u32,
    height: u32,
    file: String,
    /// The web still-image ladder, in the order `qr-decode.ts` walks it.
    ladder: Vec<Variant>,
    /// One camera frame: the image drawn 1000 wide (`CAMERA_FRAME_WIDTH`).
    camera: Variant,
    params: Value,
}

#[derive(Serialize)]
struct Manifest {
    seed: u64,
    per_category: usize,
    resize_filter: &'static str,
    samples: Vec<Sample>,
}

const LADDER: [u32; 5] = [1200, 1000, 800, 600, 400];
const CAMERA_WIDTH: u32 = 1000;

// ---------------------------------------------------------------------------
// Payloads — what a Vela scanner is actually pointed at
// ---------------------------------------------------------------------------

fn hex(rng: &mut StdRng, n: usize) -> String {
    const H: &[u8] = b"0123456789abcdef";
    (0..n).map(|_| H[rng.gen_range(0..16)] as char).collect()
}

fn address(rng: &mut StdRng) -> String {
    let body: String = hex(rng, 40)
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() && rng.gen_bool(0.5) {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    format!("0x{body}")
}

fn payload(rng: &mut StdRng) -> (&'static str, String) {
    let chains = [1u64, 10, 56, 100, 137, 8453, 42161, 5042002];
    let chain = chains[rng.gen_range(0..chains.len())];
    match rng.gen_range(0..100) {
        0..=24 => ("address", address(rng)),
        25..=39 => {
            let wei: u128 = rng.gen_range(1..10_000) * 10u128.pow(rng.gen_range(12..19));
            ("eip681-native", format!("ethereum:{}@{chain}?value={wei}", address(rng)))
        }
        40..=54 => {
            let amount: u64 = rng.gen_range(1..100_000_000_000);
            (
                "eip681-token",
                format!(
                    "ethereum:{}@{chain}/transfer?address={}&uint256={amount}",
                    address(rng),
                    address(rng)
                ),
            )
        }
        55..=79 => {
            let ts: u64 = rng.gen_range(1_790_000_000..1_800_000_000);
            (
                "walletconnect",
                format!(
                    "wc:{}@2?relay-protocol=irn&symKey={}&expiryTimestamp={ts}",
                    hex(rng, 64),
                    hex(rng, 64)
                ),
            )
        }
        80..=89 => {
            let n = rng.gen_range(120..220);
            let digits: String = (0..n).map(|_| char::from(b'0' + rng.gen_range(0..10u8))).collect();
            ("fido", format!("FIDO:/{digits}"))
        }
        _ => {
            const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let n = rng.gen_range(60..180);
            let tail: String = (0..n).map(|_| B64[rng.gen_range(0..64)] as char).collect();
            ("long-url", format!("https://getvela.app/link#{tail}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Encoding and rendering
// ---------------------------------------------------------------------------

struct Qr {
    n: u32,
    dark: Vec<bool>,
}

fn encode(text: &str, high: bool) -> Qr {
    let ec = if high { qrcode::EcLevel::H } else { qrcode::EcLevel::M };
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), ec).expect("payload fits");
    Qr {
        n: code.width() as u32,
        dark: code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect(),
    }
}

fn render(qr: &Qr, module: u32, quiet: u32, dark: u8, light: u8) -> GrayImage {
    let side = (qr.n + 2 * quiet) * module;
    GrayImage::from_fn(side, side, |x, y| {
        let mx = (x / module) as i64 - i64::from(quiet);
        let my = (y / module) as i64 - i64::from(quiet);
        let n = i64::from(qr.n);
        let on = mx >= 0 && my >= 0 && mx < n && my < n && qr.dark[(my * n + mx) as usize];
        Luma([if on { dark } else { light }])
    })
}

/// The share card's centre: a light plate carrying a dark logo disc.
fn logo_plate(img: &mut GrayImage, frac: f64) {
    let side = f64::from(img.width());
    let plate = side * frac;
    let c = side / 2.0;
    let r_logo = plate * 0.33;
    for y in 0..img.height() {
        for x in 0..img.width() {
            let (dx, dy) = (f64::from(x) + 0.5 - c, f64::from(y) + 0.5 - c);
            if dx.abs() <= plate / 2.0 && dy.abs() <= plate / 2.0 {
                let on_logo = dx * dx + dy * dy <= r_logo * r_logo;
                img.put_pixel(x, y, Luma([if on_logo { 40 } else { 250 }]));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pixel operations
// ---------------------------------------------------------------------------

fn resize(img: &GrayImage, w: u32, h: u32) -> GrayImage {
    imageops::resize(img, w.max(1), h.max(1), imageops::FilterType::Triangle)
}

/// `drawScaled(src, w)`: height = round(w * src.h / src.w), as the web does.
fn scale_to_width(img: &GrayImage, w: u32) -> GrayImage {
    let h = ((f64::from(w) * f64::from(img.height())) / f64::from(img.width())).round() as u32;
    resize(img, w, h)
}

fn gauss(rng: &mut StdRng) -> f64 {
    let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
}

fn add_noise(img: &mut GrayImage, sigma: f64, rng: &mut StdRng) {
    for p in img.pixels_mut() {
        p[0] = (f64::from(p[0]) + gauss(rng) * sigma).round().clamp(0.0, 255.0) as u8;
    }
}

fn jpeg(img: &GrayImage, quality: u8) -> GrayImage {
    let mut buf = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    img.write_with_encoder(enc).expect("jpeg encode");
    image::load_from_memory(&buf).expect("jpeg decode").to_luma8()
}

fn blur(img: &GrayImage, sigma: f64) -> GrayImage {
    if sigma < 0.05 {
        return img.clone();
    }
    imageops::blur(img, sigma as f32)
}

fn sample(img: &GrayImage, x: f64, y: f64) -> f64 {
    let (w, h) = (i64::from(img.width()), i64::from(img.height()));
    let raw = img.as_raw();
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let g = |xi: i64, yi: i64| f64::from(raw[(yi.clamp(0, h - 1) * w + xi.clamp(0, w - 1)) as usize]);
    g(x0, y0) * (1.0 - fx) * (1.0 - fy)
        + g(x0 + 1, y0) * fx * (1.0 - fy)
        + g(x0, y0 + 1) * (1.0 - fx) * fy
        + g(x0 + 1, y0 + 1) * fx * fy
}

type Homography = [f64; 9];

/// The projective map taking `from[i]` to `to[i]`.
fn homography(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Homography {
    let mut a = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let ((x, y), (u, v)) = (from[i], to[i]);
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for col in 0..8 {
        let pivot = (col..8)
            .max_by(|&p, &q| a[p][col].abs().total_cmp(&a[q][col].abs()))
            .expect("rows");
        a.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let f = a[row][col] / a[col][col];
                for k in col..9 {
                    a[row][k] -= f * a[col][k];
                }
            }
        }
    }
    let mut h = [0.0; 9];
    for i in 0..8 {
        h[i] = a[i][8] / a[i][i];
    }
    h[8] = 1.0;
    h
}

fn apply(h: &Homography, x: f64, y: f64) -> (f64, f64) {
    let d = h[6] * x + h[7] * y + h[8];
    ((h[0] * x + h[1] * y + h[2]) / d, (h[3] * x + h[4] * y + h[5]) / d)
}

/// Draw `src` into `bg` so that its corners land on `quad` (clockwise from
/// top-left). `ss` sub-samples per axis: 2 antialiases, 1 aliases (moiré).
fn composite(bg: &mut GrayImage, src: &GrayImage, quad: [(f64, f64); 4], ss: u32) {
    let (sw, sh) = (f64::from(src.width()), f64::from(src.height()));
    let h = homography(quad, [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)]);
    let xs = quad.iter().map(|p| p.0);
    let ys = quad.iter().map(|p| p.1);
    let x0 = xs.clone().fold(f64::MAX, f64::min).floor().max(0.0) as u32;
    let x1 = (xs.fold(f64::MIN, f64::max).ceil() as u32).min(bg.width());
    let y0 = ys.clone().fold(f64::MAX, f64::min).floor().max(0.0) as u32;
    let y1 = (ys.fold(f64::MIN, f64::max).ceil() as u32).min(bg.height());
    let n = f64::from(ss * ss);
    for y in y0..y1 {
        for x in x0..x1 {
            let here = f64::from(bg.get_pixel(x, y)[0]);
            let mut acc = 0.0;
            for j in 0..ss {
                for i in 0..ss {
                    let px = f64::from(x) + (f64::from(i) + 0.5) / f64::from(ss);
                    let py = f64::from(y) + (f64::from(j) + 0.5) / f64::from(ss);
                    let (u, v) = apply(&h, px, py);
                    acc += if u >= 0.0 && v >= 0.0 && u < sw && v < sh {
                        sample(src, u - 0.5, v - 0.5)
                    } else {
                        here
                    };
                }
            }
            bg.put_pixel(x, y, Luma([(acc / n).round().clamp(0.0, 255.0) as u8]));
        }
    }
}

/// A square of `side` centred at (cx, cy), rotated, each corner pushed by up
/// to `persp * side` — a code held at an angle to the lens.
fn quad(rng: &mut StdRng, cx: f64, cy: f64, side: f64, angle_deg: f64, persp: f64) -> [(f64, f64); 4] {
    let (s, c) = angle_deg.to_radians().sin_cos();
    let half = side / 2.0;
    let corners = [(-half, -half), (half, -half), (half, half), (-half, half)];
    corners.map(|(x, y)| {
        let jx = rng.gen_range(-1.0..=1.0) * persp * side;
        let jy = rng.gen_range(-1.0..=1.0) * persp * side;
        (cx + x * c - y * s + jx, cy + x * s + y * c + jy)
    })
}

/// A busy background: smooth blobs, text-like strokes, boxes, grain.
fn texture(rng: &mut StdRng, w: u32, h: u32, lo: u8, hi: u8) -> GrayImage {
    let (gw, gh) = (rng.gen_range(3..10), rng.gen_range(3..10));
    let small = GrayImage::from_fn(gw, gh, |_, _| Luma([rng.gen_range(lo..=hi)]));
    let mut bg = resize(&small, w, h);
    let strokes = rng.gen_range(30..120);
    for _ in 0..strokes {
        let text_like = rng.gen_bool(0.7);
        let (rw, rh) = if text_like {
            (rng.gen_range(w / 60..w / 4).max(2), rng.gen_range(2..(h / 150).max(3) + 6))
        } else {
            (rng.gen_range(w / 40..w / 5).max(2), rng.gen_range(h / 40..h / 5).max(2))
        };
        let (rx, ry) = (rng.gen_range(0..w), rng.gen_range(0..h));
        let v: u8 = if rng.gen_bool(0.6) { rng.gen_range(10..90) } else { rng.gen_range(170..250) };
        let hollow = !text_like && rng.gen_bool(0.5);
        for y in ry..(ry + rh).min(h) {
            for x in rx..(rx + rw).min(w) {
                let edge = x < rx + 3 || y < ry + 3 || x + 3 >= rx + rw || y + 3 >= ry + rh;
                if !hollow || edge {
                    bg.put_pixel(x, y, Luma([v]));
                }
            }
        }
    }
    add_noise(&mut bg, 3.0, rng);
    bg
}

/// Uneven light: a gradient that dims one side, a vignette, an optional glare spot.
fn lighting(rng: &mut StdRng, img: &mut GrayImage, strength: f64, glare: f64) {
    let (w, h) = (f64::from(img.width()), f64::from(img.height()));
    let (s, c) = (rng.gen::<f64>() * TAU).sin_cos();
    let proj = [0.0, w * c, h * s, w * c + h * s];
    let (pmin, pmax) = (proj.iter().cloned().fold(f64::MAX, f64::min), proj.iter().cloned().fold(f64::MIN, f64::max));
    let gmin = 1.0 - strength;
    let (gx, gy) = (rng.gen::<f64>() * w, rng.gen::<f64>() * h);
    let gr = w.min(h) * rng.gen_range(0.08..0.25);
    let (cx, cy, rmax2) = (w / 2.0, h / 2.0, (w * w + h * h) / 4.0);
    let vig = rng.gen_range(0.0..0.35);
    for y in 0..img.height() {
        for x in 0..img.width() {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let t = (fx * c + fy * s - pmin) / (pmax - pmin);
            let gain = (gmin + (1.0 - gmin) * t) * (1.0 - vig * ((fx - cx).powi(2) + (fy - cy).powi(2)) / rmax2);
            let spot = glare * (-((fx - gx).powi(2) + (fy - gy).powi(2)) / (2.0 * gr * gr)).exp();
            let p = img.get_pixel(x, y)[0];
            img.put_pixel(x, y, Luma([(f64::from(p) * gain + spot).round().clamp(0.0, 255.0) as u8]));
        }
    }
}

fn motion_blur(img: &GrayImage, len: f64, angle_deg: f64) -> GrayImage {
    if len < 1.0 {
        return img.clone();
    }
    let (s, c) = angle_deg.to_radians().sin_cos();
    let taps = len.ceil() as i32;
    GrayImage::from_fn(img.width(), img.height(), |x, y| {
        let mut acc = 0.0;
        for k in 0..=taps {
            let t = f64::from(k) / f64::from(taps) - 0.5;
            acc += sample(img, f64::from(x) + t * len * c, f64::from(y) + t * len * s);
        }
        Luma([(acc / f64::from(taps + 1)).round() as u8])
    })
}

/// A code photographed off a screen: the screen's pixel grid and subpixel
/// stripes, sampled by a camera pixel of about one screen pixel without a
/// prefilter — which is what makes moiré.
fn screen_photo(rng: &mut StdRng, qr: &Qr) -> (GrayImage, Value) {
    let k = rng.gen_range(3..=6u32);
    let f = 4u32;
    let mut screen = render(qr, k * f, 4, 20, 235);
    let grid = rng.gen_range(0.5..0.8);
    let stripes = [rng.gen_range(0.6..0.9), 1.0, rng.gen_range(0.8..1.0), rng.gen_range(0.55..0.85)];
    for y in 0..screen.height() {
        for x in 0..screen.width() {
            let mut g = stripes[(x % f) as usize];
            if y % f == 0 {
                g *= grid;
            }
            let p = screen.get_pixel(x, y)[0];
            screen.put_pixel(x, y, Luma([(f64::from(p) * g) as u8]));
        }
    }
    let pitch = f64::from(f) * rng.gen_range(0.75..1.45);
    let side = f64::from(screen.width()) / pitch;
    let scene = (side * rng.gen_range(1.25..1.6)) as u32;
    let mut bg = texture(rng, scene, scene, 20, 80);
    let angle = rng.gen_range(0.5..10.0) * if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
    let persp = rng.gen_range(0.0..0.06);
    let q = quad(rng, f64::from(scene) / 2.0, f64::from(scene) / 2.0, side, angle, persp);
    composite(&mut bg, &screen, q, 1);
    let lens = rng.gen_range(0.3..0.9);
    let mut img = blur(&bg, lens);
    let (light_s, light_g) = (rng.gen_range(0.1..0.4), rng.gen_range(0.0..90.0));
    lighting(rng, &mut img, light_s, light_g);
    let noise = rng.gen_range(3.0..8.0);
    add_noise(&mut img, noise, rng);
    let quality = rng.gen_range(70..93);
    let img = jpeg(&img, quality);
    (img, json!({"screen_px_per_module": k, "camera_px_per_screen_px": pitch / f64::from(f), "angle": angle, "persp": persp, "lens_blur": lens, "noise": noise, "jpeg": quality}))
}

/// A scene of `w`×`h` with the code drawn in at `side` px, angle, perspective.
fn scene_with_code(rng: &mut StdRng, qr: &Qr, w: u32, h: u32, side: f64, angle: f64, persp: f64, bg: GrayImage) -> GrayImage {
    let module = ((side / f64::from(qr.n + 8)).ceil() as u32).max(2);
    let code = render(qr, module, 4, 15, 240);
    let mut bg = bg;
    let margin = side * 0.6;
    let cx = rng.gen_range(margin.min(f64::from(w) / 2.0)..=(f64::from(w) - margin).max(f64::from(w) / 2.0));
    let cy = rng.gen_range(margin.min(f64::from(h) / 2.0)..=(f64::from(h) - margin).max(f64::from(h) / 2.0));
    let q = quad(rng, cx, cy, side, angle, persp);
    composite(&mut bg, &code, q, 2);
    let _ = (w, h);
    bg
}

fn sign(rng: &mut StdRng) -> f64 {
    if rng.gen_bool(0.5) {
        1.0
    } else {
        -1.0
    }
}

fn photo_dims(rng: &mut StdRng, big: bool) -> (u32, u32) {
    let (a, b) = if big { (3024, 4032) } else { (1512, 2016) };
    if rng.gen_bool(0.5) {
        (a, b)
    } else {
        (b, a)
    }
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

const CATEGORIES: &[&str] = &[
    "clean",
    "big-modules",
    "tiny-modules",
    "no-quiet-zone",
    "inverted",
    "low-contrast",
    "blur",
    "noise",
    "jpeg",
    "rotation",
    "perspective",
    "uneven-light",
    "moire",
    "share-card-logo",
    "clutter-large",
    "phone-photo",
    "camera-frame",
    "real-fixture-jpeg",
    "real-fixture-photo",
];

struct Fixture {
    img: GrayImage,
    expected: &'static str,
    name: &'static str,
}

struct Generated {
    img: GrayImage,
    kind: &'static str,
    ec: &'static str,
    expected: String,
    params: Value,
}

fn generate(cat: &str, i: usize, rng: &mut StdRng, fixtures: &[Fixture]) -> Generated {
    if cat.starts_with("real-fixture") {
        let fx = &fixtures[i % fixtures.len()];
        let (img, params) = if cat == "real-fixture-jpeg" {
            // A chat app's recompression of a saved receive card.
            let w = rng.gen_range(260..=fx.img.width());
            let q = rng.gen_range(10..=60);
            (jpeg(&scale_to_width(&fx.img, w), q), json!({"fixture": fx.name, "width": w, "jpeg": q}))
        } else {
            // The receive card on one phone, photographed by another.
            let (w, h) = photo_dims(rng, false);
            let side = f64::from(w.min(h)) * rng.gen_range(0.3..0.7);
            let (angle, persp) = (rng.gen_range(0.0..25.0) * sign(rng), rng.gen_range(0.02..0.15));
            let bg = texture(rng, w, h, 40, 200);
            let s = (side / f64::from(fx.img.width())) * f64::from(fx.img.width());
            let mut scene = bg;
            let margin = s * 0.6;
            let cx = rng.gen_range(margin..=(f64::from(w) - margin).max(margin + 1.0));
            let cy = rng.gen_range(margin..=(f64::from(h) - margin).max(margin + 1.0));
            let q = quad(rng, cx, cy, s, angle, persp);
            composite(&mut scene, &fx.img, q, 2);
            let sigma = rng.gen_range(0.5..1.8);
            let mut img = blur(&scene, sigma);
            let (light_s, light_g) = (rng.gen_range(0.2..0.6), rng.gen_range(0.0..80.0));
    lighting(rng, &mut img, light_s, light_g);
            let noise = rng.gen_range(4.0..12.0);
            add_noise(&mut img, noise, rng);
            let q = rng.gen_range(65..92);
            (jpeg(&img, q), json!({"fixture": fx.name, "side": side, "angle": angle, "persp": persp, "blur": sigma, "noise": noise, "jpeg": q}))
        };
        return Generated { img, kind: "fixture", ec: "M", expected: fx.expected.to_owned(), params };
    }

    let high = cat == "share-card-logo";
    let (kind, text) = payload(rng);
    let qr = encode(&text, high);
    let ec = if high { "H" } else { "M" };
    let (img, params) = match cat {
        "clean" => {
            let m = rng.gen_range(2..=12);
            (render(&qr, m, 4, 0, 255), json!({"module": m}))
        }
        "big-modules" => {
            let m = rng.gen_range(16..=32);
            (render(&qr, m, 4, 0, 255), json!({"module": m}))
        }
        "tiny-modules" => {
            let base = render(&qr, 8, 4, 0, 255);
            let mt: f64 = rng.gen_range(1.2..2.2);
            let w = (f64::from(base.width()) * mt / 8.0).round() as u32;
            (scale_to_width(&base, w), json!({"module": mt}))
        }
        "no-quiet-zone" => {
            let (m, q) = (rng.gen_range(4..=12), rng.gen_range(0..=1));
            (render(&qr, m, q, 0, 255), json!({"module": m, "quiet": q}))
        }
        "inverted" => {
            let m = rng.gen_range(4..=12);
            (render(&qr, m, 4, 255, 0), json!({"module": m}))
        }
        "low-contrast" => {
            let d = rng.gen_range(40..=130u8);
            let l = d.saturating_add(rng.gen_range(35..=80)).min(235);
            let m = rng.gen_range(4..=10);
            (render(&qr, m, 4, d, l), json!({"module": m, "dark": d, "light": l}))
        }
        "blur" => {
            let m = rng.gen_range(5..=10);
            let s = f64::from(m) * rng.gen_range(0.25..0.7);
            (blur(&render(&qr, m, 4, 0, 255), s), json!({"module": m, "sigma": s}))
        }
        "noise" => {
            let m = rng.gen_range(4..=10);
            let s = rng.gen_range(15.0..50.0);
            let mut img = render(&qr, m, 4, 0, 255);
            add_noise(&mut img, s, rng);
            (img, json!({"module": m, "sigma": s}))
        }
        "jpeg" => {
            let base = render(&qr, 8, 4, 0, 255);
            let mt: f64 = rng.gen_range(2.0..5.0);
            let q = rng.gen_range(5..=30);
            let w = (f64::from(base.width()) * mt / 8.0).round() as u32;
            (jpeg(&scale_to_width(&base, w), q), json!({"module": mt, "jpeg": q}))
        }
        "rotation" | "perspective" => {
            let side = rng.gen_range(250.0..600.0);
            let scene = (side * rng.gen_range(1.5..2.0)) as u32;
            let (angle, persp) = if cat == "rotation" {
                (rng.gen_range(5.0..85.0), 0.0)
            } else {
                (rng.gen_range(-15.0..15.0), rng.gen_range(0.08..0.25))
            };
            let bg = texture(rng, scene, scene, 170, 240);
            let img = scene_with_code(rng, &qr, scene, scene, side, angle, persp, bg);
            (img, json!({"side": side, "angle": angle, "persp": persp}))
        }
        "uneven-light" => {
            let m = rng.gen_range(5..=10);
            let mut img = render(&qr, m, 4, 0, 255);
            let (st, gl) = (rng.gen_range(0.5..0.85), rng.gen_range(0.0..150.0));
            lighting(rng, &mut img, st, gl);
            (img, json!({"module": m, "strength": st, "glare": gl}))
        }
        "moire" => screen_photo(rng, &qr),
        "share-card-logo" => {
            let m = rng.gen_range(5..=10);
            let mut img = render(&qr, m, 4, 0, 255);
            let frac = rng.gen_range(0.22..0.30);
            logo_plate(&mut img, frac);
            let mt: f64 = rng.gen_range(3.0..6.0);
            let w = (f64::from(img.width()) * mt / f64::from(m)).round() as u32;
            let q = rng.gen_range(30..=80);
            (jpeg(&scale_to_width(&img, w), q), json!({"module": mt, "plate": frac, "jpeg": q}))
        }
        "clutter-large" => {
            let big = rng.gen_bool(1.0 / 3.0);
            let (w, h) = photo_dims(rng, big);
            let side = f64::from(w.min(h)) * rng.gen_range(0.10..0.35);
            let (angle, persp) = (rng.gen_range(0.0..20.0) * sign(rng), rng.gen_range(0.0..0.12));
            let bg = texture(rng, w, h, 30, 230);
            let img = scene_with_code(rng, &qr, w, h, side, angle, persp, bg);
            let sigma = rng.gen_range(0.5..1.2);
            let mut img = blur(&img, sigma);
            let noise = rng.gen_range(3.0..8.0);
            add_noise(&mut img, noise, rng);
            let q = rng.gen_range(75..=92);
            (jpeg(&img, q), json!({"w": w, "h": h, "side": side, "angle": angle, "persp": persp, "blur": sigma, "noise": noise, "jpeg": q}))
        }
        "phone-photo" => {
            let (w, h) = photo_dims(rng, false);
            let side = f64::from(w.min(h)) * rng.gen_range(0.25..0.6);
            let (angle, persp) = (rng.gen_range(0.0..30.0) * sign(rng), rng.gen_range(0.04..0.18));
            let bg = texture(rng, w, h, 40, 220);
            let img = scene_with_code(rng, &qr, w, h, side, angle, persp, bg);
            let sigma = rng.gen_range(0.6..2.0);
            let mut img = blur(&img, sigma);
            let (st, gl) = (rng.gen_range(0.3..0.7), rng.gen_range(0.0..100.0));
            lighting(rng, &mut img, st, gl);
            let noise = rng.gen_range(5.0..15.0);
            add_noise(&mut img, noise, rng);
            let q = rng.gen_range(60..=90);
            (jpeg(&img, q), json!({"w": w, "h": h, "side": side, "angle": angle, "persp": persp, "blur": sigma, "light": st, "glare": gl, "noise": noise, "jpeg": q}))
        }
        "camera-frame" => {
            let (w, h) = if rng.gen_bool(0.5) { (1280, 720) } else { (1920, 1080) };
            let side = f64::from(h) * rng.gen_range(0.2..0.55);
            let (angle, persp) = (rng.gen_range(0.0..25.0) * sign(rng), rng.gen_range(0.0..0.15));
            let bg = texture(rng, w, h, 40, 220);
            let img = scene_with_code(rng, &qr, w, h, side, angle, persp, bg);
            let (len, dir) = (rng.gen_range(0.0..10.0), rng.gen_range(0.0..180.0));
            let mut img = motion_blur(&img, len, dir);
            let st = rng.gen_range(0.2..0.6);
            let light_g = rng.gen_range(0.0..60.0);
            lighting(rng, &mut img, st, light_g);
            let noise = rng.gen_range(4.0..12.0);
            add_noise(&mut img, noise, rng);
            (img, json!({"w": w, "h": h, "side": side, "angle": angle, "persp": persp, "motion": len, "light": st, "noise": noise}))
        }
        other => unreachable!("unknown category {other}"),
    };
    Generated { img, kind, ec, expected: text, params }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn write_gray(path: &Path, img: &GrayImage) {
    let file = fs::File::create(path).expect("create");
    let mut enc = GzEncoder::new(BufWriter::new(file), Compression::fast());
    enc.write_all(img.as_raw()).expect("write");
    enc.finish().expect("finish");
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = arg(&args, "--out").map_or_else(|| here.join("../out/corpus"), PathBuf::from);
    let per: usize = arg(&args, "--n").map_or(30, |v| v.parse().expect("--n"));
    let seed: u64 = arg(&args, "--seed").map_or(1, |v| v.parse().expect("--seed"));
    let only = arg(&args, "--only");
    let preview_dir = out.join("../preview");
    fs::create_dir_all(&out).expect("out dir");
    fs::create_dir_all(&preview_dir).expect("preview dir");

    // The receive cards every platform's suite already decodes (web, iOS and
    // Android share these two files); the expected text is their tests'.
    let fixture_dir = here.join("../../../app-web/vela-wallet/src/lib/services/__fixtures__");
    let fixtures = [
        ("receive-code-off.png", "0x88cCA0EeDbF2C4426110bbFc998F048689266894"),
        ("receive-code-on.png", "ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@137"),
    ]
    .map(|(name, expected)| Fixture {
        img: image::open(fixture_dir.join(name)).expect("fixture").to_luma8(),
        expected,
        name,
    });

    let mut samples = Vec::new();
    let mut emit = |id: String, category: &'static str, g: Generated| {
        let (w, h) = g.img.dimensions();
        let file = format!("{id}.gray.gz");
        write_gray(&out.join(&file), &g.img);
        let mut ladder = Vec::new();
        let mut cam = None;
        for target in LADDER.into_iter().filter(|&t| t < w) {
            let v = scale_to_width(&g.img, target);
            let vf = format!("{id}@{target}.gray.gz");
            write_gray(&out.join(&vf), &v);
            let variant = Variant { w: v.width(), h: v.height(), file: vf };
            if target == CAMERA_WIDTH {
                cam = Some(variant.clone());
            }
            ladder.push(variant);
        }
        let camera = cam.unwrap_or_else(|| {
            let v = if w == CAMERA_WIDTH { g.img.clone() } else { scale_to_width(&g.img, CAMERA_WIDTH) };
            let vf = format!("{id}@cam.gray.gz");
            write_gray(&out.join(&vf), &v);
            Variant { w: v.width(), h: v.height(), file: vf }
        });
        samples.push(Sample {
            id,
            category,
            payload_kind: g.kind,
            ec: g.ec,
            expected: g.expected,
            width: w,
            height: h,
            file,
            ladder,
            camera,
            params: g.params,
        });
        (w, h)
    };

    // The fixtures themselves, untouched.
    if only.as_deref().map_or(true, |o| o == "real-fixture") {
        for fx in &fixtures {
            let g = Generated { img: fx.img.clone(), kind: "fixture", ec: "M", expected: fx.expected.to_owned(), params: json!({"fixture": fx.name}) };
            fx.img.save(preview_dir.join(format!("real-fixture-{}", fx.name))).expect("preview");
            emit(format!("real-fixture-{}", fx.name.trim_end_matches(".png")), "real-fixture", g);
        }
    }

    for (ci, &cat) in CATEGORIES.iter().enumerate() {
        if only.as_deref().is_some_and(|o| o != cat) {
            continue;
        }
        let mut rng = StdRng::seed_from_u64(seed.wrapping_mul(1_000_003).wrapping_add(ci as u64));
        let started = std::time::Instant::now();
        for i in 0..per {
            let g = generate(cat, i, &mut rng, &fixtures);
            if i < 3 {
                let p = &g.img;
                let pv = if p.width() > 900 { scale_to_width(p, 900) } else { p.clone() };
                pv.save(preview_dir.join(format!("{cat}-{i:03}.png"))).expect("preview");
            }
            emit(format!("{cat}-{i:03}"), cat, g);
        }
        eprintln!("{cat:>20}: {per} samples in {:.1}s", started.elapsed().as_secs_f64());
    }

    let manifest = Manifest { seed, per_category: per, resize_filter: "image::imageops::FilterType::Triangle", samples };
    fs::write(out.join("manifest.json"), serde_json::to_vec_pretty(&manifest).expect("json")).expect("manifest");
    eprintln!("wrote {} samples to {}", manifest.samples.len(), out.display());
}
