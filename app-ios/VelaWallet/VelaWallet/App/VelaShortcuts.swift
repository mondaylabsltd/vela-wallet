//
//  VelaShortcuts.swift
//  VelaWallet
//
//  The App Shortcuts (native parity N-I2): Send, Receive and Scan in the
//  Shortcuts app, in Spotlight and for Siri, with no setup. The phrases are
//  keys of `AppShortcuts.xcstrings` and the short titles of
//  `Localizable.xcstrings`, both written from the corpus for the 15
//  languages by `app-ios/scripts/gen-intent-strings.mjs` — the dock's own
//  words, so Siri and the app name the doors alike.
//

import AppIntents

struct VelaShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: OpenSendIntent(),
            phrases: [
                "\(.applicationName) send",
                "Send with \(.applicationName)",
            ],
            shortTitle: "intent.send.title",
            systemImageName: "arrow.up.right"
        )
        AppShortcut(
            intent: OpenReceiveIntent(),
            phrases: [
                "\(.applicationName) receive",
                "Receive with \(.applicationName)",
            ],
            shortTitle: "intent.receive.title",
            systemImageName: "arrow.down.left"
        )
        AppShortcut(
            intent: OpenScanIntent(),
            phrases: [
                "\(.applicationName) scan",
                "Scan with \(.applicationName)",
            ],
            shortTitle: "intent.scan.title",
            systemImageName: "qrcode.viewfinder"
        )
    }

    static let shortcutTileColor: ShortcutTileColor = .orange
}
