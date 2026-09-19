//
//  RegistryChainReaderTests.swift
//  VelaWalletTests
//
//  Spec 062: signing in when the index service is gone. The REAL core reads the
//  REAL contract bytes (the fixture web recorded from Gnosis and Ethereum), so
//  what is pinned is the whole native path: plan → `eth_call` → index-shaped
//  JSON → the client's own guards. Unit ids are per deployment, which is the
//  mistake this is most likely to make: Gnosis's unit 10 is Ethereum's unit 0.
//

import Foundation
import Testing
@testable import VelaWallet

struct RegistryChainReaderTests {
    private final class Asked: @unchecked Sendable {
        private let lock = NSLock()
        private var chains: [Int] = []
        func note(_ chain: Int) { lock.lock(); chains.append(chain); lock.unlock() }
        func clear() { lock.lock(); chains.removeAll(); lock.unlock() }
        var all: Set<Int> { lock.lock(); defer { lock.unlock() }; return Set(chains) }
    }

    private struct Fixture: @unchecked Sendable {
        let publicKey: String
        let answers: [String: [String: String]]
    }

    private static let fixture: Fixture = {
        let url = ProviderBundleTests.repoRoot
            .appendingPathComponent("app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: Any] ?? [:]
        return Fixture(
            publicKey: json["publicKey"] as? String ?? "",
            answers: json["answers"] as? [String: [String: String]] ?? [:]
        )
    }()

    private var key: String { Self.fixture.publicKey }

    /// The recorded chains, minus the ones that are "down".
    private func reader(down: Set<Int> = [], asked: Asked = Asked()) -> RegistryChainReader {
        RegistryChainReader(ethCall: { chainId, _, data in
            asked.note(chainId)
            return down.contains(chainId) ? nil : Self.fixture.answers[String(chainId)]?[data]
        })
    }

    /// An index nobody can reach: a closed local port refuses at once.
    private func client(_ reader: RegistryChainReader?) -> RegistryClient {
        RegistryClient(baseURL: "http://127.0.0.1:9", chain: reader)
    }

    @Test func indexUnreachable_theKeyAndItsUnitAreReadFromGnosis() async throws {
        let asked = Asked()
        let client = client(reader(asked: asked))
        let status = try await client.queryByPublicKey(key)
        #expect(status.registered)
        #expect(status.unitIds == [12, 10, 8])

        let unit = try await client.queryUnit(10)
        #expect(unit.members.count == 3)
        #expect(unit.members.first?.publicKeyHex == key)
        #expect(unit.members.allSatisfy { !$0.credentialIdHex.isEmpty })
        // Ethereum is not asked while Gnosis answers.
        #expect(asked.all == [100])
    }

    @Test func gnosisSilentToo_ethereumAnswersUnderItsOwnUnitIds() async throws {
        let asked = Asked()
        let client = client(reader(down: [100], asked: asked))
        let status = try await client.queryByPublicKey(key)
        #expect(status.unitIds == [0])

        asked.clear()
        let unit = try await client.queryUnit(0)
        #expect(unit.members.count == 3)
        #expect(unit.members.first?.publicKeyHex == key)
        // A chain listing is continued on that chain and nowhere else.
        #expect(asked.all == [1])
    }

    @Test func nobodyAnswers_theIndexsOwnFailureIsReported() async {
        await #expect(throws: RegistryFailure.self) {
            try await client(reader(down: [100, 1])).queryByPublicKey(key)
        }
        // And with no reader at all the client is exactly what it was.
        await #expect(throws: RegistryFailure.self) { try await client(nil).queryByPublicKey(key) }
    }

    @Test func aKeyThisBuildCannotReadPlansNothing() async {
        let asked = Asked()
        #expect(await reader(asked: asked).keyProfile("not a key") == nil)
        #expect(asked.all.isEmpty)
    }

    @Test func onlyAnAbsentIndexIsReplaced_aRefusalIsAnAnswer() {
        #expect(RegistryFailure(message: "Query failed: timeout", network: true).indexIsGone)
        #expect(RegistryFailure(message: "Query failed: 503", network: false).indexIsGone)
        #expect(!RegistryFailure(message: "Query failed: 400", network: false).indexIsGone)
        #expect(!RegistryFailure(message: "Query failed: 404", network: false).indexIsGone)
        #expect(!RegistryFailure(message: "Query failed: malformed response", network: false).indexIsGone)
    }
}
