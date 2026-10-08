//
//  SigningFixturesTests.swift
//  VelaWalletTests
//
//  Spec 022 gates for the signing layer.
//
//  Two of these are product contracts rather than style checks — the confirm
//  always says what it confirms, and an unlimited approval is kept as asked
//  only where it is said — so they are asserted here: a later refactor has to
//  break a test to break the promise.
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
            m.signer.label, m.signer.name, m.panelTitle, m.tech.title,
        ]
        if let confirm = m.confirm { out.append(confirm.action) }
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
            case .allowance(let label, let value, _, let chips, let note, let total, let custom, _):
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
        case .onchain(let label, let value, let selector, _, _):
            out += [label, value]
            if let selector {
                out.append(selector.title)
                out += selector.options.flatMap { [$0.name, $0.balance, $0.fee] }
            }
        case .offchain(let note): out.append(note)
        case .hidden: break
        // A refused request carries no fee at all (spec 081); `.hidden` is the
        // off-chain case that shows the row with nothing in it.
        case .none: break
        }
        return out
    }

    @Test func everyScenarioBuilds() {
        // The 33 drawn boards and cs36, the wallet's own backup.
        #expect(SigningStateId.allCases.count == 34)
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

    /// Issue #461: the confirm is a tap on a button labelled with the action
    /// alone — never "滑动以确认 · …", whose key the corpus no longer has.
    @Test func theConfirmAlwaysSaysWhatItConfirms() {
        for state in SigningStateId.allCases {
            let m = model(state)
            // Every DRAWN state offers the confirm; the refusal state has no
            // fixture, because it is reached from the core, not the gallery.
            let action = m.confirm?.action ?? ""
            #expect(!action.isEmpty, "\(state) has no confirm label")
            #expect(!action.contains("滑动") && !action.contains("·"), "\(state) still says a slide: \(action)")
        }
    }

    /// Unlimited kept as asked, and said (spec 022 §4, 2026-09-26 ruling).
    @Test func unlimitedApprovalIsKeptAsRequestedAndSaid() {
        let m = model(.cs5)
        #expect(m.confirm?.enabled == true, "cs5 must be confirmable as asked")
        guard case .allowance(_, _, _, let chips, _, _, _, _) = m.blocks.first(where: {
            if case .allowance = $0 { return true } else { return false }
        }) else { Issue.record("cs5 has no allowance editor"); return }
        #expect(chips.first { $0.id == "requested" }?.state == .selected)
        #expect(m.blocks.contains { if case .warning(.danger, _) = $0 { true } else { false } })
    }

    @Test func choosingAFiniteCapReEnablesTheConfirm() {
        for state: SigningStateId in [.cs6, .cs8] {
            #expect(model(state).confirm?.enabled == true, "\(state) should be confirmable")
        }
    }

    @Test func aFiniteRequestMayBeSignedAsAsked() {
        guard case .allowance(_, _, _, let chips, _, let total, _, _) = model(.cs7).blocks.first(where: {
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
        if case .onchain(_, _, let selector, _, _) = model(.cs33).fee {
            #expect(selector?.options.count == 2)
        } else {
            Issue.record("cs33 opens the fee-token selector")
        }
    }

    /// The wallet's own backup (first party): no requester — its intent is
    /// the header's title, not repeated in the form — the rows in the core's
    /// words with the network first, the confirm reading the intent, and no
    /// contract summary on the technical details.
    @Test func theWalletsOwnBackupHasAHeadlineAndNoRequester() {
        let own = model(.cs36)
        #expect(own.dappOwn)
        #expect(own.headline?.text == "备份公钥")
        #expect(!own.formBlocks.contains { if case .intent = $0 { true } else { false } },
                "the headline is drawn once, in the header")
        #expect(own.formBlocks.count == own.blocks.count - 1)
        guard case .rows(let rows) = own.formBlocks.first else {
            Issue.record("the backup's rows lead the form"); return
        }
        #expect(rows.map(\.label) == ["网络", "地址", "公钥数量"])
        #expect(rows.first?.value == "Ethereum")
        #expect(own.confirm?.action == "备份公钥")
        #expect(own.tech.summary == nil)

        // A site's sheet keeps its header and its intent in the form.
        let site = model(.cs1)
        #expect(site.headline == nil)
        #expect(site.formBlocks.count == site.blocks.count)
    }

    @Test func cs29IsCs1WithTheTechnicalPanelOpen() {
        #expect(model(.cs29).techOpen)
        #expect(!model(.cs1).techOpen)
        #expect(model(.cs29).tech.identities.count == 2)
    }

    // MARK: - Coins wear the token mark (DESIGN L, S4)

    /// Every drawn amount line names its coin with the coin's token mark,
    /// lettered from the very ticker the line prints — never a first letter
    /// on a brand disc, which drew USDC and USDT as the same "U". A coin the
    /// drawings give a contract for carries its logo candidates; spWETH,
    /// which they give none, gets no guessed logo.
    @Test func everyAmountLinesCoinWearsItsOwnTokenMark() {
        var lines: [AmountLine] = []
        for state in SigningStateId.allCases {
            for block in model(state).blocks {
                switch block {
                case .amount(let line, _, _): lines.append(line)
                case .swap(let pay, let receive): lines += [pay, receive]
                default: break
                }
            }
        }
        let marked = lines.filter { $0.token != nil }
        #expect(!marked.isEmpty, "no drawn amount line wears a mark")
        for line in marked {
            guard let mark = line.token else { continue }
            #expect(mark.glyph == String(line.symbol.prefix(3)).uppercased(),
                    "\(line.symbol) drawn as \(mark.glyph)")
            if line.symbol == "spWETH" {
                #expect(mark.logoURLs.isEmpty, "a guessed logo for a coin with no contract")
            } else {
                #expect(!mark.logoURLs.isEmpty, "\(line.symbol) has no logo to try")
            }
        }
        let usdc = marked.first { $0.symbol == "USDC" }?.token
        let usdt = marked.first { $0.symbol == "USDT" }?.token
        #expect(usdc?.logoURLs.first != nil && usdc?.logoURLs.first != usdt?.logoURLs.first,
                "USDC and USDT wear the same picture")
    }

    /// The technical details' identities: the token wears its mark, the
    /// person their identicon from their address — not a letter.
    @Test func theTechnicalIdentitiesWearAMarkOrAFace() {
        let identities = model(.cs1).tech.identities
        #expect(!identities.isEmpty)
        for identity in identities {
            switch identity.lead {
            case .token(let mark)?:
                #expect(!mark.logoURLs.isEmpty)
            case .identicon(let seed)?:
                #expect(seed == identity.address)
            default:
                Issue.record("\(identity.name) wears nothing")
            }
        }
    }
}
