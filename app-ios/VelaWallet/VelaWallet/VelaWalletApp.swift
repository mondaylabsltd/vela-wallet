//
//  VelaWalletApp.swift
//  VelaWallet
//

import BackgroundTasks
import SwiftUI

@main
struct VelaWalletApp: App {
    private let loc = Loc()

    init() {
        // The parallel space's seam, installed before any view exists. The
        // binding — and the keyset it reaches — are excluded from Release
        // builds by the build configuration, so this call has nothing to
        // install there and the symbols are not in the binary at all.
        #if DEBUG
        ParallelSpaceBinding.install()
        #endif
    }

    /// The identifier `Info.plist` permits and the scene registers.
    static let trackerTask = "app.getvela.VelaWallet.tracker"

    var body: some Scene {
        WindowGroup {
            RootView(loc: loc)
        }
        // A background refresh, which iOS runs at ITS discretion and may never
        // run at all. It is best effort by design, and the wallet's promise
        // does not rest on it: every launch resumes the pending set from
        // storage, so a refresh that never happens delays a verdict rather
        // than losing one.
        .backgroundTask(.appRefresh(Self.trackerTask)) {
            await TrackerBackgroundBridge.run()
        }
    }
}
