//! WebView2's own word on a load (Windows, spec 083 W3–W5).
//!
//! wry 0.56 turns WebView2's `ContentLoading` into "started" and
//! `NavigationCompleted` into "finished" and reads neither's arguments. So
//! Edge's own error page ("当前无法使用此页面", "嗯… 无法访问此页面") arrived as
//! a page committing, took Vela's failure panel down and stopped its retries;
//! a certificate error showed Edge's interstitial with its "continue anyway";
//! a renderer that died left Edge's sad page with the chrome still green.
//! This module listens beside wry, on the same view, and says which is which.

use std::cell::RefCell;
use std::rc::Rc;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL, COREWEBVIEW2_WEB_ERROR_STATUS,
    ICoreWebView2, ICoreWebView2_14,
};
use webview2_com::{
    ContentLoadingEventHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    ProcessFailedEventHandler, ServerCertificateErrorDetectedEventHandler, take_pwstr,
};
use windows_core::{BOOL, Interface as _, PWSTR};

/// What WebView2 said about the top document.
pub enum EngineLoad {
    /// The SITE's document committed.
    Committed(String),
    /// The engine put its own error page where the document was: the old one
    /// is gone, and nothing of the site arrived — not a commit.
    ErrorPage(String),
    Finished(String),
    /// The navigation failed; `status` is a `COREWEBVIEW2_WEB_ERROR_STATUS`,
    /// for the core's `browser_load::classify`.
    Failed {
        url: String,
        status: i64,
        certificate: bool,
    },
    RendererGone,
    BrowserGone,
}

/// The top navigation in flight, and whether the site committed for it.
#[derive(Default)]
struct Navigations {
    pending: Option<(u64, String)>,
    committed: Option<u64>,
}

impl Navigations {
    /// The address a navigation was going to — the redirect's last hop — else
    /// the view's own.
    fn address(&self, id: u64, source: String) -> String {
        match &self.pending {
            Some((pending, uri)) if *pending == id => uri.clone(),
            _ => source,
        }
    }

    fn completed(&self, id: u64, success: bool, status: i64, source: String) -> EngineLoad {
        // The site's own document came (its 404 page, a `window.stop()`):
        // what the site shows is not ours to call a failure.
        if success || self.committed == Some(id) {
            return EngineLoad::Finished(source);
        }
        EngineLoad::Failed {
            url: self.address(id, source),
            status,
            certificate: false,
        }
    }

    /// A certificate error is the PAGE's only for the origin the top
    /// navigation is going to; a subresource's bad certificate is not.
    fn certificate(&self, uri: &str) -> Option<String> {
        self.pending
            .as_ref()
            .filter(|(_, pending)| origin(pending) == origin(uri))
            .map(|(_, pending)| pending.clone())
    }
}

fn origin(url: &str) -> &str {
    let start = url.find("://").map_or(0, |i| i + 3);
    url[start..]
        .find(['/', '?', '#'])
        .map_or(url, |end| &url[..start + end])
}

fn source(webview: &ICoreWebView2) -> String {
    let mut uri = PWSTR::null();
    // SAFETY: an out-param of the view WebView2 handed this event.
    match unsafe { webview.Source(&mut uri) } {
        Ok(()) => take_pwstr(uri),
        Err(_) => String::new(),
    }
}

/// Listen to the view wry just built. Every handler runs on its UI thread.
pub fn subscribe(
    webview: &ICoreWebView2,
    sink: impl Fn(EngineLoad) + 'static,
) -> windows_core::Result<()> {
    let sink: Rc<dyn Fn(EngineLoad)> = Rc::new(sink);
    let navigations = Rc::new(RefCell::new(Navigations::default()));
    let mut token = 0_i64;

    let n = navigations.clone();
    let starting = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let (mut id, mut uri) = (0_u64, PWSTR::null());
        // SAFETY: out-params of this event's args, during the event.
        unsafe {
            args.NavigationId(&mut id)?;
            args.Uri(&mut uri)?;
        }
        // Raised again for each redirect: the last hop is the address.
        n.borrow_mut().pending = Some((id, take_pwstr(uri)));
        Ok(())
    }));
    // SAFETY: COM calls on the view wry just built, on the thread that built it.
    unsafe { webview.add_NavigationStarting(&starting, &mut token)? };

    let (n, s) = (navigations.clone(), sink.clone());
    let content = ContentLoadingEventHandler::create(Box::new(move |webview, args| {
        let (Some(webview), Some(args)) = (webview, args) else {
            return Ok(());
        };
        let (mut id, mut error) = (0_u64, BOOL::default());
        // SAFETY: as above.
        unsafe {
            args.NavigationId(&mut id)?;
            args.IsErrorPage(&mut error)?;
        }
        let load = if error.as_bool() {
            EngineLoad::ErrorPage(n.borrow().address(id, source(&webview)))
        } else {
            n.borrow_mut().committed = Some(id);
            EngineLoad::Committed(source(&webview))
        };
        s(load);
        Ok(())
    }));
    // SAFETY: as above.
    unsafe { webview.add_ContentLoading(&content, &mut token)? };

    let (n, s) = (navigations.clone(), sink.clone());
    let completed = NavigationCompletedEventHandler::create(Box::new(move |webview, args| {
        let (Some(webview), Some(args)) = (webview, args) else {
            return Ok(());
        };
        let (mut id, mut ok, mut status) = (
            0_u64,
            BOOL::default(),
            COREWEBVIEW2_WEB_ERROR_STATUS::default(),
        );
        // SAFETY: as above.
        unsafe {
            args.NavigationId(&mut id)?;
            args.IsSuccess(&mut ok)?;
            args.WebErrorStatus(&mut status)?;
        }
        let load = n
            .borrow()
            .completed(id, ok.as_bool(), i64::from(status.0), source(&webview));
        s(load);
        Ok(())
    }));
    // SAFETY: as above.
    unsafe { webview.add_NavigationCompleted(&completed, &mut token)? };

    let s = sink.clone();
    let failed = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
        // SAFETY: as above.
        unsafe { args.ProcessFailedKind(&mut kind)? };
        // Only the two that take the page with them: the engine restarts its
        // GPU and utility processes itself, and a frame's renderer is not the
        // page.
        if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED {
            s(EngineLoad::RendererGone);
        } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
            s(EngineLoad::BrowserGone);
        }
        Ok(())
    }));
    // SAFETY: as above.
    unsafe { webview.add_ProcessFailed(&failed, &mut token)? };

    // Runtime 1.0.1245 and later. On an older one the certificate error still
    // arrives as `Failed` (status 1–5) and Vela's panel covers the engine's.
    if let Ok(webview14) = webview.cast::<ICoreWebView2_14>() {
        let (n, s) = (navigations, sink);
        let certificate =
            ServerCertificateErrorDetectedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let (mut status, mut uri) =
                    (COREWEBVIEW2_WEB_ERROR_STATUS::default(), PWSTR::null());
                // SAFETY: as above. Cancel FIRST: never "continue anyway", for
                // the page or anything on it (083 W4).
                unsafe {
                    args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL)?;
                    args.ErrorStatus(&mut status)?;
                    args.RequestUri(&mut uri)?;
                }
                let page = n.borrow().certificate(&take_pwstr(uri));
                if let Some(url) = page {
                    s(EngineLoad::Failed {
                        url,
                        status: i64::from(status.0),
                        certificate: true,
                    });
                }
                Ok(())
            }));
        // SAFETY: as above.
        unsafe { webview14.add_ServerCertificateErrorDetected(&certificate, &mut token)? };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://app.example/swap";

    fn pending(id: u64) -> Navigations {
        Navigations {
            pending: Some((id, SITE.to_owned())),
            committed: None,
        }
    }

    /// An error page, then the navigation completing unsuccessfully: a
    /// failure, named by the address that was asked for.
    #[test]
    fn a_navigation_that_ends_on_the_engines_page_failed() {
        match pending(7).completed(7, false, 8, "https://other.example/".to_owned()) {
            EngineLoad::Failed {
                url,
                status,
                certificate,
            } => {
                assert_eq!(url, SITE);
                assert_eq!(status, 8);
                assert!(!certificate);
            }
            _ => unreachable!("expected a failure"),
        }
    }

    /// The site committed, then the load ended badly (a stop, its own error
    /// page): what the site shows is the site's.
    #[test]
    fn a_site_that_committed_is_not_a_failure() {
        let mut navigations = pending(7);
        navigations.committed = Some(7);
        assert!(matches!(
            navigations.completed(7, false, 14, SITE.to_owned()),
            EngineLoad::Finished(_)
        ));
        assert!(matches!(
            pending(7).completed(7, true, 0, SITE.to_owned()),
            EngineLoad::Finished(_)
        ));
    }

    /// A subresource's bad certificate is not the page's.
    #[test]
    fn a_certificate_error_is_the_pages_only_for_its_origin() {
        let navigations = pending(1);
        assert_eq!(
            navigations
                .certificate("https://app.example/x.js")
                .as_deref(),
            Some(SITE)
        );
        assert_eq!(navigations.certificate("https://cdn.example/x.js"), None);
        assert_eq!(Navigations::default().certificate(SITE), None);
    }
}
