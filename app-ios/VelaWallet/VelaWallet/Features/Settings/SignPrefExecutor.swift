//
//  SignPrefExecutor.swift
//  VelaWallet
//
//  The only place the `sign_pref` core touches the outside world (spec 071):
//  which Trusted Signer page this device opens. The fee tier's two sentences,
//  against the same store, for one key.
//
//  The value goes back RAW — whether an address is a page it may open is the
//  core's call, and a stored address the core would refuse opens the official
//  page instead. `vela.trustedSignerUrl` survives sign-out: which page a person
//  checks their signatures on belongs to them and the device, not to one
//  account.
//

import Foundation

final class SignPrefExecutor {

    static let operations = ["read_stored", "write_signer_url"]

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        // Absent ALWAYS means "never chose".
        case "read_stored":
            return CoreJSON.string([
                "type": "stored",
                "signer_url": store.readString(VelaStore.Key.trustedSignerUrl) ?? NSNull(),
            ])
        // `null` is the official page: the key goes, rather than pinning a
        // copy of a default that may move.
        case "write_signer_url":
            store.writeString(VelaStore.Key.trustedSignerUrl, operation["url"] as? String)
            return CoreJSON.string(["type": "written"])
        default:
            print("[vela-wallet] sign_pref: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "stored", "signer_url": NSNull()])
        }
    }
}
