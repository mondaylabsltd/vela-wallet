//
//  RecipientIdentity.swift
//  VelaWallet
//
//  Who an address belongs to, if anybody can say.
//
//  Ported from `app-web/vela-wallet/src/lib/services/recipient-identity.ts`.
//  The waterfall, in order:
//
//    1. the **passkey index** — a Vela wallet's own name, by wallet ref;
//    2. **name services**, read on-chain with no third-party API:
//       `.bnb` (BSC) · `.arb` (Arbitrum) · `.g` (Gravity) · Basenames (Base) ·
//       ENS (Ethereum).
//
//  Adding a service is one row in `services` — anything following the ENS
//  registry pattern (`registry.resolver(node)` → `resolver.name(node)`) works
//  as it stands.
//
//  ## Two callers, one resolver
//
//  `contacts::resolve_identity` and the activity feed's alias arm both want the
//  same answer for the same address. Two lookups would mean two caches, two
//  ban interactions with the pool, and — the part that shows — two different
//  names for one counterparty on two screens.
//
//  ## Only positive answers are cached
//
//  `contacts.rs` invariant ⑦ and web's own comment. A name that could not be
//  found today is not a fact about the address; caching the absence would keep
//  a person nameless for 24 hours after they registered one.
//

import Foundation
import VelaCore

@MainActor
final class RecipientIdentity {

    struct Identity: Equatable {
        let name: String
        /// The label the shell shows: `passkey`, `ENS`, `.bnb`, `Basename`…
        let source: String
    }

    /// One ENS-compatible registry.
    private struct Service {
        let label: String
        let chainId: Int
        let registry: String
        /// ENSIP-19 chains answer the reverse node from a registrar rather
        /// than from `namehash("<addr>.addr.reverse")`.
        var reverseRegistrar: String?
    }

    private static let services: [Service] = [
        // SPACE ID, one SID registry per chain.
        Service(label: ".bnb", chainId: 56,
                registry: "0x08CEd32a7f3eeC915Ba84415e9C07a7286977956"),
        Service(label: ".arb", chainId: 42_161,
                registry: "0x4a067EE58e73ac5E4a43722E008DFdf65B2bF348"),
        Service(label: ".g", chainId: 1_625,
                registry: "0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17"),
        Service(label: "Basename", chainId: 8_453,
                registry: "0xb94704422c2a1e396835a571837aa5ae53285a95",
                reverseRegistrar: "0x79ea96012eea67a83431f1701b3dff7e37f9e282"),
        Service(label: "ENS", chainId: 1,
                registry: "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"),
    ]

    /// `recipient_id:<address>` — web's key, so a name resolved in one client
    /// is not re-resolved in the other.
    private static let cachePrefix = "recipient_id:"
    private static let cacheTTLMs: Double = 24 * 60 * 60 * 1000

    private let store: VelaStore
    private let pool: RpcPool
    private let accounts: AccountStore
    private var memo: [String: Identity] = [:]

    init(store: VelaStore, pool: RpcPool, accounts: AccountStore) {
        self.store = store
        self.pool = pool
        self.accounts = accounts
    }

    /// A display identity for `address`, or `nil` when nobody could name it.
    func resolve(_ address: String) async -> Identity? {
        let key = address.lowercased()
        guard key.count == 42, key.hasPrefix("0x") else { return nil }
        // The zero address is a mint/burn counterparty (EIP-7708 native
        // events), not a recipient — and asking the index about it is a 404
        // nobody needs.
        guard !Ens.isZeroAddress(address) else { return nil }

        if let remembered = memo[key] { return remembered }
        if let cached = readCache(key) {
            memo[key] = cached
            return cached
        }

        if let name = await passkeyName(address), !name.isEmpty {
            return remember(key, Identity(name: name, source: "passkey"))
        }

        // Every service is asked in parallel and the first match **by
        // priority** wins — not the first to answer. A race would give the
        // same address different names on different days.
        let answers = await withTaskGroup(of: (Int, String?).self) { group in
            for (index, service) in Self.services.enumerated() {
                group.addTask { [weak self] in
                    (index, await self?.reverseResolve(address, service: service) ?? nil)
                }
            }
            var collected: [Int: String] = [:]
            for await (index, name) in group {
                if let name, !name.isEmpty { collected[index] = name }
            }
            return collected
        }
        for (index, service) in Self.services.enumerated() {
            guard let name = answers[index] else { continue }
            return remember(key, Identity(name: name, source: service.label))
        }
        return nil
    }

    // MARK: - The rungs

    /// The passkey index's name for a Vela wallet — by ADDRESS, which the v2
    /// index cannot be asked directly, so the core walks chain → founding key
    /// → units (`RegistryNameLookup`, issue 191) and this only carries the
    /// requests. The index URL is read per lookup: a settings edit reaches the
    /// next one.
    private func passkeyName(_ address: String) async -> String? {
        await registryNames.name(for: address)
    }

    private lazy var registryNames = RegistryNameLookup(
        store: store,
        ethCall: { [pool] chainId, to, data in
            let outcome = await pool.call(
                chainId: chainId, method: "eth_call",
                params: [["to": to, "data": data], "latest"]
            )
            // The RAW result, a bare `0x` included: a chain without the signer
            // contract is an answer ("not here"), not a silence.
            guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x")
            else { return nil }
            return hex
        },
        indexGet: { [accounts] path in
            let client = RegistryClient()
            if let base = await accounts.loadRegistryURL(), !base.isEmpty {
                await client.setBaseURL(base)
            }
            guard let got = await client.rawGet(path) else { return .failed }
            if got.status == 404 { return .notFound }
            return (200..<300).contains(got.status) ? .ok(got.body) : .failed
        }
    )

    /// `registry.resolver(node)` → `resolver.name(node)`, the ENS reverse
    /// pattern, entirely on-chain.
    private func reverseResolve(_ address: String, service: Service) async -> String? {
        guard let resolverSelector = Multicall.selector("resolver(bytes32)"),
              let nameSelector = Multicall.selector("name(bytes32)")
        else { return nil }

        let node: Data
        if let registrar = service.reverseRegistrar {
            // ENSIP-19: the chain's own reverse node, from the registrar.
            guard let nodeSelector = Multicall.selector("node(address)"),
                  let padded = try? abiEncodeAddress(addressHex: address),
                  let answer = await call(service.chainId, to: registrar,
                                          data: nodeSelector + padded),
                  answer.count >= 32
            else { return nil }
            node = answer.prefix(32)
        } else {
            node = Ens.reverseNode(address)
        }

        guard let resolverWord = await call(service.chainId, to: service.registry,
                                            data: resolverSelector + node),
              let resolver = Ens.addressWord(resolverWord), !Ens.isZeroAddress(resolver),
              let nameWord = await call(service.chainId, to: resolver,
                                        data: nameSelector + node)
        else { return nil }

        let name = TokenMetadata.decodeString(nameWord)
        return name.isEmpty ? nil : name
    }

    /// One `eth_call`, routed. `nil` covers every unresolved path — an RPC
    /// failure, an empty answer, a chain nobody serves — because the caller
    /// treats them all as "this service does not know them".
    private func call(_ chainId: Int, to: String, data: Data) async -> Data? {
        let outcome = await pool.call(
            chainId: chainId, method: "eth_call",
            params: [["to": to, "data": "0x" + data.hexString], "latest"]
        )
        guard case .ok(let value) = outcome, let hex = value as? String, hex != "0x",
              let bytes = Data(hexString: hex), !bytes.isEmpty
        else { return nil }
        return bytes
    }

    // MARK: - The cache

    private func remember(_ key: String, _ identity: Identity) -> Identity {
        memo[key] = identity
        store.writeObject(Self.cachePrefix + key, [
            "identity": ["name": identity.name, "source": identity.source],
            "cachedAt": Date().timeIntervalSince1970 * 1000,
        ])
        return identity
    }

    private func readCache(_ key: String) -> Identity? {
        let stored = store.readObject(Self.cachePrefix + key)
        guard let at = (stored["cachedAt"] as? NSNumber)?.doubleValue,
              Date().timeIntervalSince1970 * 1000 - at <= Self.cacheTTLMs,
              let identity = stored["identity"] as? [String: Any],
              let name = identity["name"] as? String, !name.isEmpty
        else { return nil }
        return Identity(name: name, source: identity["source"] as? String ?? "")
    }
}
