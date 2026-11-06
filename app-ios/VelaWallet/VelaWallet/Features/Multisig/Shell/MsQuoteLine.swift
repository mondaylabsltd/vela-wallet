//
//  MsQuoteLine.swift
//  VelaWallet
//
//  r16-7: the multisig's fee quotes, one at a time.
//

import Foundation

/// r16-7: the multisig's fee quotes, one at a time — a FIFO line with
/// single-flight for identical asks. The fee session (`FeeStore.quote`)
/// supersedes an older quote when a newer one is asked, answering the older
/// `nil`; two looks of one execution (the sheet's timer and the press's own
/// re-quote) asked side by side made the older say "failed" — "Couldn't work
/// out the network fee right now." for ~2 s under a figure about to land.
/// In line, every ask gets its own answer; the same ask while one is out
/// shares that one's.
@MainActor
final class MsQuoteLine {
    /// The answer, boxed: a quote's JSON crosses the line's task.
    struct Answer: @unchecked Sendable {
        let value: [String: Any]
        init(_ value: [String: Any]) { self.value = value }
    }

    private var tail: Task<Void, Never>?
    private var inFlight: [String: (id: UUID, task: Task<Answer, Never>)] = [:]

    /// What makes two asks the same: the network, the executor and the calls.
    nonisolated static func key(_ op: [String: Any]) -> String {
        let calls = op["calls"].flatMap { try? JSONSerialization.data(withJSONObject: $0, options: [.sortedKeys]) }
            .flatMap { String(data: $0, encoding: .utf8) } ?? ""
        return "\(op["chain_id"] ?? ""):\((op["from"] as? String ?? "").lowercased()):\(calls)"
    }

    func run(key: String, _ work: @escaping @MainActor () async -> Answer) async -> Answer {
        if let same = inFlight[key] { return await same.task.value }
        let before = tail
        let task = Task { @MainActor () -> Answer in
            await before?.value
            return await work()
        }
        let id = UUID()
        inFlight[key] = (id, task)
        tail = Task { @MainActor in _ = await task.value }
        let answer = await task.value
        if inFlight[key]?.id == id { inFlight[key] = nil }
        return answer
    }
}
