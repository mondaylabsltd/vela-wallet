//
//  RegistryResolverTests.swift
//  VelaWalletTests
//
//  The three layers (067), through `RegistryClient`, with the REAL core over the
//  REAL bytes both chains answered (the fixture web recorded) and an index that
//  says whatever the test tells it to.
//
//  The rules are `vela_core::registry_resolve` and are tested there. Pinned HERE
//  is that this shell really hands its reads to that walk: an index answer is
//  PROVED rather than believed, a forged one is thrown away, and a listing's
//  unit ids are asked of whoever listed them (Gnosis's unit 10 is Ethereum's 0).
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// The index, answered from memory: a listing for `?publicKey=`, a unit for
/// `?unitId=`, a 503 for whichever is `nil`.
///
/// Handed to `RegistryClient` as its transport. It was a `URLProtocol` stub
/// under `URLSession.shared`, which kept URLSession's 15 s idle timer under
/// every assertion here — and on a CI run where the whole unit suite was
/// starved (these 0.1 s tests took 57 s) the stubbed index "timed out", the
/// walk fell back to the chain, and two tests failed on the clock
/// (2026-09-28). Nothing here waits on a clock now: an answer is returned in
/// the call that asked for it.
struct IndexScript: Sendable {
    var listing: String?
    var unit: String?

    var transport: RegistryClient.Transport {
        { request in
            let body = (request.url?.query ?? "").contains("publicKey=") ? listing : unit
            guard let url = request.url,
                  let response = HTTPURLResponse(
                      url: url, statusCode: body == nil ? 503 : 200, httpVersion: nil, headerFields: nil
                  )
            else { throw URLError(.badURL) }
            return (Data((body ?? "{}").utf8), response)
        }
    }

    /// An index nobody can reach: the request never arrives, at once.
    static let unreachable: RegistryClient.Transport = { _ in throw URLError(.cannotConnectToHost) }
}

struct RegistryResolverTests {
    private final class Asked: @unchecked Sendable {
        private let lock = NSLock()
        private var chains: [Int] = []
        func note(_ chain: Int) { lock.lock(); chains.append(chain); lock.unlock() }
        func clear() { lock.lock(); chains.removeAll(); lock.unlock() }
        var all: [Int] { lock.lock(); defer { lock.unlock() }; return chains }
    }

    private struct Fixture: @unchecked Sendable {
        let publicKey: String
        let answers: [String: [String: String]]
    }

    private static let fixture: Fixture = {
        // The repo root, from this file (`ProviderBundleTests` held it until
        // spec 070 retired the bundled provider).
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
        let url = repoRoot
            .appendingPathComponent("app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: Any] ?? [:]
        return Fixture(
            publicKey: json["publicKey"] as? String ?? "",
            answers: json["answers"] as? [String: [String: String]] ?? [:]
        )
    }()

    private var key: String { Self.fixture.publicKey }
    private static let listing = #"{"entry":{},"groups":{"total":1,"unitIds":[10]}}"#

    /// The recorded chains, minus the ones that are "down".
    private func resolver(down: Set<Int> = [], asked: Asked = Asked()) -> RegistryResolver {
        RegistryResolver(ethCall: { chainId, _, data in
            asked.note(chainId)
            return down.contains(chainId) ? nil : Self.fixture.answers[String(chainId)]?[data]
        })
    }

    /// An index nobody can reach.
    private func unreachable(_ resolver: RegistryResolver?) -> RegistryClient {
        RegistryClient(baseURL: "https://index.test", resolver: resolver, transport: IndexScript.unreachable)
    }

    private func served(_ script: IndexScript, _ resolver: RegistryResolver) -> RegistryClient {
        RegistryClient(baseURL: "https://index.test", resolver: resolver, transport: script.transport)
    }

    /// Gnosis unit 10 in the index's shape, built from the CHAIN's own bytes —
    /// what an honest index returns.
    private func honestUnit() throws -> [String: Any] {
        let plan = try #require(Self.object(registryChainUnitPlan(unitId: 10)))
        let calls = try #require(plan["calls"] as? [[String: Any]])
        let gnosis = try #require(Self.fixture.answers["100"])
        let hexes = try calls.map { try #require(gnosis[$0["data"] as? String ?? ""]) }
        return try #require(Self.object(registryChainUnit(unitId: 10, unitHex: hexes[0], membersHex: hexes[1])))
    }

    private static func object(_ json: String?) -> [String: Any]? {
        guard let json else { return nil }
        return (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
    }

    private static func text(_ object: [String: Any]) -> String {
        String(decoding: (try? JSONSerialization.data(withJSONObject: object)) ?? Data(), as: UTF8.self)
    }

    @Test func anHonestIndexIsProvedByOneCallToGnosis() async throws {
        let asked = Asked()
        let client = served(IndexScript(listing: Self.listing, unit: Self.text(try honestUnit())), resolver(asked: asked))
        #expect(try await client.queryByPublicKey(key).unitIds == [10])
        asked.clear()
        #expect(try await client.queryUnit(10).members.count == 3)
        #expect(asked.all == [100])
    }

    @Test func aForgedMemberIsCaught_theChainsFoundingSetIsReturned() async throws {
        let honest = try honestUnit()
        var forged = honest
        var box = try #require(forged["members"] as? [String: Any])
        var items = try #require(box["items"] as? [[String: Any]])
        items[2]["publicKey"] = items[1]["publicKey"]
        box["items"] = items
        forged["members"] = box

        let client = served(IndexScript(listing: Self.listing, unit: Self.text(forged)), resolver())
        _ = try await client.queryByPublicKey(key)
        let members = try await client.queryUnit(10).members.map(\.publicKeyHex)
        let real = ((honest["members"] as? [String: Any])?["items"] as? [[String: Any]])?
            .compactMap { $0["publicKey"] as? String }
        #expect(members == real)
        #expect(Set(members).count == 3)
    }

    @Test func noChainReachable_thePersonStillSignsInOnTheIndexsWord() async throws {
        let client = served(
            IndexScript(listing: Self.listing, unit: Self.text(try honestUnit())), resolver(down: [100, 1])
        )
        _ = try await client.queryByPublicKey(key)
        #expect(try await client.queryUnit(10).members.count == 3)
    }

    @Test func aFailingIndexIsCheckedWithTheContract() async throws {
        let failing = served(IndexScript(listing: nil, unit: nil), resolver())
        #expect(try await failing.queryByPublicKey(key).unitIds == [12, 10, 8])
        let denying = served(
            IndexScript(listing: #"{"entry":null,"groups":{"total":0,"unitIds":[]}}"#, unit: nil), resolver()
        )
        #expect(try await denying.queryByPublicKey(key).unitIds == [12, 10, 8])
    }

    @Test func indexUnreachable_theKeyAndItsUnitAreReadFromGnosis() async throws {
        let asked = Asked()
        let client = unreachable(resolver(asked: asked))
        let status = try await client.queryByPublicKey(key)
        #expect(status.registered)
        #expect(status.unitIds == [12, 10, 8])
        // PR 2: who vouched travels to the core untouched — Gnosis here.
        #expect(status.verifiedBy == "gnosis")

        let unit = try await client.queryUnit(10)
        #expect(unit.members.count == 3)
        #expect(unit.members.first?.publicKeyHex == key)
        #expect(unit.members.allSatisfy { !$0.credentialIdHex.isEmpty })
        // Ethereum is not asked while Gnosis answers.
        #expect(Set(asked.all) == [100])
    }

    @Test func gnosisSilentToo_ethereumAnswersUnderItsOwnUnitIds() async throws {
        let asked = Asked()
        let client = unreachable(resolver(down: [100], asked: asked))
        let status = try await client.queryByPublicKey(key)
        #expect(status.unitIds == [0])
        // Ethereum's backup answered: never Gnosis's verdict, so never a
        // rebuild offer.
        #expect(status.verifiedBy == "ethereum")

        asked.clear()
        let unit = try await client.queryUnit(0)
        #expect(unit.members.count == 3)
        #expect(unit.members.first?.publicKeyHex == key)
        // A chain listing is continued on that chain and nowhere else.
        #expect(Set(asked.all) == [1])
    }

    @Test func nobodyAnswers_theIndexsOwnFailureIsReported() async {
        do {
            _ = try await unreachable(resolver(down: [100, 1])).queryByPublicKey(key)
            Issue.record("expected the index's failure")
        } catch let failure as RegistryFailure {
            #expect(failure.network)
        } catch {
            Issue.record("unexpected \(error)")
        }
        // And with no resolver at all the client is exactly what it was.
        await #expect(throws: RegistryFailure.self) { try await unreachable(nil).queryByPublicKey(key) }
    }

    /// PR 2: an index "nothing" with no chain to check it stands as the
    /// index's word — vouched for by nobody (`none`), which the core reads as
    /// "no verdict": no rebuild, no one-key wallet. A Gnosis-checked
    /// listing says `gnosis`; the index-only REST path (no resolver) `none`.
    @Test func whoVouchedForAKeyListingReachesTheCore() async throws {
        let nothing = #"{"entry":null,"groups":{"total":0,"unitIds":[]}}"#
        let unchecked = try await served(IndexScript(listing: nothing, unit: nil), resolver(down: [100, 1]))
            .queryByPublicKey(key)
        #expect(!unchecked.registered)
        #expect(unchecked.verifiedBy == "none")
        let checked = try await served(IndexScript(listing: nothing, unit: nil), resolver())
            .queryByPublicKey(key)
        #expect(checked.unitIds == [12, 10, 8])
        #expect(checked.verifiedBy == "gnosis")
        let legacy = try await RegistryClient(
            baseURL: "https://index.test", resolver: nil,
            transport: IndexScript(listing: Self.listing, unit: nil).transport
        ).queryByPublicKey(key)
        #expect(legacy.verifiedBy == "none")
    }
}
