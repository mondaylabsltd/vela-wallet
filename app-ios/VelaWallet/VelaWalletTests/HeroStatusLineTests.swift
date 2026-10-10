//
//  HeroStatusLineTests.swift
//  VelaWalletTests
//
//  The ONE line under the home's total (PR 3 final notes F19, F16).
//
//  F19 — "live" and "Checking…" are the core's. A wallet that held nothing
//  last session opens with a cached total of 0; this shell called any zero
//  "live", so the home would have said "Live · listening for payments" over
//  a wallet nothing had read, then swapped it for "Can't reach 24 networks".
//  Now the first read says "Checking…" (`BalanceView.checkingKey`), and the
//  zero is live exactly when the core says so (`liveKey`).
//
//  The views come from the REAL `balance_dashboard` core
//  (`BalanceCoreScene.zeroWallet`), and the hero is the real `WalletScreen`,
//  hosted in a window: the control under the line is read from the tree an
//  assistive client gets, and it is at the same place under all three.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct HeroStatusLineTests {
    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    private var drawn: WalletHomeModel { WalletFixtures.buildMobileState(.h1, loc: en) }

    /// The hero as the live home builds it — its refresh control included.
    private func hero(_ view: BalanceViewWire, loc: Loc? = nil) -> BalanceModel {
        WalletLive.apply(view, on: drawn, loc: loc ?? en).balance
    }

    // MARK: - F19: what the line says

    /// A cached zero before the first settle: "Checking…", the cached figure,
    /// and neither "live" nor a reason.
    @Test func aCachedZeroIsCheckingAndNotLive() throws {
        let round = try #require(BalanceCoreScene.zeroWallet())
        let view = round.checking
        #expect(view.displayTotalUsd == 0, "the cached zero is shown")
        #expect(view.checkingKey == "componentsUi.funding.checking")
        #expect(view.liveKey == nil, "nothing has read this wallet yet")

        let model = hero(view)
        #expect(model.checkingText == "Checking…")
        #expect(model.state == .normal, "a zero nobody has read is not the live state")
        #expect(model.liveText == nil)
        #expect(model.status == nil, "\"Checking…\" stands alone on the line")
        #expect(model.integer == "$0" && model.decimals == "00")
        #expect(hero(view, loc: zh).checkingText == "正在检查…")
    }

    /// The round ended with every chain answering: live, in the core's words.
    @Test func aSettledZeroEveryChainAnsweredForIsLive() throws {
        let view = try #require(BalanceCoreScene.zeroWallet()).settled
        #expect(view.checkingKey == nil)
        #expect(view.liveKey == "home.liveIndicator")

        let model = hero(view)
        #expect(model.state == .zeroLive)
        #expect(model.liveText == "Live · listening for payments")
        #expect(model.checkingText == nil && model.status == nil)
        #expect(hero(view, loc: zh).liveText == zh.t("home.liveIndicator"))
    }

    /// The round ended with networks missing: "Can't reach", and never live.
    @Test func aSettledZeroWithNetworksMissingSaysCantReach() throws {
        let view = try #require(BalanceCoreScene.zeroWallet(missing: [137, 10])).settled
        #expect(view.checkingKey == nil && view.liveKey == nil)

        let model = hero(view)
        #expect(model.state == .normal, "a zero with a network out of reach is not a listening wallet")
        #expect(model.liveText == nil && model.checkingText == nil)
        #expect(model.status?.kind == .warning)
        #expect(model.status?.text == "Can't reach 2 networks right now")
    }

    /// A read that threw over the cached zero is neither checking (it ended)
    /// nor live (no chain answered).
    @Test func aReadThatThrewIsNeitherCheckingNorLive() throws {
        let view = try #require(BalanceCoreScene.zeroWallet(threw: true)).settled
        let model = hero(view)
        #expect(model.checkingText == nil && model.liveText == nil)
        #expect(model.state != .zeroLive)
    }

    /// "Zero, live" is the core's key and NOTHING else: the old rule (a total
    /// of 0) no longer makes it, and the key alone does.
    @Test func zeroLiveIsTheCoresKeyAndNothingElse() throws {
        var view = try #require(BalanceCoreScene.zeroWallet()).settled
        view.liveKey = nil
        #expect(view.displayTotalUsd == 0 && !view.balancePartial && view.tokens.isEmpty)
        #expect(hero(view).state == .normal, "a zero total alone made the hero live")
        #expect(hero(view).liveText == nil)

        // …and "Checking…" outranks everything else the line could say.
        var busy = try #require(BalanceCoreScene.zeroWallet(missing: [137])).settled
        busy.checkingKey = "componentsUi.funding.checking"
        let model = hero(busy)
        #expect(model.checkingText == "Checking…")
        #expect(model.status == nil)
    }

    /// A view from before the two keys (an older core, a stored fixture)
    /// decodes, and reads with neither line.
    @Test func aViewFromBeforeTheKeysStillDecodes() throws {
        let json: [String: Any] = [
            "address": BalanceCoreScene.address, "display_total_usd": 0.0,
            "balance_unknown": false, "balance_partial": false, "unreachable": false,
            "notice": NSNull(), "hidden": false, "refreshing": false,
            "last_refreshed_at_ms": NSNull(), "tokens": [[String: Any]](),
            "unpriced_tokens": [[String: Any]](), "failed_chain_ids": [Int](),
            "rate_limited_chain_ids": [Int](), "unreachable_networks": [[String: Any]](),
            "holdings_loading": false, "cached_total_usd": 0.0,
            "switcher": ["open": false, "loading": false, "balances": [[String: Any]]()],
        ]
        let old = try CoreJSON.decode(BalanceViewWire.self, from: json)
        #expect(old.checkingKey == nil && old.liveKey == nil)
        #expect(hero(old).state == .normal)
        var keyed = json
        keyed["checking_key"] = "componentsUi.funding.checking"
        keyed["live_key"] = "home.liveIndicator"
        let new = try CoreJSON.decode(BalanceViewWire.self, from: keyed)
        #expect(new.checkingKey == "componentsUi.funding.checking" && new.liveKey == "home.liveIndicator")
    }

    // MARK: - Nothing moves

    private struct Element {
        let label: String
        let id: String
        let frame: CGRect
    }

    private static let automation: Void = {
        guard let library = dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW) else { return }
        for name in ["_AXSSetAutomationEnabled", "_AXSApplicationAccessibilitySetEnabled"] {
            if let symbol = dlsym(library, name) {
                typealias Switch = @convention(c) (Int32) -> Void
                unsafeBitCast(symbol, to: Switch.self)(1)
            }
        }
    }()

    private static func identifier(of object: NSObject) -> String {
        let getter = #selector(getter: UIAccessibilityIdentification.accessibilityIdentifier)
        guard object.responds(to: getter) else { return "" }
        return object.perform(getter)?.takeUnretainedValue() as? String ?? ""
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        if object.isAccessibilityElement {
            found.append(Element(label: object.accessibilityLabel ?? "",
                                 id: Self.identifier(of: object), frame: object.accessibilityFrame))
        }
        if let children = object.accessibilityElements {
            for child in children { collect(child, depth: depth + 1, into: &found) }
        } else if case let count = object.accessibilityElementCount(), count != NSNotFound, count > 0 {
            for index in 0..<count {
                if let child = object.accessibilityElement(at: index) {
                    collect(child, depth: depth + 1, into: &found)
                }
            }
        } else if let view = object as? UIView {
            for sub in view.subviews { collect(sub, depth: depth + 1, into: &found) }
        }
    }

    /// The home under `balance`, hosted `width` points wide: every element an
    /// assistive client is given, once two reads agree on where the refresh
    /// control is.
    private func tree(_ balance: BalanceModel, width: CGFloat = 390, textScale: CGFloat = 1) async throws -> [Element] {
        // The 1.35× board is the drawn home at the largest text size.
        var model = textScale == 1 ? drawn : WalletFixtures.buildMobileState(.h7x, loc: en)
        model.balance = balance
        return try await tree(home: model, width: width)
    }

    /// The whole home as `model` has it — the hero and the lists under it.
    private func tree(home model: WalletHomeModel, width: CGFloat = 390) async throws -> [Element] {
        _ = Self.automation
        let host = UIHostingController(
            rootView: WalletScreen(model: model, loc: en, onToggleBalance: {}, onStatusTap: {},
                                   onRefreshNow: {})
                .themed(.light)
        )
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: width, height: 844))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        var last: CGRect?
        var found: [Element] = []
        for _ in 0..<40 {
            host.view.layoutIfNeeded()
            _ = UIGraphicsImageRenderer(bounds: host.view.bounds).image { _ in
                host.view.drawHierarchy(in: host.view.bounds, afterScreenUpdates: true)
            }
            try await Task.sleep(for: .milliseconds(150))
            found = []
            collect(host.view as Any, depth: 0, into: &found)
            if let control = found.first(where: { $0.id == BalanceRefreshControl.testId }) {
                if control.frame == last { break }
                last = control.frame
            }
        }
        return found
    }

    private func refreshY(_ tree: [Element]) throws -> CGFloat {
        try #require(tree.first { $0.id == BalanceRefreshControl.testId }, "the refresh control is in the tree")
            .frame.minY
    }

    /// The slot is the one held since round 2: the control under the line —
    /// and so Receive / Send and every row below — sits at the same place
    /// under "Checking…", "Live · listening" and "Can't reach", and with no
    /// line at all.
    @Test func theLineHoldsItsPlaceAcrossCheckingLiveAndCantReach() async throws {
        let round = try #require(BalanceCoreScene.zeroWallet())
        let missing = try #require(BalanceCoreScene.zeroWallet(missing: [137, 10])).settled
        var bare = hero(round.settled)
        bare.liveText = nil

        let checking = try await tree(hero(round.checking))
        let live = try await tree(hero(round.settled))
        let cantReach = try await tree(hero(missing))
        let none = try await tree(bare)

        #expect(checking.contains { $0.label == "Checking…" }, "the line is said aloud")
        #expect(live.contains { $0.label == "Live · listening for payments" })
        #expect(cantReach.contains { $0.label.contains("Can't reach 2 networks right now") })
        #expect(!checking.contains { $0.label == "Live · listening for payments" })

        let at = try refreshY(checking)
        let liveAt = try refreshY(live)
        let cantReachAt = try refreshY(cantReach)
        let noneAt = try refreshY(none)
        #expect(liveAt == at, "\"Live\" moved the control: \(liveAt) vs \(at)")
        #expect(cantReachAt == at, "\"Can't reach\" moved the control: \(cantReachAt) vs \(at)")
        #expect(noneAt == at, "no line moved the control: \(noneAt) vs \(at)")
    }

    // MARK: - The Assets list under the line (device round, item 2)

    /// A wallet that held nothing last session opens with a cached total of
    /// 0. This shell took "no rows, a known total" for an empty wallet, so
    /// the home invited a first deposit under "Checking…" — an answer before
    /// anything had been read, and one the first round could take back.
    ///
    /// The real core's views on the real home: before the first read ends,
    /// "Checking…" and NO "Deposit your first asset"; after a read that
    /// found nothing, the empty state — exactly when the core's `emptyKey`
    /// is there.
    @Test func aCachedZeroInvitesNoDepositUntilAReadFoundNothing() async throws {
        let round = try #require(BalanceCoreScene.zeroWallet())
        #expect(round.checking.emptyKey == nil, "nothing has read this wallet yet")
        #expect(round.settled.emptyKey == "assets.emptyTitle")
        let invite = en.t("assets.emptyTitle")
        #expect(invite == "Deposit your first asset")

        let before = WalletLive.apply(round.checking, on: drawn, loc: en)
        #expect(before.assetRows.isEmpty)
        #expect(before.assetsSection.mode == .loading, "a cached zero is not an empty wallet")
        let after = WalletLive.apply(round.settled, on: drawn, loc: en)
        #expect(after.assetsSection.mode == .empty)
        #expect(after.assetsSection.empty?.title == invite)
        #expect(after.assetsSection.empty?.caption == en.t("assets.emptySubtext"))
        #expect(WalletLive.apply(round.settled, on: WalletFixtures.buildMobileState(.h1, loc: zh), loc: zh)
            .assetsSection.empty?.title == zh.t("assets.emptyTitle"))

        // Drawn: what the home says, read from the tree.
        let checking = try await tree(home: before)
        #expect(checking.contains { $0.label == "Checking…" })
        #expect(!checking.contains { $0.label.contains(invite) }, "\"\(invite)\" under \"Checking…\"")
        let settled = try await tree(home: after)
        #expect(settled.contains { $0.label.contains(invite) }, "a read found nothing and the list does not say so")
        #expect(!settled.contains { $0.label == "Checking…" })

        // The key and nothing else: the same settled view without it draws
        // no empty state, and the unread one with it does.
        var unkeyed = round.settled
        unkeyed.emptyKey = nil
        #expect(WalletLive.apply(unkeyed, on: drawn, loc: en).assetsSection.mode == .loading)
        var keyed = round.checking
        keyed.emptyKey = "assets.emptyTitle"
        #expect(WalletLive.apply(keyed, on: drawn, loc: en).assetsSection.mode == .empty)

        // The Assets page (T1) reads the same key.
        guard case .assets(let page) = WalletFlowFixtures.build(.t1, loc: en).base else {
            Issue.record("T1 does not draw the assets list")
            return
        }
        #expect(FlowsLive.assets(round.checking, currency: nil, on: page, loc: en).empty == nil)
        #expect(FlowsLive.assets(round.settled, currency: nil, on: page, loc: en).empty?.title == invite)
    }

    /// A view from before the key (an older core, a stored fixture) decodes,
    /// and is not an empty wallet.
    @Test func aViewFromBeforeTheEmptyKeyIsNotEmpty() throws {
        var object: [String: Any] = [
            "address": BalanceCoreScene.address, "display_total_usd": 0.0,
            "balance_unknown": false, "balance_partial": false, "unreachable": false,
            "notice": NSNull(), "hidden": false, "refreshing": false,
            "last_refreshed_at_ms": NSNull(), "tokens": [[String: Any]](),
            "unpriced_tokens": [[String: Any]](), "failed_chain_ids": [Int](),
            "rate_limited_chain_ids": [Int](), "unreachable_networks": [[String: Any]](),
            "holdings_loading": false, "cached_total_usd": 0.0,
            "switcher": ["open": false, "loading": false, "balances": [[String: Any]]()],
        ]
        let old = try CoreJSON.decode(BalanceViewWire.self, from: object)
        #expect(old.emptyKey == nil)
        #expect(WalletLive.apply(old, on: drawn, loc: en).assetsSection.mode == .loading)
        object["empty_key"] = "assets.emptyTitle"
        let new = try CoreJSON.decode(BalanceViewWire.self, from: object)
        #expect(new.emptyKey == "assets.emptyTitle")
        #expect(WalletLive.apply(new, on: drawn, loc: en).assetsSection.mode == .empty)
    }

    // MARK: - F16: one line, always

    private func status(_ tree: [Element]) throws -> Element {
        try #require(tree.first { $0.id == BalanceDisplay.statusId }, "the status line is in the tree")
    }

    /// A sentence longer than the line — Tempo's token list, in the two
    /// longest languages, on an iPhone SE's 375 pt and at 320 — is ONE line:
    /// as tall as a short sentence's, inside the hero's column, with the
    /// control under it where it was. The whole sentence is what is read
    /// aloud, and it titles the list the line opens.
    @Test func aLongSentenceIsOneLineAndSaidWholeWhereItLeads() async throws {
        let view = try #require(BalanceCoreScene.view(failedChain: 4_217, internalFault: false, tokenListFault: true))
        let short = try #require(BalanceCoreScene.view(failedChain: 4_217, internalFault: false))
        for width in [375.0, 320.0] as [CGFloat] {
            let plain = try await tree(hero(short), width: width)
            let line = try status(plain).frame
            let at = try refreshY(plain)
            for lang in ["es-MX", "it", "pt-BR", "ru"] {
                let loc = Loc(overrideTag: lang, preferredLanguages: [])
                let sentence = try #require(WalletLive.unreachableLine(view, loc: loc))
                #expect(sentence == loc.t("assets.tokenListUnreachable", vars: ["name": "Tempo"]))
                let long = try await tree(hero(view, loc: loc), width: width)
                let drawn = try status(long)
                print("MEASURE hero-status w=\(Int(width)) \(lang) chars=\(sentence.count) line=\(drawn.frame)"
                    + " short=\(line) refresh.y=\(try refreshY(long)) (short \(at))")
                #expect(drawn.frame.height <= line.height + 0.5,
                        "\(lang) at \(Int(width)): the line is \(drawn.frame.height) tall, a short one \(line.height)")
                #expect(drawn.frame.maxX <= width - 23.5, "\(lang) at \(Int(width)): the line runs past the hero: \(drawn.frame)")
                #expect(drawn.label == sentence, "the whole sentence is not what is read: \(drawn.label)")
                let refresh = try refreshY(long)
                #expect(refresh == at, "\(lang) at \(Int(width)): the control moved, \(refresh) vs \(at)")
                // The list the line opens is titled by the same sentence, whole.
                let list = SettingsLive.withUnreachable(
                    view, display: .usd, on: SettingsFixtures.build(.sr6, loc: loc), loc: loc
                ).unreachable
                #expect(list.title == sentence)
            }
        }
    }

    /// The two long reasons a hero can carry that open the BREAKDOWN — Vela's
    /// own fault, and a request that never arrived — are one line too, at
    /// the largest text size as well; the breakdown says each in full at its
    /// top. A healthy wallet's breakdown says nothing there.
    @Test func aLongReasonIsOneLineAndStandsAtTheTopOfTheBreakdown() async throws {
        let short = try #require(BalanceCoreScene.view(failedChain: 4_217, internalFault: false))
        let fault = try #require(BalanceCoreScene.view(internalFault: true))
        let nothing = try #require(BalanceCoreScene.view(internalFault: false, everyChain: true))
        for (width, scale) in [(375.0, 1.0), (320.0, 1.0), (375.0, 1.35)] as [(CGFloat, CGFloat)] {
            let plain = try await tree(hero(short), width: width, textScale: scale)
            let line = try status(plain).frame
            for (name, view) in [("internal", fault), ("unreachable", nothing)] {
                let sentence = try #require(WalletLive.statusSentence(view, loc: en)?.text)
                let long = try await tree(hero(view), width: width, textScale: scale)
                let drawn = try status(long)
                print("MEASURE hero-reason w=\(Int(width)) x\(scale) \(name) chars=\(sentence.count) line=\(drawn.frame) short=\(line)")
                #expect(drawn.frame.height <= line.height + 0.5, "\(name) at \(Int(width)) ×\(scale): \(drawn.frame.height) vs \(line.height)")
                #expect(drawn.label == sentence)
                let detail = SettingsLive.withBalanceDetail(
                    view, display: .usd, on: SettingsFixtures.build(.st1, loc: en), loc: en
                ).balanceDetail
                #expect(detail.reason?.text == sentence, "the breakdown does not say the line's sentence")
                #expect(detail.reason?.tone == .warning)
            }
        }
        #expect(WalletLive.statusSentence(fault, loc: en)?.text == en.t("componentsUi.gas.reasonInternal"))
        #expect(WalletLive.statusSentence(nothing, loc: en)?.text == en.t("onboarding.common.networkBody"))
        // "Checking…" is no door, and a healthy wallet has no sentence.
        let round = try #require(BalanceCoreScene.zeroWallet())
        #expect(WalletLive.statusSentence(round.checking, loc: en) == nil)
        #expect(WalletLive.statusSentence(round.settled, loc: en) == nil)
        #expect(SettingsLive.withBalanceDetail(
            round.settled, display: .usd, on: SettingsFixtures.build(.st1, loc: en), loc: en
        ).balanceDetail.reason == nil)
        // The drawn board has none either.
        #expect(SettingsFixtures.build(.sr3, loc: en).balanceDetail.reason == nil)
    }
}
