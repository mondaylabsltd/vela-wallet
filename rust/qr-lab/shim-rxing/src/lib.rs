//! rxing (a Rust port of ZXing) as `detect_in_luma`: QR only, TRY_HARDER,
//! HybridBinarizer — its zxing-cpp detector first, the Java one as fallback.
fn decode(luma: &[u8], w: u32, h: u32) -> Option<String> {
    let mut hints = rxing::DecodeHints::default().with(rxing::DecodeHintValue::TryHarder(true));
    rxing::helpers::detect_in_luma_slice_with_hints(luma, w, h, Some(rxing::BarcodeFormat::QR_CODE), &mut hints)
        .ok()
        .map(|r| r.getText().to_owned())
}

lab_abi::export_decoder!(decode);
