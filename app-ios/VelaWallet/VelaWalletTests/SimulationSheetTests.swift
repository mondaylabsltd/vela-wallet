//
//  SimulationSheetTests.swift
//  VelaWalletTests
//
//  The balance block, end to end: simulated logs → `token_trust`'s asymmetric
//  judgment → the block a person reads before they sign.
//
//  The asymmetry is the whole point and it is the core's, so these drive the
//  REAL machine rather than asserting on a hand-made verdict.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SimulationSheetTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let usdc = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    private func context(sim: TrustSimViewWire?, simulation: SigningController.Simulation) -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .gray, nativeSymbol: "xDAI",
            walletName: "me", walletAddress: me, origin: "https://app.example",
            sim: sim, simulation: simulation
        )
    }

    /// Drive the real machine with deltas and read back its verdict.
    ///
    /// An ERC-20 delta whose token the core has never heard of makes it ASK for
    /// metadata first and hold the verdict until the answer lands — which is
    /// the behaviour, so the test answers. `meta` says what comes back; `nil`
    /// is "nobody could name this token", which is the unverified case.
    private func judged(
        _ deltas: [[String: Any]], meta: (symbol: String, decimals: Int)? = nil
    ) throws -> TrustSimViewWire {
        // This machine has no account event — it is told what is HELD, and a
        // judgment about a delta is made from the delta and the trusted set.
        let core = TokenTrustCore()
        var result = try core.dispatch(eventJson: CoreJSON.string([
            "type": "sim_deltas_computed", "address": me, "chain_id": 100, "deltas": deltas,
        ]))
        let effects = try CoreJSON.object(result)["effects"] as? [[String: Any]] ?? []
        for effect in effects {
            guard let operation = effect["operation"] as? [String: Any],
                  operation["type"] as? String == "multicall_erc20_meta",
                  let id = (effect["id"] as? NSNumber)?.uint64Value
            else { continue }
            let addrs = operation["addrs"] as? [String] ?? []
            result = try core.resolveEffect(effectId: id, resultJson: CoreJSON.string([
                "type": "erc_meta", "chain_id": 100,
                "entries": addrs.map { addr -> [String: Any] in
                    guard let meta else { return ["addr": addr, "meta": NSNull()] }
                    return ["addr": addr,
                            "meta": ["symbol": meta.symbol, "decimals": meta.decimals]]
                },
            ]))
        }
        let decoded = try CoreJSON.decode(TrustViewWire.self, from: try view(from: result))
        return try #require(decoded.sim, "the core did not project a simulation view")
    }

    // MARK: - Drift

    /// `sim` was deliberately absent from `TrustViewWire` until this cut. Its
    /// three arms decode now, which is the only thing standing between a
    /// renamed core field and a silent empty block.
    @Test func theSimViewDecodes() throws {
        let sim = try judged([["kind": "native", "token": NSNull(), "delta": "-1000"]])
        #expect(sim.ready)
        #expect(sim.judgments.count == 1, "judgments: \(sim.judgments)")
        guard let first = sim.judgments.first else { return }
        if case .native(let delta) = first {
            #expect(delta == "-1000")
        } else {
            Issue.record("a native move was judged as something else")
        }
    }

    // MARK: - The asymmetry

    /// An OUTFLOW of an unknown token still renders as unverified — the shell
    /// never invents a symbol, and the core never hands one over it does not
    /// have.
    @Test func aTrustedOutflowRendersItsAmount() throws {
        // Metadata resolved, and it is an OUTFLOW — the real token emits its
        // own log, so an outflow cannot be understated.
        let sim = try judged(
            [["kind": "erc20", "token": usdc, "delta": "-5000000"]],
            meta: (symbol: "USDC", decimals: 6)
        )
        guard let first = sim.judgments.first, case .erc20Trusted(_, _, let symbol, let decimals) = first
        else {
            Issue.record("a named outflow was not trusted: \(sim)")
            return
        }
        #expect(symbol == "USDC")
        #expect(decimals == 6)

        let blocks = SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: sim, simulation: .answered)
        )
        guard case .balances(_, let rows, let note, _)? = blocks.first, let row = rows.first else {
            Issue.record("no balance row")
            return
        }
        #expect(row.symbol == "USDC")
        #expect(row.delta == "−5")
        #expect(note == nil)
    }

    /// **The asymmetry.** The same token, the same metadata, the other
    /// direction: an INFLOW of something not already trusted renders no number,
    /// because a `Transfer` log is something anybody can emit.
    @Test func aNamedInflowIsStillNotATrustedOne() throws {
        let sim = try judged(
            [["kind": "erc20", "token": usdc, "delta": "5000000"]],
            meta: (symbol: "USDC", decimals: 6)
        )
        guard let first = sim.judgments.first else {
            Issue.record("no judgment: \(sim)")
            return
        }
        guard case .erc20Unverified = first else {
            Issue.record("an inflow of an untrusted token rendered a confident amount")
            return
        }
    }

    @Test func anUnknownTokenIsUnverifiedEitherWay() throws {
        let out = try judged([["kind": "erc20", "token": usdc, "delta": "-5000000"]])
        guard let outFirst = out.judgments.first else {
            Issue.record("the core judged an outflow as nothing: \(out)")
            return
        }
        guard case .erc20Unverified = outFirst else {
            Issue.record("an unknown token resolved a symbol from nowhere")
            return
        }
        let into = try judged([["kind": "erc20", "token": usdc, "delta": "5000000"]])
        guard let intoFirst = into.judgments.first else {
            Issue.record("the core judged an inflow as nothing: \(into)")
            return
        }
        guard case .erc20Unverified = intoFirst else {
            Issue.record("an unknown INFLOW must never render a confident amount")
            return
        }
    }

    // MARK: - The block

    /// An unverified inflow shows its DIRECTION and the words "unverified
    /// token" and no number at all: the amount in a simulated log is whatever
    /// the site being signed for chose to emit.
    @Test func anUnverifiedInflowShowsNoNumber() throws {
        let sim = try judged([["kind": "erc20", "token": usdc, "delta": "123456789"]])
        let blocks = SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: sim, simulation: .answered)
        )
        guard case .balances(_, let rows, let note, let tone)? = blocks.first else {
            Issue.record("no balance block was built")
            return
        }
        #expect(rows.count == 1, "rows: \(rows)")
        guard let row = rows.first else { return }
        #expect(row.symbol == loc.t("componentsUi.signing.balanceUnverifiedToken"))
        #expect(row.delta == "+", "the site's own number must not be printed")
        #expect(!row.delta.contains("123"))
        #expect(note == loc.t("componentsUi.signing.unverifiedWarning"))
        #expect(tone == .caution)
    }

    /// A native move renders with the chain's symbol and 18 decimals — the
    /// shell's vocabulary, which is what the core leaves to it.
    @Test func aNativeMoveRendersItsAmount() throws {
        let sim = try judged([["kind": "native", "token": NSNull(), "delta": "-1500000000000000000"]])
        let blocks = SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: sim, simulation: .answered)
        )
        guard case .balances(let title, let rows, let note, _)? = blocks.first else {
            Issue.record("no balance block was built")
            return
        }
        #expect(title == loc.t("componentsUi.signing.balanceChangesTitle"))
        guard let row = rows.first else {
            Issue.record("a native move produced no row")
            return
        }
        #expect(row.symbol == "xDAI")
        #expect(row.delta == "−1.5")
        #expect(note == nil, "nothing unverified moved, so there is nothing to caution about")
    }

    /// "It ran and nothing moved" is a SENTENCE, not an absence.
    @Test func nothingMovingIsSaidOutLoud() throws {
        let sim = try judged([])
        let blocks = SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: sim, simulation: .answered)
        )
        guard case .balances(_, let rows, let note, _)? = blocks.first else {
            Issue.record("an empty simulation said nothing at all")
            return
        }
        #expect(rows.isEmpty)
        #expect(note == loc.t("componentsUi.signing.balanceNoAssetsMove"))
    }

    /// **The one that matters.** A node that could not simulate is a DANGER
    /// warning, never silence: a wallet that says nothing when it could not
    /// look teaches people that silence means safe.
    @Test func aSimulationThatCouldNotRunWarns() {
        let blocks = SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: nil, simulation: .unavailable)
        )
        guard case .warning(let tone, let text)? = blocks.first else {
            Issue.record("a refused simulation was silent")
            return
        }
        #expect(tone == .danger)
        #expect(text == loc.t("componentsUi.signing.simUnavailableWarning"))
    }

    /// Still running is silence — a block that appears and then changes its
    /// mind is worse than one that arrives late.
    @Test func aPendingSimulationIsQuiet() {
        #expect(SigningLive.balanceBlocks(
            isTransaction: true, context: context(sim: nil, simulation: .pending)
        ).isEmpty)
    }

    /// A MESSAGE moves nothing, so it gets no block — not even the warning.
    @Test func aMessageSheetHasNoBalanceBlock() {
        #expect(SigningLive.balanceBlocks(
            isTransaction: false, context: context(sim: nil, simulation: .unavailable)
        ).isEmpty)
    }

    // MARK: - Invariant ⑤

    /// **A simulation may never admit a token.** The core enforces it; this
    /// asserts the enforcement from the shell's side, because the shell is
    /// where the temptation lives — the deltas arrive with an address and a
    /// balance, which is everything `manage_tokens` would need.
    @Test func nothingSimulatedEverEntersTheTokenList() throws {
        let core = TokenTrustCore()
        let computed = try core.dispatch(eventJson: CoreJSON.string([
            "type": "sim_deltas_computed", "address": me, "chain_id": 100,
            "deltas": [["kind": "erc20", "token": usdc, "delta": "1000000"]],
        ]))
        // Not one operation, and certainly not a write. The simulation's whole
        // effect is a projection.
        let effects = try CoreJSON.object(computed)["effects"] as? [[String: Any]] ?? []
        let writes = effects.compactMap { ($0["operation"] as? [String: Any])?["type"] as? String }
            .filter { $0.contains("write") || $0.contains("admit") || $0.contains("save") }
        #expect(writes.isEmpty, "a simulated delta asked for \(writes)")
    }
}
