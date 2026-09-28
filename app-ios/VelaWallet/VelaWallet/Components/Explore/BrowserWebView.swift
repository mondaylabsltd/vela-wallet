//
//  BrowserWebView.swift
//  VelaWallet
//
//  The seam. `DemoPageView` drew a picture of a web page since spec 021 and
//  said in its own comment that a real `WKWebView` would replace it wholesale;
//  this is that view.
//
//  It owns nothing. The engine — and therefore the page, its JavaScript, its
//  session and any request in flight — belongs to `BrowserController`, so
//  leaving 探索 and coming back finds the page where it was, and a sheet
//  opening over it does not reload anything.
//

import SwiftUI
import WebKit

struct BrowserWebView: UIViewRepresentable {
    let engine: BrowserEngine

    func makeUIView(context: Context) -> WKWebView {
        engine.webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        // Nothing. Every property this view could set is the engine's, and a
        // representable that writes back into its model on every layout pass
        // is a loop waiting for a reason.
    }
}

/// What a page that would not load says.
///
/// Opaque, over the web view: `WKWebView` keeps whatever it was showing (a
/// white frame, or the previous page on a failed second navigation), and a
/// message half-visible over a blank rectangle reads as a rendering bug. It
/// stays up through a retry (spec 079), saying so, and gives way only to a
/// page that got through — the blank or previous page is never shown in
/// between.
///
/// The sentences are the corpus's own: `connect.browser.loadFailed`, then the
/// REASON the core chose for the failure's class (`explore.loadOffline`,
/// `loadNotFound`, `loadCertificate` — no connection, a wrong name and a bad
/// certificate need different things done), the host, and the button, which
/// reads `explore.loadRetrying` while an attempt runs. Until 079 the reason
/// was the system's own English sentence and an error code.
struct BrowserFailureView: View {
    @Environment(\.theme) private var theme

    let title: String
    /// The core's sentence for the class; `nil` when it would repeat the title.
    var reason: String?
    /// The host that would not load.
    let detail: String
    let retry: String
    /// An attempt is running: the button says `retryingLabel` and takes no tap.
    var retrying = false
    var retryingLabel = ""
    var onRetry: () -> Void = {}

    var body: some View {
        VStack(spacing: Tokens.Space.s12) {
            Text(verbatim: title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
            if let reason {
                Text(verbatim: reason)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, Tokens.Space.s24)
                    .accessibilityIdentifier("explore.loadFailed.reason")
            }
            if !detail.isEmpty {
                Text(verbatim: detail)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, Tokens.Space.s24)
            }
            VelaButton(title: retrying && !retryingLabel.isEmpty ? retryingLabel : retry,
                       kind: .secondary, enabled: !retrying, action: onRetry)
                .padding(.horizontal, Tokens.Space.s24)
                .padding(.top, Tokens.Space.s8)
                .accessibilityIdentifier("explore.loadFailed.retry")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bgBase)
        .accessibilityIdentifier("explore.loadFailed")
    }
}

/// What a tab whose renderer died says (spec 070 FR-013).
///
/// Before 070 `webViewWebContentProcessDidTerminate` was not implemented: the
/// page went white and every request it had open waited forever. Now the core
/// has settled those requests (4900) by the time this is drawn, and the one
/// thing left to offer is a reload — the wallet itself is untouched, and the
/// sentence says so.
struct BrowserCrashedView: View {
    @Environment(\.theme) private var theme

    let title: String
    let detail: String
    let reload: String
    var onReload: () -> Void = {}

    var body: some View {
        VStack(spacing: Tokens.Space.s12) {
            LucideIcon(.triangleAlert, size: LucideIconSize.action)
                .foregroundStyle(theme.fgMuted)
            Text(verbatim: title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
            Text(verbatim: detail)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .multilineTextAlignment(.center)
                .padding(.horizontal, Tokens.Space.s24)
            VelaButton(title: reload, kind: .secondary, action: onReload)
                .padding(.horizontal, Tokens.Space.s24)
                .padding(.top, Tokens.Space.s8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bgBase)
        .accessibilityIdentifier("explore.pageCrashed")
    }
}
