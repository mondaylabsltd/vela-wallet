//! Why a page did not load: one native HEAD, classified by the core
//! (spec 079 US3, spec 082 RD4).
//!
//! The load's rules — when to probe, what a probe's answer means, when to
//! give up, when to try again — are the core's `browser_load::LoadWatch`
//! (spec 082 moved them there with their tests). What stays here is the one
//! thing the core cannot do: ask the network, over the wallet's own routes
//! (`executor::proxy`), and name the failure with the probe's codes.
//!
//! A proxy that could not be used — not reached, timed out, or a PAC that
//! could not run — is `probe_code::PROXY`, the panel's "your proxy isn't
//! responding". A proxy that answered and refused the tunnel spoke for the
//! host: that keeps the host's own code (refused / connect), as RD9 rules.

use std::time::Duration;

use vela_core::app::browser_load::probe_code;

use crate::executor::proxy::ProxyFailure;

/// What one probe said, and the route it took — for the log line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeAnswer {
    /// `Ok` — the site answered (any status); `Err` — a `probe_code`.
    pub verdict: Result<(), i64>,
    /// The proxy that failed, or `None`.
    pub proxy: Option<ProxyFailure>,
}

/// The probe's code for a transport error (`browser_load::probe_code`).
#[must_use]
pub fn probe_code_of(error: &ureq::Error) -> i64 {
    match error {
        ureq::Error::HostNotFound => probe_code::DNS,
        ureq::Error::Timeout(_) => probe_code::TIMEOUT,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => probe_code::TLS,
        ureq::Error::ConnectionFailed => probe_code::CONNECT,
        // The proxy answered and would not open the tunnel: the site's
        // failure, as the proxy reported it (RX) — never "the proxy".
        ureq::Error::ConnectProxyFailed(_) => probe_code::REFUSED,
        ureq::Error::Io(io) => io_code(io),
        _ => 0,
    }
}

/// The code for a failure, the proxy's own taking precedence.
#[must_use]
pub fn code_of(error: &ureq::Error, proxy: Option<&ProxyFailure>) -> i64 {
    match proxy {
        Some(failure) if failure.kind.is_the_proxys() => probe_code::PROXY,
        _ => probe_code_of(error),
    }
}

fn io_code(io: &std::io::Error) -> i64 {
    use std::io::ErrorKind;
    // Spec 082 RJ10 (G45): ureq hands a failed handshake over as an I/O
    // error of kind `InvalidData` wrapping rustls's own — an expired or
    // untrusted certificate read as `other`, drew no certificate sentence and
    // was retried automatically (FR-003). The certificate itself is TLS;
    // any other rustls error keeps the class its kind gives it.
    if let Some(tls) = io
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        && matches!(
            tls,
            rustls::Error::InvalidCertificate(_) | rustls::Error::NoCertificatesPresented
        )
    {
        return probe_code::TLS;
    }
    match io.kind() {
        ErrorKind::ConnectionRefused => probe_code::REFUSED,
        ErrorKind::TimedOut => probe_code::TIMEOUT,
        ErrorKind::NetworkUnreachable
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkDown
        | ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::NotConnected
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof => probe_code::CONNECT,
        // `std` surfaces a resolver failure as an uncategorized error
        // carrying the libc sentence (`executor::proxy`'s note).
        _ => {
            let text = io.to_string();
            if text.contains("failed to lookup address information")
                || text.contains("nodename nor servname")
                || text.contains("Name or service not known")
                || text.contains("No such host is known")
                || text.contains("No address associated with hostname")
            {
                probe_code::DNS
            } else {
                0
            }
        }
    }
}

/// Ask whether `url` answers at all: one HEAD over the app's own routes out,
/// within `budget`. **Blocks** — run it off the frame. Any HTTP status is an
/// answer: the site is up, whatever it thinks of a HEAD.
pub fn probe(url: &str, budget: Duration) -> ProbeAnswer {
    let (tx, rx) = std::sync::mpsc::channel();
    let target = url.to_owned();
    let spawned = std::thread::Builder::new()
        .name("vela-load-probe".to_owned())
        .spawn(move || {
            let answer = crate::executor::proxy::with_routes(&target, budget, |agent| {
                agent.head(&target).call()
            });
            let _ = tx.send(match answer {
                Ok(_) => ProbeAnswer {
                    verdict: Ok(()),
                    proxy: None,
                },
                Err(failure) => match failure.error {
                    ureq::Error::StatusCode(_) => ProbeAnswer {
                        verdict: Ok(()),
                        proxy: None,
                    },
                    ref error => ProbeAnswer {
                        verdict: Err(code_of(error, failure.proxy.as_ref())),
                        proxy: failure.proxy.clone(),
                    },
                },
            });
        });
    if spawned.is_err() {
        return ProbeAnswer {
            verdict: Err(0),
            proxy: None,
        };
    }
    // The whole walk of routes gets the one budget, not one each.
    rx.recv_timeout(budget).unwrap_or(ProbeAnswer {
        verdict: Err(probe_code::TIMEOUT),
        proxy: None,
    })
}

/// A probe verdict as a log word.
#[must_use]
pub fn verdict_word(verdict: Result<(), i64>) -> &'static str {
    match verdict {
        Ok(()) => "ok",
        Err(probe_code::DNS) => "dns",
        Err(probe_code::REFUSED) => "refused",
        Err(probe_code::TIMEOUT) => "timeout",
        Err(probe_code::TLS) => "tls",
        Err(probe_code::CONNECT) => "connect",
        Err(probe_code::PROXY) => "proxy",
        Err(_) => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_errors_map_to_the_probe_codes() {
        use std::io::{Error, ErrorKind};
        assert_eq!(probe_code_of(&ureq::Error::HostNotFound), probe_code::DNS);
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::from(ErrorKind::ConnectionRefused))),
            probe_code::REFUSED
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::from(ErrorKind::NetworkUnreachable))),
            probe_code::CONNECT
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::other(
                "failed to lookup address information: nodename nor servname provided"
            ))),
            probe_code::DNS
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Tls("bad cert")),
            probe_code::TLS
        );
        assert_eq!(
            probe_code_of(&ureq::Error::ConnectionFailed),
            probe_code::CONNECT
        );
        assert_eq!(probe_code_of(&ureq::Error::RedirectFailed), 0);
    }

    /// Spec 082 RJ10 (G45, L5): an expired certificate reaches the probe as an
    /// I/O error wrapping rustls's own — and is the certificate class, which
    /// the core words as such and never retries by itself.
    #[test]
    fn an_expired_certificate_is_tls() {
        use std::io::{Error, ErrorKind};
        let expired = ureq::Error::Io(Error::new(
            ErrorKind::InvalidData,
            rustls::Error::InvalidCertificate(rustls::CertificateError::Expired),
        ));
        assert_eq!(probe_code_of(&expired), probe_code::TLS);
        assert_eq!(verdict_word(Err(probe_code_of(&expired))), "tls");
        let none = ureq::Error::Io(Error::new(
            ErrorKind::InvalidData,
            rustls::Error::NoCertificatesPresented,
        ));
        assert_eq!(probe_code_of(&none), probe_code::TLS);
        // Another rustls error keeps what its kind says.
        let other = ureq::Error::Io(Error::new(
            ErrorKind::InvalidData,
            rustls::Error::DecryptError,
        ));
        assert_eq!(probe_code_of(&other), 0);
        let failure = vela_core::app::browser_load::classify(
            vela_core::app::browser_load::LoadPlatform::Probe,
            probe_code_of(&expired),
            None,
            false,
        )
        .unwrap_or_else(|| unreachable!("a failure"));
        assert_eq!(
            failure.class,
            vela_core::app::browser_load::LoadFailureClass::Certificate
        );
        assert!(
            !failure.auto_retry,
            "a certificate is never retried by itself"
        );
        assert_eq!(failure.reason_key, "explore.loadCertificate");
    }

    /// Spec 082 RD9: a proxy that could not be used is the proxy's failure;
    /// one that answered and refused the tunnel spoke for the site.
    #[test]
    fn only_a_proxy_that_cannot_be_used_is_the_proxy_class() {
        use crate::executor::proxy::ProxyFailureKind;
        for kind in [
            ProxyFailureKind::Unreachable,
            ProxyFailureKind::Timeout,
            ProxyFailureKind::PacFailed,
        ] {
            let failure = ProxyFailure {
                proxy: "127.0.0.1:9".to_owned(),
                kind,
            };
            assert_eq!(
                code_of(&ureq::Error::ConnectionFailed, Some(&failure)),
                probe_code::PROXY,
                "{kind:?}"
            );
        }
        let refused = ProxyFailure {
            proxy: "127.0.0.1:8899".to_owned(),
            kind: ProxyFailureKind::RefusedTunnel,
        };
        assert_eq!(
            code_of(
                &ureq::Error::ConnectProxyFailed("proxy server responded 502/Bad Gateway".into()),
                Some(&refused)
            ),
            probe_code::REFUSED
        );
        // And the core reads code 6 as the proxy class, with its words.
        let failure = vela_core::app::browser_load::classify(
            vela_core::app::browser_load::LoadPlatform::Probe,
            probe_code::PROXY,
            None,
            false,
        )
        .unwrap_or_else(|| unreachable!("a failure"));
        assert_eq!(
            failure.class,
            vela_core::app::browser_load::LoadFailureClass::Proxy
        );
        assert_eq!(failure.reason_key, "explore.loadProxy");
    }

    /// A real probe against this machine: a closed port is refused.
    #[test]
    fn the_probe_hears_a_refused_port() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        // The listener is dropped: nothing listens there now.
        let refused = probe(
            &format!("http://127.0.0.1:{closed}/"),
            Duration::from_millis(u64::from(vela_core::app::browser_load::PROBE_BUDGET_MS)),
        );
        assert_eq!(refused.verdict, Err(probe_code::REFUSED));
        assert_eq!(verdict_word(refused.verdict), "refused");
    }
}
