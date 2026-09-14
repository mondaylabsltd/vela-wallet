//
//  SimDeltasTests.swift
//  VelaWalletTests
//
//  The simulation's shell half, against the two things it can get wrong:
//  the payload the node is asked, and the netting of what comes back.
//

import Foundation
import Testing
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

struct SimDeltasTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"

    private func transferLog(from: String, to: String, token: String, value: String) -> [String: Any] {
        [
            "address": token,
            "topics": [SimDeltas.transferTopic, topic(from), topic(to)],
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

    // MARK: - The logs

    /// A node that errored has told us NOTHING, and `nil` is how that stays
    /// distinguishable from "it ran and nothing moved".
    @Test func anErrorIsNotAnEmptyResult() {
        #expect(SimDeltas.logsOf(["error": ["code": -32601]]) == nil)
        #expect(SimDeltas.logsOf(["result": []]) == nil)
        #expect(SimDeltas.logsOf([:]) == nil)
        #expect(SimDeltas.logsOf(["result": [["calls": []]]])?.isEmpty == true,
                "a block with no calls RAN — that is an empty result, not a refusal")
    }

    /// Only the calls that succeeded. A reverted leg's logs describe a world
    /// that will not exist.
    @Test func onlySucceededCallsContributeLogs() throws {
        let logs = try #require(SimDeltas.logsOf([
            "result": [[
                "calls": [
                    ["status": "0x1", "logs": [transferLog(from: me, to: usdc, token: usdc, value: word(5))]],
                    ["status": "0x0", "logs": [transferLog(from: usdc, to: me, token: usdc, value: word(99))]],
                ],
            ]],
        ]))
        #expect(logs.count == 1)
    }

    // MARK: - The netting

    @Test func transfersNetPerTokenInFirstSeenOrder() throws {
        let logs = [
            transferLog(from: me, to: usdc, token: usdc, value: word(30)),
            transferLog(from: usdc, to: me, token: usdc, value: word(10)),
            transferLog(from: usdc, to: me, token: SimDeltas.nativeSentinel, value: word(7)),
        ]
        let deltas = SimDeltas.deriveDeltas(logs: logs, user: me)
        #expect(deltas.count == 2)
        #expect(deltas[0]["kind"] as? String == "erc20")
        #expect(deltas[0]["token"] as? String == usdc)
        #expect(deltas[0]["delta"] as? String == "-20")
        // The sentinel is the NATIVE coin, not a contract.
        #expect(deltas[1]["kind"] as? String == "native")
        #expect(deltas[1]["token"] is NSNull)
        #expect(deltas[1]["delta"] as? String == "7")
    }

    /// In and out of the same token nets to nothing, and a pair of moves that
    /// cancel is how a swap looks like a theft.
    @Test func aMoveThatCancelsIsNotAMove() {
        let logs = [
            transferLog(from: me, to: usdc, token: usdc, value: word(12)),
            transferLog(from: usdc, to: me, token: usdc, value: word(12)),
        ]
        #expect(SimDeltas.deriveDeltas(logs: logs, user: me).isEmpty)
    }

    /// Somebody else's transfer is somebody else's business.
    @Test func transfersThatMissTheWalletAreIgnored() {
        let other = "0x1111111111111111111111111111111111111111"
        let logs = [transferLog(from: other, to: usdc, token: usdc, value: word(5))]
        #expect(SimDeltas.deriveDeltas(logs: logs, user: me).isEmpty)
    }

    /// A log that is not a `Transfer`, or is malformed, contributes nothing —
    /// silently, because a simulation is untrusted input by definition.
    @Test func onlyWellFormedTransfersCount() {
        let logs: [[String: Any]] = [
            ["address": usdc, "topics": ["0xdeadbeef", topic(me), topic(usdc)], "data": word(5)],
            ["address": usdc, "topics": [SimDeltas.transferTopic, topic(me)], "data": word(5)],
            ["address": "", "topics": [SimDeltas.transferTopic, topic(usdc), topic(me)], "data": word(5)],
            ["address": usdc, "topics": [SimDeltas.transferTopic, topic(usdc), topic(me)], "data": "0x"],
        ]
        #expect(SimDeltas.deriveDeltas(logs: logs, user: me).isEmpty)
    }

    @Test func topicsBecomeAddresses() {
        #expect(SimDeltas.topicAddress(topic(me)) == me.lowercased())
        #expect(SimDeltas.topicAddress("0x00") == "")
    }
}
