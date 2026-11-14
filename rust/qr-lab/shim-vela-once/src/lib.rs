//! vela-qr's single pass: one size, dark-on-light only — the algorithm alone,
//! as the other shims are measured.
fn decode(luma: &[u8], w: u32, h: u32) -> Option<String> {
    vela_qr::read_once(luma, w as usize, h as usize).and_then(|c| c.text())
}

lab_abi::export_decoder!(decode);
