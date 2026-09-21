//
//  StorageCatalogTests.swift
//  VelaWalletTests
//
//  The storage page from the core's catalog (spec 072 T034).
//
//  Which row a key is, what "clear all caches" takes and how bytes are said
//  are `vela_core::storage_catalog`'s. This client's own copy had filed
//  `vela.balanceHidden` — a preference — as a balance cache, so clearing the
//  caches un-hid somebody's balance.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct StorageCatalogTests {

    private func fresh() -> (UserDefaults, VelaStore) {
        let defaults = UserDefaults(suiteName: "vela.tests.storage.\(UUID().uuidString)")!
        return (defaults, VelaStore(defaults: defaults))
    }

    /// "Clear all caches" takes the caches — and leaves a hidden balance
    /// hidden, the person's data, and their connections.
    @Test func clearingCachesKeepsAHiddenBalanceHidden() {
        let (defaults, store) = fresh()
        store.writeString("vela.balanceHidden", "true")
        store.writeString(VelaStore.Key.balanceCache, #"{"0x1":{"usd":1}}"#)
        store.writeString(VelaStore.Key.fiatRates, #"{"CNY":7.1}"#)
        store.writeString("vela.tokenMeta.100:0xabc", #"{"symbol":"X"}"#)
        store.writeString("recipient_id:0xabc", #"{"name":"alice"}"#)
        store.writeList(VelaStore.Key.contacts, [["address": "0x1"]])
        store.writeString("vela.perm.https://app.uniswap.org", #"{"accounts":["0x1"]}"#)

        let removed = Set(DeviceStorage.clearCaches(store))

        #expect(defaults.string(forKey: "vela.balanceHidden") == "true", "clearing caches un-hid the balance")
        #expect(removed == [
            VelaStore.Key.balanceCache, VelaStore.Key.fiatRates,
            "vela.tokenMeta.100:0xabc", "recipient_id:0xabc",
        ])
        #expect(!store.readList(VelaStore.Key.contacts).isEmpty)
        #expect(defaults.string(forKey: "vela.perm.https://app.uniswap.org") != nil)
    }

    /// The "custom" row is tokens, networks and their overrides — not
    /// somebody's RPC provider keys or service endpoints, which have pages of
    /// their own.
    @Test func theCustomRowNeverTakesProviderKeys() {
        let (defaults, store) = fresh()
        store.writeList(VelaStore.Key.customTokens, [["address": "0x1"]])
        store.writeList(VelaStore.Key.customNetworks, [["id": "custom-196"]])
        store.writeList(VelaStore.Key.networkConfig, [["chainId": 100]])
        store.writeObject(VelaStore.Key.rpcProviders, ["alchemy": "k3y"])
        store.writeObject(VelaStore.Key.serviceEndpoints, ["passkeyIndexURL": "https://x.example"])

        DeviceStorage.clear(store, item: "custom")

        #expect(defaults.object(forKey: VelaStore.Key.customTokens) == nil)
        #expect(defaults.object(forKey: VelaStore.Key.customNetworks) == nil)
        #expect(defaults.object(forKey: VelaStore.Key.networkConfig) == nil)
        #expect(defaults.object(forKey: VelaStore.Key.rpcProviders) != nil)
        #expect(defaults.object(forKey: VelaStore.Key.serviceEndpoints) != nil)
    }

    /// The rows are the core's, in its order; the name cache is weighed under
    /// the scan row, and the wallet and its preferences count as the person's
    /// data without being anybody's row.
    @Test func theMeasureFollowsTheCatalog() {
        let (_, store) = fresh()
        store.writeString("recipient_id:0xabc", #"{"name":"alice"}"#)
        store.writeString("vela.balanceHidden", "true")
        store.writeList(VelaStore.Key.accounts, [["id": "a"]])

        let report = DeviceStorage.measure(store)

        #expect(report.items.map(\.id)
                == ["transactions", "contacts", "custom", "browsing", "balances", "rates", "scan", "dapps"])
        #expect(report.item("scan")?.keys == ["recipient_id:0xabc"])
        #expect(report.item("balances")?.bytes == 0, "a preference was weighed as a balance cache")
        #expect(report.unnamedBytes > 0)
        #expect(report.totalBytes == report.items.reduce(0) { $0 + $1.bytes } + report.unnamedBytes)
        #expect(report.bytes(of: .user) >= report.unnamedBytes)
    }

    /// Bytes in 1024s, the number in the person's own format.
    @Test func bytesAreSaidIn1024sInThePersonsNumbers() {
        #expect(DeviceStorage.sizeText(512, format: .commaDot) == "512 B")
        #expect(DeviceStorage.sizeText(1536, format: .commaDot) == "1.5 KB")
        #expect(DeviceStorage.sizeText(1536, format: .dotComma) == "1,5 KB")
        #expect(DeviceStorage.sizeText(3 * 1024 * 1024, format: .commaDot) == "3 MB")
        // 1,000,000 bytes is NOT a megabyte in 1024s.
        #expect(DeviceStorage.sizeText(1_000_000, format: .commaDot) == "976.6 KB")
    }
}
