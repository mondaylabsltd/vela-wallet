//
//  WalletTabBarA11yTests.swift
//  VelaWalletTests
//
//  What VoiceOver can do with the app's tab bar (DESIGN N). Under a page the
//  bar is the way home: 探索 again — the tab already selected — returns to
//  the Explore home with the page kept. A selected tab VoiceOver announced
//  and could not activate (a "selected" that is not a button, or one marked
//  not enabled) would leave a VoiceOver user on the page with only ‹.
//
//  These read the tree SwiftUI hands an assistive client, as
//  `BalanceHeroA11yTests` do — the real bar, hosted in a window, with the
//  simulator's automation switch on — and activate the element the way a
//  VoiceOver double-tap does (`accessibilityActivate`).
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct WalletTabBarA11yTests {

    private struct Element {
        let object: NSObject
        let label: String
        let traits: UIAccessibilityTraits
    }

    /// What the bar's taps reached.
    private final class Picks {
        var tabs: [WalletTab] = []
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

    private let names = TabsModel(wallet: "钱包", contacts: "通讯录", explore: "探索", settings: "设置")

    /// The tree, once all four tabs are in it — read again until they are
    /// (up to ~8 s): a first read can come before the window's first commit.
    private func elements(of view: some View) async throws -> (tree: [Element], window: UIWindow) {
        _ = Self.automation
        let host = UIHostingController(rootView: view.themed(.light))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = host
        window.makeKeyAndVisible()
        var found: [Element] = []
        for _ in 0..<40 {
            host.view.layoutIfNeeded()
            _ = UIGraphicsImageRenderer(bounds: host.view.bounds).image { _ in
                host.view.drawHierarchy(in: host.view.bounds, afterScreenUpdates: true)
            }
            try await Task.sleep(for: .milliseconds(200))
            found = []
            collect(host.view as Any, depth: 0, into: &found)
            let labels = Set(found.map(\.label))
            if [names.wallet, names.contacts, names.explore, names.settings].allSatisfy(labels.contains) { break }
        }
        return (found, window)
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        if object.isAccessibilityElement {
            found.append(Element(
                object: object, label: object.accessibilityLabel ?? "", traits: object.accessibilityTraits
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

    /// 探索, selected, is a button VoiceOver says is selected AND can press:
    /// its double-tap reaches the bar's owner, which takes 探索 again as the
    /// way home from a page (`ExploreEntry.reselect`).
    @Test func theSelectedExploreTabIsAButtonVoiceOverCanPress() async throws {
        let picks = Picks()
        let bar = WalletTabBar(tabs: names, selected: .explore, onSelect: { picks.tabs.append($0) })
        let (tree, window) = try await elements(of: bar)
        defer { window.isHidden = true }

        let explore = try #require(tree.first { $0.label == names.explore }, "探索 is in the tree")
        #expect(explore.traits.contains(.button), "the selected tab is not announced as a button")
        #expect(explore.traits.contains(.selected), "the selected tab is not announced as selected")
        #expect(!explore.traits.contains(.notEnabled), "the selected tab is announced as dimmed")
        #expect(explore.object.accessibilityActivate(), "VoiceOver's double-tap does nothing on the selected tab")
        #expect(picks.tabs == [.explore])

        // The others are buttons too, and not selected.
        for name in [names.wallet, names.contacts, names.settings] {
            let tab = try #require(tree.first { $0.label == name })
            #expect(tab.traits.contains(.button))
            #expect(!tab.traits.contains(.selected))
        }
    }
}
