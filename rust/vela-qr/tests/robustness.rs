//! Untrusted input: whatever an image holds, decoding returns — it never
//! panics (a panic in the browser is a wasm trap). Real codes are damaged in
//! every way at random; the iteration count scales with VELA_QR_FUZZ.

use qrcode::{EcLevel, QrCode};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn iterations(base: usize) -> usize {
    base * std::env::var("VELA_QR_FUZZ")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

fn render(text: &str, module: usize, quiet: usize) -> (Vec<u8>, usize) {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).expect("fits");
    let n = code.width();
    let colors = code.to_colors();
    let side = (n + 2 * quiet) * module;
    let mut px = vec![255u8; side * side];
    for y in 0..side {
        for x in 0..side {
            let (mx, my) = (
                (x / module) as isize - quiet as isize,
                (y / module) as isize - quiet as isize,
            );
            if mx >= 0
                && my >= 0
                && (mx as usize) < n
                && (my as usize) < n
                && colors[my as usize * n + mx as usize] == qrcode::Color::Dark
            {
                px[y * side + x] = 0;
            }
        }
    }
    (px, side)
}

#[test]
fn damaged_codes_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let texts = [
        "0xD400866e00B055B20752a826CD5C89b811de130b",
        &"y".repeat(300),
        &"z".repeat(1200),
    ];
    for i in 0..iterations(300) {
        let (mut px, side) = render(texts[i % texts.len()], 1 + rng.below(6), rng.below(5));
        match rng.below(5) {
            // Flip pixels.
            0 => {
                for _ in 0..rng.below(px.len() / 4 + 1) {
                    let k = rng.below(px.len());
                    px[k] ^= 0xFF;
                }
            }
            // Wipe rectangles.
            1 => {
                for _ in 0..1 + rng.below(6) {
                    let (x0, y0) = (rng.below(side), rng.below(side));
                    let (w, h) = (rng.below(side - x0) + 1, rng.below(side - y0) + 1);
                    let v = if rng.next() & 1 == 0 { 0 } else { 255 };
                    for y in y0..y0 + h {
                        px[y * side + x0..y * side + x0 + w].fill(v);
                    }
                }
            }
            // Shift rows (shear).
            2 => {
                for y in 0..side {
                    let s = rng.below(1 + side / 20);
                    px[y * side..(y + 1) * side].rotate_left(s);
                }
            }
            // Random grey noise.
            3 => {
                for v in px.iter_mut() {
                    *v = v.wrapping_add((rng.next() & 0x3F) as u8);
                }
            }
            // Crop.
            _ => {
                let c = rng.below(side / 2);
                let w = side - c;
                let cropped: Vec<u8> = (0..w)
                    .flat_map(|y| px[y * side..y * side + w].to_vec())
                    .collect();
                let _ = vela_qr::read_all(&cropped, w, w);
                continue;
            }
        }
        let _ = vela_qr::read_all(&px, side, side);
    }
}

#[test]
fn arbitrary_images_never_panic() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for _ in 0..iterations(300) {
        let (w, h) = (1 + rng.below(400), 1 + rng.below(400));
        let mut px = vec![0u8; w * h];
        match rng.below(3) {
            0 => px.iter_mut().for_each(|v| *v = rng.next() as u8),
            // Blocky: finder-like structure everywhere.
            1 => {
                let cell = 1 + rng.below(8);
                let bits: Vec<u8> = (0..(w / cell + 1) * (h / cell + 1))
                    .map(|_| if rng.next() & 1 == 0 { 0 } else { 255 })
                    .collect();
                for y in 0..h {
                    for x in 0..w {
                        px[y * w + x] = bits[(y / cell) * (w / cell + 1) + x / cell];
                    }
                }
            }
            // Concentric squares (fake finder patterns) at random.
            _ => {
                px.fill(255);
                for _ in 0..1 + rng.below(12) {
                    let (cx, cy, m) = (rng.below(w), rng.below(h), 1 + rng.below(6));
                    for y in cy.saturating_sub(4 * m)..(cy + 4 * m).min(h) {
                        for x in cx.saturating_sub(4 * m)..(cx + 4 * m).min(w) {
                            let r = (x as isize - cx as isize)
                                .unsigned_abs()
                                .max((y as isize - cy as isize).unsigned_abs())
                                / m;
                            px[y * w + x] = if r == 0 || r == 1 || r == 3 { 0 } else { 255 };
                        }
                    }
                }
            }
        }
        let _ = vela_qr::read_all(&px, w, h);
    }
}
