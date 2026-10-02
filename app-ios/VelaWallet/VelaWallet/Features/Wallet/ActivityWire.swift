//
//  ActivityWire.swift
//  VelaWallet
//
//  The `activity_feed` machine's view model, in Swift.
//
//  ## Structured values, not formatted strings
//
//  The Expo original carried `"+1 USDT"` and `"$1.00"` through its model, which
//  forced a whole re-adapt on a language change and made the receipt toast
//  reverse-parse its own amount string. Here every row carries `value` +
//  `decimals` + `symbol` + `usdValue`, and the **shell formats**. A locale
//  change is a re-render.
//
//  ## Headers are rows
//
//  The core emits date headers already interleaved with items, in render
//  order, so a header can never inter-sort with an item — that is invariant ⑥,
//  and it is why this is one array rather than a dictionary the shell would
//  have to sort back into shape.
//

import Foundation

enum FeedDirectionWire: String, Decodable {
    case `in`, out
}

enum FeedTxStatusWire: String, Decodable {
    case pending, confirmed, failed
    /// 087 F04: a pending record nothing will ever settle (no operation hash
    /// past the core's grace, or past the tracker's 24 h). Only a row says it.
    case unknown

    /// A lifecycle this build has never heard of reads as pending — never as
    /// confirmed, and never as a view that fails to decode.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = FeedTxStatusWire(rawValue: raw) ?? .pending
    }
}

enum FeedTxKindWire: String, Decodable {
    case send, receive
    case dappTx = "dapp_tx"
    case signMessage = "sign_message"
    case signTypedData = "sign_typed_data"
    case connect

    /// A kind this build has never heard of reads as a send, the core's own
    /// default for an untyped record — the row still shows, with its status.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = FeedTxKindWire(rawValue: raw) ?? .send
    }
}

/// Who a row's `counterparty` is (spec 082 RJ16, G52): the person the money
/// went to, or the contract a dApp's call went to — which the detail labels
/// `componentsUi.signing.interactingLabel`, never "To". The core decides.
enum FeedCounterpartyRoleWire: String, Decodable {
    case recipient, contract

    /// A role this build has never heard of reads as the recipient, the
    /// core's own default — the row still shows, with its address.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = FeedCounterpartyRoleWire(rawValue: raw) ?? .recipient
    }
}

enum FeedBatchKindWire: String, Decodable {
    /// One token → N recipients.
    case split
    /// N tokens → one recipient.
    case multiSelect = "multi_select"
}

/// One stored transaction, as the core reads it back to the detail sheet.
struct FeedTxRecordWire: Decodable, Equatable {
    let id: String
    let userOpHash: String
    let txHash: String
    let from: String
    let to: String
    let toName: String?
    let value: String
    let symbol: String
    let decimals: Int
    let logoUrls: [String]?
    let chainId: Int
    /// Stored epoch **seconds**.
    let timestamp: Double
    let dayStartMs: Double
    let status: FeedTxStatusWire
    /// `nil` = a legacy untyped record, which the core reads as `send`.
    let kind: FeedTxKindWire?
    let usd: String?
    /// The origin a dApp request arrived from (spec 082 RG1, 083 H2, 093);
    /// `nil` for every other kind.
    var dappUrl: String? = nil
    /// The verb recorded at approve time, for a dApp record (spec 093).
    var intent: String? = nil
    /// The call's `data`, for a `dapp_tx` record (spec 082 RJ16) — what the
    /// core reads the counterparty from.
    var callData: String? = nil
}

struct FeedBatchTransferWire: Decodable, Equatable {
    let to: String
    let toName: String?
    let value: String
    let symbol: String
    let decimals: Int
    let usdValue: Double
    let logoUrls: [String]?
}

/// A batch send, summarised from its per-line records.
struct FeedBatchWire: Decodable, Equatable {
    let kind: FeedBatchKindWire
    let count: Int
    let totalUsd: Double
    let transfers: [FeedBatchTransferWire]
    let ids: [String]
    let from: String
    let chainId: Int
    let timestamp: Double
    let status: FeedTxStatusWire
    let txHash: String
    let userOpHash: String
    let symbol: String?
    let logoUrls: [String]?
    let to: String?
    let toName: String?
}

/// One row's payload.
struct FeedItemWire: Decodable, Equatable {
    let id: String
    let direction: FeedDirectionWire
    /// Sender for an inbound row, recipient for an outbound one; `nil` for a
    /// split batch, which has no single counterparty.
    let counterparty: String?
    /// The resolved name, or the one captured at send time. Never invented.
    let alias: String?
    /// `nil` for a multi-token batch row — mixed tokens cannot be summed, and
    /// the drawing shows the asset count instead.
    let value: String?
    let symbol: String
    let decimals: Int?
    /// Numeric USD; `0` when nothing could price it.
    let usdValue: Double
    let chainId: Int
    /// Epoch seconds.
    let timestamp: Double
    let dayStartMs: Double
    let txHash: String?
    let batch: FeedBatchWire?
    /// What the row is (spec 082 RG1): `send` (a folded batch too),
    /// `receive` or `dappTx`. The shell draws from this and never guesses.
    var kind: FeedTxKindWire = .send
    /// The record's lifecycle; the tracker alone moves it off `pending`.
    var status: FeedTxStatusWire = .confirmed
    /// `dappTx` only: `host[:port]` of the site that asked.
    var site: String? = nil
    /// Whether `counterparty` got the money or is the contract a call went
    /// to (spec 082 RJ16). Always `recipient` except on a `dappTx` row.
    var counterpartyRole: FeedCounterpartyRoleWire = .recipient
    /// A dApp interaction — a transaction OR a signature (spec 093): what it
    /// was, where, what it moved or granted, and its detail. A row with this
    /// is a dApp row; `nil` on every other row.
    var dapp: FeedDappWire? = nil
    /// The row's second line, in order (spec 093) — every row. The shell
    /// words each part and joins them with " · "; which parts, and their
    /// order, are the core's.
    var subtitle: [FeedLineWire] = []

    private enum CodingKeys: String, CodingKey {
        case id, direction, counterparty, alias, value, symbol, decimals, usdValue, chainId
        case timestamp, dayStartMs, txHash, batch, kind, status, site, counterpartyRole
        case dapp, subtitle
    }
}

extension FeedItemWire {
    /// Written out so the two spec-093 fields read as absent on a view from
    /// an older core instead of failing it; everything else as before.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            id: try c.decode(String.self, forKey: .id),
            direction: try c.decode(FeedDirectionWire.self, forKey: .direction),
            counterparty: try c.decodeIfPresent(String.self, forKey: .counterparty),
            alias: try c.decodeIfPresent(String.self, forKey: .alias),
            value: try c.decodeIfPresent(String.self, forKey: .value),
            symbol: try c.decode(String.self, forKey: .symbol),
            decimals: try c.decodeIfPresent(Int.self, forKey: .decimals),
            usdValue: try c.decode(Double.self, forKey: .usdValue),
            chainId: try c.decode(Int.self, forKey: .chainId),
            timestamp: try c.decode(Double.self, forKey: .timestamp),
            dayStartMs: try c.decode(Double.self, forKey: .dayStartMs),
            txHash: try c.decodeIfPresent(String.self, forKey: .txHash),
            batch: try c.decodeIfPresent(FeedBatchWire.self, forKey: .batch),
            kind: try c.decode(FeedTxKindWire.self, forKey: .kind),
            status: try c.decode(FeedTxStatusWire.self, forKey: .status),
            site: try c.decodeIfPresent(String.self, forKey: .site),
            counterpartyRole: try c.decode(FeedCounterpartyRoleWire.self, forKey: .counterpartyRole),
            dapp: try c.decodeIfPresent(FeedDappWire.self, forKey: .dapp),
            subtitle: try c.decodeIfPresent([FeedLineWire].self, forKey: .subtitle) ?? []
        )
    }
}

// MARK: - Spec 093: a dApp interaction, worded by the core

/// One part of a row's second line. A part this build has never heard of is
/// `unknown` and is left out — the row still shows, with the parts it knows.
enum FeedLineWire: Decodable, Equatable {
    /// Where a record that is not confirmed stands — never on a signature.
    case status(FeedTxStatusWire)
    /// "To {{name}}" — `name`, else the shell's short form of `address`.
    case to(address: String, name: String?)
    /// "From {{name}}".
    case from(address: String, name: String?)
    /// A site's `host[:port]`, verbatim.
    case site(String)
    /// The network, by its chain id; the shell names it.
    case network(chainId: Int)
    /// The day, by its local-midnight key — worded as a date header is
    /// (今天 / 昨天 / a date). Only on a contact's rows, which have no headers.
    case day(dayStartMs: Double)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type, status, address, name, site, chainId, dayStartMs
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "status": self = .status(try c.decode(FeedTxStatusWire.self, forKey: .status))
        case "to":
            self = .to(address: try c.decode(String.self, forKey: .address),
                       name: try c.decodeIfPresent(String.self, forKey: .name))
        case "from":
            self = .from(address: try c.decode(String.self, forKey: .address),
                         name: try c.decodeIfPresent(String.self, forKey: .name))
        case "site": self = .site(try c.decode(String.self, forKey: .site))
        case "network": self = .network(chainId: try c.decode(Int.self, forKey: .chainId))
        case "day": self = .day(dayStartMs: try c.decode(Double.self, forKey: .dayStartMs))
        default: self = .unknown
        }
    }
}

/// What a dApp request did (`DappAction`). A word this build has never heard
/// of reads as `call` — a row titled by its recorded verb, as before 093.
enum DappActionWire: String, Decodable {
    case call, batch, approve, permit
    case signIn = "sign_in"
    case message
    case typedData = "typed_data"
    case blindSign = "blind_sign"

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = DappActionWire(rawValue: raw) ?? .call
    }
}

/// One line of what a dApp transaction moved, from the signing sheet's own
/// simulation (083 F1) — or, folded in (spec 093), the chain's own receipt.
struct FeedDappChangeWire: Decodable, Equatable {
    let direction: FeedDirectionWire
    /// `false`: an unverified token — a direction, never a number.
    let verified: Bool
    let symbol: String
    let value: String?
    let decimals: Int?
    /// The wallet can vouch for this figure to the unit; otherwise "≈".
    var exact = false

    private enum CodingKeys: String, CodingKey {
        case direction, verified, symbol, value, decimals, exact
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        direction = try c.decode(FeedDirectionWire.self, forKey: .direction)
        verified = try c.decode(Bool.self, forKey: .verified)
        symbol = try c.decodeIfPresent(String.self, forKey: .symbol) ?? ""
        value = try c.decodeIfPresent(String.self, forKey: .value)
        decimals = try c.decodeIfPresent(Int.self, forKey: .decimals)
        exact = try c.decodeIfPresent(Bool.self, forKey: .exact) ?? false
    }
}

/// An allowance as Activity states it (`FeedAllowance`).
struct FeedAllowanceWire: Decodable, Equatable {
    /// Empty when nobody could name the token.
    let symbol: String
    /// The cap as a human decimal; `nil` when unlimited.
    let value: String?
    let decimals: Int?
    /// No limit — `componentsUi.signingApprove.unlimitedValue`, danger tone.
    let unlimited: Bool
    /// The token's contract.
    let token: String?
}

/// What a dApp record's operation was (`FeedDappOperation`).
enum FeedDappOperationWire: Decodable, Equatable {
    case contractInteraction
    case batch(calls: Int)
    case signature
    case typedDataSignature
    case unknown

    private enum CodingKeys: String, CodingKey { case type, calls }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "contract_interaction": self = .contractInteraction
        case "batch": self = .batch(calls: try c.decodeIfPresent(Int.self, forKey: .calls) ?? 0)
        case "signature": self = .signature
        case "typed_data_signature": self = .typedDataSignature
        default: self = .unknown
        }
    }
}

/// What a stored request holds (`FeedDappContent`).
enum FeedDappContentWire: String, Decodable {
    case callData = "call_data"
    case typedData = "typed_data"
    case message
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = FeedDappContentWire(rawValue: raw) ?? .unknown
    }
}

/// One line of a dApp row's detail (`FeedFact`) — its label and format are
/// the shell's, which lines and their order the core's. A line this build
/// has never heard of is `unknown`, and is left out.
enum FeedFactWire: Decodable, Equatable {
    case site(String)
    case network(chainId: Int)
    /// The contract a call went to, when it paid nobody.
    case contract(address: String, name: String?)
    /// Who got the money — a plain send's recipient, or the one a token
    /// `transfer` names; `name` is the row's name for them.
    case recipient(address: String, name: String?)
    case spender(address: String, name: String?)
    case spendingCap(FeedAllowanceWire)
    /// Epoch seconds, or `nil`: no expiry.
    case expires(at: Double?)
    case balanceChanges
    case date(timestamp: Double)
    case operation(FeedDappOperationWire)
    case content(FeedDappContentWire)
    case primaryType(String)
    case hash(txHash: String)
    case userOpHash(String)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type, site, chainId, address, name, allowance, at, timestamp
        case operation, content, txHash, hash
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "site": self = .site(try c.decode(String.self, forKey: .site))
        case "network": self = .network(chainId: try c.decode(Int.self, forKey: .chainId))
        case "contract":
            self = .contract(address: try c.decode(String.self, forKey: .address),
                             name: try c.decodeIfPresent(String.self, forKey: .name))
        case "recipient":
            self = .recipient(address: try c.decode(String.self, forKey: .address),
                              name: try c.decodeIfPresent(String.self, forKey: .name))
        case "spender":
            self = .spender(address: try c.decode(String.self, forKey: .address),
                            name: try c.decodeIfPresent(String.self, forKey: .name))
        case "spending_cap": self = .spendingCap(try c.decode(FeedAllowanceWire.self, forKey: .allowance))
        case "expires": self = .expires(at: try c.decodeIfPresent(Double.self, forKey: .at))
        case "balance_changes": self = .balanceChanges
        case "date": self = .date(timestamp: try c.decode(Double.self, forKey: .timestamp))
        case "operation": self = .operation(try c.decode(FeedDappOperationWire.self, forKey: .operation))
        case "content": self = .content(try c.decode(FeedDappContentWire.self, forKey: .content))
        case "primary_type": self = .primaryType(try c.decode(String.self, forKey: .name))
        case "hash": self = .hash(txHash: try c.decode(String.self, forKey: .txHash))
        case "user_op_hash": self = .userOpHash(try c.decode(String.self, forKey: .hash))
        default: self = .unknown
        }
    }
}

/// What a dApp row says beyond its money (`FeedDapp`, 083 H2, spec 093).
struct FeedDappWire: Decodable, Equatable {
    /// The site's host — never the dApp's own name.
    let site: String?
    /// The verb as text — shown only when `intentTerm` is `nil`.
    let intent: String?
    /// The headline verb: a key leaf under `componentsUi.signing`.
    let intentTerm: String?
    /// What the operation moved, as the sheet's simulation measured it.
    var changes: [FeedDappChangeWire] = []
    /// A swap's one coin back, beside the figure.
    var received: FeedDappChangeWire? = nil
    /// The row's figure is the simulation's expectation: "≈".
    var estimated = false
    var contractCall = false
    var action: DappActionWire = .call
    /// The title's `{{place}}`; `nil` → the title is the verb alone.
    var place: String? = nil
    /// A grant's allowance, drawn where a figure would be.
    var allowance: FeedAllowanceWire? = nil
    /// A signature: nothing was sent, nothing settles — no status chip.
    var offChain = false
    /// The detail's facts, in order.
    var facts: [FeedFactWire] = []
    /// The detail's collapsed "Technical details", in order.
    var technical: [FeedFactWire] = []

    private enum CodingKeys: String, CodingKey {
        case site, intent, intentTerm, changes, received, estimated, contractCall
        case action, place, allowance, offChain, facts, technical
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        site = try c.decodeIfPresent(String.self, forKey: .site)
        intent = try c.decodeIfPresent(String.self, forKey: .intent)
        intentTerm = try c.decodeIfPresent(String.self, forKey: .intentTerm)
        changes = try c.decodeIfPresent([FeedDappChangeWire].self, forKey: .changes) ?? []
        received = try c.decodeIfPresent(FeedDappChangeWire.self, forKey: .received)
        estimated = try c.decodeIfPresent(Bool.self, forKey: .estimated) ?? false
        contractCall = try c.decodeIfPresent(Bool.self, forKey: .contractCall) ?? false
        action = try c.decodeIfPresent(DappActionWire.self, forKey: .action) ?? .call
        place = try c.decodeIfPresent(String.self, forKey: .place)
        allowance = try c.decodeIfPresent(FeedAllowanceWire.self, forKey: .allowance)
        offChain = try c.decodeIfPresent(Bool.self, forKey: .offChain) ?? false
        facts = try c.decodeIfPresent([FeedFactWire].self, forKey: .facts) ?? []
        technical = try c.decodeIfPresent([FeedFactWire].self, forKey: .technical) ?? []
    }
}

/// A date header or an item, in render order.
enum FeedRowWire: Decodable, Equatable {
    case header(id: String, dayStartMs: Double, timestamp: Double)
    case item(FeedItemWire)

    private enum CodingKeys: String, CodingKey {
        case type, id, dayStartMs, timestamp, item
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "header":
            self = .header(
                id: try container.decode(String.self, forKey: .id),
                dayStartMs: try container.decode(Double.self, forKey: .dayStartMs),
                timestamp: try container.decode(Double.self, forKey: .timestamp)
            )
        case "item":
            self = .item(try container.decode(FeedItemWire.self, forKey: .item))
        case let other:
            // A row kind this build has never heard of. Reported rather than
            // dropped: a silently skipped row is a transaction missing from
            // somebody's history with nothing to say why.
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown feed row `\(other)`"
            )
        }
    }
}

/// The receipt toast — structured, so the shell formats the amount rather than
/// stripping a symbol back off a string.
struct FeedToastWire: Decodable, Equatable {
    let itemId: String
    let value: String
    let symbol: String
    let deadlineMs: Double
}

struct FeedViewWire: Decodable, Equatable {
    let rows: [FeedRowWire]
    /// The raw account-scoped records, for the detail sheet.
    let transactions: [FeedTxRecordWire]
    /// The row that just landed, still glowing.
    let newItemId: String?
    /// `nil` while balance privacy is on — the core withholds it there rather
    /// than trusting the shell to mask it.
    let toast: FeedToastWire?
    /// The corpus key of History's empty line (spec 082 RG5), chosen by the
    /// core from the chain filter. Loading vs empty stays the shell's.
    var historyEmptyKey: String = "history.emptyTitle"
    /// The corpus key of the home Activity's empty line, chosen the same way.
    var homeEmptyKey: String = "home.emptyNoActivity"
    /// What passed between the account and the contact whose page is open
    /// (`contact_filter_changed`, spec 093): the same items Activity draws,
    /// on every network, newest first, their second line the status, the
    /// network and the day. Empty when no contact is open.
    var contactRows: [FeedItemWire] = []

    private enum CodingKeys: String, CodingKey {
        case rows, transactions, newItemId, toast, historyEmptyKey, homeEmptyKey, contactRows
    }
}

extension FeedViewWire {
    /// Written out so a view from a core without `contact_rows` (or the
    /// empty-line keys) still reads, with none.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            rows: try c.decode([FeedRowWire].self, forKey: .rows),
            transactions: try c.decode([FeedTxRecordWire].self, forKey: .transactions),
            newItemId: try c.decodeIfPresent(String.self, forKey: .newItemId),
            toast: try c.decodeIfPresent(FeedToastWire.self, forKey: .toast),
            historyEmptyKey: try c.decodeIfPresent(String.self, forKey: .historyEmptyKey) ?? "history.emptyTitle",
            homeEmptyKey: try c.decodeIfPresent(String.self, forKey: .homeEmptyKey) ?? "home.emptyNoActivity",
            contactRows: try c.decodeIfPresent([FeedItemWire].self, forKey: .contactRows) ?? []
        )
    }
}
