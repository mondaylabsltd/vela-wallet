//
//  ScreenModelStackTests.swift
//  VelaWalletTests
//
//  Opening 设置 crashed an iPhone's Debug build every time (spec 091,
//  device-found 2026-10-02): SIGSEGV in the main thread's stack guard, with
//  only 89 frames on the stack. The frames were huge, not many:
//
//  - `SettingsScreenModel` was a 4.4 KB value, and the live builder
//    (`RootView.settingsModel`) passes it through some twenty
//    `SettingsLive.with…` steps. Unoptimised code gives every temporary its own
//    slot, so that one frame took 226 KB (measured in the arm64 binary), each
//    step's own frame another 18–37 KB;
//  - SwiftUI's generic body frames allocate the whole body value — the
//    settings view and its model included — several times more;
//  - an iPhone's main thread has 1 MB; the simulator's has far more, which is
//    why no simulator run ever saw it.
//
//  The model is copy-on-write now: one pointer. These tests hold that in place
//  where the simulator can see it:
//
//  1. every screen model a builder threads through its steps stays small;
//  2. the settings builder, run the way RootView runs it, stays inside a stack
//     budget — measured on a thread whose stack this test owns, painted
//     before and read back after, so a regression is a number in a failure
//     message rather than a crash on somebody's phone.
//

import Darwin
import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ScreenModelStackTests {

    // MARK: - 1. Sizes

    /// The settings model is one reference — copy-on-write — however many
    /// pages, sheets and rows it grows.
    @Test func theSettingsModelIsOnePointer() {
        #expect(MemoryLayout<SettingsScreenModel>.size == MemoryLayout<Int>.size)
        // What it holds is still a value: a copy is not changed by a write
        // to the original.
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        var model = SettingsFixtures.build(.st14, loc: loc)
        let copy = model
        let title = model.about.debugMode.title
        model.page = .storage
        model.about.debugMode.title = "changed"
        #expect(copy.page == .about)
        #expect(copy.about.debugMode.title == title)
        #expect(title != "changed")
        #expect(model.page == .storage)
        #expect(model.about.debugMode.title == "changed")
    }

    /// Every other screen model is a value a builder copies once per step,
    /// so each byte here is paid several times over in an unoptimised frame.
    /// The budget is the line past which a model is boxed like the settings
    /// one. Measured 2026-10-02: Explore 1,984 B (closest), Flow 1,280 B,
    /// Send 1,130 B, Signing 1,010 B, Contacts 705 B, Wallet 536 B.
    @Test func everyScreenModelStaysUnderTheBudget() {
        let budget = 2_048
        let sizes: [(String, Int)] = [
            ("SettingsScreenModel", MemoryLayout<SettingsScreenModel>.size),
            ("ExploreHomeModel", MemoryLayout<ExploreHomeModel>.size),
            ("FlowScreenModel", MemoryLayout<FlowScreenModel>.size),
            ("SendViewWire", MemoryLayout<SendViewWire>.size),
            ("SigningModel", MemoryLayout<SigningModel>.size),
            ("ContactsScene", MemoryLayout<ContactsScene>.size),
            ("WalletHomeModel", MemoryLayout<WalletHomeModel>.size),
        ]
        for (name, size) in sizes {
            #expect(size <= budget, "\(name) is \(size) B, over the \(budget) B budget — box it (see SettingsScreenModel)")
        }
        // The views that carry a model inline are values too.
        #expect(MemoryLayout<SettingsScreen>.size <= 2 * budget,
                "SettingsScreen is \(MemoryLayout<SettingsScreen>.size) B")
    }

    // MARK: - 2. The builder's stack

    /// The settings builder, step for step as `RootView.settingsModel` runs the
    /// ones that need no machine, uses at most `budget` bytes of stack.
    /// Measured 2026-10-02 on the simulator (arm64, Debug): 202,880 B with the
    /// model inline; 27,744–37,648 B boxed (the higher figure when this is the
    /// first thing in the process to instantiate the types' metadata). The
    /// device crash had SwiftUI's frames taking most of a 1 MB main stack, so
    /// a builder gets a small share.
    @Test func theSettingsBuilderFitsItsStackBudget() {
        let budget = 64 * 1024
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let defaults = UserDefaults(suiteName: "vela.tests.stack.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let prefs = Preferences(store: store)
        prefs.boot()
        let report = DeviceStorage.measure(store)
        let sites = [
            DbrSiteViewWire(origin: "https://app.uniswap.org", address: Self.address, chainId: 1, grantedAtMs: 1),
            DbrSiteViewWire(origin: "http://192.168.1.5:3000", address: Self.address, chainId: 100, grantedAtMs: 2),
        ]
        let facts = SettingsLive.FeedbackFacts(version: "0.9.5", commit: "abc1234", platform: "iOS 26.5",
                                               language: "zh", unreachable: [])
        var built: SettingsScreenModel?
        let used = Self.stackBytesUsed {
            // The main thread waits in `pthread_join` meanwhile: nothing runs
            // beside this, so reading main-actor state from here is sequential.
            built = Self.buildLikeRootView(
                loc: loc, prefs: prefs, report: report, sites: sites, facts: facts
            )
        }
        print("ScreenModelStackTests: the settings builder used \(used) B of stack")
        #expect(built?.about.title.isEmpty == false, "the chain ran")
        #expect(used > 0, "the stack was measured")
        #expect(used <= budget, "the settings builder used \(used) B of stack, over its \(budget) B budget")
    }

    /// The other screens' builders that run on the main thread's body pass —
    /// signing's fallback (`RootView.signingModel` builds it on every sheet),
    /// the send flow's base, the wallet home, Explore — each across all its
    /// states, against a budget of its own. Measured 2026-10-02 (simulator,
    /// arm64, Debug), the deepest state of each: signing 90,720 B, the flow
    /// 74,064 B, wallet home 36,576 B, Explore 30,864 B — all under the
    /// settings builder's pre-fix 202,880 B; signing is the one to watch.
    @Test func theOtherScreenBuildersFitTheirBudget() {
        let budget = 128 * 1024
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        var worst: [String: Int] = [:]
        for state in SigningStateId.allCases {
            worst["SigningFixtures.build", default: 0] = max(worst["SigningFixtures.build", default: 0],
                Self.stackBytesUsed { _ = MainActor.assumeIsolatedUnchecked { SigningFixtures.build(state, loc: loc) } })
        }
        for state in FlowStateId.allCases {
            worst["WalletFlowFixtures.build", default: 0] = max(worst["WalletFlowFixtures.build", default: 0],
                Self.stackBytesUsed { _ = MainActor.assumeIsolatedUnchecked { WalletFlowFixtures.build(state, loc: loc) } })
        }
        for state in MobileStateId.allCases {
            worst["WalletFixtures.buildMobileState", default: 0] = max(worst["WalletFixtures.buildMobileState", default: 0],
                Self.stackBytesUsed { _ = MainActor.assumeIsolatedUnchecked { WalletFixtures.buildMobileState(state, loc: loc) } })
        }
        for state in ExploreStateId.allCases {
            worst["ExploreFixtures.buildMobileState", default: 0] = max(worst["ExploreFixtures.buildMobileState", default: 0],
                Self.stackBytesUsed { _ = MainActor.assumeIsolatedUnchecked { ExploreFixtures.buildMobileState(state, loc: loc) } })
        }
        for (name, used) in worst.sorted(by: { $0.key < $1.key }) {
            print("ScreenModelStackTests: \(name) used \(used) B of stack at most")
            #expect(used > 0 && used <= budget, "\(name) used \(used) B of stack, over its \(budget) B budget")
        }
    }

    private static let address = "0x7687c0bc1DD2b9d7E9A5b1B4e1b0CBd8E0C3D141"

    /// `RootView.settingsModel`'s unconditional steps, in its order and its
    /// shape (`model = SettingsLive.with…(…, on: model, loc:)`).
    nonisolated private static func buildLikeRootView(
        loc: Loc,
        prefs: Preferences,
        report: DeviceStorage.Report,
        sites: [DbrSiteViewWire],
        facts: SettingsLive.FeedbackFacts
    ) -> SettingsScreenModel {
        MainActor.assumeIsolatedUnchecked {
            let base = SettingsFixtures.build(.st1, loc: loc)
                .withIdentity(name: "Vela", address: address, display: "0x7687…D141")
            var model = base
            model = SettingsLive.withAccounts(
                session: .booting, balances: [], display: WalletLive.Display.from(nil), on: model, loc: loc
            )
            model = SettingsLive.withPreferences(prefs, on: model, loc: loc)
            model = SettingsLive.withEraseFailure(["vela.contacts"], on: model, loc: loc)
            model = SettingsLive.withProviderTests(on: model, loc: loc)
            model.page = .about
            model = SettingsLive.withStorage(report, on: model, loc: loc)
            model = SettingsLive.withConnections(sites, on: model, loc: loc)
            model = SettingsLive.withAbout(version: "0.9.5", commit: "abc1234", networkCount: 24, on: model, loc: loc)
            model = SettingsLive.withFeedback(facts, on: model, loc: loc)
            return model
        }
    }

    /// Runs `body` on a thread whose stack this test allocates and paints, and
    /// answers how many bytes of it `body` reached. The stack grows down, so
    /// the lowest byte that no longer holds the paint is the deepest point.
    nonisolated private static func stackBytesUsed(
        stackSize: Int = 8 << 20, _ body: @escaping () -> Void
    ) -> Int {
        let paint: UInt8 = 0xA5
        let page = Int(getpagesize())
        let stack = UnsafeMutableRawPointer.allocate(byteCount: stackSize, alignment: page)
        defer { stack.deallocate() }
        stack.initializeMemory(as: UInt8.self, repeating: paint, count: stackSize)

        final class Job { let body: () -> Void; init(_ body: @escaping () -> Void) { self.body = body } }
        var attr = pthread_attr_t()
        pthread_attr_init(&attr)
        defer { pthread_attr_destroy(&attr) }
        pthread_attr_setstack(&attr, stack, stackSize)
        var thread: pthread_t?
        let job = Unmanaged.passRetained(Job(body)).toOpaque()
        let created = pthread_create(&thread, &attr, { context in
            Unmanaged<Job>.fromOpaque(context).takeRetainedValue().body()
            return nil
        }, job)
        guard created == 0, let thread else { return -1 }
        pthread_join(thread, nil)

        let bytes = stack.assumingMemoryBound(to: UInt8.self)
        var low = 0
        while low < stackSize, bytes[low] == paint { low += 1 }
        return stackSize - low
    }
}

private extension MainActor {
    /// Run main-actor code on the test's own thread while the main thread is
    /// blocked waiting for it. `assumeIsolated` would trap off the main
    /// thread; nothing else runs meanwhile, so the access is sequential.
    nonisolated static func assumeIsolatedUnchecked<T>(_ body: @MainActor () -> T) -> T {
        withoutActuallyEscaping(body) { body in
            unsafeBitCast(body, to: (() -> T).self)()
        }
    }
}
