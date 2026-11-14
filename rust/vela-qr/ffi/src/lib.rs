//! vela-qr for native hosts, behind a plain C ABI. Android loads it over JNA
//! (`app/.../feature/scan/QrDecoder.kt`), which is already in the app for the
//! core — so no JNI glue, and nothing of the core's touched.
//!
//! ```text
//! vela_qr_read_luma(pixels, width, height) -> n
//!     8-bit luminance, row-major; n == 0: no code. The result (n bytes, in
//!     `vela_qr::wire`'s format) is kept for this thread until the next read.
//! vela_qr_result(out, capacity) -> copied
//!     copies this thread's last result into `out`.
//! ```
//!
//! A panic never crosses the boundary: it is caught and reads as no code.

use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// # Safety
/// `pixels` must point at `width * height` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn vela_qr_read_luma(pixels: *const u8, width: u32, height: u32) -> u32 {
    let len = (width as usize).saturating_mul(height as usize);
    let bytes = if pixels.is_null() || len == 0 {
        Vec::new()
    } else {
        let luma = std::slice::from_raw_parts(pixels, len);
        catch_unwind(AssertUnwindSafe(|| {
            vela_qr::read(luma, width as usize, height as usize)
                .map(|code| vela_qr::wire::encode(&code))
                .unwrap_or_default()
        }))
        .unwrap_or_default()
    };
    RESULT.with(|r| {
        let n = u32::try_from(bytes.len()).unwrap_or(0);
        *r.borrow_mut() = bytes;
        n
    })
}

/// # Safety
/// `out` must point at `capacity` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn vela_qr_result(out: *mut u8, capacity: u32) -> u32 {
    RESULT.with(|r| {
        let r = r.borrow();
        let n = r.len().min(capacity as usize);
        if !out.is_null() && n > 0 {
            std::ptr::copy_nonoverlapping(r.as_ptr(), out, n);
        }
        n as u32
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_in_a_blank_image_and_no_crash_on_null() {
        let blank = vec![255u8; 100 * 100];
        assert_eq!(unsafe { vela_qr_read_luma(blank.as_ptr(), 100, 100) }, 0);
        assert_eq!(unsafe { vela_qr_read_luma(std::ptr::null(), 100, 100) }, 0);
        assert_eq!(unsafe { vela_qr_result(std::ptr::null_mut(), 10) }, 0);
    }
}
