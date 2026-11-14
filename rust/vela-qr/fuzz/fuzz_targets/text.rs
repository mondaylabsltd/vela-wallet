//! Any bytes through charset guessing and every in-crate text decoder.
#![no_main]
use vela_qr::{Charset, Segment};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let seg = Segment { charset: Charset::Latin1, guessed: true, bytes: data.to_vec() };
    let candidates = seg.candidates();
    // Never left unreadable: non-ASCII bytes always end in ISO-8859-1.
    if !data.is_ascii() {
        assert_eq!(candidates.last(), Some(&Charset::Latin1));
    }
    let _ = seg.text();
    for c in [Charset::Ascii, Charset::Latin1, Charset::Utf8, Charset::Utf16Be, Charset::Utf16Le, Charset::Utf32Be, Charset::Utf32Le, Charset::Cp437] {
        let _ = c.decode(data);
    }
});
