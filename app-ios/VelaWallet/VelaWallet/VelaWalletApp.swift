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
        // `VELA_DEV_PROXY` (spec 082): the browser's store, before any tab.
        DevProxy.install()
        #endif
        // The object graph, once, after the seams above are in (issue #483):
        // never in a view's init, which SwiftUI may run again at any time.
        _graph = State(initialValue: AppGraph(loc: loc, firstLaunch: true))
    }

    /// The identifier `Info.plist` permits and the scene registers.
    static let trackerTask = "app.getvela.VelaWallet.tracker"

    /// Every resident machine, the request pool and the money path (issue
    /// #483, `AppGraph`). An erase (spec 072) replaces it — every machine built
    /// again from the emptied store, which is exactly the first run — and the
    /// new graph is a new `RootView` identity. The web reloads the page for
    /// the same reason: nothing that held the erased wallet in memory may live
    /// on to write it back. Nothing else ever builds one.
    @State private var graph: AppGraph
    /// Which graph this is, for the root's identity.
    @State private var lifetime = 0

    var body: some Scene {
        WindowGroup {
            RootView(graph: graph, onErased: {
                graph = AppGraph(loc: loc, firstLaunch: false)
                lifetime += 1
            })
            .id(lifetime)
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
