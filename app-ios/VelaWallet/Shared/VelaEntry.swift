//
//  VelaEntry.swift
//  VelaWallet + VelaWidgets
//
//  The three doors into the wallet from outside the app (native parity
//  N-I2): a Home Screen quick action, a widget's button, a Shortcut or Siri,
//  and — on iOS 18 — the Control Center's Scan. Each one names WHERE to
//  open and nothing else: no address, no amount, no account. What happens
//  there is the wallet's own screens and the core's rules, exactly as if the
//  person had tapped the home's button.
//
//  Shared by the app and the widget extension, so a widget's link and the
//  app's reading of it are one spelling.
//

import Foundation

enum VelaEntry: String, CaseIterable, Codable, Sendable {
    case send
    case receive
    case scan

    /// `velawallet://` — the scheme the app already registers (spec 056).
    static let scheme = "velawallet"

    /// The link a widget opens: `velawallet://send`, `…/receive`, `…/scan`.
    var url: URL {
        var parts = URLComponents()
        parts.scheme = Self.scheme
        parts.host = rawValue
        // Built from constants, so it always forms; the fallback is the
        // scheme's root, which the app reads as no entry at all.
        return parts.url ?? URL(fileURLWithPath: "/")
    }

    /// A link read back. Exactly `velawallet://<entry>` — with no path, no
    /// query and no fragment: an entry carries nothing, so a link that tries
    /// to carry something is not one of these (and `velawallet://pay?…` is
    /// the payment link's, read by `PayLink`).
    init?(url: URL) {
        guard url.scheme?.lowercased() == Self.scheme,
              let host = url.host?.lowercased(),
              let entry = VelaEntry(rawValue: host)
        else { return nil }
        let path = url.path
        guard path.isEmpty || path == "/", url.query == nil, url.fragment == nil else { return nil }
        self = entry
    }

    /// The quick action's type (`UIApplicationShortcutItem.type`).
    var shortcutType: String { "app.getvela.VelaWallet.entry.\(rawValue)" }

    init?(shortcutType: String) {
        guard let entry = Self.allCases.first(where: { $0.shortcutType == shortcutType }) else { return nil }
        self = entry
    }

    /// The SF Symbol each door wears on the Home Screen, in Shortcuts and in
    /// the Control Center — the system's own surfaces speak SF Symbols.
    var systemImage: String {
        switch self {
        case .send: "arrow.up.right"
        case .receive: "arrow.down.left"
        case .scan: "qrcode.viewfinder"
        }
    }

    /// The corpus key of the door's name: the home dock's own words.
    var titleKey: String {
        switch self {
        case .send: "componentsUi.dock.send"
        case .receive: "componentsUi.dock.receive"
        case .scan: "componentsUi.dock.scan"
        }
    }
}
