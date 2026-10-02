//! Developer switches: the `VELA_*` environment variables that pin a page, a
//! gallery state, a language, a theme, a fixture file or an endpoint, so a
//! screenshot pass, a headless run or a debugging session can reach a state it
//! cannot click to (spec 095).
//!
//! Owner rule (2026-10-02): a release or store build exposes no developer
//! feature. Every such read therefore goes through these macros, which a
//! DEVELOPER build expands into an environment read and any other build into
//! `None` — at compile time, so a release binary does not carry even the
//! variable's name. `scripts/check-store-binary.sh` asserts exactly that on
//! the built binary, for every name read through here.
//!
//! A developer build is a debug build (`cargo run`, `cargo build`), a test
//! build, or one with the `dev-fixtures` feature (`cargo dev`, the parallel
//! space). `cargo build --release` — every package and the Mac App Store
//! build — is none of them. Test builds are named on their own because the
//! storage tests' `VELA_STATE_DIR` is what keeps `cargo test --release` off
//! the developer's real wallet.
//!
//! Switches with a compile-time gate of their own (`VELA_PARALLEL_SPACE`,
//! `VELA_DEV_PROXY`: `dev-fixtures` only) and reads inside `#[cfg(test)]`
//! code keep reading the environment directly.

/// `std::env::var(name).ok()` in a developer build; `None` otherwise.
macro_rules! var {
    ($name:literal) => {{
        #[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
        let value: Option<String> = std::env::var($name).ok();
        #[cfg(not(any(test, debug_assertions, feature = "dev-fixtures")))]
        let value: Option<String> = None;
        value
    }};
}

/// `std::env::var_os(name)` in a developer build; `None` otherwise.
macro_rules! var_os {
    ($name:literal) => {{
        #[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
        let value: Option<std::ffi::OsString> = std::env::var_os($name);
        #[cfg(not(any(test, debug_assertions, feature = "dev-fixtures")))]
        let value: Option<std::ffi::OsString> = None;
        value
    }};
}

/// The switch is set to exactly `1` (the family's on-value).
macro_rules! flag {
    ($name:literal) => {
        $crate::dev_env::var!($name).as_deref() == Some("1")
    };
}

pub(crate) use {flag, var, var_os};

#[cfg(test)]
mod tests {
    // Test builds are developer builds: the switches are read. (A release
    // build's side is proven on the binary — the names are not in it — by
    // scripts/check-store-binary.sh.)
    #[test]
    fn a_developer_build_reads_the_switches() {
        // SAFETY: the variable is unique to this test; nothing else reads it.
        unsafe { std::env::set_var("VELA_DEV_ENV_PROBE", "1") };
        assert_eq!(var!("VELA_DEV_ENV_PROBE").as_deref(), Some("1"));
        assert_eq!(var_os!("VELA_DEV_ENV_PROBE"), Some("1".into()));
        assert!(flag!("VELA_DEV_ENV_PROBE"));
        unsafe { std::env::set_var("VELA_DEV_ENV_PROBE", "yes") };
        assert!(!flag!("VELA_DEV_ENV_PROBE"), "only `1` turns a flag on");
        unsafe { std::env::remove_var("VELA_DEV_ENV_PROBE") };
        assert_eq!(var!("VELA_DEV_ENV_PROBE"), None);
        assert!(!flag!("VELA_DEV_ENV_PROBE"));
    }
}
