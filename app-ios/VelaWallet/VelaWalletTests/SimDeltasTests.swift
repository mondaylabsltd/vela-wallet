//
//  SimDeltasTests.swift
//  VelaWalletTests
//
//  The simulation's shell half, against the one thing it still does — the
//  payload the node is asked — and the mapping of the node's answer into the
//  core's `simOutcome` (spec 082 T110, RG6). The reading of that answer (a
//  revert, a node that cannot simulate, the person's net moves) moved to the
//  core with its vectors (T039); what is pinned here is that the shell hands
//  the core the reply AS IT CAME and draws the core's verdict.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

struct SignedDigitsTests {

    @Test func signsAndMagnitudes() throws {
        #expect(SignedDigits("0")?.text == "0")
        #expect(SignedDigits("-0")?.text == "0", "zero is never negative")
        #expect(SignedDigits("007")?.text == "7")
        #expect(SignedDigits("-42")?.text == "-42")
        #expect(SignedDigits("") == nil)
        // Not a number is not zero: answering 0 here would turn a malformed
        // log into a real balance of nothing.
        #expect(SignedDigits("12a") == nil)
        #expect(SignedDigits("--1") == nil)
    }

    /// Past `Int64`, which is the whole reason this type exists.
    @Test func arithmeticSurvivesAWordSizedValue() throws {
        let huge = try #require(SignedDigits("115792089237316195423570985008687907853269984665640564039457584007913129639935"))
        #expect(huge.adding(try #require(SignedDigits("1"))).text
            == "115792089237316195423570985008687907853269984665640564039457584007913129639936")
        #expect(huge.subtracting(huge).isZero)
    }

    @Test func signedAdditionCrossesZero() throws {
        let five = try #require(SignedDigits("5"))
        let seven = try #require(SignedDigits("7"))
        #expect(five.subtracting(seven).text == "-2")
        #expect(seven.subtracting(five).text == "2")
        #expect(five.subtracting(five).text == "0")
        #expect(five.adding(try #require(SignedDigits("-12"))).text == "-7")
    }

    /// A 32-byte word, as a `Transfer` log carries it.
    @Test func hexWordsDecode() throws {
        #expect(SignedDigits.firstWord(
            hex: "0x0000000000000000000000000000000000000000000000000de0b6b3a7640000"
        )?.text == "1000000000000000000")
        // Anything past the first word belongs to another field.
        let padded = "0x" + String(repeating: "0", count: 62) + "ff" + String(repeating: "f", count: 64)
        #expect(SignedDigits.firstWord(hex: padded)?.text == "255")
        #expect(SignedDigits.firstWord(hex: "0x") == nil)
        #expect(SignedDigits.firstWord(hex: "0xzz") == nil)
    }
}

@MainActor
struct SimDeltasTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
    /// `keccak256("Transfer(address,address,uint256)")` — test data only.
    private let transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    private func transferLog(from: String, to: String, token: String, value: String) -> [String: Any] {
        [
            "address": token,
            "topics": [transferTopic, topic(from), topic(to)],
            "data": value,
        ]
    }

    private func topic(_ address: String) -> String {
        "0x" + String(repeating: "0", count: 24) + address.dropFirst(2).lowercased()
    }

    /// A 32-byte word for a small integer.
    private func word(_ value: Int) -> String {
        let hex = String(value, radix: 16)
        return "0x" + String(repeating: "0", count: 64 - hex.count) + hex
    }

    // MARK: - The payload

    /// Every leg rides ONE block-state call, so the legs see each other's
    /// effects — simulating them separately would price a batch nobody submits.
    @Test func thePayloadCarriesEveryLegInOneBlock() throws {
        let payload = try #require(SimDeltas.payload(from: me, calls: [
            SimDeltas.Call(to: usdc, value: nil, data: "0xa9059cbb"),
            SimDeltas.Call(to: me, value: "1000", data: nil),
        ]))
        let body = try #require(payload.first as? [String: Any])
        #expect(payload.last as? String == "latest")
        let blocks = try #require(body["blockStateCalls"] as? [[String: Any]])
        #expect(blocks.count == 1)
        let calls = try #require(blocks[0]["calls"] as? [[String: Any]])
        #expect(calls.count == 2)
        // No data key at all on a plain value move — an empty one reads as a
        // contract call with no selector.
        #expect(calls[1]["data"] == nil)
        #expect(calls[1]["value"] as? String == "0x3e8")
        #expect(body["validation"] as? Bool == false)
        #expect(body["traceTransfers"] as? Bool == true, "native moves are only countable as logs")
    }

    @Test func thereIsNothingToSimulateWithoutACall() {
        #expect(SimDeltas.payload(from: me, calls: []) == nil)
        #expect(SimDeltas.payload(from: me, calls: [SimDeltas.Call(to: "", value: nil, data: nil)]) == nil)
    }

    @Test func valuesBecomeTheHexTheNodeWants() {
        #expect(SimDeltas.hexValue(nil) == "0x0")
        #expect(SimDeltas.hexValue("") == "0x0")
        #expect(SimDeltas.hexValue("0x00ff") == "0xff")
        #expect(SimDeltas.hexValue("1000000000000000000") == "0xde0b6b3a7640000")
        // Not a number, and a negative: both "moves nothing" rather than a
        // guess at what was meant.
        #expect(SimDeltas.hexValue("abc") == "0x0")
        #expect(SimDeltas.hexValue("-5") == "0x0")
    }

    // MARK: - The node's answer, to the core (spec 082 T110)

    private func outcome(_ answer: RpcOutcome) -> SigningController.Simulation {
        SigningController.simulation(of: simOutcome(user: me, replyJson: SigningController.simReply(answer)))
    }

    /// A call that reverted is DANGER, with its reason — the old parser read
    /// it as "nothing moves" (L-D5).
    @Test func aRevertIsDangerWithItsReason() {
        // Error(string) "nope": 0x08c379a0, offset 0x20, length 4, "nope".
        let revertData = "0x08c379a0" + String(repeating: "0", count: 62) + "20"
            + String(repeating: "0", count: 63) + "4" + "6e6f7065" + String(repeating: "0", count: 56)
        let result: [[String: Any]] = [[
            "calls": [["status": "0x0", "returnData": revertData, "logs": [[String: Any]](),
                       "error": ["code": 3, "message": "execution reverted", "data": revertData]]],
        ]]
        guard case .notice(let risk, let key, let reason) = outcome(.ok(result)) else {
            Issue.record("a revert was not a notice")
            return
        }
        #expect(risk == "danger")
        #expect(key == "componentsUi.signing.simWillFailReason" || key == "componentsUi.signing.simWillFail")
        if key == "componentsUi.signing.simWillFailReason" { #expect(reason == "nope") }
    }

    /// A node that does not offer the method, or answers junk, is CAUTION —
    /// "couldn't check", never danger and never "nothing moves".
    @Test func aNodeThatCannotCheckIsCaution() {
        for answer: RpcOutcome in [
            .rpcError(code: -32601, message: "method not found"),
            .rpcError(code: -32603, message: "method handler crashed"),
            .ok(NSNull()),
            .ok([Any]()),
        ] {
            #expect(outcome(answer) == .notice(
                risk: "caution", key: "componentsUi.signing.simUnavailableWarning", reason: nil
            ), "\(answer)")
        }
        // Nobody answered at all: the same caution.
        #expect(outcome(.failed(rateLimited: false)) == .notice(
            risk: "caution", key: "componentsUi.signing.simUnavailableWarning", reason: nil
        ))
    }

    /// A clean run is the balance block — the deltas go to `token_trust`,
    /// the one machine that judges them.
    @Test func aCleanRunIsAnsweredWithTheCoresDeltas() throws {
        let result: [[String: Any]] = [[
            "calls": [["status": "0x1", "logs": [
                transferLog(from: me, to: usdc, token: usdc, value: word(30)),
            ]]],
        ]]
        #expect(outcome(.ok(result)) == .answered)
        let record = simOutcome(user: me, replyJson: SigningController.simReply(.ok(result)))
        let deltas = try #require(
            try JSONSerialization.jsonObject(with: Data(record.deltasJson.utf8)) as? [[String: Any]]
        )
        #expect(deltas.count == 1)
        #expect((deltas.first?["token"] as? String)?.lowercased() == usdc)
        #expect(deltas.first?["delta"] as? String == "-30")
    }

    /// The reply is handed over as it came: a result, the error member, or
    /// nothing at all.
    @Test func theReplyIsNormalisedNotJudged() throws {
        let error = try CoreJSON.object(SigningController.simReply(.rpcError(code: -32601, message: "no")))
        #expect((error["error"] as? [String: Any])?["message"] as? String == "no")
        #expect(SigningController.simReply(.failed(rateLimited: true)) == #"{"unreachable":true}"#)
        let result = try CoreJSON.object(SigningController.simReply(.ok([["calls": []]])))
        #expect(result["result"] is [Any])
    }
}
