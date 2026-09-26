//
//  SingleFlight.swift
//  VelaWallet
//
//  One read in flight per question.
//
//  Since spec 069 a send prices ONE operation in up to three `fee_policy`
//  sessions at once — the speed in force and a preview per other speed — and
//  each session asks the relay the same questions at the same instant: the gas
//  signals, `pimlico_getUserOperationGasPrice`, `vela_getInBandGasQuote`, the
//  simulation. A cache alone does not help there, because all three miss
//  together; they each paid a round trip, and the three rows of the speed
//  control settled one after another (founder, 2026-09-26: "it should take ONE
//  request and then compute").
//
//  This is the other half of the cache: while a question is being answered,
//  anybody who asks the same question waits for that answer instead of asking
//  again. Everything here is `@MainActor`, so the map needs no lock and a
//  waiter can never miss the flight it joined.
//
//  ## Keys decide freshness
//
//  A flight is shared only with callers that ask the SAME key. A reader that
//  must not join a flight started before somebody asked for a fresh reading
//  (the refresh control, a submit — `RelayClient.invalidateFeeSignals`) puts
//  its epoch in the key, and the old flight simply finishes for the callers it
//  already had.
//

import Foundation

@MainActor
final class SingleFlight<Key: Hashable, Value> {

    /// The flight answering each key, by id, so a flight that finished after
    /// its key moved on answers only its own waiters.
    private var current: [Key: Int] = [:]
    private var waiters: [Int: [(Value) -> Void]] = [:]
    private var nextId = 0

    /// How many flights actually ran — the test seam for "N callers, one read".
    private(set) var started = 0

    init() {}

    /// Answer `key`: by joining the flight already out for it, or by running
    /// `work` and answering everybody who joined meanwhile with the same value.
    func run(_ key: Key, _ work: () async -> Value) async -> Value {
        if let id = current[key] {
            return await withCheckedContinuation { continuation in
                waiters[id, default: []].append { continuation.resume(returning: $0) }
            }
        }
        nextId += 1
        started += 1
        let id = nextId
        current[key] = id
        waiters[id] = []
        let value = await work()
        if current[key] == id { current[key] = nil }
        let joined = waiters.removeValue(forKey: id) ?? []
        for resume in joined { resume(value) }
        return value
    }

    /// Whether a flight is out for `key` right now.
    func inFlight(_ key: Key) -> Bool { current[key] != nil }

    /// How many callers are waiting on the flight out for `key` — a test seam.
    func joined(_ key: Key) -> Int { current[key].flatMap { waiters[$0]?.count } ?? 0 }

    /// Written for the compiler: a generic class in this target must declare
    /// it (`app-ios/scripts/check-generic-class-deinit.mjs` says why).
    nonisolated deinit {}
}
