//! Reading a QR code out of a picture.
//!
//! The desktop's scanner has always been a drawing: DS1 renders a viewfinder
//! and `componentsUi.scanner.fromGallery` has been on its toolbar since spec
//! 021 with nothing behind it. This is the half that needs no camera — a
//! screenshot, a saved share card, a photo somebody sent — and it is the half
//! that works on all three desktops, which the camera half will not.
//!
//! The decoder is `vela-qr` (rust/vela-qr), the one the web scanner ships:
//! pure Rust, no C toolchain — the file path must not be the reason a
//! platform stops building. It replaced `rqrr` (quirc's port), which read 77.6%
//! of the decoder lab's 572 images to vela-qr's 97.2% (rust/qr-lab).
//!
//! Text in a charset the crate leaves to the platform (Shift_JIS, GB18030,
//! Big5, …, named by their WHATWG labels) is decoded here with `encoding_rs`,
//! already in this build's graph.
//!
//! What this module does NOT do is decide anything. It answers with the text
//! that was in the picture; what that text MEANS is `eip681`'s to tokenize and
//! the send machine's to rule on.

/// Every QR payload in one image, in detection order.
///
/// A photo can hold more than one code — a poster with two, a screenshot of a
/// page with a code beside an avatar — and the caller decides which it wants
/// rather than this file guessing.
#[must_use]
pub fn decode_all(bytes: &[u8]) -> Vec<String> {
    let Ok(image) = image::load_from_memory(bytes) else {
        // Not an image this build can read. An empty answer, never a panic:
        // the person chose a file, and a wallet that dies on a wrong pick is
        // worse than one that says it found nothing.
        return Vec::new();
    };
    let luma = image.to_luma8();
    let (w, h) = (luma.width() as usize, luma.height() as usize);
    vela_qr::read_all(luma.as_raw(), w, h)
        .into_iter()
        .filter_map(message)
        .collect()
}

/// A QR in a frame the camera just handed over.
///
/// Separate from [`decode_all`] because a camera frame is already decoded
/// pixels — going back through PNG bytes to reach the same decoder would cost
/// an encode per frame for nothing.
#[must_use]
pub fn decode_luma(rgb: &image::RgbImage) -> Option<String> {
    let luma = image::DynamicImage::ImageRgb8(rgb.clone()).to_luma8();
    let (w, h) = (luma.width() as usize, luma.height() as usize);
    vela_qr::read(luma.as_raw(), w, h).and_then(message)
}

/// A code's whole text. One part of a structured-append sequence is not the
/// message, and data that is not text has nothing to act on: both are `None`.
fn message(code: vela_qr::Code) -> Option<String> {
    if code.part.is_some() {
        return None;
    }
    code.segments.iter().map(segment_text).collect()
}

/// The first of the segment's candidate charsets that decodes it strictly.
fn segment_text(segment: &vela_qr::Segment) -> Option<String> {
    segment
        .candidates()
        .into_iter()
        .find_map(|charset| match charset {
            vela_qr::Charset::Table(label) => {
                let encoding = encoding_rs::Encoding::for_label(label.as_bytes())?;
                let (text, had_errors) = encoding.decode_without_bom_handling(&segment.bytes);
                (!had_errors).then(|| text.into_owned())
            }
            other => other.decode(&segment.bytes),
        })
}

/// The first payload, if the picture holds one.
#[must_use]
pub fn decode_first(bytes: &[u8]) -> Option<String> {
    decode_all(bytes).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A code this app WROTE reads back — the round trip that matters, since
    /// the picture a person is most likely to hand the scanner is the share
    /// card another Vela made.
    #[test]
    fn a_code_this_app_wrote_reads_back() {
        const PAYLOAD: &str = "ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@100";
        let code = vela_core::qr::qr_matrix(PAYLOAD, vela_core::qr::QrLevel::Medium)
            .unwrap_or_else(|| unreachable!("the payload encodes"));
        // Painted module by module, the way `share_card` does it — the crate's
        // `render` needs an image feature this build does not carry, and the
        // modules are the only part that matters. Eight pixels each, with a
        // four-module quiet zone: a QR with no margin is one most decoders
        // cannot find, which is a property of the format rather than of the reader.
        let modules = code.width as usize;
        let colors = code.modules;
        const SCALE: usize = 8;
        const QUIET: usize = 4;
        let side = u32::try_from((modules + QUIET * 2) * SCALE)
            .unwrap_or_else(|error| unreachable!("a test-sized code: {error}"));
        let mut image = image::GrayImage::from_pixel(side, side, image::Luma([255]));
        for y in 0..modules {
            for x in 0..modules {
                if !colors[y * modules + x] {
                    continue;
                }
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        let px = u32::try_from((x + QUIET) * SCALE + dx).unwrap_or(0);
                        let py = u32::try_from((y + QUIET) * SCALE + dy).unwrap_or(0);
                        image.put_pixel(px, py, image::Luma([0]));
                    }
                }
            }
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap_or_else(|error| unreachable!("the image encodes: {error}"));

        assert_eq!(decode_first(&png.into_inner()).as_deref(), Some(PAYLOAD));
    }

    /// The receive code's value, from the real core: the bare address, or with
    /// "include network" on, the URI for Polygon.
    fn core_code(include: bool) -> String {
        use vela_core::app::payment_request::{Event, PaymentRequest};
        const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let mut host = crate::core_host::CoreHost::<PaymentRequest>::new();
        let _ = host.dispatch(Event::Start {
            account: ME.to_owned(),
            recipient: ME.to_owned(),
            base_url: "https://getvela.app".to_owned(),
        });
        let _ = host.dispatch(Event::AssetPicked {
            chain_id: 137,
            token_address: None,
            symbol: "POL".to_owned(),
            decimals: 18,
            network_name: "Polygon".to_owned(),
        });
        let _ = host.dispatch(Event::IncludeNetworkChanged { include });
        host.view().qr_value
    }

    /// Spec 090: Vela's own receive code, cut from a 1080×2400 Android
    /// screenshot (modules ~24 px across), read from the file as it is.
    ///
    /// No ladder here, and none needed: the same picture defeats ZXing and
    /// jsQR at full size (their local binarizer windows sit inside one
    /// module), and vela-qr halves the image itself until the modules fit.
    #[test]
    fn vela_reads_its_own_receive_code_from_a_screenshot() {
        for (file, include) in [
            ("receive-code-on.png", true),
            ("receive-code-off.png", false),
        ] {
            let bytes = std::fs::read(format!(
                "{}/tests/fixtures/qr/{file}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap_or_else(|error| unreachable!("the fixture {file} is checked in: {error}"));
            assert_eq!(decode_first(&bytes), Some(core_code(include)), "{file}");
        }
    }

    /// Text in a charset the platform decodes: GBK bytes, as an older
    /// generator writes them with no ECI, come back as Chinese.
    #[test]
    fn a_gbk_segment_is_decoded_here() {
        let segment = vela_qr::Segment {
            charset: vela_qr::Charset::Table("gb18030"),
            guessed: true,
            bytes: vec![0xC4, 0xE3, 0xBA, 0xC3],
        };
        assert_eq!(segment_text(&segment).as_deref(), Some("你好"));
    }

    /// A file that is not a picture, and a picture with no code in it, both
    /// answer "nothing found" rather than failing.
    #[test]
    fn a_file_with_no_code_finds_nothing() {
        assert!(decode_first(b"this is not a png").is_none());

        let blank = image::DynamicImage::ImageLuma8(image::GrayImage::new(64, 64));
        let mut png = std::io::Cursor::new(Vec::new());
        blank
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap_or_else(|error| unreachable!("the image encodes: {error}"));
        assert!(decode_first(&png.into_inner()).is_none());
    }
}

#[cfg(test)]
mod emit {
    /// Write a scannable PNG for a human to feed the app.
    ///
    /// `cargo test executor::qr::emit -- --ignored --nocapture`
    #[test]
    #[ignore = "writes a file for the reviewer"]
    fn write_a_request_code() {
        const PAYLOAD: &str =
            "ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@100?value=10000000000000000";
        let code = vela_core::qr::qr_matrix(PAYLOAD, vela_core::qr::QrLevel::Medium)
            .unwrap_or_else(|| unreachable!("the payload encodes"));
        let modules = code.width as usize;
        let colors = code.modules;
        const SCALE: usize = 10;
        const QUIET: usize = 4;
        let side = u32::try_from((modules + QUIET * 2) * SCALE).unwrap_or(0);
        let mut image = image::GrayImage::from_pixel(side, side, image::Luma([255]));
        for y in 0..modules {
            for x in 0..modules {
                if !colors[y * modules + x] {
                    continue;
                }
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        image.put_pixel(
                            u32::try_from((x + QUIET) * SCALE + dx).unwrap_or(0),
                            u32::try_from((y + QUIET) * SCALE + dy).unwrap_or(0),
                            image::Luma([0]),
                        );
                    }
                }
            }
        }
        let path = std::env::var("VELA_QR_OUT").unwrap_or_else(|_| "request-qr.png".to_owned());
        image
            .save(&path)
            .unwrap_or_else(|error| unreachable!("{error}"));
        println!("wrote {path}: {PAYLOAD}");
    }
}
