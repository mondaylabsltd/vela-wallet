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
//    simulator's notify center; by hand, `xcrun simctl spawn <device>
//    notifyutil -p app.getvela.VelaWallet.dev.multisigPoll`) — and then runs
//    THE SAME round the `BGAppRefreshTask` runs
//    (`MultisigBackgroundBridge.run`): the proposals machine reads every
//    multisig here, and what changed for this device is posted as a local
//    notification by `MultisigNotifier`;
//  - r9-8: while that background time lasts it also runs the round by
//    itself every `awayEvery` seconds, so a tester who presses Home and has
//    another member propose sees the banner without posting anything. A
//    trigger posted after iOS suspended the app reaches nobody: the trace
//    says when the time ran out (`away: the background time ran out`).
//
//  The steps a tester follows are in app-ios/docs/multisig-notifications.md.
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
    /// The rounds run by themselves while away (r9-8).
    private static var awayLoop: Task<Void, Never>?
    /// Seconds between those rounds.
    static let awayEvery: UInt64 = 8

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
        // At once, on the main queue: a Task scheduled here may run only
        // after iOS has suspended the app.
        for name in [UIApplication.willResignActiveNotification, UIApplication.didEnterBackgroundNotification,
                     UIScene.didEnterBackgroundNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { DevMultisigPoll.stayAwake() }
            })
        }
        observers.append(NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { DevMultisigPoll.release() }
        })
        trace("armed (\(darwinName))")
    }

    /// Away: keep running for the time iOS gives a finishing task.
    private static func stayAwake() {
        guard task == .invalid else { return }
        task = UIApplication.shared.beginBackgroundTask(withName: "dev.multisigPoll") {
            DevMultisigPoll.trace("away: the background time ran out")
            DevMultisigPoll.release()
        }
        // `backgroundTimeRemaining` is `.greatestFiniteMagnitude` while the app
        // is still active: said as it is, never converted to an Int.
        trace("away: kept running (\(UIApplication.shared.backgroundTimeRemaining) s)")
        awayLoop?.cancel()
        awayLoop = Task { @MainActor in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: awayEvery * 1_000_000_000)
                guard !Task.isCancelled, task != .invalid, UIApplicationStateProbe.isAway else { return }
                await round()
            }
        }
    }

    private static func release() {
        awayLoop?.cancel()
        awayLoop = nil
        guard task != .invalid else { return }
        UIApplication.shared.endBackgroundTask(task)
        task = .invalid
    }

    /// One round — the background refresh's own.
    static func round() async {
        let notifier = MultisigBackgroundBridge.hub?.notifier
        trace("round: start (away \(UIApplicationStateProbe.isAway)); said before \(notifier?.seen.needsYou ?? [:])")
        await MultisigBackgroundBridge.run()
        trace("round: said now \(notifier?.seen.needsYou ?? [:])")
        let presence = MultisigBackgroundBridge.hub?.proposalsView.presence ?? [:]
        let waiting = presence.map { "\($0.key):\($0.value?.needsYou ?? -1)/\($0.value?.ready ?? -1)" }.joined(separator: ",")
        trace("round: done; needs you/ready \(waiting)")
    }

    /// A line for the harness — `multisig-poll.log` in the app's Caches,
    /// which the host reads (`simctl get_app_container … data`) — and the
    /// console.
    private static func trace(_ line: String) {
        print("[vela-wallet] dev: multisig poll — \(line)")
        guard let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else { return }
        let path = caches.appendingPathComponent("multisig-poll.log").path
        let stamped = "\(Date().timeIntervalSince1970) \(line)\n"
        if let handle = FileHandle(forWritingAtPath: path) {
            handle.seekToEndOfFile()
            handle.write(Data(stamped.utf8))
            try? handle.close()
        } else {
            try? stamped.write(toFile: path, atomically: true, encoding: .utf8)
        }
    }
}

#endif
