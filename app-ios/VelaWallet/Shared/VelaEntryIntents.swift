//
//  VelaEntryIntents.swift
//  VelaWallet + VelaWidgets
//
//  The three doors as App Intents (native parity N-I2): Shortcuts, Siri,
//  Spotlight and the Action button run them, and on iOS 18 the Control
//  Center's Scan is one. Each OPENS the app at its screen and does nothing
//  else — no parameter, no amount, no address: an intent that could start a
//  payment without the person in the wallet would be a door around the
//  confirm.
//
//  In both targets because a control's action must be (the system runs it
//  in the app, `openAppWhenRun`); only the app's copy acts. The titles are
//  keys of `Localizable.xcstrings`, written from the corpus by
//  `app-ios/scripts/gen-intent-strings.mjs`.
//

import AppIntents

struct OpenSendIntent: AppIntent {
    static let title: LocalizedStringResource = "intent.send.title"
    static let openAppWhenRun = true
    static let isDiscoverable = true

    @MainActor
    func perform() async throws -> some IntentResult {
        VelaEntryIntentRoute.open(.send)
        return .result()
    }
}

struct OpenReceiveIntent: AppIntent {
    static let title: LocalizedStringResource = "intent.receive.title"
    static let openAppWhenRun = true
    static let isDiscoverable = true

    @MainActor
    func perform() async throws -> some IntentResult {
        VelaEntryIntentRoute.open(.receive)
        return .result()
    }
}

struct OpenScanIntent: AppIntent {
    static let title: LocalizedStringResource = "intent.scan.title"
    static let openAppWhenRun = true
    static let isDiscoverable = true

    @MainActor
    func perform() async throws -> some IntentResult {
        VelaEntryIntentRoute.open(.scan)
        return .result()
    }
}

/// Where an intent's door goes: the app's inbox, which the wallet reads once
/// it is open and somebody is signed in. In the widget extension there is
/// no wallet to open — the system runs the intent in the app.
enum VelaEntryIntentRoute {
    @MainActor
    static func open(_ entry: VelaEntry) {
        #if VELA_WIDGET_EXTENSION
        _ = entry
        #else
        AppEntryInbox.shared.deliver(entry)
        #endif
    }
}
