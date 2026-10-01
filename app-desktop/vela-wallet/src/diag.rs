//! The desktop's log line (spec 082 RD12, FR-018/019).
//!
//! stderr is the only log this app has, and the paths a bad network breaks —
//! a page that never commits, a proxy that never answers, a submit whose reply
//! was lost — were silent there. Every line goes through [`vlog!`], so they
//! all read the same way and can be lined up against the fault proxy's own
//! log:
//!
//! ```text
//! [vela-wallet] 15:22:50.114 relay: submit verdict=maybe_sent hash=0xc6f3544f1d attempts=2
//! ```
//!
//! ## What never reaches a line
//!
//! Key material, passkey secrets, signatures, calldata, request params or
//! results, wallet addresses, and full URLs. An RPC URL carries a provider's
//! API key in its path (`/v3/<key>`, `pool.rs`'s endpoint table), and the
//! Trusted Signer's URL carries the whole request in its fragment — so a URL
//! is logged as [`host_of`] and nothing more, and a hash as [`short`].

/// One line on stderr: `[vela-wallet] HH:MM:SS.mmm area: …`, the time in the
/// machine's own zone (`localtime_r` through [`crate::executor::local_civil`]).
///
/// `area` names the part of the app that speaks (`browser`, `proxy`, `rpc`,
/// `chain notice`, `fee`, `tracker`, `relay`, `dapp`, `trusted signer`,
/// `window` — the catalogue in the 082 contract §15).
macro_rules! vlog {
    ($area:expr, $($arg:tt)*) => {
        $crate::diag::emit($area, ::std::format_args!($($arg)*))
    };
}
pub(crate) use vlog;

/// What [`vlog!`] expands to. One `eprintln!` per line, so two threads never
/// interleave inside one.
pub fn emit(area: &str, message: std::fmt::Arguments<'_>) {
    let now = crate::executor::now_ms();
    let civil = crate::executor::local_civil(now);
    eprintln!(
        "{}",
        line(
            civil.hour,
            civil.minute,
            civil.second,
            millis_of(now),
            area,
            message
        )
    );
}

/// The millisecond part of an epoch stamp.
fn millis_of(epoch_ms: f64) -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rem_euclid(1000) is 0..1000"
    )]
    let ms = epoch_ms.rem_euclid(1000.0) as u32;
    ms
}

/// The line itself, apart from the clock so a test can pin its shape.
fn line(
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
    area: &str,
    message: std::fmt::Arguments<'_>,
) -> String {
    format!("[vela-wallet] {hour:02}:{minute:02}:{second:02}.{millis:03} {area}: {message}")
}

/// The host of `url`, and the port when it names one — never the scheme, the
/// userinfo, the path, the query or the fragment. The core's rule
/// (`browser_load::host_of`), so a log line and the failure panel name a host
/// the same way.
#[must_use]
pub fn host_of(url: &str) -> String {
    vela_core::app::browser_load::host_of(url)
}

/// A hash as a log line carries it: `0x` and the first ten hex digits — enough
/// to find it in an explorer or a relay log, too little to be mistaken for a
/// whole one pasted somewhere.
#[must_use]
pub fn short(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_keeps_the_host_and_port_and_nothing_else() {
        assert_eq!(
            host_of("https://app.uniswap.org/#/swap?x=1"),
            "app.uniswap.org"
        );
        assert_eq!(
            host_of("http://127.0.0.1:8137/path?q#frag"),
            "127.0.0.1:8137"
        );
        assert_eq!(
            host_of("https://user:secret@rpc.example.com:8443/x"),
            "rpc.example.com:8443"
        );
        assert_eq!(host_of("example.com/a/b"), "example.com");
        assert_eq!(host_of("https://node.example?apikey=abc"), "node.example");
        assert_eq!(host_of("https://node.example#token=abc"), "node.example");
    }

    #[test]
    fn a_provider_key_in_the_path_never_survives() {
        let url = "https://mainnet.infura.io/v3/0123456789abcdef0123456789abcdef";
        let host = host_of(url);
        assert_eq!(host, "mainnet.infura.io");
        assert!(!host.contains("0123456789abcdef"));
        assert!(!host.contains("/v3/"));
        // The Trusted Signer page's fragment carries the request and its token.
        let signer = "https://sign.getvela.app/b/abc/sign#req=eyJ0b2tlbiI6InNlY3JldCJ9";
        assert_eq!(host_of(signer), "sign.getvela.app");
    }

    #[test]
    fn a_hash_is_cut_to_twelve_characters() {
        let hash = "0xc6f3544f1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f80";
        assert_eq!(short(hash), "0xc6f3544f1d");
        assert_eq!(short("0xabc"), "0xabc");
        assert_eq!(short(""), "");
    }

    #[test]
    fn the_line_reads_prefix_time_area_message() {
        let text = line(
            9,
            5,
            7,
            42,
            "browser",
            format_args!("asked host={} gen={}", "example.com", 1),
        );
        assert_eq!(
            text,
            "[vela-wallet] 09:05:07.042 browser: asked host=example.com gen=1"
        );
        assert_eq!(millis_of(1_700_000_000_123.0), 123);
        assert_eq!(millis_of(999.9), 999);
    }
}
