//
//  BalanceHeroA11yTests.swift
//  VelaWalletTests
//
//  What VoiceOver hears on the wallet home's hero (issue 462). The hero's
//  "tap the figure to hide it" (spec 051) was a trait and a hint on the whole
//  balance stack — which is not one accessibility element, so the hint
//  reached every element inside and overrode their own: the new "↻ Updated"
//  control was announced "…, button, Hide balance", and double-tapping it
//  refreshed. And before any read had settled, the idle control was called
//  "Updating…" though nothing was running.
//
//  These read the tree SwiftUI hands an assistive client: the real
//  `WalletScreen`, hosted in a window, with the simulator's automation switch
//  on (what XCUITest turns on) so the tree is published.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct BalanceHeroA11yTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private struct Element {
        let label: String
        let hint: String
        let id: String
        let button: Bool
    }

    /// SwiftUI publishes its accessibility tree only when something assistive
    /// asks. libAccessibility's switches are how XCUITest asks.
    private static let automation: Void = {
        guard let library = dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW) else { return }
        for name in ["_AXSSetAutomationEnabled", "_AXSApplicationAccessibilitySetEnabled"] {
            if let symbol = dlsym(library, name) {
                typealias Switch = @convention(c) (Int32) -> Void
                unsafeBitCast(symbol, to: Switch.self)(1)
            }
        }
    }()

    /// The tree, once it has the hero's refresh control in it. Read again
    /// until it does (up to ~8 s): beside a thousand other tests on one main
    /// actor, a first read can come before the window's first commit. Each
    /// pass draws the window, which waits for the screen's update.
    private func elements(of view: some View) async throws -> [Element] {
        _ = Self.automation
        let host = UIHostingController(rootView: view.themed(.light))
        let window = HostedWindow.show(host, size: CGSize(width: 390, height: 844))
        defer { window.isHidden = true }
        var found: [Element] = []
        for _ in 0..<40 {
            host.view.layoutIfNeeded()
            _ = UIGraphicsImageRenderer(bounds: host.view.bounds).image { _ in
                host.view.drawHierarchy(in: host.view.bounds, afterScreenUpdates: true)
            }
            try await Task.sleep(for: .milliseconds(200))
            found = []
            collect(host.view as Any, depth: 0, into: &found)
            if found.contains(where: { $0.id == BalanceRefreshControl.testId }) { break }
        }
        return found
    }

    /// SwiftUI's nodes answer `accessibilityIdentifier` without declaring
    /// `UIAccessibilityIdentification`, so it is asked for by selector.
    private static func identifier(of object: NSObject) -> String {
        let getter = #selector(getter: UIAccessibilityIdentification.accessibilityIdentifier)
        guard object.responds(to: getter) else { return "" }
        return object.perform(getter)?.takeUnretainedValue() as? String ?? ""
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        if object.isAccessibilityElement {
            found.append(Element(
                label: object.accessibilityLabel ?? "",
                hint: object.accessibilityHint ?? "",
                id: Self.identifier(of: object),
                button: object.accessibilityTraits.contains(.button)
            ))
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

    private func home(_ state: MobileStateId) -> WalletScreen {
        WalletScreen(
            model: WalletFixtures.buildMobileState(state, loc: loc), loc: loc,
            onToggleBalance: {}, onRefreshNow: {}
        )
    }

    /// The refresh control says when, and what a double-tap does — never the
    /// figure's "Hide balance". The figure keeps its own switch.
    @Test func theRefreshControlIsNotAnnouncedAsTheFiguresSwitch() async throws {
        let model = WalletFixtures.buildMobileState(.h1, loc: loc)
        let refresh = try #require(model.balance.refresh)
        let tree = try await elements(of: home(.h1))

        let control = try #require(tree.first { $0.id == BalanceRefreshControl.testId },
                                   "the hero's refresh control is in the tree")
        #expect(control.hint != model.balance.a11yHide, "the figure's hint reached the refresh control")
        #expect(control.hint != model.balance.a11yShow)
        #expect(control.hint == refresh.named)
        #expect(control.label == refresh.updated)
        #expect(control.button)

        let figure = try #require(tree.first { $0.hint == model.balance.a11yHide },
                                  "the figure still says a tap hides it")
        #expect(figure.button)
        #expect(figure.id != BalanceRefreshControl.testId)
        #expect(tree.filter { $0.hint == model.balance.a11yHide }.count == 1,
                "only the figure is the hide switch")
    }

    /// Before any read has settled the control is the glyph alone, at rest:
    /// it is "Refresh balance", not "Updating…".
    @Test func anIdleControlNeverReadIsCalledRefreshNotUpdating() async throws {
        let never = WalletFixtures.refresh(loc: loc, read: false)
        #expect(never.updated == nil)
        #expect(BalanceRefreshControl.spoken(never) == loc.t("home.refreshBalance"))
        #expect(BalanceRefreshControl.spoken(never) != loc.t("home.updating"))
        #expect(!never.named.isEmpty && never.named != "home.refreshBalance", "the corpus has the key")

        let turning = WalletFixtures.refresh(loc: loc, read: false, refreshing: true)
        #expect(BalanceRefreshControl.spoken(turning) == loc.t("home.updating"), "only while it turns")

        let tree = try await elements(of: home(.h3))
        let control = try #require(tree.first { $0.id == BalanceRefreshControl.testId })
        #expect(control.label == loc.t("home.refreshBalance"))
        #expect(control.label != loc.t("home.updating"))
    }

    /// The live builder names the control the same way.
    @Test func theLiveModelCarriesTheRestingName() {
        let view = BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: 10, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        let model = WalletLive.refresh(view, loc: loc)
        #expect(model.named == loc.t("home.refreshBalance"))
        #expect(BalanceRefreshControl.spoken(model) == loc.t("home.refreshBalance"))
    }
}
