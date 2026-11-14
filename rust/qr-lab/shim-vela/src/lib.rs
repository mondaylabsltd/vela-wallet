//! vela-qr's public `decode`: its own strategy (inversion, size pyramid) on
//! top of whatever the harness does.
fn decode(luma: &[u8], w: u32, h: u32) -> Option<String> {
    vela_qr::read(luma, w as usize, h as usize).and_then(|c| c.text())
}

lab_abi::export_decoder!(decode);
