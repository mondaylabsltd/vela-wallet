//! Reading a QR code out of a picture.
//!
//! The desktop's scanner has always been a drawing: DS1 renders a viewfinder
//! and `componentsUi.scanner.fromGallery` has been on its toolbar since spec
//! 021 with nothing behind it. This is the half that needs no camera — a
//! screenshot, a saved share card, a photo somebody sent — and it is the half
//! that works on all three desktops, which the camera half will not.
//!
//! Pure Rust on purpose (`rqrr`, no zbar and no C toolchain): the file path
//! must not be the reason a platform stops building.
//!
//! What this module does NOT do is decide anything. It answers with the text
//! that was in the picture; what that text MEANS is `eip681`'s to tokenize and
//! the send machine's to rule on.

/// Every QR payload in one image, in the order `rqrr` found them.
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
    let mut prepared = rqrr::PreparedImage::prepare(image.to_luma8());
    prepared
        .detect_grids()
        .into_iter()
        .filter_map(|grid| grid.decode().ok().map(|(_, content)| content))
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
    let mut prepared = rqrr::PreparedImage::prepare(luma);
    prepared
        .detect_grids()
        .into_iter()
        .find_map(|grid| grid.decode().ok().map(|(_, content)| content))
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
        let code = qrcode::QrCode::new(PAYLOAD.as_bytes())
            .unwrap_or_else(|error| unreachable!("the payload encodes: {error}"));
        // Painted module by module, the way `share_card` does it — the crate's
        // `render` needs an image feature this build does not carry, and the
        // modules are the only part that matters. Eight pixels each, with a
        // four-module quiet zone: a QR with no margin is one most decoders
        // cannot find, which is a property of the format rather than of rqrr.
        let modules = code.width();
        let colors = code.to_colors();
        const SCALE: usize = 8;
        const QUIET: usize = 4;
        let side = u32::try_from((modules + QUIET * 2) * SCALE)
            .unwrap_or_else(|error| unreachable!("a test-sized code: {error}"));
        let mut image = image::GrayImage::from_pixel(side, side, image::Luma([255]));
        for y in 0..modules {
            for x in 0..modules {
                if colors[y * modules + x] != qrcode::Color::Dark {
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
        let code =
            qrcode::QrCode::new(PAYLOAD.as_bytes()).unwrap_or_else(|error| unreachable!("{error}"));
        let modules = code.width();
        let colors = code.to_colors();
        const SCALE: usize = 10;
        const QUIET: usize = 4;
        let side = u32::try_from((modules + QUIET * 2) * SCALE).unwrap_or(0);
        let mut image = image::GrayImage::from_pixel(side, side, image::Luma([255]));
        for y in 0..modules {
            for x in 0..modules {
                if colors[y * modules + x] != qrcode::Color::Dark {
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
