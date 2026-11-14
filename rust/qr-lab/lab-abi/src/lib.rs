//! Plain C ABI, no wasm-bindgen, identical across every shim in this lab:
//!
//!   qr_alloc(len) -> ptr        a buffer the host writes luma pixels into
//!   qr_free(ptr, len)
//!   qr_decode(ptr, w, h) -> n   n >= 0: bytes of UTF-8 text at qr_result_ptr()
//!                               -1: no code found
//!   qr_result_ptr() -> ptr
//!
//! Inversion and the size ladder are the HOST's job, so every decoder is
//! measured under exactly the same preprocessing.

#[doc(hidden)]
pub use std::cell::RefCell as __RefCell;

/// `export_decoder!(path::to::decode)` — `decode(&[u8], u32, u32) -> Option<String>`
/// behind the lab's four exports.
#[macro_export]
macro_rules! export_decoder {
    ($decode:path) => {
        thread_local! {
            static RESULT: $crate::__RefCell<Vec<u8>> = const { $crate::__RefCell::new(Vec::new()) };
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
        /// `ptr` must point at `w * h` initialised bytes.
        #[no_mangle]
        pub unsafe extern "C" fn qr_decode(ptr: *const u8, w: u32, h: u32) -> i32 {
            let luma = std::slice::from_raw_parts(ptr, (w as usize) * (h as usize));
            match $decode(luma, w, h) {
                Some(text) => RESULT.with(|r| {
                    let mut r = r.borrow_mut();
                    r.clear();
                    r.extend_from_slice(text.as_bytes());
                    i32::try_from(r.len()).unwrap_or(-1)
                }),
                None => -1,
            }
        }
    };
}
