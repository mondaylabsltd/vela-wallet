//
//  ContactsWiringTests.swift
//  VelaWalletTests
//
//  The address book's write half (spec 054 US5b–7), driven through the REAL
//  `contacts` core.
//
//  Everything here was DRAWN before this spec and did nothing: the form had no
//  machine behind it, the star did not exist, the detail's pencil and its
//  删除联系人 were both no-ops, the search field was a `Text`, and a picked
//  file had nowhere to go. Each test below is one of those, asserted against
//  the machine that now answers it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ContactsWiringTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let alice = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    private func effects(from dispatchResult: String) throws -> [[String: Any]] {
        try CoreJSON.object(dispatchResult)["effects"] as? [[String: Any]] ?? []
    }

    /// A booted book with its stores read and nothing in them.
    ///
    /// The core drops every mutation that arrives before `loaded`, so a test
    /// that skipped this would be asserting about events the machine ignored —
    /// and would pass for the wrong reason.
    private func loaded() throws -> ContactsCore {
        let core = ContactsCore()
        var pending = try effects(from: core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched",
            "my_address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ])))
        // Answer the reads with "nothing stored", following whatever the core
        // asks for next until it stops asking.
        var rounds = 0
        while rounds < 12 {
            rounds += 1
            let answerable = pending.compactMap { effect -> (UInt64, String)? in
                guard let type = (effect["operation"] as? [String: Any])?["type"] as? String,
                      let id = (effect["id"] as? NSNumber)?.uint64Value
                else { return nil }
                return (id, type)
            }
            guard !answerable.isEmpty else { break }
            var next: [[String: Any]] = []
            for (id, type) in answerable {
                let answer: [String: Any] = switch type {
                case "read_store":
                    ["type": "store_loaded", "contacts": [], "tombstones": [], "groups": []]
                case "load_send_history":
                    ["type": "history_loaded", "txs": []]
                case "resolve_identity":
                    ["type": "identity_resolved", "address": "", "identity": NSNull()]
                case "classify_recipient":
                    ["type": "recipient_classified", "address": "", "code": NSNull()]
                default:
                    ["type": "written"]
                }
                next += try effects(from: core.resolveEffect(
                    effectId: id, resultJson: CoreJSON.string(answer)
                ))
            }
            pending = next
        }
        return core
    }

    private func decoded(_ core: ContactsCore) throws -> ContactsViewWire {
        try CoreJSON.decode(ContactsViewWire.self, from: try CoreJSON.object(core.view()))
    }

    // MARK: - Drift

    /// The three view fields 050 dropped on the floor — `sections`,
    /// `import_failure` and `export` — decode now. Until this spec they were
    /// silently discarded, which is why nothing could show an import's verdict.
    @Test func theWholeViewDecodes() throws {
        let core = try loaded()
        let view = try decoded(core)
        #expect(view.loaded)
        #expect(view.contacts.isEmpty)
        #expect(view.sections.isEmpty)
        #expect(view.importFailure == nil)
        #expect(view.export == nil)
    }

    // MARK: - The form

    /// 保存 puts the contact in the book, name and all.
    @Test func savingAddsTheContact() throws {
        let core = try loaded()
        let saved = try core.dispatch(eventJson: CoreJSON.string([
            "type": "save",
            "input": [
                "address": alice, "name": "Alice", "note": NSNull(),
                "favorite": NSNull(), "kind": NSNull(),
                "resolved_name": NSNull(), "resolved_source": NSNull(),
            ],
            "now_ms": 1_700_000_000_000,
        ]))
        let view = try CoreJSON.decode(ContactsViewWire.self, from: try self.view(from: saved))
        #expect(view.contacts.count == 1)
        #expect(view.contacts.first?.name == "Alice")
        // Lowercased by the core — the address IS the key, and two spellings of
        // one address would be two contacts.
        #expect(view.contacts.first?.address == alice.lowercased())
    }

    /// Saving the same address twice UPDATES rather than duplicating. The form
    /// uses one event for both new and edit precisely because of this.
    @Test func savingTwiceUpdatesInPlace() throws {
        let core = try loaded()
        for name in ["Alice", "Alice Chen"] {
            _ = try core.dispatch(eventJson: CoreJSON.string([
                "type": "save",
                "input": [
                    "address": alice, "name": name, "note": NSNull(),
                    "favorite": NSNull(), "kind": NSNull(),
                    "resolved_name": NSNull(), "resolved_source": NSNull(),
                ],
                "now_ms": 1_700_000_000_000,
            ]))
        }
        let view = try decoded(core)
        #expect(view.contacts.count == 1)
        #expect(view.contacts.first?.name == "Alice Chen")
    }

    /// The star flips, and flips back.
    @Test func theStarFlips() throws {
        let core = try loaded()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "save",
            "input": [
                "address": alice, "name": "Alice", "note": NSNull(),
                "favorite": NSNull(), "kind": NSNull(),
                "resolved_name": NSNull(), "resolved_source": NSNull(),
            ],
            "now_ms": 1_700_000_000_000,
        ]))
        #expect(try decoded(core).contacts.first?.favorite == false)

        for expected in [true, false] {
            _ = try core.dispatch(eventJson: CoreJSON.string([
                "type": "toggle_favorite", "address": alice, "now_ms": 1_700_000_000_000,
            ]))
            #expect(try decoded(core).contacts.first?.favorite == expected)
        }
    }

    /// Deleting from the detail page removes the contact — the same event the
    /// row swipe uses, because they are the same act.
    @Test func deletingRemovesTheContact() throws {
        let core = try loaded()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "save",
            "input": [
                "address": alice, "name": "Alice", "note": NSNull(),
                "favorite": NSNull(), "kind": NSNull(),
                "resolved_name": NSNull(), "resolved_source": NSNull(),
            ],
            "now_ms": 1_700_000_000_000,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "delete", "address": alice, "now_ms": 1_700_000_000_001,
        ]))
        #expect(try decoded(core).contacts.isEmpty)
    }

    // MARK: - The form's own gate

    /// Save is shut on an empty address and open on a plausible one. The CORE
    /// still decides whether the contact is saved; this keeps the button from
    /// offering to save nothing.
    @Test func theFormGatesOnTheAddress() {
        let empty = ContactsLive.ContactDraft(editing: nil, name: "Alice", address: "")
        #expect(!ContactsLive.formModel(empty, loc: loc).saveEnabled)

        let short = ContactsLive.ContactDraft(editing: nil, name: "", address: "0x1234")
        #expect(!ContactsLive.formModel(short, loc: loc).saveEnabled)

        // A name is optional: a saved address with no name is a legitimate
        // contact, and a contact with no address is nothing at all.
        let nameless = ContactsLive.ContactDraft(editing: nil, name: "", address: alice)
        #expect(ContactsLive.formModel(nameless, loc: loc).saveEnabled)

        // Editing: the address is the identity, so it is locked and Save is
        // open regardless of what is in the field.
        let editing = ContactsLive.ContactDraft(editing: alice, name: "", address: alice)
        let model = ContactsLive.formModel(editing, loc: loc)
        #expect(model.saveEnabled)
        #expect(model.addressLocked)
        #expect(model.title == loc.t("contacts.editTitle"))
    }

    // MARK: - Import and export

    /// A JSON backup goes in, and the report says what happened. The SHELL
    /// never parses the file — the core sniffs JSON from CSV itself.
    @Test func aBackupImportsAndReports() throws {
        let core = try loaded()
        let backup = """
        {"version":1,"exportedAt":"2026-09-14T00:00:00Z","contacts":[
          {"address":"\(alice)","name":"Alice"},
          {"address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","name":"Treasury"}
        ],"groups":[]}
        """
        let imported = try core.dispatch(eventJson: CoreJSON.string([
            "type": "import_file", "content": backup, "filename": "vela-contacts.json",
            "into_group": NSNull(), "now_ms": 1_700_000_000_000,
        ]))
        let view = try CoreJSON.decode(ContactsViewWire.self, from: try self.view(from: imported))
        #expect(view.contacts.count == 2)
        #expect(view.lastImport?.added == 2)
        #expect(view.importFailure == nil)

        let notice = try #require(ContactsLive.importNotice(view, loc: loc))
        #expect(notice.title == loc.t("contacts.importDoneTitle"))
        // No invalid rows, so no second sentence about them.
        #expect(!notice.message.contains(loc.t("contacts.importDoneInvalid", vars: ["invalid": "0"])))

        // Acknowledged, the line leaves the view — it is a one-shot, and a
        // report that stayed would re-announce itself on every glance.
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "import_acknowledged"]))
        #expect(try decoded(core).lastImport == nil)
    }

    /// A file that is not an address book at all is REFUSED — and refused
    /// before any write, so the book is untouched. "0 added" would be a
    /// different and wrong sentence.
    @Test func aRubbishFileIsRefusedWholesale() throws {
        let core = try loaded()
        let refused = try core.dispatch(eventJson: CoreJSON.string([
            "type": "import_file", "content": "{ not json at all", "filename": "notes.json",
            "into_group": NSNull(), "now_ms": 1_700_000_000_000,
        ]))
        let view = try CoreJSON.decode(ContactsViewWire.self, from: try self.view(from: refused))
        #expect(view.importFailure != nil)
        #expect(view.lastImport == nil)
        #expect(view.contacts.isEmpty)

        let notice = try #require(ContactsLive.importNotice(view, loc: loc))
        #expect(notice.title == loc.t("contacts.importFailTitle"))
    }

    /// Export writes a file into the view, and `export_taken` removes it.
    @Test func exportProducesAFileAndThenLetsGoOfIt() throws {
        let core = try loaded()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "save",
            "input": [
                "address": alice, "name": "Alice", "note": NSNull(),
                "favorite": NSNull(), "kind": NSNull(),
                "resolved_name": NSNull(), "resolved_source": NSNull(),
            ],
            "now_ms": 1_700_000_000_000,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "export_requested",
            "scope": ["type": "all"], "format": "json",
            "exported_at_iso": "2026-09-14T12:00:00Z",
        ]))
        let file = try #require(try decoded(core).export)
        #expect(file.contacts == 1)
        #expect(file.content.contains(alice.lowercased()))
        #expect(file.filename.hasSuffix(".json"))

        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "export_taken"]))
        #expect(try decoded(core).export == nil)
    }

    // MARK: - The inspection

    /// The two tags on a contact's page are the core's reading of THAT
    /// address. A projection left over from another recipient tags the wrong
    /// person, so it is matched on the address before it is shown.
    @Test func theInspectionBelongsToTheAddressItNames() {
        func recipient(_ address: String, contract: Bool?, first: Bool) -> ContactRecipientWire {
            ContactRecipientWire(
                address: address, saved: true, verified: false, displayName: nil,
                identity: nil, kind: .eoa, isContract: contract, firstInteraction: first
            )
        }
        let contact = ContactWire(
            address: alice.lowercased(), name: "Alice", resolvedName: nil, resolvedSource: nil,
            kind: .eoa, favorite: true, note: nil, txCount: 0,
            lastUsedMs: 0, firstSeenMs: 0, source: .manual
        )
        func detail(_ recipient: ContactRecipientWire?) -> ContactDetailModel {
            ContactsLive.detail(
                contact,
                view: ContactsViewWire(
                    loaded: true, contacts: [contact], sections: [], groups: [],
                    lastImport: nil, importFailure: nil, export: nil, recipient: recipient
                ),
                loc: loc
            )
        }
        // Somebody else's reading: no tags at all.
        #expect(detail(recipient("0xdead", contract: true, first: true)).inspection == nil)

        let wallet = detail(recipient(alice.lowercased(), contract: false, first: true))
        #expect(wallet.inspection?.tag == loc.t("componentsUi.signing.walletTag"))
        #expect(wallet.inspection?.firstTime == loc.t("componentsUi.signing.firstTimeTagNeutral"))

        let contract = detail(recipient(alice.lowercased(), contract: true, first: false))
        #expect(contract.inspection?.tag == loc.t("componentsUi.signing.contractTag"))
        #expect(contract.inspection?.firstTime == nil)

        // Unknown says NOTHING — never a false alarm, never a false
        // reassurance.
        #expect(detail(recipient(alice.lowercased(), contract: nil, first: false)).inspection == nil)

        // The star follows the contact, on every page that has one.
        #expect(detail(nil).favourite?.on == true)
    }
}
