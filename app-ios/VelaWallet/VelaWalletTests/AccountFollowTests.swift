//
//  AccountFollowTests.swift
//  VelaWalletTests
//
//  A switch of account re-points every account-scoped store (iPhone pass
//  2026-10-09): picked from the home's switcher, the header took the new name
//  and the home kept the PREVIOUS account's balance, assets and activity, a
//  refresh or not. The home opened its stores only when it appeared, and the
//  switcher told the address book and asked for a refresh — of the account
//  the balances were still pointed at.
//
//  RootView is the app's root and is not hosted by a unit test, so these read
//  its source, the way Android's `SettingsPagesTest` keeps its wiring: the
//  session's address is followed at the ROOT, whatever moved it, and the
//  follow points the balances, the feed and the book at it.
//

import Foundation
import Testing

struct AccountFollowTests {

    /// RootView.swift, comments dropped, one trimmed line per entry.
    private func rootView() throws -> [String] {
        let source = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()   // VelaWalletTests
                .deletingLastPathComponent()   // VelaWallet (project dir)
                .appendingPathComponent("VelaWallet/App/RootView.swift"),
            encoding: .utf8
        )
        return source.components(separatedBy: "\n")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.hasPrefix("//") && !$0.isEmpty }
    }

    /// The lines of `func name(` up to the next `func`.
    private func body(of name: String, in lines: [String]) throws -> [String] {
        let start = try #require(lines.firstIndex { $0.contains("func \(name)(") }, "no \(name)")
        let rest = lines[(start + 1)...]
        let end = rest.firstIndex { $0.contains("func ") } ?? lines.endIndex
        return Array(lines[start..<end])
    }

    @Test func theAccountInFrontIsFollowedAtTheRootWhateverMovedIt() throws {
        let lines = try rootView()
        let watch = try #require(
            lines.firstIndex(of: ".onChange(of: session.view.address) { _, address in"),
            "nothing follows the session's address"
        )
        #expect(lines[watch + 1] == "followAccountInFront(address)")
        // At the root, beside the route guard — not inside one screen's body,
        // where it would only run while that screen is up.
        let guardLine = try #require(lines.firstIndex(of: ".onChange(of: session.view.allowedRoute) { _, route in"))
        #expect(watch > guardLine && watch - guardLine < 12, "the follow moved away from the root's other session watchers")
    }

    @Test func theFollowPointsTheBalancesTheFeedAndTheBookAtTheAccount() throws {
        let follow = try body(of: "followAccountInFront", in: rootView())
        #expect(follow.contains("guard !address.isEmpty else { return }"))
        #expect(follow.contains("wallet.open(address: address)"), "the balances and assets stay on the previous account")
        #expect(follow.contains { $0.hasPrefix("activity.open(address: address") }, "the activity stays on the previous account")
        #expect(follow.contains("contacts.open(myAddress: address)"))
    }

    /// The switcher no longer asks for a refresh of the account the stores
    /// are still pointed at, nor re-points anything itself.
    @Test func theSwitcherLeavesTheStoresToTheFollow() throws {
        let switcher = try body(of: "switchToAccount", in: rootView())
        #expect(switcher.contains("session.switchAccount(index: index)"))
        #expect(!switcher.contains { $0.hasPrefix("wallet.refresh(") })
        #expect(!switcher.contains { $0.hasPrefix("contacts.open(") })
    }
}
