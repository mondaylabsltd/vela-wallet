//! The HTTP client gpui loads remote images through (issue 201).
//!
//! `img("https://…")` asks `cx.http_client()` for the bytes, and gpui's
//! default is a `NullHttpClient` that answers nothing — which is why the
//! desktop drew lettermarks where the web and the phones draw token and chain
//! logos, and why nobody could tell a network apart by looking.
//!
//! The transport is the app's own `proxy::agent`, not a second one: a machine
//! that needs a proxy to reach the RPC needs it to reach the logo index too,
//! and the proxy candidate list is where that knowledge already lives. `ureq`
//! is blocking, so each request runs on its own short-lived thread and the
//! future resolves when it lands — logos are a handful of fetches per screen,
//! cached by gpui's own image cache afterwards, so a pool would be more
//! machinery than the traffic justifies.
use std::sync::Arc;
use std::time::Duration;

use gpui::http_client::{AsyncBody, HttpClient, Response, Result, Url, anyhow, http};

use super::proxy;

/// How long a logo may take before the glyph beneath it simply stays.
const LOGO_TIMEOUT: Duration = Duration::from_secs(8);

pub struct AppHttpClient;

impl AppHttpClient {
    pub fn shared() -> Arc<dyn HttpClient> {
        Arc::new(Self)
    }
}

impl HttpClient for AppHttpClient {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        // The agent resolves its own proxy per request (the candidate list can
        // change while the app runs); reporting one here would freeze the
        // first answer for the life of the process.
        None
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> futures::future::BoxFuture<'static, Result<Response<AsyncBody>>> {
        let uri = req.uri().to_string();
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let result = fetch(&uri);
            // The receiver is gone when the element that asked has been
            // dropped — an ordinary outcome, not an error.
            let _ = tx.send(result);
        });
        Box::pin(async move { rx.await.unwrap_or_else(|_| Err(anyhow!("cancelled"))) })
    }
}

fn fetch(uri: &str) -> Result<Response<AsyncBody>> {
    let mut response = proxy::agent(LOGO_TIMEOUT).get(uri).call()?;
    let status = response.status();
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut body)?;
    Ok(http::Response::builder()
        .status(status)
        .body(AsyncBody::from(body))?)
}
