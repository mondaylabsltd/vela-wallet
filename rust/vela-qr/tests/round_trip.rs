//! Codes drawn by the encoder vela-core uses, read back.

use qrcode::{EcLevel, QrCode};

fn render(text: &str, ec: EcLevel, module: usize, quiet: usize) -> (Vec<u8>, usize) {
    let code = QrCode::with_error_correction_level(text.as_bytes(), ec).expect("fits");
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

const ADDRESS: &str = "0xD400866e00B055B20752a826CD5C89b811de130b";

fn payloads() -> Vec<String> {
    vec![
        ADDRESS.to_owned(),
        "01234567890123456789".to_owned(),
        "HELLO WORLD $%*+-./:".to_owned(),
        format!("ethereum:{ADDRESS}@137/transfer?address={ADDRESS}&uint256=1000000"),
        format!(
            "wc:{}@2?relay-protocol=irn&symKey={}&expiryTimestamp=1790000000",
            "a1".repeat(32),
            "b2".repeat(32)
        ),
        format!("FIDO:/{}", "0123456789".repeat(20)),
        "ünïcödé — 日本語 ✓".to_owned(),
        "x".repeat(700), // version 15+ at M: many alignment patterns and version info
    ]
}

#[test]
fn every_payload_at_every_level() {
    for text in payloads() {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            if QrCode::with_error_correction_level(text.as_bytes(), ec).is_err() {
                continue;
            }
            let (px, side) = render(&text, ec, 4, 4);
            assert_eq!(
                vela_qr::read(&px, side, side)
                    .and_then(|c| c.text())
                    .as_deref(),
                Some(text.as_str()),
                "{ec:?} {}",
                &text[..text.len().min(40)]
            );
        }
    }
}

#[test]
fn module_sizes_from_two_to_twenty_pixels() {
    for module in [2, 3, 5, 8, 13, 20] {
        let (px, side) = render(ADDRESS, EcLevel::M, module, 4);
        assert_eq!(
            vela_qr::read(&px, side, side)
                .and_then(|c| c.text())
                .as_deref(),
            Some(ADDRESS),
            "module {module}"
        );
    }
}

#[test]
fn a_mirrored_code_reads() {
    let (px, side) = render(ADDRESS, EcLevel::M, 4, 4);
    let mut t = vec![0u8; px.len()];
    for y in 0..side {
        for x in 0..side {
            t[x * side + y] = px[y * side + x];
        }
    }
    assert_eq!(
        vela_qr::read(&t, side, side)
            .and_then(|c| c.text())
            .as_deref(),
        Some(ADDRESS)
    );
}

#[test]
fn two_codes_in_one_image() {
    let (a, sa) = render(ADDRESS, EcLevel::M, 4, 4);
    let (b, sb) = render("second", EcLevel::M, 4, 4);
    let (w, h) = (sa + sb, sa.max(sb));
    let mut px = vec![255u8; w * h];
    for y in 0..sa {
        px[y * w..y * w + sa].copy_from_slice(&a[y * sa..(y + 1) * sa]);
    }
    for y in 0..sb {
        px[y * w + sa..y * w + sa + sb].copy_from_slice(&b[y * sb..(y + 1) * sb]);
    }
    let mut all: Vec<String> = vela_qr::read_all(&px, w, h)
        .iter()
        .filter_map(vela_qr::Code::text)
        .collect();
    all.sort();
    assert_eq!(all, vec![ADDRESS.to_owned(), "second".to_owned()]);
}

#[test]
fn nothing_is_read_from_noise_or_blank() {
    let (w, h) = (300, 200);
    let mut s = 0x1234_5678u32;
    let noise: Vec<u8> = (0..w * h)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s >> 24) as u8
        })
        .collect();
    assert_eq!(vela_qr::read(&noise, w, h), None);
    assert_eq!(vela_qr::read(&vec![255; w * h], w, h), None);
    assert_eq!(vela_qr::read(&[0; 10], 5, 5), None);
    assert_eq!(vela_qr::read(&[0; 10], 4, 4), None, "size mismatch");
}

#[test]
fn light_on_dark_reads() {
    let (px, side) = render(ADDRESS, EcLevel::M, 4, 4);
    let inv: Vec<u8> = px.iter().map(|&v| 255 - v).collect();
    assert_eq!(
        vela_qr::read(&inv, side, side)
            .and_then(|c| c.text())
            .as_deref(),
        Some(ADDRESS)
    );
}

#[test]
fn modules_wider_than_the_binarizer_window_read() {
    // A screenshot of the receive card: ~25 px modules, mid-grey "black".
    let (px, side) = render(ADDRESS, EcLevel::M, 26, 4);
    let grey: Vec<u8> = px.iter().map(|&v| if v == 0 { 40 } else { 235 }).collect();
    assert_eq!(
        vela_qr::read(&grey, side, side)
            .and_then(|c| c.text())
            .as_deref(),
        Some(ADDRESS)
    );
}

#[test]
fn modules_under_two_pixels_read() {
    // Drawn at 8 px/module, then averaged down to ~1.6 px/module.
    let (px, side) = render(ADDRESS, EcLevel::M, 8, 4);
    let small_side = side / 5;
    let mut small = vec![0u8; small_side * small_side];
    for y in 0..small_side {
        for x in 0..small_side {
            let mut sum = 0u32;
            for dy in 0..5 {
                for dx in 0..5 {
                    sum += u32::from(px[(y * 5 + dy) * side + x * 5 + dx]);
                }
            }
            small[y * small_side + x] = (sum / 25) as u8;
        }
    }
    assert_eq!(
        vela_qr::read(&small, small_side, small_side)
            .and_then(|c| c.text())
            .as_deref(),
        Some(ADDRESS)
    );
}

// ---------------------------------------------------------------------------
// Character sets, end to end through an image
// ---------------------------------------------------------------------------

fn render_bits(bits: qrcode::bits::Bits, ec: EcLevel) -> (Vec<u8>, usize) {
    let code = QrCode::with_bits(bits, ec).expect("fits");
    let n = code.width();
    let colors = code.to_colors();
    let (module, quiet) = (4, 4);
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

fn read_segments(push: impl FnOnce(&mut qrcode::bits::Bits)) -> vela_qr::Code {
    let mut bits = qrcode::bits::Bits::new(qrcode::Version::Normal(5));
    push(&mut bits);
    bits.push_terminator(EcLevel::M).expect("terminator");
    let (px, side) = render_bits(bits, EcLevel::M);
    vela_qr::read(&px, side, side).expect("a code")
}

#[test]
fn gbk_bytes_without_an_eci_are_offered_as_gb18030() {
    // "扫码支付" in GBK, as older generators write it.
    let gbk = [0xC9, 0xA8, 0xC2, 0xEB, 0xD6, 0xA7, 0xB8, 0xB6];
    let code = read_segments(|b| b.push_byte_data(&gbk).expect("bytes"));
    let seg = &code.segments[0];
    assert!(seg.guessed);
    assert_eq!(seg.charset, vela_qr::Charset::Table("gb18030"));
    assert_eq!(seg.bytes, gbk);
    assert_eq!(
        *seg.candidates().last().expect("fallback"),
        vela_qr::Charset::Latin1
    );
}

#[test]
fn an_eci_declared_shift_jis_segment() {
    let sjis = [0x93, 0xFA, 0x96, 0x7B]; // 日本
    let code = read_segments(|b| {
        b.push_eci_designator(20).expect("eci");
        b.push_byte_data(&sjis).expect("bytes");
    });
    assert_eq!(
        code.segments[0].charset,
        vela_qr::Charset::Table("shift_jis")
    );
    assert!(!code.segments[0].guessed);
    assert_eq!(code.segments[0].bytes, sjis);
}

#[test]
fn kanji_mode_end_to_end() {
    let sjis = [0x93, 0x5F, 0xE4, 0xAA]; // 点茗, the standard's example
    let code = read_segments(|b| b.push_kanji_data(&sjis).expect("kanji"));
    assert_eq!(
        code.segments[0].charset,
        vela_qr::Charset::Table("shift_jis")
    );
    assert_eq!(code.segments[0].bytes, sjis);
}

#[test]
fn utf8_chinese_needs_no_platform() {
    let text = "扫码支付 ✓";
    let code = read_segments(|b| b.push_byte_data(text.as_bytes()).expect("bytes"));
    assert_eq!(code.text().as_deref(), Some(text));
}
