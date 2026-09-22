//
//  SignPrefExecutor.swift
//  VelaWallet
//
//  The only place the `sign_pref` core touches the outside world (spec 071):
//  how this device signs by default, and which Clear Signer page it opens.
//  The fee tier's two sentences, against the same store, for two keys.
//
//  Both values go back RAW — whether a string is a method this build offers,
//  and whether an address is a page it may open, are the core's calls. An
//  unknown method reads as `auto` without being rewritten; a stored address
//  the core would refuse opens the official page instead. `vela.signMethod`
//  and `vela.clearSignerUrl` survive sign-out: how a person signs belongs to
//  them and the device, not to one account.
//

import Foundation

final class SignPrefExecutor {

    static let operations = ["read_stored", "write_method", "write_signer_url", "write_relay_url"]

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
                "method": store.readString(VelaStore.Key.signMethod) ?? NSNull(),
                "signer_url": store.readString(VelaStore.Key.clearSignerUrl) ?? NSNull(),
                "relay_url": store.readString(VelaStore.Key.clearSignerRelay) ?? NSNull(),
            ])
        // Best effort: the committed choice stays on screen either way.
        case "write_method":
            store.writeString(VelaStore.Key.signMethod, operation["method"] as? String)
            return CoreJSON.string(["type": "written"])
        // `null` is the official page: the key goes, rather than pinning a
        // copy of a default that may move.
        case "write_signer_url":
            store.writeString(VelaStore.Key.clearSignerUrl, operation["url"] as? String)
            return CoreJSON.string(["type": "written"])
        // Spec 075: the relay, under the same rule — `null` is the official
        // one, and the key goes rather than pinning a copy of a default that
        // may move.
        case "write_relay_url":
            store.writeString(VelaStore.Key.clearSignerRelay, operation["url"] as? String)
            return CoreJSON.string(["type": "written"])
        default:
            print("[vela-wallet] sign_pref: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string([
                "type": "stored", "method": NSNull(), "signer_url": NSNull(), "relay_url": NSNull(),
            ])
        }
    }
}
