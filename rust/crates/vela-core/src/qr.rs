//! The one QR encoder every platform draws with.
//!
//! A receive code, a share card, an invite link, a `FIDO:/` pairing payload:
//! each is "a string somebody points a camera at", and each must come out the
//! same module-for-module on the web, iOS, Android and the desktop. A wallet
//! whose QR is drawn by four encoders is a wallet where one platform's code
//! scans and another's does not, and nobody can say which version is right.
//!
//! So the matrix is decided here, and the shells own only pixels. The web
//! reaches this through `vela-core-wasm` (`qrMatrix`), the phones through
//! `vela-core-uniffi` (`qr_matrix`, `share_card_qr_matrix`), the desktop
//! directly.

/// How much of the code may be lost and still read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrLevel {
    /// About 15% — every on-screen code.
    Medium,
    /// About 30% — the saved share card, whose centre carries the network's
    /// logo on a plate and which every chat app recompresses. A plain address
    /// is 37 modules across at this level instead of 29.
    High,
}

/// A QR code as a square module matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrMatrix {
    /// Modules per side.
    pub width: u32,
    /// `width * width` modules, row-major, `true` = dark.
    pub modules: Vec<bool>,
}

/// `text` as a QR code at `level`, in the smallest version that holds it.
///
/// `None` only when the text does not fit a QR code at all (or is empty). It
/// is a real answer: a caller must render it as "no code", never as a
/// placeholder that looks scannable — that is the failure this exists to end.
#[must_use]
pub fn qr_matrix(text: &str, level: QrLevel) -> Option<QrMatrix> {
    if text.is_empty() {
        return None;
    }
    let ec = match level {
        QrLevel::Medium => qrcode::EcLevel::M,
        QrLevel::High => qrcode::EcLevel::H,
    };
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), ec).ok()?;
    let width = u32::try_from(code.width()).ok()?;
    Some(QrMatrix {
        width,
        modules: code
            .to_colors()
            .into_iter()
            .map(|color| color == qrcode::Color::Dark)
            .collect(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const ADDRESS: &str = "0xD400866e00B055B20752a826CD5C89b811de130b";

    #[test]
    fn an_address_is_29_modules_at_m_and_37_at_h() {
        // The receive card's geometry was drawn against 29; the share card's
        // logo plate against 37.
        assert_eq!(
            qr_matrix(ADDRESS, QrLevel::Medium)
                .expect("an address fits")
                .width,
            29
        );
        assert_eq!(
            qr_matrix(ADDRESS, QrLevel::High)
                .expect("an address fits")
                .width,
            37
        );
    }

    #[test]
    fn the_matrix_is_square_and_carries_finder_patterns() {
        let code = qr_matrix(ADDRESS, QrLevel::Medium).expect("an address fits");
        let w = code.width as usize;
        assert_eq!(code.modules.len(), w * w);
        // Each finder's outer ring is dark along its top edge.
        for (row, col) in [(0, 0), (0, w - 7), (w - 7, 0)] {
            assert!((0..7).all(|c| code.modules[row * w + col + c]));
        }
    }

    #[test]
    fn empty_and_oversized_text_have_no_code() {
        assert_eq!(qr_matrix("", QrLevel::Medium), None);
        assert_eq!(qr_matrix(&"x".repeat(3000), QrLevel::Medium), None);
    }
}
