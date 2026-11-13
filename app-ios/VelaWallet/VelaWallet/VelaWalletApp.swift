//
//  VelaWalletApp.swift
//  VelaWallet
//

import BackgroundTasks
import SwiftUI

@main
struct VelaWalletApp: App {
    private let loc: Loc
    /// The quick actions' delegate (native parity N-I2): SwiftUI's scene
    /// hands over neither a cold launch's shortcut item nor a live one.
    @UIApplicationDelegateAdaptor(VelaAppDelegate.self) private var appDelegate

    init() {
        // The first line that runs: "cold start to cached home" is timed from
        // here (`PerfMarks`, native parity Phase 4).
        PerfMarks.launchBegan()
        let loc = PerfMarks.span("CatalogLoad") { Loc() }
        self.loc = loc
        // The parallel space's seam, installed before any view exists. The
        // binding — and the keyset it reaches — are excluded from Release
        // builds by the build configuration, so this call has nothing to
        // install there and the symbols are not in the binary at all.
        #if DEBUG
        // §28: a test build never writes to production — before any request.
        ProductionWriteGuard.install()
        ParallelSpaceBinding.install()
        // `VELA_DEV_PROXY` (spec 082): the browser's store, before any tab.
        DevProxy.install()
        // Local services for a harness run (native parity F0.5): written
        // before the graph, so no machine has read a store yet.
        DevLaunchInputs.install()
        // `VELA_DEV_MULTISIG_POLL`: the multisig's background round on a
        // Darwin notification, for a simulator iOS never refreshes.
        DevMultisigPoll.install()
        // `VELA_PERF_SEED` (native parity Phase 4): the performance tests'
        // history and cached home, for a Release build to be measured on.
        PerfSeed.install()
        #endif
        // The object graph, once, after the seams above are in (issue #483):
        // never in a view's init, which SwiftUI may run again at any time.
        _graph = State(initialValue: PerfMarks.span("AppGraph") { AppGraph(loc: loc, firstLaunch: true) })
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
        // The multisig's poll (native multisig M6): every multisig here read
        // as the screen reads it, and what changed for this device said.
        .backgroundTask(.appRefresh(MultisigNotifier.task)) {
            await MultisigBackgroundBridge.run()
        }
    }
}
