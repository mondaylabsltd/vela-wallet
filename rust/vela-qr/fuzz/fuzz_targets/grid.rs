//! Any bytes as a module grid of a valid QR size (version from the first byte).
#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((&v, rest)) = data.split_first() else {
        return;
    };
    let dim = 17 + 4 * (1 + usize::from(v) % 40);
    let mut modules = vec![0u8; dim * dim];
    for (m, b) in modules.iter_mut().zip(rest.iter().cycle()) {
        *m = *b;
    }
    // Bits from the bytes, so short inputs still vary every module.
    for (i, m) in modules.iter_mut().enumerate() {
        *m = (rest.get(i / 8).copied().unwrap_or(*m) >> (i % 8)) & 1;
    }
    if let Some(code) = vela_qr::decode_grid(&modules, dim) {
        let _ = code.text();
        for s in &code.segments {
            let _ = s.candidates();
        }
    }
});
