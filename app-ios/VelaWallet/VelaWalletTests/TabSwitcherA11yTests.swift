//
//  TabSwitcherA11yTests.swift
//  VelaWalletTests
//
//  What VoiceOver hears in the tab switcher (mock E5). The card that is
//  "this tab" carried only an accent border — nothing an assistive client
//  can read — and a card whose preview is a snapshot or the start page's
//  sail was a button with no words at all (device pass 2026-10-09).
//
//  These read the tree SwiftUI hands an assistive client, as
//  `WalletTabBarA11yTests` and `BalanceHeroA11yTests` do — the real
//  switcher, hosted in a window, with the simulator's automation switch on.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct TabSwitcherA11yTests {

    private struct Element {
        let object: NSObject
        let label: String
        let traits: UIAccessibilityTraits
    }

    /// What the cards' taps reached.
    private final class Opened {
        var ids: [String] = []
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

    private let copy = TabsScreenCopy(
        title: "标签页", done: "完成", newTab: "新建标签页", closeAll: "关闭全部标签页",
        close: "关闭标签页", closeOthers: "关闭其他标签页", closeRight: "关闭右侧标签页"
    )

    /// A dApp's tab not yet photographed (its preview is its host) and the
    /// start page's own tab (its preview is the sail, which says nothing) —
    /// the start page is "this tab".
    private let tabs = [
        TabModel(id: "t1", title: "Uniswap", site: nil, selected: false, startPage: false),
        TabModel(id: "t2", title: "起始页", site: nil, selected: true, startPage: true),
    ]

    /// The tree, once both cards are in it — read again until they are (up to
    /// ~8 s): a first read can come before the window's first commit.
    private func elements(of view: some View) async throws -> (tree: [Element], window: UIWindow) {
        _ = Self.automation
        let host = UIHostingController(rootView: view.themed(.light))
        let window = HostedWindow.show(host, size: CGSize(width: 390, height: 844))
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
            if tabs.map(\.title).allSatisfy(labels.contains) { break }
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

    /// The card in front is a button named by its title that VoiceOver says
    /// is selected; the other card is a button named by its title, not
    /// selected; each title is said once, by its card.
    @Test func theCardInFrontIsSaidToBeSelected() async throws {
        let opened = Opened()
        let screen = ExploreTabsScreen(tabs: tabs, copy: copy, onOpen: { opened.ids.append($0) })
        let (tree, window) = try await elements(of: screen)
        defer { window.isHidden = true }

        let front = try #require(tree.first { $0.label == "起始页" }, "the start page's card has its title, though its preview has no words")
        #expect(front.traits.contains(.button))
        #expect(front.traits.contains(.selected), "the card in front is not announced as selected")
        #expect(front.object.accessibilityActivate(), "VoiceOver's double-tap does nothing on the card")
        #expect(opened.ids == ["t2"])

        let other = try #require(tree.first { $0.label == "Uniswap" })
        #expect(other.traits.contains(.button))
        #expect(!other.traits.contains(.selected), "a card that is not in front is announced as selected")

        for title in tabs.map(\.title) {
            #expect(tree.filter { $0.label == title }.count == 1, "\(title) is said more than once")
        }
        // The ✕ keeps its own name and is never the selected one.
        let closes = tree.filter { $0.label == copy.close }
        #expect(closes.count == tabs.count)
        #expect(closes.allSatisfy { !$0.traits.contains(.selected) })
    }
}
