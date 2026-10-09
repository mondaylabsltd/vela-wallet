//
//  SigningPagesExecutor.swift
//  VelaWallet
//
//  The only place the `signing_pages` core touches the outside world (spec
//  102, Settings → Signing pages): the pages this device trusts to show and
//  sign requests.
//
//  Everything goes back RAW — which addresses are pages a browser would sign
//  on, which are duplicates, and how the 071 "Trusted Signer page" is imported
//  are the core's calls. Both keys live under the `vela.` prefix that survives
//  sign-out: which pages a person trusts belongs to them and the device, not
//  to one account.
//

import Foundation

final class SigningPagesExecutor {

    static let operations = ["read_stored", "write_pages"]

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        // Absent ALWAYS means "never saved": the core reads `null` as such.
        case "read_stored":
            return CoreJSON.string([
                "type": "stored",
                "pages_json": store.rawValue(VelaStore.Key.signingPages) ?? NSNull(),
                "legacy_url": store.readString(VelaStore.Key.trustedSignerUrl) ?? NSNull(),
            ])
        // The list as the core wants it stored; the official page is never in
        // it. `remove_legacy_url`: the 071 page was just imported, and leaving
        // the old key would import it again after the person removed it.
        case "write_pages":
            store.writeList(VelaStore.Key.signingPages, operation["pages"] as? [[String: Any]] ?? [])
            if operation["remove_legacy_url"] as? Bool == true {
                store.remove(VelaStore.Key.trustedSignerUrl)
            }
            return CoreJSON.string(["type": "written"])
        default:
            print("[vela-wallet] signing_pages: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "stored", "pages_json": NSNull(), "legacy_url": NSNull()])
        }
    }
}
