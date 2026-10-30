//
//  AppEntryInbox.swift
//  VelaWallet
//
//  Where a door from outside the app waits for the wallet (native parity
//  N-I2): a Home Screen quick action, a widget's button, a Shortcut, Siri,
//  the Control Center. They arrive whenever the system says — before the
//  first frame, under the launch animation, while nobody is signed in — and
//  the wallet takes the one waiting when it can act on it.
//
//  ONE door waits at a time, the last one asked for: two quick actions in a
//  row mean the second. A door that arrives while nobody is signed in waits
//  until somebody is (the onboarding is not interrupted), and a door is
//  never opened over a signature the person is in the middle of.
//

import Observation
import UIKit

@MainActor
@Observable
final class AppEntryInbox {
    static let shared = AppEntryInbox()

    /// The door waiting to be opened.
    private(set) var pending: VelaEntry?

    init() {}

    func deliver(_ entry: VelaEntry) {
        pending = entry
    }

    /// The door waiting, taken: it opens once.
    func take() -> VelaEntry? {
        defer { pending = nil }
        return pending
    }

    /// Whether the wallet can open a door now — the rule, apart from the
    /// view, so it is said once and tested once.
    static func canOpen(signedIn: Bool, launching: Bool, signing: Bool, pageOverride: Bool) -> Bool {
        signedIn && !launching && !signing && !pageOverride
    }
}

/// The Home Screen's quick actions: the three doors, in the person's
/// language, and only while there is a wallet behind them — "Send" on the
/// icon of a phone with no account is a door to nowhere.
enum QuickActions {
    static func items(signedIn: Bool, title: (VelaEntry) -> String) -> [UIApplicationShortcutItem] {
        guard signedIn else { return [] }
        return VelaEntry.allCases.map { entry in
            UIApplicationShortcutItem(
                type: entry.shortcutType,
                localizedTitle: title(entry),
                localizedSubtitle: nil,
                icon: UIApplicationShortcutIcon(systemImageName: entry.systemImage),
                userInfo: nil
            )
        }
    }

    /// A quick action the system handed over, into the inbox. Whether it was
    /// one of these.
    @discardableResult
    static func handle(_ item: UIApplicationShortcutItem, inbox: AppEntryInbox? = nil) -> Bool {
        guard let entry = VelaEntry(shortcutType: item.type) else { return false }
        (inbox ?? .shared).deliver(entry)
        return true
    }
}

/// The app's delegate, for the one thing SwiftUI's scene does not hand over:
/// a quick action, at a cold launch and while running.
final class VelaAppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        // A cold launch from a quick action carries it here, once.
        if let item = options.shortcutItem { QuickActions.handle(item) }
        let configuration = UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
        configuration.delegateClass = VelaSceneDelegate.self
        return configuration
    }
}

/// The scene's delegate: a quick action pressed while the app is alive.
final class VelaSceneDelegate: NSObject, UIWindowSceneDelegate {
    func windowScene(
        _ windowScene: UIWindowScene,
        performActionFor shortcutItem: UIApplicationShortcutItem,
        completionHandler: @escaping (Bool) -> Void
    ) {
        completionHandler(QuickActions.handle(shortcutItem))
    }
}
