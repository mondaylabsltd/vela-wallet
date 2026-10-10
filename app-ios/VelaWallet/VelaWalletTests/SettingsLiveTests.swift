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

    private func contract(
        _ name: String,
        _ deployed: Bool,
        multiKeyOnly: Bool = false
    ) -> NetContractStatusWire {
        NetContractStatusWire(
            name: name, address: "0x", deployed: deployed, multiKeyOnly: multiKeyOnly
        )
    }

    /// The core's twelve, with every one deployed unless named otherwise. The
    /// last two are the passkey-signer pair only a multi-key wallet needs
    /// (spec 081 FR-009); the CompatibilityFallbackHandler left the list,
    /// because a Vela account's fallback handler is the 4337 module.
    private func contracts(missing: Set<String> = []) -> [NetContractStatusWire] {
        let core = [
            "Deterministic Deployment Proxy", "Safe Singleton Factory", "Multicall3",
            "EntryPoint v0.7", "Safe L2", "Safe Proxy Factory", "Safe 4337 Module",
            "Safe Module Setup", "WebAuthn Signer", "MultiSend",
        ].map { contract($0, !missing.contains($0)) }
        let multiKey = [
            "Safe Passkey Signer Factory", "Safe Passkey Signer Singleton",
        ].map { contract($0, !missing.contains($0), multiKeyOnly: true) }
        return core + multiKey
    }

    /// A finished check, with the core's own ruling on a refusal
    /// (`net_blocker`): no P-256 wins over missing contracts, and only
    /// missing contracts carries Chain Setup's address.
    private func compat(
        compatible: Bool,
        missing: Set<String> = [],
        latency: Double? = 182,
        p256: Bool? = true
    ) -> NetCompatibilityWire {
        let noP256 = !compatible && p256 == false
        return NetCompatibilityWire(
            chainId: 7_777_777, compatible: compatible,
            multiKeyReady: compatible && missing.isEmpty,
            contracts: contracts(missing: missing),
            p256Available: p256, bestRpcUrl: "https://rpc.test",
            bestRpcLatencyMs: latency, rpcFailure: nil,
            blocker: compatible ? nil : (noP256 ? "no_p256" : "missing_contracts"),
            hintKey: compatible ? nil : "settingsModals.addNetwork." + (noP256 ? "noP256Hint" : "incompatibleHint"),
            setupUrl: compatible || noP256 ? nil : "https://getvela.app/chain-setup?chain=7777777"
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

    /// A chain missing contracts gets the re-check, never a greyed accent
    /// CTA: an action you cannot take should not be dressed as the action you
    /// came for. And "Open Chain Setup Tool" — on the page for THIS chain,
    /// the core's `setup_url` (PR 3; live Settings never offered it, and the
    /// drawing's button went to the tool's front page).
    @Test func aChainMissingContractsOffersChainSetupForThatChain() {
        let model = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: compat(compatible: false, missing: ["Multicall3"]), canAdd: false),
            loc: loc, fallback: fallback()
        )
        #expect(model.primary == nil)
        #expect(model.secondary == loc.t("settingsModals.addNetwork.openChainSetupTool"))
        #expect(model.secondaryUrl == "https://getvela.app/chain-setup?chain=7777777")
        #expect(model.recheck != nil)
        #expect(model.callout?.text == loc.t("settingsModals.addNetwork.incompatibleHint"))
    }

    /// A chain with no P-256 verifier is told so plainly — Vela wallets
    /// cannot work there, and money sent would be stuck — with NO Chain
    /// Setup button: a precompile is the chain's own to add, and the old
    /// line sent people to a tool that could not help. Its row says ✗ while
    /// every contract ticks.
    @Test func aChainWithNoP256VerifierSaysSoAndOffersNoSetupTool() {
        let refused = compat(compatible: false, p256: false)
        let model = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: refused, canAdd: false),
            loc: loc, fallback: fallback()
        )
        #expect(model.primary == nil)
        #expect(model.secondary == nil, "a button to a tool that cannot help")
        #expect(model.secondaryUrl == nil)
        #expect(model.recheck != nil, "another RPC may answer differently")
        #expect(model.callout?.text == loc.t("settingsModals.addNetwork.noP256Hint"))
        #expect(model.callout?.text != loc.t("settingsModals.addNetwork.incompatibleHint"))
        #expect(model.candidate?.badge?.tone == .error)

        let rows = SettingsLive.checks(refused, loc: loc)
        #expect(rows.map(\.ok) == [true, true, false, true], "only the precompile's row fails: \(rows)")
        #expect(rows[2].label == loc.t("settingsModals.addNetwork.checkSigner"))
    }

    /// The reason rides on the wire under these names; an absent one (a core
    /// from before PR 3) still decodes, and keeps the contracts line.
    @Test func theRefusalsReasonDecodesAndAnAbsentOneIsTolerated() throws {
        func decode(_ extra: [String: Any]) throws -> NetCompatibilityWire {
            var json: [String: Any] = [
                "chain_id": 5, "compatible": false, "multi_key_ready": false, "contracts": [],
                "p256_available": false, "best_rpc_url": NSNull(), "best_rpc_latency_ms": NSNull(),
                "rpc_failure": NSNull(),
            ]
            json.merge(extra) { _, new in new }
            return try CoreJSON.decode(NetCompatibilityWire.self, from: json)
        }
        let noP256 = try decode([
            "blocker": "no_p256", "hint_key": "settingsModals.addNetwork.noP256Hint", "setup_url": NSNull(),
        ])
        #expect(noP256.blocker == "no_p256")
        #expect(noP256.hintKey == "settingsModals.addNetwork.noP256Hint")
        #expect(noP256.setupUrl == nil)
        let missing = try decode([
            "blocker": "missing_contracts", "hint_key": "settingsModals.addNetwork.incompatibleHint",
            "setup_url": "https://getvela.app/chain-setup?chain=5",
        ])
        #expect(SettingsLive.refusal(missing, loc: loc)?.setup?.url == "https://getvela.app/chain-setup?chain=5")
        // A reason this build has never heard of is still a refusal with a line.
        #expect(try decode(["blocker": "something_new"]).blocker == "something_new")
        let old = try decode([:])
        #expect(old.blocker == nil && old.hintKey == nil && old.setupUrl == nil)
        #expect(SettingsLive.refusal(old, loc: loc)?.callout.text == loc.t("settingsModals.addNetwork.incompatibleHint"))
        #expect(SettingsLive.refusal(old, loc: loc)?.setup == nil)
    }

    // MARK: - The four drawn rows over the core's contracts

    /// Everything deployed → four ticks: EntryPoint, Safe L2, the P-256
    /// precompile, and the count of the other eight. Eight: the ten a
    /// one-key wallet needs less the two named (the WebAuthn Signer contract
    /// is counted here now — the third row is the precompile, as its words
    /// say); spec 081 moved the two passkey-signer contracts out of this
    /// count into their own sentence.
    @Test func theChecksSummariseTheContractsIntoTheDrawnFour() {
        let rows = SettingsLive.checks(compat(compatible: true), loc: loc)
        #expect(rows.count == 4)
        #expect(rows.allSatisfy { $0.ok })
        #expect(rows[0].label == "EntryPoint v0.7")
        #expect(rows[2].label == loc.t("settingsModals.addNetwork.checkSigner"))
        #expect(rows[3].label.contains("8"), "the remaining row must count 8, got \(rows[3].label)")

        // Never probed is not "absent": the row is left out, not failed.
        let unprobed = SettingsLive.checks(compat(compatible: true, p256: nil), loc: loc)
        #expect(unprobed.count == 3)
        #expect(!unprobed.contains { $0.label == loc.t("settingsModals.addNetwork.checkSigner") })
    }

    /// Spec 081 FR-009. A chain with everything but Safe's passkey signer
    /// factory is a working chain for a one-key wallet — the rows must stay
    /// green — and the callout is what tells somebody with several keys that
    /// their wallet cannot be created there.
    @Test func aChainWithoutThePasskeyFactoryKeepsItsTicksAndSaysWhatIsMissing() {
        let partial = compat(compatible: true, missing: ["Safe Passkey Signer Factory"])
        let rows = SettingsLive.checks(partial, loc: loc)
        #expect(rows.allSatisfy { $0.ok }, "a one-key wallet works here")

        let model = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: partial, canAdd: true),
            loc: loc, fallback: fallback()
        )
        #expect(model.callout?.tone == .warning)
        #expect(model.candidate?.badge?.tone == .ok, "the chain is still compatible")

        let whole = compat(compatible: true)
        let wholeModel = SettingsLive.wizard(
            wizardView(chainInfo: zora, compat: whole, canAdd: true),
            loc: loc, fallback: fallback()
        )
        #expect(wholeModel.callout == nil, "nothing extra to say about a whole chain")
    }

    /// A missing contract fails the row that names it, and only that row.
    /// "Incompatible" is only legible as an answer if it shows WHICH
    /// requirement failed.
    @Test func aMissingContractFailsExactlyTheRowThatNamesIt() {
        let safe = SettingsLive.checks(compat(compatible: false, missing: ["Safe L2"]), loc: loc)
        #expect(safe.map(\.ok) == [true, false, true, true], "the Safe row must fail, alone")

        // The signer CONTRACT is one of the counted; the precompile's row is
        // not its row, and stays ticked while the precompile answers.
        let signer = SettingsLive.checks(compat(compatible: false, missing: ["WebAuthn Signer"]),
                                         loc: loc)
        #expect(signer.map(\.ok) == [true, true, true, false])

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
