//
//  ConnectedSitesSettingsTests.swift
//  VelaWalletTests
//
//  Settings → Storage → Connections (spec 070 FR-017): one row per connected
//  site, from the browser core's `sites`, the web's `withLiveConnections`.
//

import Foundation
import Testing
@testable import VelaWallet

private let a1 = "0x1111111111111111111111111111111111111111"
private let a2 = "0x2222222222222222222222222222222222222222"
private let t0: Double = 1_757_000_000_000

@MainActor
struct ConnectedSitesSettingsTests {

    private func loc() -> Loc { Loc(overrideTag: "en", preferredLanguages: []) }

    /// One row per connected site, like the web: its host, the account it
    /// sees, and a singular Disconnect — keyed by the origin a tap revokes.
    @Test func eachConnectedSiteIsARow() {
        let base = SettingsFixtures.build(.st1, loc: loc())
        let sites = [
            DbrSiteViewWire(origin: "https://app.uniswap.org", address: a1, chainId: 1, grantedAtMs: t0 + 1),
            DbrSiteViewWire(origin: "http://127.0.0.1:8137", address: a2, chainId: 100, grantedAtMs: t0),
        ]
        let live = SettingsLive.withConnections(sites, on: base, loc: loc())
        let label = loc().t(I18nKeys.SettingsUi.storageConnections)
        let group = live.storage.groups.first { $0.label == label }
        #expect(group?.items.map(\.id) == ["https://app.uniswap.org", "http://127.0.0.1:8137"])
        #expect(group?.items.map(\.label) == ["app.uniswap.org", "127.0.0.1:8137"])
        #expect(group?.items.first?.meta == AddressText.short(a1))
        #expect(group?.items.allSatisfy { $0.action == loc().t("explore.disconnect") && $0.destructive } == true)
        #expect(group?.items.allSatisfy { SettingsLive.isConnectionRow($0.id) } == true)
        // The other groups are untouched.
        #expect(live.storage.groups.count == base.storage.groups.count)
    }

    /// Nothing connected: the measured "dApp permissions" row stays.
    @Test func noSitesKeepsTheMeasuredRow() {
        let base = SettingsFixtures.build(.st1, loc: loc())
        let live = SettingsLive.withConnections([], on: base, loc: loc())
        let label = loc().t(I18nKeys.SettingsUi.storageConnections)
        #expect(live.storage.groups.first { $0.label == label }?.items.map(\.id) == ["dapps"])
        #expect(!SettingsLive.isConnectionRow("dapps"))
    }
}
