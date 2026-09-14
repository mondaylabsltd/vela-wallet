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
