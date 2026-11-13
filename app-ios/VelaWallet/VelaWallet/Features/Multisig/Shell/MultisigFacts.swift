//
//  MultisigFacts.swift
//  VelaWallet
//
//  The app's facts as the multisig machines read them (the web's
//  `local-accounts.ts` and `machines.svelte.ts` facts): this device's Vela
//  accounts as `MultisigLocalAccount` — address, name, founding keys (the
//  owner proof and an approval are made with them) and how each approves
//  (the core's `multisigApprovalRoute`: in Vela, on its trusted page, or not
//  from here) — every network this device has and the space's, the service
//  in use, and the addresses the person knows (a review's look-alikes and
//  first sends). Read, never decided.
//

import Foundation
import VelaCore

@MainActor
enum MultisigFacts {

    static func read(accounts store: AccountStore, port: SendAccountPort, shelf: VelaStore) async -> MultisigHub.Facts {
        let records = await store.loadAccounts()
        let local = await localAccounts(records, port: port)
        var known: [[String: Any]] = []
        for contact in shelf.readList(VelaStore.Key.contacts) {
            guard let address = contact["address"] as? String else { continue }
            known.append(["address": address, "label": contactName(contact) ?? address])
        }
        for record in records {
            guard let address = record["address"] as? String else { continue }
            known.append(["address": address, "label": record["name"] as? String ?? address, "kind": "own"])
        }
        return MultisigHub.Facts(
            serviceUrl: MultisigServiceURL.current(),
            accounts: local,
            networks: networks(shelf),
            space: NetworkSpace.chainIds(store: shelf),
            known: known
        )
    }

    /// A stored contact's name as every screen says it (the core's
    /// `contact_display_name`, the web's `c.name ?? c.resolved_name`): what
    /// the person called them, else the name resolved for the address (a
    /// Vela name, ENS, …), an empty one counting as none. The store keeps
    /// contacts camelCased (`resolvedName`, ContactsWire.swift).
    static func contactName(_ contact: [String: Any]) -> String? {
        let named = (contact["name"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        return named ?? (contact["resolvedName"] as? String).flatMap { $0.isEmpty ? nil : $0 }
    }

    /// This device's Vela accounts, as the machines need them.
    static func localAccounts(_ records: [[String: Any]], port: SendAccountPort) async -> [[String: Any]] {
        var out: [[String: Any]] = []
        for record in records {
            guard let address = record["address"] as? String else { continue }
            let keys = await port.keys(of: address)
            let json = await port.accountJson(of: address)
            out.append([
                "address": address,
                "name": record["name"] as? String ?? "",
                "keys": keys.map { ["credential_id": $0.credentialId, "public_key_hex": $0.publicKeyHex] },
                "route": json.map(route(accountJson:)) ?? "vela",
            ])
        }
        return out
    }

    /// How an account approves (`vela` / `page` / `blocked`), the core's.
    static func route(accountJson: String) -> String {
        guard let answer = multisigApprovalRoute(accountJson: accountJson),
              let object = try? CoreJSON.object(answer)
        else { return "vela" }
        switch object["type"] as? String {
        case "page": return "page"
        case "blocked": return "blocked"
        default: return "vela"
        }
    }

    /// Every network this device has: the catalogue's and the person's own.
    static func networks(_ shelf: VelaStore) -> [Int] {
        var ids = ChainCatalog.chains.map(\.chainId)
        for network in shelf.readList(VelaStore.Key.customNetworks) {
            guard let chainId = (network["chainId"] as? NSNumber)?.intValue, !ids.contains(chainId) else { continue }
            ids.append(chainId)
        }
        return ids
    }
}
