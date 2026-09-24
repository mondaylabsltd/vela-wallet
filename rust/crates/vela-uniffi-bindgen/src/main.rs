//! The uniffi bindings generator, as its own build-time-only binary.
//!
//! Was `rust/crates/vela-core-uniffi/uniffi-bindgen.rs` (and a twin in
//! `vela-dev-fixtures-uniffi`). Living in the shipped crates meant those crates
//! enabled `uniffi/cli`, which put the whole generator inside
//! `libvela_core_uniffi.a` — the static library the iOS app links, and the
//! Android `.so` — where only the linker's dead-stripping kept it out of what
//! users install. Spec 081 FR-014 moved it here; the callers only swapped
//! `-p vela-core-uniffi` for `-p vela-uniffi-bindgen`.
//!
//!   cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- \
//!     generate --library <built library> --language swift --out-dir <dir>
//!
//! `--library` mode reads the interface metadata back out of the compiled
//! library, so this binary serves `vela-core-uniffi` and
//! `vela-dev-fixtures-uniffi` alike without depending on either.
fn main() {
    uniffi::uniffi_bindgen_main()
}
