//
//  TrackerNotifier.swift
//  VelaWallet
//
//  The one notification this app posts: a send it was following has landed.
//
//  ## Asked at the first submit, never at launch
//
//  A wallet that asks for notifications on first launch is asking before it has
//  anything to say. The permission is requested when a send first reaches its
//  receipt (Android 043's D6, and the same on web), and a refusal degrades to
//  the in-app receipt with no second ask — a person who said no has answered.
//
//  ## Only when the app is away
//
//  The shell decides WHETHER to post; the core decides THAT it happened. A
//  banner over the receipt screen somebody is already reading is noise, so
//  `willPresent` returns nothing and a foreground confirmation is not posted at
//  all.
//

import Foundation
import UIKit
import UserNotifications

@MainActor
final class TrackerNotifier: NSObject {

    /// Read by `RootView` when a notification is tapped: the operation to open.
    private(set) var pendingReceipt: String?

    private let loc: Loc
    /// Whether the first submit has already asked. One ask per install.
    private var asked = false

    init(loc: Loc) {
        self.loc = loc
        super.init()
        UNUserNotificationCenter.current().delegate = self
    }

    /// Called when a send reaches the point of having something to report.
    func askOnceIfNeeded() {
        guard !asked else { return }
        asked = true
        let center = UNUserNotificationCenter.current()
        center.getNotificationSettings { settings in
            guard settings.authorizationStatus == .notDetermined else { return }
            center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
        }
    }

    /// A confirmation landed while the app was away.
    func confirmed(userOpHash: String, chainId: Int, txHash: String) {
        guard UIApplicationStateProbe.isAway else { return }
        let content = UNMutableNotificationContent()
        content.title = loc.t("componentsTx.receipt.statusConfirmed")
        content.body = loc.t("send.txSubmittedTitle")
        content.sound = .default
        // One thread, so several confirmations group rather than stack.
        content.threadIdentifier = "transactions"
        content.userInfo = ["vela.receipt": userOpHash, "vela.chain": chainId]
        // The hash IS the identifier: a re-delivery of the same confirmation
        // replaces its notification rather than posting a second one.
        let request = UNNotificationRequest(
            identifier: userOpHash, content: content, trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }

    func clearPendingReceipt() { pendingReceipt = nil }
}

extension TrackerNotifier: UNUserNotificationCenterDelegate {
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        // Nothing over a screen the person is already looking at.
        []
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let hash = response.notification.request.content.userInfo["vela.receipt"] as? String
        await MainActor.run { [weak self] in self?.pendingReceipt = hash }
    }
}

/// `UIApplication.shared.applicationState`, reachable from a `@MainActor` type
/// without importing UIKit into every caller.
@MainActor
enum UIApplicationStateProbe {
    static var isAway: Bool {
        #if canImport(UIKit)
        UIApplication.shared.applicationState != .active
        #else
        false
        #endif
    }
}
