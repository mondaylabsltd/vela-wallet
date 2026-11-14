//! vela-qr for the web, behind a plain C ABI (no wasm-bindgen glue to ship or
//! keep in step with the core's).
//!
//! ```text
//! qr_alloc(len) -> ptr          a buffer the host writes pixels into
//! qr_free(ptr, len)
//! qr_read_rgba(ptr, w, h) -> n  RGBA pixels (a canvas's ImageData)
//! qr_read_luma(ptr, w, h) -> n  8-bit luminance
//! qr_result_ptr() -> ptr        n bytes of result; n == 0: no code
//! ```
//!
//! The result is the first code found, in `vela_qr::wire`'s format.

use std::cell::RefCell;
use vela_qr::Code;

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn qr_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr`/`len` must come from one `qr_alloc` call.
#[no_mangle]
pub unsafe extern "C" fn qr_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

#[no_mangle]
pub extern "C" fn qr_result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}

/// # Safety
/// `ptr` must point at `w * h * 4` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn qr_read_rgba(ptr: *const u8, w: u32, h: u32) -> u32 {
    let n = (w as usize) * (h as usize);
    let rgba = std::slice::from_raw_parts(ptr, n * 4);
    // Integer luminance, the weights the web scanner has always used.
    let luma: Vec<u8> = rgba
        .chunks_exact(4)
        .map(|p| ((u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8) as u8)
        .collect();
    respond(vela_qr::read(&luma, w as usize, h as usize))
}

/// # Safety
/// `ptr` must point at `w * h` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn qr_read_luma(ptr: *const u8, w: u32, h: u32) -> u32 {
    let luma = std::slice::from_raw_parts(ptr, (w as usize) * (h as usize));
    respond(vela_qr::read(luma, w as usize, h as usize))
}

fn respond(code: Option<Code>) -> u32 {
    RESULT.with(|r| {
        let mut out = r.borrow_mut();
        *out = code.map(|c| vela_qr::wire::encode(&c)).unwrap_or_default();
        out.len() as u32
    })
}
