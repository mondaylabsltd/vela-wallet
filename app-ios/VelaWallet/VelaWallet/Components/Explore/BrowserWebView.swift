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
/// message half-visible over a blank rectangle reads as a rendering bug.
///
/// The two sentences are the corpus's own — `connect.browser.loadFailed` and
/// `connect.browser.retry` — which **no client had ever resolved**. Under them
/// is the SYSTEM's reason, verbatim and untranslated: "the host could not be
/// found" is a different problem from "the request timed out", and a person
/// debugging their own network needs the difference. Wrapping it in prose of
/// our own would be a fifth sentence to translate and a fact lost.
struct BrowserFailureView: View {
    @Environment(\.theme) private var theme

    let title: String
    let detail: String
    let retry: String
    var onRetry: () -> Void = {}

    var body: some View {
        VStack(spacing: Tokens.Space.s12) {
            Text(verbatim: title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
            Text(verbatim: detail)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .multilineTextAlignment(.center)
                .padding(.horizontal, Tokens.Space.s24)
            VelaButton(title: retry, kind: .secondary, action: onRetry)
                .padding(.horizontal, Tokens.Space.s24)
                .padding(.top, Tokens.Space.s8)
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
