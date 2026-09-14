//
//  ManageTokensTests.swift
//  VelaWalletTests
//
//  Adding a token by hand: the storage both token machines share, and the
//  drawn sheet's live states.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ManageTokensTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func freshStore() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
    }

    private func token(
        chainId: Int = 100,
        contract: String = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83",
        symbol: String = "USDC"
    ) -> [String: Any] {
        [
            "chainId": chainId, "contractAddress": contract, "symbol": symbol,
            "name": "USD Coin", "decimals": 6,
        ]
    }

    // MARK: - The shared storage

    /// One spelling of the de-dupe key, and it is lowercased. Two spellings is
    /// how the same token gets listed twice — once by the person, once by the
    /// receipt scanner.
    @Test func theIdIsOneLowercasedSpelling() {
        #expect(CustomTokens.id(chainId: 100, contract: "0xABC") == "100_0xabc")
        #expect(CustomTokens.id(chainId: 100, contract: "0xabc") == "100_0xabc")
    }

    @Test func savingReplacesByIdRatherThanAppending() {
        let store = freshStore()
        #expect(CustomTokens.save(token(), store: store))
        #expect(CustomTokens.save(token(symbol: "USDC.e"), store: store))
        let stored = CustomTokens.load(store: store)
        #expect(stored.count == 1, "the same contract was listed twice")
        #expect(stored.first?["symbol"] as? String == "USDC.e")
        // The display name every client's record carries, frozen at save time.
        #expect(stored.first?["networkName"] as? String == "Gnosis")
    }

    /// A record with no contract, no chain or no symbol is not a token. The
    /// core models that refusal, so the shell must actually report it.
    @Test func anIncompleteTokenIsRefused() {
        let store = freshStore()
        #expect(!CustomTokens.save(["chainId": 100, "symbol": "X"], store: store))
        #expect(!CustomTokens.save(["contractAddress": "0xabc", "symbol": "X"], store: store))
        #expect(!CustomTokens.save(token(symbol: ""), store: store))
        #expect(CustomTokens.load(store: store).isEmpty)
    }

    @Test func removingAnsWersWhetherTheRowWasThere() {
        let store = freshStore()
        _ = CustomTokens.save(token(), store: store)
        let id = CustomTokens.id(chainId: 100,
                                 contract: "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83")
        #expect(CustomTokens.remove(id: id, store: store))
        #expect(!CustomTokens.remove(id: id, store: store), "a second delete found nothing")
        #expect(CustomTokens.load(store: store).isEmpty)
    }

    /// The wire round trip keeps the bytes and drops the display word.
    @Test func theWireRoundTripKeepsTheStoredShape() {
        let wire = CustomTokens.toWire(CustomTokens.fromWire([
            "chain_id": 100, "contract_address": "0xabc", "symbol": "USDC",
            "name": "USD Coin", "decimals": 6,
        ]))
        #expect(wire?["id"] as? String == "100_0xabc")
        #expect(wire?["decimals"] as? Int == 6)
        #expect(wire?["network_name"] == nil, "chain naming must not travel to the core")
        // `manage_tokens` is the one machine that DOES take the name, because
        // it freezes it into the record it saves.
        let mtok = ManageTokensExecutor.toWire(CustomTokens.fromWire([
            "chain_id": 100, "contract_address": "0xabc", "symbol": "USDC",
            "name": "USD Coin", "decimals": 6,
        ]))
        #expect(mtok?["network_name"] as? String == "Gnosis")
    }

    // MARK: - The executor

    @Test func everyOperationIsAnswered() async {
        let store = freshStore()
        let accounts = AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        // Deliberately UNBOOTED: the pool refuses a call before it has its
        // ban map, so the probe answers "this chain said nothing" without a
        // packet leaving the machine. Before that guard existed this suite
        // hung — the continuation waited on an event `CoreStore` had dropped.
        let executor = ManageTokensExecutor(
            store: store, pool: RpcPool(store: store, accounts: accounts)
        )
        #expect(ManageTokensExecutor.operations.count == 5)
        for name in ManageTokensExecutor.operations {
            let reply = (try? CoreJSON.object(await executor.perform([
                "type": name,
                "chain_id": 100,
                // Unroutable on purpose: this asserts the ANSWER exists, not
                // that a chain replied.
                "address": "0x0000000000000000000000000000000000000000",
                "id": "100_0xabc",
                "token": ["chain_id": 100, "contract_address": "0xabc", "symbol": "X"],
            ]))) ?? [:]
            #expect(!(reply["type"] as? String ?? "").isEmpty, "no answer for `\(name)`")
        }
    }

    /// A save the storage refused is `save_failed`, which the core renders as
    /// an error rather than as a token that quietly did not appear.
    @Test func aRefusedSaveIsReportedAsOne() async {
        let store = freshStore()
        let accounts = AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        // Deliberately UNBOOTED: the pool refuses a call before it has its
        // ban map, so the probe answers "this chain said nothing" without a
        // packet leaving the machine. Before that guard existed this suite
        // hung — the continuation waited on an event `CoreStore` had dropped.
        let executor = ManageTokensExecutor(
            store: store, pool: RpcPool(store: store, accounts: accounts)
        )
        let reply = (try? CoreJSON.object(await executor.perform([
            "type": "write_custom_token",
            "token": ["chain_id": 100, "contract_address": "", "symbol": ""],
        ]))) ?? [:]
        #expect(reply["type"] as? String == "save_failed")
    }

    // MARK: - The drawn sheet, live

    private func view(
        input: String = "",
        valid: Bool = false,
        detecting: Bool = false,
        found: [MtokFoundWire] = [],
        notFound: Bool = false,
        saving: Bool = false,
        saveError: Bool = false
    ) -> MtokViewWire {
        MtokViewWire(
            inputAddress: input, addressValid: valid, detecting: detecting, found: found,
            saving: saving, customTokens: [], notFound: notFound, saveError: saveError
        )
    }

    private var card: MtokFoundWire {
        MtokFoundWire(chainId: 100, networkName: "Gnosis", name: "USD Coin",
                      symbol: "USDC", decimals: 6, added: false)
    }

    private var base: AddTokenModel {
        guard case .addToken(let model)? = WalletFlowFixtures.build(.t3, loc: loc).sheet
        else { fatalError("the T3 fixture lost its add-token sheet") }
        return model
    }

    /// An empty field is not an invalid address. Saying so as you type the
    /// first character is the app arguing with somebody mid-word.
    @Test func anEmptyFieldIsNotAnError() {
        #expect(FlowsLive.addToken(view(), on: base, loc: loc).fieldError == nil)
        let typing = FlowsLive.addToken(view(input: "0x12", valid: false), on: base, loc: loc)
        #expect(typing.fieldError == loc.t("addToken.invalidAddress"))
    }

    @Test func theSheetShowsWhatTheCoreIsDoing() {
        let searching = FlowsLive.addToken(
            view(input: "0xabc", valid: true, detecting: true), on: base, loc: loc
        )
        guard case .searching(let text) = searching.result else {
            Issue.record("a running sweep did not say so")
            return
        }
        #expect(text == loc.t("addToken.searchingNetworks"))
        #expect(searching.ctaDisabled, "nothing found yet, and the CTA offered anyway")

        let nothing = FlowsLive.addToken(
            view(input: "0xabc", valid: true, notFound: true), on: base, loc: loc
        )
        guard case .notFound = nothing.result else {
            Issue.record("every chain answered and the sheet stayed blank")
            return
        }
        #expect(nothing.ctaDisabled)
    }

    @Test func aFoundTokenIsAddableAndNamesItsChain() {
        let live = FlowsLive.addToken(
            view(input: "0xabc", valid: true, found: [card]), on: base, loc: loc
        )
        guard case .token(_, let name, let detail, let chip) = live.result else {
            Issue.record("a found token did not render as one")
            return
        }
        #expect(name == "USD Coin")
        #expect(detail.contains("USDC") && detail.contains("6") && detail.contains("Gnosis"))
        #expect(chip == nil, "an unadded token must not wear the added chip")
        #expect(!live.ctaDisabled)
        // The network row names the chain it was found on.
        #expect(live.network?.name == "Gnosis")
    }

    /// A token already in the wallet cannot be added twice, and the sheet says
    /// so rather than silently doing nothing when the button is pressed.
    @Test func anAlreadyAddedTokenShutsTheGate() {
        let added = MtokFoundWire(chainId: 100, networkName: "Gnosis", name: "USD Coin",
                                  symbol: "USDC", decimals: 6, added: true)
        let live = FlowsLive.addToken(
            view(input: "0xabc", valid: true, found: [added]), on: base, loc: loc
        )
        #expect(live.ctaDisabled)
        guard case .token(_, _, _, let chip) = live.result else {
            Issue.record("the added token stopped rendering")
            return
        }
        #expect(chip?.text == loc.t("addToken.tokenAdded"))
    }

    /// While the write is in flight the button is shut — a second tap would
    /// ask the core to save the same token twice.
    @Test func aSaveInFlightShutsTheGateToo() {
        let live = FlowsLive.addToken(
            view(input: "0xabc", valid: true, found: [card], saving: true), on: base, loc: loc
        )
        #expect(live.ctaDisabled)
    }

    /// A failed write is said out loud. The mock has no alert, so it goes
    /// under the CTA — a tap that changes nothing has told the person nothing.
    @Test func aFailedSaveIsSaidOutLoud() {
        #expect(FlowsLive.saveErrorText(view(), loc: loc) == nil)
        let text = FlowsLive.saveErrorText(view(saveError: true), loc: loc)
        #expect(text?.contains(loc.t("addToken.errorSaveToken")) == true)
    }

    // MARK: - Drift

    /// `MtokView` decodes, and the panel asks for nothing this build cannot do.
    @Test func manageTokensViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = ManageTokensCore()
        let initial = try CoreJSON.decode(MtokViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.addressValid)
        #expect(initial.found.isEmpty)

        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "start"])))
        _ = try CoreJSON.decode(MtokViewWire.self, from: result["view"] as? [String: Any] ?? [:])
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "an opened panel must read what is already added")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                ManageTokensExecutor.operations.contains(tag),
                "the panel asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }
    /// A card the core has already found outranks the spinner.
    ///
    /// The core probes every chain in parallel and keeps `detecting` true until
    /// the LAST one settles. Reading `detecting` first therefore hides an
    /// answer the wallet already has — on the founder's iPhone, for over a
    /// minute (spec 052, found on the device: 16.9 s on the simulator against
    /// more than 60 s on the phone, with the field filled and the status line
    /// still spinning). `not_found` is untouched: it is only true once every
    /// chain has answered, so nothing here can say "not found" early, which is
    /// the dangerous direction.
    @Test func aFoundCardOutranksTheStillSearchingSpinner() {
        let midSweep = FlowsLive.addToken(
            view(input: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
                 valid: true, detecting: true, found: [card]),
            on: base, loc: loc
        )
        guard case .token(_, let name, _, _) = midSweep.result else {
            Issue.record("a found card must show while other chains are still answering")
            return
        }
        #expect(name == "USD Coin")
        // And it is addable at that moment, not a minute later.
        #expect(!midSweep.ctaDisabled)

        // With nothing found yet, the sweep still says it is searching.
        let empty = FlowsLive.addToken(
            view(input: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
                 valid: true, detecting: true),
            on: base, loc: loc
        )
        guard case .searching = empty.result else {
            Issue.record("with nothing found, the sheet still says it is searching")
            return
        }
    }

}
