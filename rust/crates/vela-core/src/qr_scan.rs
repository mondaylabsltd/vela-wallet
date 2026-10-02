//! Reading a QR code out of a still image: a picked photo or a dropped file
//! (spec 090).
//!
//! ZXing's `HybridBinarizer` (Android) and jsQR binarise with a LOCAL window a
//! few dozen pixels wide. A screenshot of Vela's own receive code on a
//! 1080×2400 phone draws each module about 24 px across. A window that sits
//! inside one module sees no edge, so Vela could not read its own code from the
//! album. The same picture made smaller decodes: shrinking puts the module
//! edges back inside the window, and the averaging is the low-pass filter a
//! photo needs anyway. (Measured on the device screenshots: the desktop's rqrr
//! and iOS's CoreImage read them as they are, and the web reads them through
//! its own zbar photo ladder, so only Android climbs this one today.)
//!
//! The rule, written once: try the image as it is. Then shrink it so its
//! LONGEST side is each rung of [`STILL_QR_LADDER`] in turn, using only rungs
//! smaller than the image (never enlarge), and stop at the first hit. Camera
//! frames are not touched: they arrive at the size the scanner asked for.

/// The longest-side targets after the image's own size, largest first.
pub const STILL_QR_LADDER: [u32; 3] = [1024, 640, 400];

/// The sizes to decode a `width × height` still image at, in order, as
/// `(width, height)`: the image as it is, then the image scaled (aspect kept)
/// so its longest side is each rung of [`STILL_QR_LADDER`] smaller than it.
/// An empty image has nothing to try.
#[must_use]
pub fn still_qr_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let longest = width.max(height);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let scaled = |side: u32| {
        let at = |edge: u32| {
            let exact = u64::from(edge) * u64::from(side);
            // Round to nearest; the longest edge lands exactly on `side`.
            let rounded = (exact + u64::from(longest) / 2) / u64::from(longest);
            u32::try_from(rounded).unwrap_or(side).max(1)
        };
        (at(width), at(height))
    };
    std::iter::once((width, height))
        .chain(
            STILL_QR_LADDER
                .into_iter()
                .filter(|side| *side < longest)
                .map(scaled),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full phone screenshot gets every rung, the longest side landing on it.
    #[test]
    fn a_phone_screenshot_climbs_the_whole_ladder() {
        assert_eq!(
            still_qr_sizes(1080, 2400),
            vec![(1080, 2400), (461, 1024), (288, 640), (180, 400)]
        );
    }

    /// Never enlarge: a 945-wide crop skips 1024.
    #[test]
    fn rungs_at_or_above_the_image_are_skipped() {
        assert_eq!(
            still_qr_sizes(945, 948),
            vec![(945, 948), (638, 640), (399, 400)]
        );
        assert_eq!(
            still_qr_sizes(1024, 512),
            vec![(1024, 512), (640, 320), (400, 200)]
        );
        assert_eq!(still_qr_sizes(400, 300), vec![(400, 300)]);
    }

    /// Landscape images scale by their width.
    #[test]
    fn the_longest_side_is_the_one_that_lands() {
        assert_eq!(still_qr_sizes(2400, 1080)[1], (1024, 461));
    }

    /// Nothing to try in an empty image, and a sliver never rounds to zero.
    #[test]
    fn degenerate_images() {
        assert!(still_qr_sizes(0, 100).is_empty());
        assert!(still_qr_sizes(100, 0).is_empty());
        assert_eq!(still_qr_sizes(1, 5000)[1], (1, 1024));
    }
}
