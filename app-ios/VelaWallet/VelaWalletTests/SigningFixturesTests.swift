//
//  SigningFixturesTests.swift
//  VelaWalletTests
//
//  Spec 022 gates for the signing layer.
//
//  Two of these are product contracts rather than style checks — the slide is
//  the only confirmation, and an unlimited approval can never be confirmed as
//  requested — so they are asserted here: a later refactor has to break a test
//  to break the promise.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SigningFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func model(_ state: SigningStateId) -> SigningModel {
        SigningFixtures.build(state, loc: loc)
    }

    /// Every string a scenario carries, flattened.
    private func strings(_ m: SigningModel) -> [String] {
        var out = [
            m.dapp.name, m.dapp.host, m.network.name,
            m.signer.label, m.signer.name,
            m.confirm.hint, m.confirm.action, m.panelTitle, m.tech.title,
        ]
        for block in m.blocks {
            switch block {
            case .intent(let text, _): out.append(text)
            case .amount(let line, _, let note):
                out += [line.value, line.symbol] + [line.caption, line.fiat, note].compactMap { $0 }
            case .swap(let pay, let receive):
                out += [pay.symbol, receive.symbol]
                out += [pay.caption, receive.caption, pay.fiat, receive.fiat].compactMap { $0 }
            case .nft(let id, let collection): out += [id, collection]
            case .sentence(let text, _): out.append(text)
            case .allowance(let label, let value, _, let chips, let note, let total, let custom):
                out += [label, value] + chips.map(\.label)
                if let note { out.append(note) }
                if let total { out += [total.label, total.value] }
                if let custom { out += [custom.placeholder, custom.symbol] + [custom.error].compactMap { $0 } }
            case .party(let label, let name, let address, let badge):
                out += [label, name] + [address, badge?.text].compactMap { $0 }
            case .rows(let rows): out += rows.flatMap { [$0.label, $0.value] }
            case .warning(_, let text): out.append(text)
            case .positive(let text): out.append(text)
            case .code(_, let note): if let note { out.append(note) }
            case .card(let title, let rows, _):
                if let title { out.append(title) }
                out += rows.flatMap { [$0.label, $0.value] }
            case .balances(let title, _, let note, _):
                out.append(title)
                if let note { out.append(note) }
            }
        }
        switch m.fee {
        case .onchain(let label, let value, let selector, _):
            out += [label, value]
            if let selector {
                out.append(selector.title)
                out += selector.options.flatMap { [$0.name, $0.balance, $0.fee] }
            }
        case .offchain(let note): out.append(note)
        case .hidden: break
        }
        return out
    }

    @Test func allThirtyThreeScenariosBuild() {
        #expect(SigningStateId.allCases.count == 33)
        for state in SigningStateId.allCases {
            let m = model(state)
            #expect(m.id == state)
            #expect(!m.blocks.isEmpty, "\(state) has no blocks")
            if case .intent = m.blocks.first { } else {
                Issue.record("\(state) does not open with an intent")
            }
        }
    }

    @Test func noStringEchoesItsKeyAndNoTemplateIsLeftUnfilled() {
        for state in SigningStateId.allCases {
            for value in strings(model(state)) {
                #expect(!value.hasPrefix("componentsUi."), "\(value) in \(state) is unresolved")
                #expect(!value.contains("{{"), "\(value) in \(state) still carries a {{var}}")
            }
        }
    }

    @Test func theSlideAlwaysSaysWhatItConfirms() {
        for state in SigningStateId.allCases {
            let m = model(state)
            #expect(!m.confirm.hint.isEmpty, "\(state) has no slide hint")
            #expect(!m.confirm.action.isEmpty, "\(state) has no slide action")
        }
    }

    /// The never-unlimited mandate (spec 022 §4).
    @Test func unlimitedApprovalCannotBeConfirmedAsRequested() {
        let m = model(.cs5)
        #expect(!m.confirm.enabled, "cs5 must not be confirmable")
        guard case .allowance(_, _, _, let chips, _, _, _) = m.blocks.first(where: {
            if case .allowance = $0 { return true } else { return false }
        }) else { Issue.record("cs5 has no allowance editor"); return }
        #expect(chips.first { $0.id == "requested" }?.state == .disabled)
    }

    @Test func choosingAFiniteCapReEnablesTheSlide() {
        for state: SigningStateId in [.cs6, .cs8] {
            #expect(model(state).confirm.enabled, "\(state) should be confirmable")
        }
    }

    @Test func aFiniteRequestMayBeSignedAsAsked() {
        guard case .allowance(_, _, _, let chips, _, let total, _) = model(.cs7).blocks.first(where: {
            if case .allowance = $0 { return true } else { return false }
        }) else { Issue.record("cs7 has no allowance editor"); return }
        #expect(chips.first { $0.id == "requested" }?.state == .selected)
        // An increment only means something next to the total it lands on.
        #expect(total?.value == "350 USDC")
    }

    @Test func theLadderPromotesSimulationWhereDecodingFailed() {
        for state: SigningStateId in [.cs23, .cs30, .cs31] {
            let balances = model(state).blocks.filter {
                if case .balances = $0 { return true } else { return false }
            }
            #expect(balances.count == 1, "\(state) should show balance changes")
        }
    }

    @Test func theDeepestRungsWarnInDanger() {
        for state: SigningStateId in [.cs24, .cs32] {
            let danger = model(state).blocks.contains {
                if case .warning(let tone, _) = $0 { return tone == .danger } else { return false }
            }
            #expect(danger, "\(state) should carry a danger warning")
        }
        // cs32 states BOTH failures and still shows the amount it does know.
        let deepest = model(.cs32)
        let warnings = deepest.blocks.filter {
            if case .warning = $0 { return true } else { return false }
        }
        #expect(warnings.count == 2)
    }

    @Test func feeShapesMatchTheirMocks() {
        if case .onchain = model(.cs1).fee { } else { Issue.record("cs1 pays gas") }
        for state: SigningStateId in [.cs16, .cs17, .cs18, .cs19] {
            if case .offchain = model(state).fee { } else {
                Issue.record("\(state) is an off-chain signature")
            }
        }
        for state: SigningStateId in [.cs20, .cs21, .cs22] {
            if case .hidden = model(state).fee { } else {
                Issue.record("\(state) shows no fee row at all")
            }
        }
        if case .onchain(_, _, let selector, _) = model(.cs33).fee {
            #expect(selector?.options.count == 2)
        } else {
            Issue.record("cs33 opens the fee-token selector")
        }
    }

    @Test func cs29IsCs1WithTheTechnicalPanelOpen() {
        #expect(model(.cs29).techOpen)
        #expect(!model(.cs1).techOpen)
        #expect(model(.cs29).tech.identities.count == 2)
    }
}
