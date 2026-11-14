//! rxing's QR reader, its Java ZXing port alone — HybridBinarizer, TRY_HARDER.
use rxing::Reader;

fn decode(luma: &[u8], w: u32, h: u32) -> Option<String> {
    let hints = rxing::DecodeHints::default().with(rxing::DecodeHintValue::TryHarder(true));
    let source = rxing::Luma8Source::new_with_slice(luma, w, h).ok()?;
    let mut image = rxing::BinaryBitmap::new(rxing::common::HybridBinarizer::new(source));
    rxing::qrcode::QRCodeReader
        .decode_with_hints(&mut image, &hints)
        .ok()
        .map(|r| r.getText().to_owned())
}

lab_abi::export_decoder!(decode);
