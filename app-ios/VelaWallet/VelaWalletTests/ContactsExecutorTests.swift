//
//  ContactsExecutorTests.swift
//  VelaWalletTests
//
//  The contacts executor's two jobs: answer everything, and translate shapes.
//
//  Both are failure modes the compiler cannot see. A JSON tag has no
//  exhaustiveness check, so an operation nobody handles leaves the core waiting
//  forever — a screen that never loads, with no error anywhere. And the stored
//  shapes are a contract with three other clients, so a field written under the
//  wrong name is data three other wallets cannot read.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ContactsExecutorTests {

    /// A store nobody else is using, so tests cannot see each other's writes.
    private func freshStore(_ name: String = UUID().uuidString) -> (VelaStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: name)!
        return (VelaStore(defaults: defaults), defaults)
    }

    private func answer(_ executor: ContactsExecutor, _ operation: [String: Any]) async -> [String: Any] {
        let json = await executor.perform(operation)
        return (try? CoreJSON.object(json)) ?? [:]
    }

    private func type(of answer: [String: Any]) -> String { answer["type"] as? String ?? "" }

    // MARK: - The failure contract

    /// Every operation the contract names must come back with something the
    /// core can parse. An unanswered operation is an effect the core waits on
    /// forever.
    @Test func everyOperationIsAnswered() async {
        let (store, _) = freshStore()
        let executor = ContactsExecutor(store: store)
        #expect(ContactsExecutor.operations.count == 7)
        for name in ContactsExecutor.operations {
            let reply = await answer(executor, ["type": name, "address": "0xabc", "chain_id": 100])
            #expect(!type(of: reply).isEmpty, "no answer for `\(name)`")
        }
    }

    /// `history_failed`, not an empty list.
    ///
    /// An empty list is a CLAIM — "this person has never sent to anybody" — and
    /// the core would then treat every address as a first interaction, showing
    /// the address-poisoning warning to everyone, forever. The failure variant
    /// says the true thing: nobody looked.
    @Test func sendHistoryFailsRatherThanClaimingEmptiness() async {
        let (store, _) = freshStore()
        let reply = await answer(ContactsExecutor(store: store), ["type": "load_send_history"])
        #expect(type(of: reply) == "history_failed")
    }

    /// `null`, not `""`. An empty string means "definitely an EOA", which the
    /// core would show as a settled classification (invariant ⑦).
    @Test func classificationIsUnknownRatherThanAnEoa() async {
        let (store, _) = freshStore()
        let reply = await answer(ContactsExecutor(store: store), [
            "type": "classify_recipient", "chain_id": 100, "address": "0xabc",
        ])
        #expect(type(of: reply) == "recipient_classified")
        #expect(reply["code"] is NSNull)
    }

    /// An operation from a newer core must not hang the machine.
    @Test func anUnknownOperationStillAnswers() async {
        let (store, _) = freshStore()
        let reply = await answer(ContactsExecutor(store: store), ["type": "invented_by_a_later_spec"])
        #expect(!type(of: reply).isEmpty)
    }

    // MARK: - Storage

    @Test func anAbsentStoreReadsEmptyRatherThanFailing() async {
        let (store, _) = freshStore()
        let reply = await answer(ContactsExecutor(store: store), ["type": "read_store"])
        #expect(type(of: reply) == "store_loaded")
        #expect((reply["contacts"] as? [Any])?.isEmpty == true)
        #expect((reply["groups"] as? [Any])?.isEmpty == true)
        #expect((reply["tombstones"] as? [Any])?.isEmpty == true)
    }

    /// Corrupt bytes must not make the screen permanently unopenable — and,
    /// more sharply, must not be reported as a failure: a machine that receives
    /// a failed store read stays unloaded forever and drops every write after.
    @Test func corruptBytesReadEmptyRatherThanFailing() async {
        let (store, defaults) = freshStore()
        defaults.set("{not json at all", forKey: VelaStore.Key.contacts)
        let reply = await answer(ContactsExecutor(store: store), ["type": "read_store"])
        #expect(type(of: reply) == "store_loaded")
        #expect((reply["contacts"] as? [Any])?.isEmpty == true)
    }

    /// The stored blob is an OBJECT keyed by address; the wire is a LIST.
    /// Writing a list here is silently accepted and read back as empty by every
    /// client, which resurrects every deleted contact.
    @Test func tombstonesPivotBetweenObjectAndList() async {
        let (store, defaults) = freshStore()
        let executor = ContactsExecutor(store: store)

        _ = await executor.perform([
            "type": "write_dismissed",
            "tombstones": [["address": "0xdead", "dismissed_at_ms": 1_756_944_000_000]],
        ])

        let raw = defaults.string(forKey: VelaStore.Key.contactsDismissed) ?? ""
        let stored = (try? JSONSerialization.jsonObject(with: Data(raw.utf8))) as? [String: Any]
        #expect(stored?["0xdead"] != nil, "stored as \(raw) — must be an address→ms object")

        let reply = await answer(executor, ["type": "read_store"])
        let tombstones = reply["tombstones"] as? [[String: Any]] ?? []
        #expect(tombstones.count == 1)
        #expect(tombstones.first?["address"] as? String == "0xdead")
    }

    /// A contact survives a round trip through storage with every field intact
    /// and the two alphabets translated — which is the same thing as saying a
    /// record written here reads on web.
    @Test func aContactRoundTripsThroughTheStoredShape() async {
        let (store, defaults) = freshStore()
        let executor = ContactsExecutor(store: store)

        _ = await executor.perform([
            "type": "write_contacts",
            "contacts": [[
                "address": "0xabc", "name": "Alice", "resolved_name": "alice.eth",
                "resolved_source": "ENS", "kind": "eoa", "favorite": true,
                "note": "landlord", "tx_count": 3,
                "last_used_ms": 1_700_000_000_000, "first_seen_ms": 1_600_000_000_000,
                "source": "manual",
            ]],
        ])

        // The stored side speaks camelCase.
        let raw = defaults.string(forKey: VelaStore.Key.contacts) ?? ""
        let stored = ((try? JSONSerialization.jsonObject(with: Data(raw.utf8))) as? [[String: Any]])?.first
        #expect(stored?["resolvedName"] as? String == "alice.eth")
        #expect(stored?["txCount"] as? Int == 3)
        #expect(stored?["lastUsed"] as? Double == 1_700_000_000_000)
        #expect(stored?["firstSeen"] as? Double == 1_600_000_000_000)

        // The wire side speaks snake_case, with nothing lost on the way back.
        let reply = await answer(executor, ["type": "read_store"])
        let wire = (reply["contacts"] as? [[String: Any]])?.first
        #expect(wire?["resolved_name"] as? String == "alice.eth")
        #expect(wire?["tx_count"] as? Int == 3)
        #expect(wire?["favorite"] as? Bool == true)
        #expect(wire?["note"] as? String == "landlord")
    }

    /// Absent optionals are OMITTED. Writing `null` produces bytes the Expo
    /// services never wrote, which is a difference three other clients would
    /// have to tolerate for no reason.
    @Test func absentOptionalsAreOmittedRatherThanWrittenAsNull() async {
        let (store, defaults) = freshStore()
        _ = await ContactsExecutor(store: store).perform([
            "type": "write_contacts",
            "contacts": [[
                "address": "0xabc", "name": NSNull(), "resolved_name": NSNull(),
                "resolved_source": NSNull(), "kind": "unknown", "favorite": false,
                "note": NSNull(), "tx_count": 0, "last_used_ms": 0,
                "first_seen_ms": 0, "source": "auto",
            ]],
        ])
        let raw = defaults.string(forKey: VelaStore.Key.contacts) ?? ""
        #expect(!raw.contains("null"), "wrote nulls: \(raw)")
        #expect(!raw.contains("\"name\""))
        #expect(!raw.contains("\"note\""))
    }

    /// Junk in a stored record is flattened, never rejected. serde refuses a
    /// mistyped record, and a refused `store_loaded` strands the machine
    /// unloaded forever — so one bad row must not cost the whole book.
    @Test func junkInAStoredRecordIsFlattenedRatherThanRejected() async {
        let (store, defaults) = freshStore()
        defaults.set(
            #"[{"address":"0xABC","kind":"nonsense","txCount":"lots","lastUsed":null,"source":"???"}]"#,
            forKey: VelaStore.Key.contacts
        )
        let reply = await answer(ContactsExecutor(store: store), ["type": "read_store"])
        let wire = (reply["contacts"] as? [[String: Any]])?.first
        #expect(wire?["address"] as? String == "0xabc", "the canonical key is lowercased")
        #expect(wire?["kind"] as? String == "unknown")
        #expect(wire?["tx_count"] as? Int == 0)
        #expect(wire?["source"] as? String == "manual")
    }
}
