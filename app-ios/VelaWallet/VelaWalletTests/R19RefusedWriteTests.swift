//
//  R19RefusedWriteTests.swift
//  VelaWalletTests
//
//  r18-8 (§28, §32): a test build's write the guard refused is its own kind.
//  The shell tells the core so (`refused: true` on `http_unreachable`,
//  `index_failed` and `registry_landing_unconfirmed`), and every sentence
//  the core then names is the test build's own
//  (`common.testBuild.productionWriteRefused`) — never "can't be reached",
//  "didn't answer" or "offline", which only a service that did not answer is.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct R19RefusedWriteTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }
    private var sentence: String { loc.t("common.testBuild.productionWriteRefused") }

    /// What the guard throws for a refused write (`Dev/ProductionWriteGuard.swift`).
    private static let guardRefusal = ProductionWriteRefused.error(host: "safe-genesis.getvela.app")

    /// Never the words of a service that did not answer.
    private func saysTheTestBuild(_ line: String) {
        #expect(line == sentence, "\(line)")
        let lower = line.lowercased()
        for outage in ["can't be reached", "can’t be reached", "didn't answer", "didn’t answer", "offline"] {
            #expect(!lower.contains(outage), "\(line)")
        }
    }

    @Test func theCatalogHasTheTestBuildsSentence() {
        #expect(productionWriteRefusedKey() == "common.testBuild.productionWriteRefused")
        #expect(sentence != "common.testBuild.productionWriteRefused")
        #expect(sentence.contains("test build"))
    }

    // MARK: - The shell tells the core

    @Test func aRefusedServiceWriteReachesTheCoreAsRefused() async {
        let ports = FakeMultisigPorts()
        ports.httpAnswer = .failure(Self.guardRefusal)
        let refused = await MultisigExecutor(ports: ports).answer([
            "type": "http", "method": "POST", "url": "https://safe-genesis.getvela.app/api/import", "body": "{}",
        ])
        #expect(refused["type"] as? String == "http_unreachable")
        #expect(refused["refused"] as? Bool == true)

        // A network that did not answer is not refused: the field stays absent.
        ports.httpAnswer = .failure(URLError(.notConnectedToInternet))
        let away = await MultisigExecutor(ports: ports).answer([
            "type": "http", "method": "POST", "url": "https://safe-genesis.getvela.app/api/import", "body": "{}",
        ])
        #expect(away["type"] as? String == "http_unreachable")
        #expect(away["refused"] == nil)
    }

    @Test func aRefusedKeyIndexWriteReachesTheCoreAsRefused() async throws {
        // The guard's own refusal, as its URL protocol fails the request: the
        // transport never reaches anything.
        let client = RegistryClient(baseURL: "https://p256-index-v2.getvela.app", transport: { _ in
            throw ProductionWriteRefused.error(host: "p256-index-v2.getvela.app")
        })
        do {
            _ = try await client.memberChallenge(groupPublicKey: "0x04", publicKey: "0x04", attestation: "", rpId: "getvela.app")
            Issue.record("the challenge went out")
        } catch let failure as RegistryFailure {
            #expect(failure.refused)
            #expect(!failure.network)
            let indexFailed = OnboardingExecutor.failure(for: ["type": "sign_member_proof"], error: failure)
            #expect(indexFailed["type"] as? String == "index_failed")
            #expect(indexFailed["refused"] as? Bool == true)
            #expect(indexFailed["network"] as? Bool == false)
            let landing = SessionExecutor.landingUnconfirmed(failure)
            #expect(landing["type"] as? String == "registry_landing_unconfirmed")
            #expect(landing["refused"] as? Bool == true)
        }
        // An index that did not answer: no `refused`.
        let away = RegistryFailure(message: "Challenge failed: offline", network: true)
        #expect(OnboardingExecutor.failure(for: ["type": "sign_member_proof"], error: away)["refused"] == nil)
        #expect(SessionExecutor.landingUnconfirmed(away)["refused"] == nil)
    }

    // MARK: - The core's words for it

    @Test func theKeptKeysSheetSaysTheTestBuildsSentence() {
        // The core's `PromptKind::key_kept_refused`.
        let kind = PromptKind(json: [
            "type": "create_failed", "detail": "Challenge: refused", "key_kept": true, "local": false,
            "words_key": "common.testBuild.productionWriteRefused",
        ])
        saysTheTestBuild(promptCopy(kind, loc: loc).message)
        // Unconfirmed for the index's reason: its own words, as before.
        let unconfirmed = PromptKind(json: ["type": "create_failed", "detail": "x", "key_kept": true, "local": false])
        #expect(promptCopy(unconfirmed, loc: loc).message == loc.t(I18nKeys.Create.keyKeptBodyIndex))
    }

    @Test func theCreatedMultisigsRefusedRecordSaysTheTestBuildsSentence() {
        saysTheTestBuild(MultisigCreateSheet.sharedLine(
            shared: false, solo: false, refused: true, working: false, words: words,
            refusedWord: "production_write_refused"
        ))
        // A record the service answered and did not take: "Share it again".
        #expect(MultisigCreateSheet.sharedLine(shared: false, solo: false, refused: true, working: false, words: words,
                                               refusedWord: "the service said no")
            == words.t("settings.shareAgainNote"))
    }

    @Test func aRefusedShareAgainSaysTheTestBuildsSentence() {
        // The core's `multisig_failure::production_write_refused()`.
        let detail = #"{"kind":"production_write_refused"}"#
        #expect(MsRules.failure(detail).key == "common.testBuild.productionWriteRefused")
        saysTheTestBuild(words.failureText(detail))
    }

    @Test func aRefusedOutboxEntrySaysTheTestBuildsSentence() throws {
        let item = MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [MultisigViewsTests.two],
                                           canExecute: [MultisigViewsTests.two])
        let json = MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":1,"threshold":2}"#, extra: ""
        )
        let view = try #require(MultisigMachine.decode(
            MsProposalsView.self, from: try JSONSerialization.jsonObject(with: Data(json.utf8))
        ))
        let model = MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        saysTheTestBuild(model.outboxLine(MsJSON(any: ["why": "production_write_refused", "retryAt": NSNull()])))
    }
}
