//
//  SendWire.swift
//  VelaWallet
//
//  The `send` machine's view model, in Swift.
//
//  4,417 lines of Rust decide what this carries: three modes, live validation,
//  a string-exact Max, the same-asset fee ceiling, the treasury pre-check and
//  the sign→submit lifecycle with its cancel checkpoints. Nothing here is a
//  judgement; every field is one the core already made.
//
//  ## A view may be a subset; an operation may not
//
//  Spec 052 wires single mode. The multi/split fields are carried anyway —
//  they cost nothing to decode and their absence is what makes a later cut's
//  "it renders but does nothing" bug. What is NOT carried is called out where
//  it sits.
//

import Foundation

/// Which drawn state the flow is in. The shell renders by this rather than by
/// remembering where it navigated from.
enum SendStageWire: String, Decodable {
    case lockError = "lock_error"
    case lockResolving = "lock_resolving"
    case receipt
    case selectToken = "select_token"
    case enterDetails = "enter_details"
    case confirm
}

/// Why a locked request cannot be fulfilled.
enum SendLockErrorWire: Decodable, Equatable {
    case network(chainId: Int)
    case token

    private enum CodingKeys: String, CodingKey { case type, chainId }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if try container.decode(String.self, forKey: .type) == "network" {
            self = .network(chainId: try container.decode(Int.self, forKey: .chainId))
        } else {
            self = .token
        }
    }
}

/// The line under the add-network button.
enum SendAddNetworkMsgWire: String, Decodable, Equatable {
    case netNotFound = "net_not_found"
    case netNotCompatible = "net_not_compatible"
    case netAddError = "net_add_error"

    private enum CodingKeys: String, CodingKey { case type }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .type)
        self = SendAddNetworkMsgWire(rawValue: raw) ?? .netAddError
    }
}

/// One holding the picker offers.
struct SendTokenWire: Decodable, Equatable {
    let network: String
    let chainId: Int
    let symbol: String
    /// A HUMAN decimal string, not raw units — the trap spec 051 phase 2a
    /// pinned with a test on both sides.
    let balance: String
    let decimals: Int
    /// `nil` = the chain's native coin.
    let tokenAddress: String?
    let priceUsd: Double?
    let logoUrls: [String]
    let spam: Bool

    /// `SendToken::id` — the core's own key, and the only spelling of it.
    var id: String { "\(network)_\(tokenAddress ?? "native")_\(symbol)" }
}

/// One row of a split, as the core holds it.
struct SendRecipientDraftWire: Decodable, Equatable {
    let id: String
    let address: String
    let amount: String
    let name: String?
}

/// A split row that pays an address an earlier row already pays (issue 203):
/// the row's id and the 1-based position of the row it repeats. The core
/// decides what a repeat is; the shell only says so.
struct SendDuplicateRowWire: Decodable, Equatable {
    let id: String
    let firstOrdinal: Int
}

/// What one field of a split row still needs — the core's `SendRowFieldState`.
enum SendRowFieldStateWire: String, Decodable, Equatable {
    case ok
    /// Nothing typed yet: unfinished, not wrong.
    case empty
    /// Something typed that is not an address / not a sendable amount.
    case invalid
}

/// A split row `Continue` will not take, and which field is why
/// (`split_row_issues`). Rows that are fine are not listed.
struct SendSplitRowIssueWire: Decodable, Equatable {
    let id: String
    /// The number the row wears ("Recipient 2").
    let ordinal: Int
    let address: SendRowFieldStateWire
    let amount: SendRowFieldStateWire
}

/// One ticked token, and what the core says will leave.
struct SendMultiSpecWire: Decodable, Equatable {
    /// `nil` is the native coin.
    let tokenAddress: String?
    let decimals: Int
    /// Base units, decimal string.
    let amount: String
}

/// Why the amount cannot be what was typed.
enum SendAmountWarningWire: Equatable {
    case notEnoughToken(symbol: String)
    case insufficientForGas(symbol: String?)
    /// The fee alone outruns the balance — the state `Max` fills `0` for.
    case insufficientGas(symbol: String?)
    case needGas(symbol: String?)
    case cannotConvert(code: String, symbol: String)
}

extension SendAmountWarningWire: Decodable {
    private enum Key: String, CodingKey { case type, symbol, code }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Key.self)
        switch try container.decode(String.self, forKey: .type) {
        case "not_enough_token":
            self = .notEnoughToken(symbol: try container.decode(String.self, forKey: .symbol))
        case "insufficient_for_gas":
            self = .insufficientForGas(symbol: try container.decodeIfPresent(String.self, forKey: .symbol))
        case "insufficient_gas":
            self = .insufficientGas(symbol: try container.decodeIfPresent(String.self, forKey: .symbol))
        case "need_gas":
            self = .needGas(symbol: try container.decodeIfPresent(String.self, forKey: .symbol))
        case "cannot_convert":
            self = .cannotConvert(
                code: try container.decode(String.self, forKey: .code),
                symbol: try container.decode(String.self, forKey: .symbol)
            )
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown amount warning \(other)"
            )
        }
    }
}

/// The same-asset ceiling: sending a coin that also pays the fee.
///
/// Every figure is a **base-unit decimal string**, and the shell formats them.
/// Android's first cut printed `5000000000000000000 XDAI` on a phone; that is
/// what this comment exists to prevent happening twice.
struct SendFeeIssueWire: Decodable, Equatable {
    let symbol: String
    let transferAmount: String
    let balance: String
    let feeAmount: String
    let total: String
    let maxTransferAmount: String
}

/// Why a unit conversion cannot happen.
struct SendUnitIssueWire: Decodable, Equatable {
    let code: String
    let symbol: String
}

/// Who the recipient is, if anybody could say.
struct SendRecipientIdentityWire: Decodable, Equatable {
    let name: String?
    let source: String?
}

/// Whose word a payee's name is (spec 097 F, S2) — the core's
/// `SendNameSource`, tagged by `type`.
///
/// A source this build has never heard of must not fail the view: it reads as
/// `unknown`, and a name nobody can say the source of is not drawn — untagged,
/// it would look like the person's own (the core's own rule for one).
enum SendNameSourceWire: Decodable, Equatable {
    /// The person's own word: their account, their contact, their list.
    case own
    /// The public wallet registry, where anyone can register any name.
    case registry
    /// A name service whose name resolves to this address — "ENS", ".bnb".
    case service(label: String)
    case unknown

    private enum CodingKeys: String, CodingKey { case type, label }

    init(from decoder: Decoder) throws {
        let container = try? decoder.container(keyedBy: CodingKeys.self)
        switch try? container?.decode(String.self, forKey: .type) {
        case "own": self = .own
        case "registry": self = .registry
        case "service":
            let label = (try? container?.decode(String.self, forKey: .label)) ?? ""
            self = label.trimmingCharacters(in: .whitespaces).isEmpty ? .unknown : .service(label: label)
        default: self = .unknown
        }
    }
}

/// Who the money goes to, as the form's line and the confirm name them
/// (spec 097 F, S2): the address always — in full, as signed — and a name
/// only beside it, with whose word it is.
struct SendPayeeWire: Decodable, Equatable {
    let address: String
    let name: String?
    /// Present exactly when `name` is.
    let nameSource: SendNameSourceWire?
}

/// What the recipient is. `nil` on either field means **not judged** — never
/// "no". A delegated EOA is a wallet, and the core owns that carve-out.
struct SendRecipientRiskWire: Decodable, Equatable {
    let isContract: Bool?
    let firstTime: Bool?
}

/// The relay's float on this chain, when it is short.
struct SendTreasuryStatusWire: Decodable, Equatable {
    let chainId: Int
    let address: String
    /// `native` or `path_usd`.
    let asset: String
    let balance: String
    let floor: String
    let bootstrapNeeded: Bool
    /// Whether this is a network Vela ships, and so one whose relayer the
    /// OPERATOR is expected to keep funded. The core decides; it changes what
    /// the person is asked to do (spec 060).
    ///
    /// Absent ⇒ `false`, mirroring the core's `#[serde(default)]`: a status
    /// written before the field existed, or a hand-built one in a test, reads
    /// as a custom network — the branch that only ever asks the person to
    /// fund the relayer, which is the safe assumption when nobody said Vela
    /// serves it.
    let operatorServed: Bool
    /// `balance` and `floor` in the coin they are counted in, and what the
    /// stop asks for — the core's, for the stop's own chain (issue #422).
    /// `nil` when the core could not read the relay's figures: then no
    /// amount is drawn at all.
    let coin: SendTreasuryCoinWire?

    private enum CodingKeys: String, CodingKey {
        case chainId, address, asset, balance, floor, bootstrapNeeded, operatorServed, coin
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try c.decode(Int.self, forKey: .chainId)
        address = try c.decode(String.self, forKey: .address)
        asset = try c.decode(String.self, forKey: .asset)
        balance = try c.decode(String.self, forKey: .balance)
        floor = try c.decode(String.self, forKey: .floor)
        bootstrapNeeded = try c.decode(Bool.self, forKey: .bootstrapNeeded)
        operatorServed = try c.decodeIfPresent(Bool.self, forKey: .operatorServed) ?? false
        coin = try c.decodeIfPresent(SendTreasuryCoinWire.self, forKey: .coin)
    }
}

/// The relay's treasury figures in the coin they are counted in (issue
/// #422): plain whole-coin decimals, the stop's own chain's coin. This app
/// used to name the coin from its chain catalog and work the figures out in
/// 18 decimals itself; the core says both now.
struct SendTreasuryCoinWire: Decodable, Equatable {
    /// pathUSD on Tempo, else the chain's own coin; `nil` = the core does
    /// not know it (never a guess).
    let symbol: String?
    let balance: String
    let floor: String
    /// The suggested contribution: the relay's shortfall for this chain.
    let suggested: String
}

/// The relay said it cannot serve this chain (spec 098 §2) — a 404 from the
/// treasury probe. Not the funding sheet: gas cannot help a relay that cannot
/// reach the network. `operatorServed` is the core's verdict, as above.
struct SendRelayUnreachableWire: Decodable, Equatable {
    let chainId: Int
    let operatorServed: Bool

    private enum CodingKeys: String, CodingKey { case chainId, operatorServed }

    init(chainId: Int, operatorServed: Bool) {
        self.chainId = chainId
        self.operatorServed = operatorServed
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try c.decode(Int.self, forKey: .chainId)
        operatorServed = try c.decodeIfPresent(Bool.self, forKey: .operatorServed) ?? false
    }
}

/// One leg of a receipt.
struct SendReceiptTransferWire: Decodable, Equatable {
    let to: String
    let toName: String?
    let amount: String
    let symbol: String
    let logoUrls: [String]
    let usdValue: Double
}

/// One coin the operation sent, summed over its recipients (spec 097 F, S3).
struct SendReceiptCoinWire: Decodable, Equatable {
    /// Token units, as signed.
    let amount: String
    let symbol: String
    let logoUrls: [String]
    /// `nil` = the chain's native coin.
    let tokenAddress: String?
    let usdValue: Double
}

struct SendReceiptWire: Decodable, Equatable {
    /// `submitted` / `confirmed` / `failed`, and since spec 082 `maybe_sent`
    /// (the submit's reply was lost: "it may have been sent", no success
    /// haptic, no retry) and `not_sent` (the relay never had it). A string on
    /// purpose: a status this build has never heard of must not fail the view.
    let status: String
    /// `fee_hold` / `fee_rejected` / `relay_funding`.
    let holdReason: String?
    /// The relay refused it (PR 2 §3): the corpus key of the sentence that
    /// says why — the fee words only for a fee refusal, "another transaction
    /// from this account went first" for a spent nonce, else "the network
    /// refused it, nothing was sent". Drawn in place of `holdReason`'s words.
    var refusalKey: String? = nil
    /// `split` / `multi_select`; absent for a plain transfer.
    let kind: String?
    let transfers: [SendReceiptTransferWire]
    /// Every coin the operation sent, in signing order (spec 097 F, S3): one
    /// for a single send or a split (its total), one per coin for a sweep.
    /// The screen lists these and never lets one stand for the rest.
    /// Optional on the wire so a hand-written receipt without it decodes.
    var coins: [SendReceiptCoinWire]?
    /// `coins[0]`'s figure when exactly one coin moved (a split's total);
    /// empty for a sweep of several.
    let amount: String
    let usdValue: Double
    let submittedAtMs: Double?
    let typicalInclusionS: Int?
}

/// `SendView.previous_pending`: the transaction this send waits for.
struct SendPreviousPendingWire: Decodable, Equatable {
    let chainId: Int
    let userOpHash: String
    /// `componentsUi.signing.confirmBlock.previousPending`.
    let key: String
}

/// A relay stop's report, whole (issue #466): `what` (its first line is the
/// title) and `steps` seed the in-app reporter, which files it under `area`
/// with `fingerprint` — stable across balance refreshes, so one stop is one
/// issue. Snapshotted when "Report this" is tapped.
struct SendRelayReportWire: Decodable, Equatable {
    let what: String
    let steps: String
    let area: String
    let fingerprint: String
}

/// The coin the form's fee row names (`SendView.fee_coin`) — the core's one
/// answer, estimate or none: the estimate in hand, else the coin in force
/// (the fee card's, else the person's pick), else the chain's own.
struct SendFeeCoinWire: Decodable, Equatable {
    /// Empty only for an ERC-20 nothing on the form names: the mark then draws
    /// its logo alone.
    let symbol: String
    /// `nil` = the chain's own coin.
    let contract: String?
    /// Never 0.
    let chainId: Int
}

struct SendViewWire: Decodable, Equatable {
    let stage: SendStageWire
    let loading: Bool
    let locked: Bool
    let amountLocked: Bool
    let resolvingLock: Bool
    let addingNetwork: Bool

    let tokens: [SendTokenWire]
    let selectedToken: SendTokenWire?
    let recipient: String
    /// Issue #312: the network a scanned code named for the payer to choose
    /// on. While set, `tokens` holds only that network's holdings.
    let requestChainId: Int?
    /// Issue #326: the form's token card opens the asset picker. Absent (a
    /// core built before the field existed) reads as no.
    let canChangeToken: Bool?
    /// As typed, in whatever unit `amountFiatCode` names.
    let amount: String
    /// `nil` = the token's own units. **The figure's own code**, not the
    /// display currency: the two differ for one instant after a currency
    /// commit, and that instant is when a figure typed in one currency used to
    /// be printed with another's glyph.
    let amountFiatCode: String?
    let denomToggleShown: Bool
    let denomToggleEnabled: Bool
    let denomToggleReason: SendUnitIssueWire?
    let confirmAmountIssue: SendUnitIssueWire?
    /// `amount` already resolved through the conversion — the ONE number the
    /// confirm page may print, because it is the number the signed batch is
    /// built from.
    let tokenAmount: String
    /// The headline beside From/To, always in token units. A split restates the
    /// sum the money gates already read, rather than the shell adding the rows
    /// up a second time.
    let confirmAmount: String

    let splitMode: Bool
    let recipients: [SendRecipientDraftWire]
    /// The core's verdicts on the split's rows: which repeat an earlier payee,
    /// and which `Continue` will not take and why. The shell never re-derives
    /// the address or amount rule to explain a row. Absent (a core built
    /// before the fields existed) reads as none, as on the web.
    let splitDuplicates: [SendDuplicateRowWire]?
    let splitRowIssues: [SendSplitRowIssueWire]?

    let splitOverBalance: Bool
    /// The balance less the rows' sum, in token units — "how much is left to
    /// give out". `nil` while a row cannot be summed or the sum is over.
    let splitRemaining: String?
    /// How many more recipients an import may add: the cap less the rows
    /// already started. The importer is opened with this as ITS cap, and it
    /// is also how the shell knows the form already has people on it
    /// (`< BatchStore.maxRecipients`).
    let splitImportRoom: Int

    let multiSelectMode: Bool
    let multiSelectedIds: [String]
    let multiValuableIds: [String]
    let multiChainId: Int?
    /// **How much of each ticked token actually moves.** A sweep is not "the
    /// whole balance" — the core reserves what the fee needs on the asset that
    /// pays it, so the sweep form and its confirm page must read these rather
    /// than the balances the picker showed.
    let multiSpecs: [SendMultiSpecWire]

    let showScanner: Bool
    let showContactPicker: Bool
    let showBatchImport: Bool
    /// Why a scanned request cannot be fulfilled (spec 055): a chain this
    /// wallet does not have, or a token it cannot identify on one it does.
    ///
    /// The core computes this and 052 was not reading it — a scanned code for
    /// an unknown chain put the send flow into a stage with nothing drawn on
    /// it.
    let lockError: SendLockErrorWire?
    /// What the last add-network attempt had to say. Semantic — the shell owns
    /// the words. (`addingNetwork` is already below.)
    let addNetworkMsg: SendAddNetworkMsgWire?

    let estimatingGas: Bool
    let feeBusy: Bool
    /// Chain-guarded by the core: never a previous network's quote.
    let fee: FeeEstimateWire?
    let gasFeeToken: String?
    /// The coin the fee row wears, whether or not a figure is beside it.
    /// `nil` only while no chain is known. Optional on the wire so a
    /// hand-written view without it decodes (as no chain known).
    var feeCoin: SendFeeCoinWire?
    let amountWarning: SendAmountWarningWire?
    let sameAssetFeeIssue: SendFeeIssueWire?

    let canContinue: Bool
    /// Fee settled ∧ nothing re-quoting ∧ no same-asset breach ∧ idle.
    let canConfirm: Bool
    let sending: Bool
    /// `idle` / `preparing` / `signing` / `submitting` / `confirmed` / `error`.
    let txStatus: String
    /// `generic` / `bundler_fund` / `venue_blocked` / `previous_pending`.
    let txError: String?
    /// PR 2 §3: this account's previous transaction on this network is still
    /// in flight, so the confirm is held (`can_confirm` false) and says so —
    /// `key`, one line under the held confirm, whatever the fee says. Opens by
    /// itself when the tracker says the first is final or stalled.
    var previousPending: SendPreviousPendingWire?
    /// Spec 102: with `txError` = `venue_blocked`, why this account cannot
    /// sign here (`VenueBlock::key()` with its domains). Optional on the wire.
    var txVenueBlock: VenueBlockWire?
    let txHash: String?
    let userOpHash: String?
    let receipt: SendReceiptWire?
    let treasuryBootstrap: SendTreasuryStatusWire?
    /// Spec 098 §2: the relay cannot serve this chain; the send stops here.
    let relayUnreachable: SendRelayUnreachableWire?
    /// Issue #466: the report a relay stop's "Report this" files, built by the
    /// core — set exactly while `treasuryBootstrap` or `relayUnreachable` is up
    /// on a built-in chain. Optional on the wire so a hand-written view
    /// without it decodes (as nothing to report).
    var relayReport: SendRelayReportWire?
    /// The resolver's raw answer. Only the receipt's caption still names the
    /// recipient by it, after the money moved (as the web does); every screen
    /// before the signature draws `payees`, and `source` is never printed.
    let recipientIdentity: SendRecipientIdentityWire?
    /// Who the money goes to (spec 097 F, S2): one payee for a single send or
    /// a sweep once the address is whole, one per `recipients` row for a
    /// split. The form's line, the picker's To line and the confirm read
    /// THIS — a name from the public registry once stood alone on the confirm
    /// for the address it claimed. Optional on the wire so a hand-written
    /// view without it decodes (as nobody named).
    var payees: [SendPayeeWire]?
    let recipientRisk: SendRecipientRiskWire?
    /// Spec 096 F12: the recipient is a token's own contract on this network
    /// (the core's `recipient_is_token_contract`) — said before the slide.
    /// Optional on the wire so a hand-written view without it reads `false`.
    var recipientIsTokenContract: Bool?
}
