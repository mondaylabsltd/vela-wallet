//
//  IdentityNetworksTests.swift
//  VelaWalletTests
//
//  A name service is asked only when its chain is one of the wallet's
//  networks (device walk 2026-10-11: a report listed chain 1625 — where the
//  `.g` names live, and not a network of the wallet's — as unreachable; the
//  pool found public nodes for it in the chain index, and every lookup sent
//  them real requests).
//
//  The REAL resolver over a recording transport: every call it would route
//  is written down with its chain, and answered from a script — a reverse
//  record on Ethereum, one on chain 1625, nothing anywhere else.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct IdentityNetworksTests {
    private static let who = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
    private static let ensRegistry = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"
    private static let gRegistry = "0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17"
    private static let resolver = "0x" + String(repeating: "ab", count: 20)
    private static let gravity = 1_625

    /// What the resolver asked, and of which chain.
    @MainActor
    final class Recorder {
        struct Call: Equatable {
            let chainId: Int
            let to: String
        }

        private(set) var calls: [Call] = []
        /// The reverse name each chain's registry gives; the rest have none.
        var names: [Int: String] = [:]

        func answer(_ chainId: Int, _ method: String, _ params: [Any]) async -> RpcOutcome {
            let request = params.first as? [String: Any] ?? [:]
            let to = (request["to"] as? String ?? "").lowercased()
            let data = request["data"] as? String ?? ""
            calls.append(Call(chainId: chainId, to: to))
            guard method == "eth_call", let name = names[chainId] else { return .failed(rateLimited: false) }
            let registries = [IdentityNetworksTests.ensRegistry, IdentityNetworksTests.gRegistry].map { $0.lowercased() }
            // `resolver(bytes32)` on the registry: where the reverse record lives.
            if registries.contains(to), data.hasPrefix("0x0178b8bf") {
                return .ok("0x" + String(repeating: "0", count: 24) + IdentityNetworksTests.resolver.dropFirst(2))
            }
            // `name(bytes32)` on that resolver: the claimed name, ABI-encoded.
            if to == IdentityNetworksTests.resolver, data.hasPrefix("0x691f3431") {
                let bytes = Array(name.utf8)
                let padded = bytes + [UInt8](repeating: 0, count: (32 - bytes.count % 32) % 32)
                func word(_ value: Int) -> String { String(repeating: "0", count: 62) + String(format: "%02x", value) }
                return .ok("0x" + word(32) + word(bytes.count) + padded.map { String(format: "%02x", $0) }.joined())
            }
            return .failed(rateLimited: false)
        }

        func chains() -> Set<Int> { Set(calls.map(\.chainId)) }
    }

    /// The resolver over `recorder`, its forward check scripted to agree with
    /// whatever name was claimed — the walk under test is which chains are
    /// asked, not `name_verify`'s (the core's own suite).
    private func resolver(_ recorder: Recorder, custom: [[String: Any]] = []) -> (VelaStore, RecipientIdentity) {
        let defaults = UserDefaults(suiteName: "identity-networks-\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        if !custom.isEmpty { store.writeList(VelaStore.Key.customNetworks, custom) }
        let identity = RecipientIdentity(
            store: store, pool: RpcPool(store: store, accounts: accounts, offline: true), accounts: accounts,
            verifyStep: { _, _, _, name, _ in
                CoreJSON.string(["type": "done", "forward_state": "verified", "name": name])
            },
            chainCall: { chainId, method, params in await recorder.answer(chainId, method, params) }
        )
        return (store, identity)
    }

    /// The wallet's networks are the built-ins and the ones the person added.
    @Test func theWalletsNetworksAreTheBuiltInsAndThePersonsOwn() {
        let recorder = Recorder()
        let (store, _) = resolver(recorder)
        let builtin = RecipientIdentity.walletChains(store: store)
        #expect(builtin == Set(ChainCatalog.chains.map(\.chainId)))
        #expect(builtin.isSuperset(of: [1, 56, 100, 8453, 42_161]))
        #expect(!builtin.contains(Self.gravity), "chain 1625 is not a network this wallet ships")

        store.writeList(VelaStore.Key.customNetworks, [["chainId": Self.gravity, "rpcURL": "https://rpc.example"]])
        #expect(RecipientIdentity.walletChains(store: store).contains(Self.gravity))
    }

    /// Chain 1625 is not one of the wallet's networks: a lookup makes NO call
    /// for it — and still resolves through the services that are.
    @Test func aServiceOnAChainTheWalletDoesNotHaveIsNeverAsked() async {
        let recorder = Recorder()
        // Both would answer: `.g` outranks ENS, so being skipped is visible.
        recorder.names = [Self.gravity: "alice.g", 1: "alice.eth"]
        let (_, identity) = resolver(recorder)

        let found = await identity.resolve(Self.who)

        #expect(!recorder.chains().contains(Self.gravity),
                "chain 1625 was asked \(recorder.calls.filter { $0.chainId == Self.gravity }.count) times")
        #expect(!recorder.calls.contains { $0.to == Self.gRegistry.lowercased() })
        #expect(found == RecipientIdentity.Identity(name: "alice.eth", source: "ENS"),
                "resolution did not fall through to the next service: \(String(describing: found))")
        // The others WERE asked, on their own chains.
        #expect(recorder.chains().isSuperset(of: [1, 56, 8453, 42_161]))
        #expect(recorder.chains().isSubset(of: RecipientIdentity.walletChains(store: VelaStore(
            defaults: UserDefaults(suiteName: "identity-networks-\(UUID().uuidString)")!
        ))), "a chain the wallet does not have was asked: \(recorder.chains())")
    }

    /// The person adds that network: its service is asked, and — being ahead
    /// of ENS — it is the one that names the address.
    @Test func addedAsACustomNetworkTheServiceIsAsked() async {
        let recorder = Recorder()
        recorder.names = [Self.gravity: "alice.g", 1: "alice.eth"]
        let (_, identity) = resolver(recorder, custom: [[
            "id": "chain-1625", "chainId": Self.gravity, "displayName": "Gravity",
            "rpcURL": "https://rpc.example", "nativeCurrencySymbol": "G",
        ]])

        let found = await identity.resolve(Self.who)

        #expect(recorder.calls.contains { $0.chainId == Self.gravity && $0.to == Self.gRegistry.lowercased() },
                "with chain 1625 among the wallet's networks, `.g` was not asked")
        #expect(found == RecipientIdentity.Identity(name: "alice.g", source: ".g"))
    }

    /// A network added AFTER the resolver was made counts for the next
    /// lookup: the list is read each time.
    @Test func aNetworkAddedLaterCountsForTheNextLookup() async {
        let recorder = Recorder()
        let (store, identity) = resolver(recorder)
        _ = await identity.resolve(Self.who)
        #expect(!recorder.chains().contains(Self.gravity))

        store.writeList(VelaStore.Key.customNetworks, [["chainId": Self.gravity, "rpcURL": "https://rpc.example"]])
        _ = await identity.resolve("0x" + String(repeating: "12", count: 20))
        #expect(recorder.chains().contains(Self.gravity))
    }
}
