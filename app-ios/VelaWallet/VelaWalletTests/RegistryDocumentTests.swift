//
//  RegistryDocumentTests.swift
//  VelaWalletTests
//
//  PR 2 polish: the chain registry document
//  (`<ethereumDataURL>/chains/eip155-<id>.json`) has three outcomes — `doc`
//  (a 2xx, parsed), `absent` (a server's 404) and `unread` (no answer, a
//  timeout, a 5xx, a 429 or any other non-2xx, a body that is not one). The
//  first two are kept for the TTL; `unread` never is. On a chain with no
//  native coin (Tempo) an unread document is a chain NOT READ: failed like a
//  chain that did not answer, never "answered, holds nothing" ($0.00).
//
//  Hermetic: the fetch is injected; the round's pool is offline.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct RegistryDocumentTests {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"

    /// 2xx → `doc`; a server's 404 → `absent`; a 5xx, a 429, a timeout, a
    /// body that is not one → `unread`. Only the first two are kept.
    @Test func theRegistryDocumentHasThreeOutcomes() async {
        final class Script {
            var answers: [CoreHTTP.RestAnswer]
            var asked: [String] = []
            init(_ answers: [CoreHTTP.RestAnswer]) { self.answers = answers }
        }
        func registry(_ script: Script) -> ChainTokens {
            ChainTokens(base: { "https://data.test" }, fetch: { url in
                script.asked.append(url)
                return script.answers.isEmpty ? .failed : script.answers.removeFirst()
            })
        }
        let body: [String: Any] = [
            "stables": [["symbol": "USDC", "contract": usdc]],
            "wrappedNativeToken": "0x4200000000000000000000000000000000000006",
        ]
        // `doc`, kept: asked once.
        let found = Script([.ok(body)])
        let docs = registry(found)
        let first = await docs.document(chainId: 8453)
        #expect(first.facts?.stables == [usdc])
        #expect(first.facts?.wrappedNative == "0x4200000000000000000000000000000000000006")
        #expect(await docs.facts(chainId: 8453)?.stableRefs.first?.symbol == "USDC")
        #expect(found.asked == ["https://data.test/chains/eip155-8453.json"], "\(found.asked)")

        // `absent`, kept: a 404 is an answer.
        let missing = Script([.status(404), .ok(body)])
        let absent = registry(missing)
        guard case .absent = await absent.document(chainId: 4217) else {
            Issue.record("a 404 was not absent")
            return
        }
        guard case .absent = await absent.document(chainId: 4217) else {
            Issue.record("an absent document was not kept")
            return
        }
        #expect(missing.asked.count == 1)
        #expect(await absent.facts(chainId: 4217) == nil, "no facts")

        // `unread`, never kept: each of these is asked again next time.
        for answer: CoreHTTP.RestAnswer in [.status(500), .status(503), .status(429), .status(403), .failed, .status(200)] {
            let script = Script([answer, .ok(body)])
            let tokens = registry(script)
            #expect(await tokens.document(chainId: 4217).isUnread, "\(answer)")
            #expect(await tokens.document(chainId: 4217).facts != nil, "an unread document was kept (\(answer))")
            #expect(script.asked.count == 2)
        }
    }

    /// Tempo's rule: an unread document on a chain with no native coin is a
    /// chain NOT READ; `absent` is an answer; a chain with a native coin
    /// keeps its native-only read.
    @Test func tempoWithAnUnreadDocumentIsNotRead() async throws {
        #expect(BalanceExecutor.notRead(chainId: 4217, document: .unread, custom: []))
        #expect(!BalanceExecutor.notRead(chainId: 4217, document: .absent, custom: []))
        #expect(!BalanceExecutor.notRead(
            chainId: 4217, document: .doc(ChainTokens.Facts(stables: [], wrappedNative: nil)), custom: []
        ))
        #expect(!BalanceExecutor.notRead(chainId: 1, document: .unread, custom: []), "a native coin is still read")
        #expect(!BalanceExecutor.notRead(chainId: 8453, document: .unread, custom: []))

        // The whole round, offline: every chain with a native coin fails as
        // before; Tempo with its document unread fails too — never answered.
        func round(_ document: ChainTokens.Document) async throws -> (failed: [Int], read: [Int]) {
            let defaults = UserDefaults(suiteName: UUID().uuidString)!
            let store = VelaStore(defaults: defaults)
            let pool = RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true)
            let executor = BalanceExecutor(store: store, pool: pool, held: HeldTokens(), chainDeadlineMs: .max)
            executor.chainDocument = { _ in document }
            let reply = try CoreJSON.object(await executor.perform([
                "type": "fetch_tokens", "address": golden, "pull": false,
            ]))
            return (reply["failed_chain_ids"] as? [Int] ?? [], reply["read_chain_ids"] as? [Int] ?? [])
        }
        let unread = try await round(.unread)
        #expect(unread.failed.contains(4217), "Tempo, not read, was counted as answered: \(unread.failed)")
        #expect(unread.read.contains(4217))
        let absent = try await round(.absent)
        #expect(!absent.failed.contains(4217), "a 404 is an answer: \(absent.failed)")
    }
}
