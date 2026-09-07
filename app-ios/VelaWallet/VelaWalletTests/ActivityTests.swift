//
//  ActivityTests.swift
//  VelaWalletTests
//
//  The receipt pipeline's shell half: the bytes that reach storage, the wire
//  translation the core reads, and the wording a row ends up wearing.
//
//  Everything that DECIDES is in Rust — which logs count, what may be admitted,
//  how rows fold into days. What is tested here is what the shell owes it.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ActivityTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func freshStore() -> (VelaStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        return (VelaStore(defaults: defaults), defaults)
    }

    private func record(id: String, timestamp: Double, type: String = "receive") -> [String: Any] {
        [
            "id": id, "userOpHash": "", "txHash": "0xabc", "from": "0x1", "to": "0x2",
            "value": "1.5", "symbol": "USDC", "decimals": 6, "chainId": 100,
            "timestamp": timestamp, "status": "confirmed", "type": type, "usd": "$1.50",
        ]
    }

    // MARK: - The local store

    /// The merge answers **how many were new** — the number the core celebrates
    /// on. A repeat of the same window must answer zero, or somebody sees a
    /// toast for a receipt they already saw.
    @Test func mergeCountsOnlyGenuinelyNewRecords() {
        let (store, _) = freshStore()
        #expect(TxRecords.merge([record(id: "a", timestamp: 10),
                                 record(id: "b", timestamp: 20)], store: store) == 2)
        #expect(TxRecords.merge([record(id: "b", timestamp: 20),
                                 record(id: "c", timestamp: 30)], store: store) == 1)
        #expect(TxRecords.load(store: store).count == 3)
        // Newest first, whatever order they arrived in.
        #expect(TxRecords.load(store: store).first?["id"] as? String == "c")
    }

    /// The 200 cap every client applies, and it keeps the NEWEST.
    @Test func theStoreIsCappedAtTwoHundredNewestFirst() {
        let (store, _) = freshStore()
        let many = (0..<250).map { record(id: "id-\($0)", timestamp: Double($0)) }
        #expect(TxRecords.merge(many, store: store) == 250)
        let stored = TxRecords.load(store: store)
        #expect(stored.count == TxRecords.cap)
        #expect(stored.first?["id"] as? String == "id-249")
        #expect(stored.last?["id"] as? String == "id-50")
    }

    @Test func deletingARecordRemovesOnlyThatOne() {
        let (store, _) = freshStore()
        _ = TxRecords.merge([record(id: "a", timestamp: 10), record(id: "b", timestamp: 20)],
                            store: store)
        TxRecords.delete(id: "a", store: store)
        #expect(TxRecords.load(store: store).map { $0["id"] as? String } == ["b"])
        // A missing id is a no-op, not an empty file.
        TxRecords.delete(id: "nobody", store: store)
        #expect(TxRecords.load(store: store).count == 1)
    }

    // MARK: - Stored shape → the core's vocabulary

    /// A `type` the feed has never heard of is **dropped rather than guessed
    /// at**. Inventing a kind for it would be a lie the core then acts on.
    @Test func anUnknownRecordTypeIsDroppedAndALegacyOneIsNot() {
        #expect(TxRecords.toWire(record(id: "a", timestamp: 1, type: "teleport")) == nil)
        // No `type` at all is the legacy row the core reads as `send`; it must
        // survive, with `kind: null` saying so.
        var legacy = record(id: "a", timestamp: 1)
        legacy.removeValue(forKey: "type")
        let wire = TxRecords.toWire(legacy)
        #expect(wire?["kind"] is NSNull)
    }

    /// Numbers are coerced fail-closed: the store is an unvalidated JSON parse,
    /// and a field serde could not accept would fault the core into a feed that
    /// never loads.
    @Test func malformedNumbersBecomeZeroRatherThanFaultingTheCore() {
        var broken = record(id: "a", timestamp: 1)
        broken["chainId"] = "one hundred"
        broken["decimals"] = -4
        broken["timestamp"] = "yesterday"
        let wire = TxRecords.toWire(broken)
        #expect(wire?["chain_id"] as? Int == 0)
        #expect(wire?["decimals"] as? Int == 0)
        #expect(wire?["timestamp"] as? Double == 0)
    }

    /// The grouping key the core cannot compute. A record written at 23:30
    /// local heads its own day — which is the whole reason the shell owns it.
    @Test func theDayKeyIsLocalMidnight() {
        let calendar = Calendar.current
        let lateEvening = calendar.date(bySettingHour: 23, minute: 30, second: 0, of: Date())!
        let key = TxRecords.dayStartMs(lateEvening.timeIntervalSince1970)
        let midnight = calendar.startOfDay(for: lateEvening).timeIntervalSince1970 * 1000
        #expect(key == midnight)
        // And a record thirty minutes later belongs to the NEXT day.
        let justAfter = TxRecords.dayStartMs(lateEvening.timeIntervalSince1970 + 3_600)
        #expect(justAfter > key)
    }

    // MARK: - The ingest valuation, which is the shell's

    @Test func aStablecoinIsWorthAboutADollarAndTheGlyphIsFolded() {
        #expect(ActivityExecutor.isStable("USDC"))
        #expect(ActivityExecutor.isStable("usdt"))
        // "USD₮0" is Tether's own glyph — the same coin as USDT0.
        #expect(ActivityExecutor.isStable("USD₮0"))
        #expect(!ActivityExecutor.isStable("MON"))
    }

    /// The stored string is `en-US` on every client, so a record written on the
    /// phone reads the same in the browser.
    @Test func theStoredUsdStringIsTheOneEveryClientWrites() {
        #expect(ActivityExecutor.formatUsd(1_234.5) == "$1,234.50")
        #expect(ActivityExecutor.formatUsd(1) == "$1.00")
        // Not a price of zero — "nobody could price it", which is what every
        // client has always stored and what the core reads back as unknown.
        #expect(ActivityExecutor.formatUsd(0) == "$0.00")
        #expect(ActivityExecutor.formatUsd(-3) == "$0.00")
        #expect(ActivityExecutor.formatUsd(.nan) == "$0.00")
    }

    // MARK: - Raw log → the core's wire

    /// A codec, not a policy: the log is carried through untouched, because
    /// whether it means anything is the core's call — it re-verifies
    /// `topics[2]` itself.
    @Test func aRawLogIsCarriedThroughWithAbsenceIntact() {
        let wire = TokenTrustExecutor.logToWire([
            "address": "0xtoken", "topics": ["0xddf", "0xfrom", "0xto"],
            "data": "0x01", "transactionHash": "0xhash", "logIndex": "0x2",
        ])
        #expect(wire["transaction_hash"] as? String == "0xhash")
        #expect((wire["topics"] as? [String])?.count == 3)
        // `null` means ABSENT; the core applies its own `?? 0x0`.
        #expect(wire["block_number"] is NSNull)
        #expect(wire["log_index"] as? String == "0x2")
    }

    @Test func aStoredCustomTokenCrossesWithoutItsDisplayVocabulary() {
        let wire = TokenTrustExecutor.tokenToWire([
            "id": "100_0xabc", "chainId": 100, "contractAddress": "0xabc",
            "symbol": "USDC", "name": "USD Coin", "decimals": 6,
            "networkName": "Gnosis",
        ])
        #expect(wire?["contract_address"] as? String == "0xabc")
        #expect(wire?["decimals"] as? Int == 6)
        // `networkName` is the shell's word for a chain and does not travel.
        #expect(wire?["networkName"] == nil)
        // A row without a contract is not a token anybody can watch.
        #expect(TokenTrustExecutor.tokenToWire(["chainId": 100]) == nil)
    }

    @Test func hexQuantitiesReadAsNumbersOrAsNothing() {
        #expect(TokenTrustExecutor.hexToNumber("0x10") == 16)
        #expect(TokenTrustExecutor.hexToNumber("10") == 16)
        // Never a zero the core would compare against.
        #expect(TokenTrustExecutor.hexToNumber("0x") == nil)
        #expect(TokenTrustExecutor.hexToNumber("later") == nil)
    }

    // MARK: - ERC-20 metadata decoding

    /// The defect this whole path exists for: an 18-decimals guess renders a
    /// 6-decimals stablecoin as "+0 tokens", so an unreadable answer must be
    /// `nil` rather than a default.
    @Test func decimalsAreReadOrRefused() {
        let six = Data(hexString: String(repeating: "0", count: 63) + "6")!
        #expect(TokenMetadata.decodeDecimals(six) == 6)
        // Zero decimals is a real token, not a failure to read one.
        #expect(TokenMetadata.decodeDecimals(Data(repeating: 0, count: 32)) == 0)
        #expect(TokenMetadata.decodeDecimals(Data()) == nil)
        let absurd = Data(hexString: String(repeating: "f", count: 64))!
        #expect(TokenMetadata.decodeDecimals(absurd) == nil)
    }

    /// Both `symbol()` encodings — the dynamic string, and the fixed word the
    /// legacy tokens (MKR) answer with — plus a multibyte symbol surviving.
    @Test func symbolsDecodeFromBothEncodings() {
        let offset = String(repeating: "0", count: 62) + "20"
        let length = String(repeating: "0", count: 62) + "04"
        let usdc = "55534443" + String(repeating: "0", count: 56)
        #expect(TokenMetadata.decodeString(Data(hexString: offset + length + usdc)!) == "USDC")

        // bytes32: "MKR" padded with NULs.
        let mkr = "4d4b52" + String(repeating: "0", count: 58)
        #expect(TokenMetadata.decodeString(Data(hexString: mkr)!) == "MKR")

        // "USD₮0" — three bytes for the glyph, and it must survive intact.
        let tether = "5553..".replacingOccurrences(of: "..", with: "44e282ae30")
        let padded = tether + String(repeating: "0", count: 64 - tether.count)
        let dynamic = offset + String(repeating: "0", count: 62) + "08" + padded
        #expect(TokenMetadata.decodeString(Data(hexString: dynamic)!) == "USD₮0")
    }
}
