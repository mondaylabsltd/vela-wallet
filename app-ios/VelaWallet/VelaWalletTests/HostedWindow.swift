//
//  HostedWindow.swift
//  VelaWalletTests
//
//  The window a test puts a real screen up in, to read where things stand.
//
//  It is a window of the app's own scene. A test used to make its own —
//  `UIWindow(frame:)`, which belongs to no scene — and such a window does
//  not know the screen at first: its top safe area is the status bar's
//  54 pt, and becomes the screen's 62 pt at its second screen update, which
//  moves everything under it down by eight. Every reader here waits until
//  two reads agree, and that covered it for as long as the second update
//  came between the first two reads. On a runner where it came after them
//  the reads agreed on the window before it had moved:
//
//      theLineHoldsItsPlaceAcrossCheckingLiveAndCantReach …
//      (liveAt → 242.0) == (at → 234.0)        (PR 489, 2026-10-10)
//
//  — the first of four windows read at 54, the other three at 62, and a
//  test about a line that does not move the control found it moved.
//
//  A scene's window has the screen's safe area from its first layout
//  (measured: 62 at every read, where the other went 54, 54, 62), so there
//  is nothing to settle and nothing to race; and a read takes two looks
//  instead of three. It is shown, not made key: the app's own window keeps
//  the keyboard and the first responder, as it did when these windows
//  belonged to no scene and could not take them.
//

import UIKit

@MainActor
enum HostedWindow {
    /// A window of `size` at the screen's origin, holding `root`, on screen.
    /// The caller hides it when it is done (`isHidden = true`).
    static func show(_ root: UIViewController, size: CGSize) -> UIWindow {
        let frame = CGRect(origin: .zero, size: size)
        let window: UIWindow
        if let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first {
            window = UIWindow(windowScene: scene)
            window.frame = frame
        } else {
            // No scene to belong to (the tests are hosted in the app, so
            // there is one): the window a test used to make.
            window = UIWindow(frame: frame)
        }
        window.rootViewController = root
        window.isHidden = false
        return window
    }
}
