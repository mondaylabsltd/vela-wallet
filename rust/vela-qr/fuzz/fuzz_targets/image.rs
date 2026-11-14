//! Any bytes as a luminance image: the first byte picks the width.
#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let w = 1 + usize::from(data[0]) % 160;
    let px = &data[1..];
    let h = px.len() / w;
    if h == 0 {
        return;
    }
    let _ = vela_qr::read_all(&px[..w * h], w, h);
});
