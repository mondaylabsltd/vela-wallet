//
//  DevMultisigPoll.swift
//  VelaWallet
//
//  The multisig's background poll, run on demand — for a simulator, where
//  iOS never runs a `BGAppRefreshTask` by itself and a UI test cannot force
//  one. Debug configurations only.
//
//  Excluded from Release by `EXCLUDED_SOURCE_FILE_NAMES` (beside
//  `DevLaunchInputs.swift`) and called only under `#if DEBUG`: a Release
//  binary holds neither the code, the input's name nor the trigger's
//  (`app-ios/scripts/check-ios-store-binary.sh` reads the archived binary).
//
//  ## What it does
//
//  With `VELA_DEV_MULTISIG_POLL=1` in the launch environment:
//
//  - when the app leaves the screen it asks iOS for the background time a
//    finishing task gets (about 30 s), so the app is still running when
//  - the Darwin notification `app.getvela.VelaWallet.dev.multisigPoll`
//    arrives (a UI test posts it: the runner and the app share the
//    simulator's notify center) — and then runs THE SAME round the
//    `BGAppRefreshTask` runs (`MultisigBackgroundBridge.run`): the proposals
//    machine reads every multisig here, and what changed for this device is
//    posted as a local notification by `MultisigNotifier`.
//
//  Nothing here decides what is said or when: that is the notifier's and the
//  machines', exactly as in the real refresh.
//

#if DEBUG

import Foundation
import UIKit

@MainActor
enum DevMultisigPoll {
    /// The launch input that arms it.
    static let envVar = "VELA_DEV_MULTISIG_POLL"
    /// The Darwin notification that runs one round.
    static let darwinName = "app.getvela.VelaWallet.dev.multisigPoll"

    private static var armed = false
    private static var task: UIBackgroundTaskIdentifier = .invalid
    private static var observers: [NSObjectProtocol] = []

    /// Read the launch environment, once, from the app's init.
    static func install(environment: [String: String] = ProcessInfo.processInfo.environment) {
        guard environment[envVar] == "1", !armed else { return }
        armed = true
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterAddObserver(
            center, nil,
            { _, _, _, _, _ in
                Task { @MainActor in await DevMultisigPoll.round() }
            },
            darwinName as CFString, nil, .deliverImmediately
        )
        observers.append(NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
        ) { _ in
            Task { @MainActor in DevMultisigPoll.stayAwake() }
        })
        observers.append(NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
        ) { _ in
            Task { @MainActor in DevMultisigPoll.release() }
        })
        print("[vela-wallet] dev: multisig poll armed (\(darwinName))")
    }

    /// Away: keep running for the time iOS gives a finishing task.
    private static func stayAwake() {
        guard task == .invalid else { return }
        task = UIApplication.shared.beginBackgroundTask(withName: "dev.multisigPoll") {
            DevMultisigPoll.release()
        }
    }

    private static func release() {
        guard task != .invalid else { return }
        UIApplication.shared.endBackgroundTask(task)
        task = .invalid
    }

    /// One round — the background refresh's own.
    static func round() async {
        print("[vela-wallet] dev: multisig poll — one background round")
        await MultisigBackgroundBridge.run()
        print("[vela-wallet] dev: multisig poll — round done")
    }
}

#endif
