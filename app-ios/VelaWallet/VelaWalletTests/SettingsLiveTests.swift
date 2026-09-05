//
//  SettingsLiveTests.swift
//  VelaWalletTests
//
//  The settings live builder: what the drawing shows for a given core verdict.
//
//  Two things are worth testing here and the rest is the core's. First, that
//  the **add gate is never re-derived** — a screen that offers 添加 for a chain
//  the core will refuse is the worst failure this surface has. Second, that the
//  four drawn check rows line up with the eleven contracts the core actually
//  reports, because that mapping is the one place a display decision touches a
//  security verdict.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SettingsLiveTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func contract(_ name: String, _ deployed: Bool) -> NetContractStatusWire {
        NetContractStatusWire(name: name, address: "0x", deployed: deployed)
    }

    /// The core's eleven, with every one deployed unless named otherwise.
    private func contracts(missing: Set<String> = []) -> [NetContractStatusWire] {
        [
            "Deterministic Deployment Proxy", "Safe Singleton Factory", "Multicall3",
            "EntryPoint v0.7", "Safe L2", "Safe Proxy Factory", "Safe 4337 Module",
            "Safe Module Setup", "WebAuthn Signer", "Fallback Handler", "MultiSend",
        ].map { contract($0, !missing.contains($0)) }
    }

    private func compat(
        compatible: Bool,
        missing: Set<String> = [],
        latency: Double? = 182
    ) -> NetCompatibilityWire {
        NetCompatibilityWire(
            chainId: 7_777_777, compatible: compatible, contracts: contracts(missing: missing),
            p256Available: compatible, bestRpcUrl: "https://rpc.test",
            bestRpcLatencyMs: latency, rpcFailure: nil
        )
    }

    private func wizardView(
        phase: NetWizardPhaseWire = .checked,
        chainInfo: NetChainInfoWire? = nil,
        compat: NetCompatibilityWire? = nil,
        error: NetWizardErrorWire? = nil,
        canAdd: Bool = false,
        suggestions: [NetChainIndexEntryWire] = []
    ) -> NetWizardViewWire {
        NetWizardViewWire(
            phase: phase, query: "", customRpc: "", suggestions: suggestions,
            chainInfo: chainInfo, compat: compat, error: error, canAdd: canAdd
        )
    }

    private var zora: NetChainInfoWire {
        NetChainInfoWire(
            chainId: 7_777_777, name: "Zora", shortName: "zora", nativeName: "Ether",
            nativeSymbol: "ETH", nativeDecimals: 18, rpcUrl: "https://rpc.zora.energy",
            rpcUrls: [], explorerUrl: "https://explorer.zora.energy", logoUrl: "",
            isTestnet: false
        )
    }

    private func fallback() -> AddNetworkModel {
        SettingsFixtures.build(.st10, loc: loc).addNetwork
    }

    // MARK: - The add gate

    /// **`can_add` is the core's word and the only word.**
    ///
    /// A compatible-looking verdict with the gate shut must still not offer the
    /// button: the core refuses a candidate whose compatibility it never
    /// verified, and a screen with a second opinion is how somebody is offered
    /// an action that then fails.
    @Test func theAddButtonAppearsOnlyWhenTheCoreOpensTheGate() {
        let shut = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: compat(compatible: true), canAdd: false),
            loc: loc, fallback: fallback()
        )
        #expect(shut.primary == nil, "offered 添加 while the core's gate was shut")

        let open = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: compat(compatible: true), canAdd: true),
            loc: loc, fallback: fallback()
        )
        #expect(open.primary != nil)
    }

    /// An incompatible chain gets the outline pair, never a greyed accent CTA:
    /// an action you cannot take should not be dressed as the action you came
    /// for.
    @Test func anIncompatibleChainOffersTheOutlinePairInstead() {
        let model = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: compat(compatible: false), canAdd: false),
            loc: loc, fallback: fallback()
        )
        #expect(model.primary == nil)
        #expect(model.secondary != nil)
        #expect(model.recheck != nil)
        #expect(model.callout != nil)
    }

    // MARK: - The four drawn rows over eleven contracts

    /// All eleven deployed → four ticks, and the fourth counts the other eight.
    @Test func theChecksSummariseElevenContractsIntoTheDrawnFour() {
        let rows = SettingsLive.checks(compat(compatible: true), loc: loc)
        #expect(rows.count == 4)
        #expect(rows.allSatisfy { $0.ok })
        #expect(rows[0].label == "EntryPoint v0.7")
        #expect(rows[3].label.contains("8"), "the remaining row must count 8, got \(rows[3].label)")
    }

    /// A missing contract fails the row that names it, and only that row.
    /// "Incompatible" is only legible as an answer if it shows WHICH
    /// requirement failed.
    @Test func aMissingContractFailsExactlyTheRowThatNamesIt() {
        let signer = SettingsLive.checks(compat(compatible: false, missing: ["WebAuthn Signer"]),
                                         loc: loc)
        #expect(signer[0].ok)
        #expect(signer[1].ok)
        #expect(!signer[2].ok, "the signer row must fail")
        #expect(signer[3].ok)

        let counted = SettingsLive.checks(compat(compatible: false, missing: ["Multicall3"]),
                                          loc: loc)
        #expect(counted[0].ok && counted[1].ok && counted[2].ok)
        #expect(!counted[3].ok, "a missing contract inside the counted eight must fail that row")
    }

    // MARK: - Rows and badges

    /// An unmeasured endpoint gets no pill. Painting a green dot before the
    /// probe answers is the screen making a claim the core has not.
    @Test func anUnmeasuredEndpointHasNoHealthPill() {
        #expect(SettingsLive.badge(nil) == nil)
        #expect(SettingsLive.badge(.checking)?.tone == .neutral)
        #expect(SettingsLive.badge(.ok(latencyMs: 45))?.label == "45ms")
        #expect(SettingsLive.badge(.ok(latencyMs: 1_500))?.tone == .warn)
        #expect(SettingsLive.badge(.error)?.tone == .error)
    }

    /// A chain nobody drew is never handed somebody else's brand.
    @Test func anUndrawnChainGetsTheNeutralMark() {
        #expect(SettingsLive.mark(chainId: 1, name: "Ethereum").color == ChainPalette.ethereum)
        #expect(SettingsLive.mark(chainId: 4217, name: "Tempo").color == ChainPalette.unbranded)
        #expect(SettingsLive.mark(chainId: 999_999, name: "Zora").letter == "Z")
    }

    /// The wizard's search list carries the core's chain ids, because a tap has
    /// to be able to name the chain it selected — the row's `id` is a slug.
    @Test func searchResultsCarryTheChainIdATapNeeds() {
        let model = SettingsLive.wizard(
            wizardView(phase: .suggested, suggestions: [
                NetChainIndexEntryWire(chainId: 7_777_777, name: "Zora", shortName: "zora",
                                       nativeCurrencySymbol: "ETH", hasLogo: false),
            ]),
            loc: loc, fallback: fallback()
        )
        #expect(model.results.first?.chainId == 7_777_777)
        #expect(model.candidate == nil)
    }

    /// The core's refusals get the corpus sentences that say the right thing —
    /// including the three whose keys live under `addToken.*` for historical
    /// reasons.
    @Test func everyRefusalKindRendersARealSentence() {
        let kinds: [NetWizardErrorWire] = [
            .alreadyAdded(chainId: 1), .notFound(chainId: 1),
            .noRpcEndpoint, .notCompatible(chainId: 1),
        ]
        for kind in kinds {
            let model = SettingsLive.wizard(
                wizardView(phase: .error, error: kind), loc: loc, fallback: fallback()
            )
            let text = model.callout?.text ?? ""
            #expect(!text.isEmpty, "no wording for \(kind)")
            #expect(!text.contains("."), "an unresolved corpus key echoed: \(text)")
        }
    }
}
