//
//  R13NativeAdoptionTests.swift
//  VelaWalletTests
//
//  Round 13's web/core rules, adopted on iOS (docs/multisig-machines.md §17):
//
//  - r13-4: the RPC pool's SILENT networks (`RpcPoolView.silent_chains`) go
//    to the balance machine beside the out-of-reach set (`pool_unreached
//    {chain_ids, silent}`), so "Can't reach X" is said at once — as the web
//    says it — not after a second read's 18 s.
//  - r13-7: a multisig's history (`ProposalsView.history`): while its first
//    read is out the home's Activity is loading, never "No activity yet";
//    one that could not be read says so, with Try again.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
struct R13NativeAdoptionTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private static let vectors = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors")

    private func vectors(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: Self.vectors.appendingPathComponent(name))
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    // MARK: - r13-4: silent networks

    /// The pool's view carries its silent networks, as the core writes them.
    @Test func thePoolViewDecodesItsSilentNetworks() throws {
        let json = #"{"failed_chains":[],"rate_limited_chains":[],"banned":[],"unreached_chains":[100],"out_of_reach_chains":[100,130],"silent_chains":[100]}"#
        let view = try CoreJSON.decoder.decode(RpcPoolViewWire.self, from: Data(json.utf8))
        #expect(view.outOfReachChains == [100, 130])
        #expect(view.silentChains == [100])
    }

    /// The pool says both sets, on a change of either — a network that goes
    /// silent while already out of reach is said again, now as silent.
    @Test func thePoolHandsOnItsSilentNetworksBesideTheOutOfReachOnes() {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let pool = RpcPool(store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults), offline: true)
        var said: [([Int], [Int])] = []
        pool.onOutOfReach = { said.append(($0, $1)) }
        pool.viewArrived(RpcPoolViewWire(failedChains: [], rateLimitedChains: [], banned: [], outOfReachChains: [100]))
        pool.viewArrived(RpcPoolViewWire(failedChains: [], rateLimitedChains: [], banned: [],
                                         outOfReachChains: [100], silentChains: [100]))
        pool.viewArrived(RpcPoolViewWire(failedChains: [], rateLimitedChains: [], banned: [],
                                         outOfReachChains: [100], silentChains: [100]))
        #expect(said.count == 2, "said once per change, never twice for one")
        #expect(said.first?.0 == [100] && said.first?.1 == [])
        #expect(said.last?.0 == [100] && said.last?.1 == [100])
    }

    /// `balance-pool-unreached.json`, replayed through the balance machine
    /// this app runs — every `pool_unreached` re-encoded the way
    /// `WalletStore.poolUnreached` sends it, so a `silent` this shell drops
    /// fails here: a silent network is named at once, a merely lost one is
    /// only read again.
    @Test func thePoolUnreachedVectorsHoldThroughTheShellsEvent() throws {
        let cases = try #require(try vectors("balance-pool-unreached.json")["cases"] as? [[String: Any]])
        #expect(cases.contains { ($0["name"] as? String ?? "").hasPrefix("r13-4") })
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let address = try #require(c["address"] as? String)
            let core = BalanceDashboardCore()
            var pending: [(id: UInt64, type: String)] = []
            func take(_ json: String) throws {
                let answer = try CoreJSON.object(json)
                for raw in answer["cancelled_effect_ids"] as? [Any] ?? [] {
                    let id = (raw as? NSNumber)?.uint64Value
                    pending.removeAll { $0.id == id }
                }
                for effect in answer["effects"] as? [[String: Any]] ?? [] {
                    guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                          let op = effect["operation"] as? [String: Any], let type = op["type"] as? String
                    else { continue }
                    pending.append((id, type))
                }
            }
            try take(core.dispatch(eventJson: CoreJSON.string(["type": "account_changed", "address": address])))
            for (i, step) in (c["steps"] as? [[String: Any]] ?? []).enumerated() {
                if let event = step["event"] as? [String: Any] {
                    let wire = event["type"] as? String == "pool_unreached"
                        ? WalletStore.poolUnreachedEvent(event["chain_ids"] as? [Int] ?? [],
                                                         silent: event["silent"] as? [Int] ?? [])
                        : CoreJSON.string(event)
                    try take(core.dispatch(eventJson: wire))
                } else if let result = step["answer"] as? [String: Any] {
                    let wanted = result["type"] as? String == "fetch_settled" ? "fetch_tokens" : "read_balance_cache"
                    let index = try #require(pending.firstIndex { $0.type == wanted }, "\(name) step \(i): no \(wanted)")
                    try take(core.resolveEffect(effectId: pending.remove(at: index).id, resultJson: CoreJSON.string(result)))
                }
                guard let expect = step["expect"] as? [String: Any] else { continue }
                let view = try CoreJSON.object(core.view())
                if let want = expect["unreachable"] as? [Int] {
                    let got = (view["unreachable_networks"] as? [[String: Any]] ?? []).compactMap { $0["chain_id"] as? Int }
                    #expect(got == want, "\(name), step \(i): unreachable")
                }
                if let live = expect["live"] as? Bool {
                    let said = view["live_key"].map { !($0 is NSNull) } ?? false
                    #expect(said == live, "\(name), step \(i): live")
                }
            }
        }
    }

    // MARK: - r13-7: a multisig's history

    private var emptyHome: WalletHomeModel { WalletFixtures.buildMobileState(.h2, loc: loc) }

    private func history(_ stand: String) -> WalletLive.MultisigHistory {
        WalletLive.MultisigHistory(
            stand: stand,
            unreadText: "Couldn't read this multisig's history on Arc Testnet",
            retryText: loc.t("common.tryAgain")
        )
    }

    @Test func withoutTheHistorysWordAnEmptyMultisigSaysNoActivityYet() {
        // The bug: this is what the home said while the first read was out.
        #expect(emptyHome.activitySection.mode == .empty)
        #expect(emptyHome.activitySection.empty?.title == loc.t("home.emptyNoActivity"))
    }

    @Test func aHistoryBeingReadIsLoadingNeverNoActivityYet() {
        let model = WalletLive.withMultisigHistory(emptyHome, history("reading"))
        #expect(model.activitySection.mode == .loading)
    }

    @Test func aHistoryReadWithNothingInItSaysNoActivityYet() {
        let model = WalletLive.withMultisigHistory(emptyHome, history("read"))
        #expect(model.activitySection.mode == .empty)
        #expect(model.activitySection.empty?.title == loc.t("home.emptyNoActivity"))
        #expect(model.activitySection.empty?.retry == nil)
    }

    @Test func aHistoryNotReadSaysSoWithTryAgain() {
        let model = WalletLive.withMultisigHistory(emptyHome, history("unread"))
        #expect(model.activitySection.mode == .empty)
        #expect(model.activitySection.empty?.title == "Couldn't read this multisig's history on Arc Testnet")
        #expect(model.activitySection.empty?.caption == "", "never the \"No activity yet\" caption")
        #expect(model.activitySection.empty?.retry == "Try again")
    }

    @Test func rowsAlreadyShownStayWhateverTheHistory() {
        let rows = WalletFixtures.buildMobileState(.h1, loc: loc)
        for stand in ["reading", "unread"] {
            let model = WalletLive.withMultisigHistory(rows, history(stand))
            #expect(model.activitySection.mode == .rows, "\(stand)")
            #expect(model.activityGroups.count == rows.activityGroups.count)
        }
    }

    /// The home drawn in each state — the button is there only for an unread
    /// history and calls back. With `VELA_SHOTS_DIR` set (xcodebuild's
    /// `TEST_RUNNER_VELA_SHOTS_DIR`), each frame is kept as a PNG for review.
    @Test func theHomeDrawsTheHistorysStates() async throws {
        let shots = ProcessInfo.processInfo.environment["VELA_SHOTS_DIR"]
        var retried = 0
        for (name, model) in [
            ("before-no-activity-yet", emptyHome),
            ("after-reading", WalletLive.withMultisigHistory(emptyHome, history("reading"))),
            ("after-unread-try-again", WalletLive.withMultisigHistory(emptyHome, history("unread"))),
        ] {
            let host = UIHostingController(
                rootView: WalletScreen(model: model, loc: loc, onActivityRetry: { retried += 1 }).themed(.light)
            )
            let window = HostedWindow.show(host, size: CGSize(width: 390, height: 1100))
            defer { window.isHidden = true }
            try await Task.sleep(for: .milliseconds(400))
            host.view.layoutIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: host.view.bounds).image { _ in
                host.view.drawHierarchy(in: host.view.bounds, afterScreenUpdates: true)
            }
            #expect(image.size.width > 0)
            if let shots, let png = image.pngData() {
                try png.write(to: URL(fileURLWithPath: shots).appendingPathComponent("ios-r13-history-\(name).png"))
            }
        }
        #expect(retried == 0, "drawing presses nothing")
    }

    /// The proposals view's `history`, as the core writes it; an older core's
    /// view without it reads as read (nothing claimed either way).
    @Test func theProposalsViewDecodesTheHistorysStand() throws {
        for stand in ["reading", "read", "unread"] {
            let view = try #require(MultisigMachine.decode(
                MsProposalsView.self,
                from: try JSONSerialization.jsonObject(with: Data(#"{"history":"\#(stand)","history_unreadable":[5042002]}"#.utf8))
            ))
            #expect(view.history == stand)
            #expect(view.historyUnreadable == [5_042_002])
        }
        let old = try #require(MultisigMachine.decode(MsProposalsView.self, from: try JSONSerialization.jsonObject(with: Data("{}".utf8))))
        #expect(old.history == "read")
    }
}
