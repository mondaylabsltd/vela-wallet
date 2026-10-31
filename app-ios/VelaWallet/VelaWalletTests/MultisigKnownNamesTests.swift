// The names a multisig review says a recipient by: the addresses the person
// knows (`known`, the proposals machine's facts) carry a contact's name as
// every screen says it — what the person called them, else the name resolved
// for the address — as the web's `knownAddresses` and Android's wiring. The
// review (`recipients[].known.label`) is the core's match of these.

import Foundation
import XCTest

@testable import VelaWallet

@MainActor
final class MultisigKnownNamesTests: XCTestCase {
    private let address = "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc"

    /// A contact as the contacts machine stores it (camelCased).
    private func stored(name: String?, resolved: String?) -> [String: Any] {
        ContactsExecutor.contactToStored([
            "address": address, "name": name ?? NSNull(), "resolved_name": resolved ?? NSNull(),
            "kind": "eoa", "favorite": false, "tx_count": 0, "last_used_ms": 0, "first_seen_ms": 0, "source": "manual",
        ])
    }

    func testAResolvedNameNamesTheRecipient() {
        XCTAssertEqual(MultisigFacts.contactName(stored(name: nil, resolved: "bob.eth")), "bob.eth")
    }

    func testThePersonsOwnNameWins() {
        XCTAssertEqual(MultisigFacts.contactName(stored(name: "Bob", resolved: "bob.eth")), "Bob")
    }

    func testEmptyNamesCountAsNone() {
        XCTAssertEqual(MultisigFacts.contactName(stored(name: "", resolved: "bob.eth")), "bob.eth")
        XCTAssertNil(MultisigFacts.contactName(stored(name: "", resolved: "")))
        XCTAssertNil(MultisigFacts.contactName(stored(name: nil, resolved: nil)))
    }
}
