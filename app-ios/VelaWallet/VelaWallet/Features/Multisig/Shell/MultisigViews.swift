//
//  MultisigViews.swift
//  VelaWallet
//
//  The multisig machines' views, typed for the screens — read from the
//  core's JSON (`app-web/vela-wallet/src/lib/multisig/generated/` is the
//  wire's mirror), never re-derived. Every field is lenient: a field this
//  build does not know is skipped, one it expects and is not there reads as
//  its empty value, so a newer core never blanks a screen.
//
//  Parts the wire carries as `unknown` (a stored record, the one review, a
//  preview) are kept whole as `MsJSON` and read through small accessors.
//

import Foundation

// MARK: - Any JSON

/// Any JSON value, kept as it came.
indirect enum MsJSON: Decodable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([MsJSON])
    case object([String: MsJSON])

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode([MsJSON].self) { self = .array(value) }
        else if let value = try? container.decode([String: MsJSON].self) { self = .object(value) }
        else { self = .null }
    }

    /// From a Foundation object (`JSONSerialization`'s).
    init(any: Any?) {
        switch any {
        case nil, is NSNull: self = .null
        case let n as NSNumber:
            if CFGetTypeID(n) == CFBooleanGetTypeID() { self = .bool(n.boolValue) } else { self = .number(n.doubleValue) }
        case let s as String: self = .string(s)
        case let a as [Any]: self = .array(a.map { MsJSON(any: $0) })
        case let o as [String: Any]: self = .object(o.mapValues { MsJSON(any: $0) })
        default: self = .null
        }
    }

    subscript(key: String) -> MsJSON? {
        if case .object(let o) = self { return o[key] }
        return nil
    }

    var string: String? {
        switch self {
        case .string(let s): return s
        case .number(let n): return n.rounded() == n ? String(Int64(n)) : String(n)
        default: return nil
        }
    }
    var int: Int? {
        switch self {
        case .number(let n): return Int(n)
        case .string(let s): return Int(s)
        default: return nil
        }
    }
    var bool: Bool? { if case .bool(let b) = self { return b }; return nil }
    /// A number as it is (a time in ms is past `Int`'s comfort on no platform,
    /// but a fraction must not be cut).
    var double: Double? {
        switch self {
        case .number(let n): return n
        case .string(let s): return Double(s)
        default: return nil
        }
    }
    var array: [MsJSON] { if case .array(let a) = self { return a }; return [] }
    var object: [String: MsJSON] { if case .object(let o) = self { return o }; return [:] }
    var isNull: Bool { self == .null }
    var kind: String? { self["kind"]?.string ?? self["type"]?.string }

    /// Back to a Foundation object, for an event that carries it whole.
    var any: Any {
        switch self {
        case .null: return NSNull()
        case .bool(let b): return b
        case .number(let n): return n.rounded() == n && abs(n) < 9e15 ? Int64(n) as Any : n
        case .string(let s): return s
        case .array(let a): return a.map(\.any)
        case .object(let o): return o.mapValues(\.any)
        }
    }
}

/// Decoding helpers: lenient reads.
extension KeyedDecodingContainer {
    func soft<T: Decodable>(_ key: Key, _ fallback: T) -> T {
        (try? decodeIfPresent(T.self, forKey: key)) ?? fallback
    }
    func soft<T: Decodable>(_ key: Key) -> T? {
        (try? decodeIfPresent(T.self, forKey: key)) ?? nil
    }
}

/// A coding key from any string.
struct MsKey: CodingKey {
    let stringValue: String
    var intValue: Int? { nil }
    init(_ s: String) { stringValue = s }
    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { nil }
}

// MARK: - Accounts

/// A multisig on this device — the stored record (`MultisigAccount`).
struct MsAccount: Decodable, Equatable, Identifiable {
    var address: String
    var owners: [String]
    var threshold: Int
    var recorded: Bool
    var shared: Bool?
    var chainIds: [Int]
    var label: String?
    var members: [String: MsMemberInfo]
    var firstRun: Bool?
    var genesisName: String?
    /// The record as stored, whole (events carry it back unchanged).
    var raw: MsJSON

    var id: String { address.lowercased() }
    var isShared: Bool { shared == true || recorded }

    init(from decoder: Decoder) throws {
        raw = try MsJSON(from: decoder)
        let c = try decoder.container(keyedBy: MsKey.self)
        address = c.soft(MsKey("address"), "")
        owners = c.soft(MsKey("owners"), [])
        threshold = c.soft(MsKey("threshold"), 1)
        recorded = c.soft(MsKey("recorded"), false)
        shared = c.soft(MsKey("shared"))
        chainIds = c.soft(MsKey("chainIds"), [])
        label = c.soft(MsKey("label"))
        members = c.soft(MsKey("members"), [:])
        firstRun = c.soft(MsKey("firstRun"))
        genesisName = raw["genesis"]?["name"]?.string
    }
}

struct MsMemberInfo: Decodable, Equatable {
    var name: String?
    var kind: String?
    var typed: Bool?
}

struct MsAccountRow: Decodable, Equatable, Identifiable {
    var account: MsAccount
    var name: String?
    var shared: Bool
    var recordWaiting: Bool
    var localMembers: [String]
    var highRiskOn: [Int]
    var refusedOn: [Int]
    var checkedOn: [Int]
    var checkingOn: [Int]
    var inviteShared: Bool
    /// r9-12: the invite's members as the core builds them (founding, then
    /// members now; a name only where one is known, and the kind) — the
    /// link door's `addLink` takes them as they are.
    var inviteMembers: MsJSON
    /// r9-14: the rule every surface draws for it (`MultisigRowRule`): the
    /// chip as the chains last said (`from_chain`), else the founding one;
    /// `last_known` when no chain read this session said it. `nil` from a
    /// core without the field.
    var rule: MsJSON?

    var id: String { account.id }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        account = try c.decode(MsAccount.self, forKey: MsKey("account"))
        name = c.soft(MsKey("name"))
        shared = c.soft(MsKey("shared"), false)
        recordWaiting = c.soft(MsKey("record_waiting"), false)
        localMembers = c.soft(MsKey("local_members"), [])
        highRiskOn = c.soft(MsKey("high_risk_on"), [])
        refusedOn = c.soft(MsKey("refused_on"), [])
        checkedOn = c.soft(MsKey("checked_on"), [])
        checkingOn = c.soft(MsKey("checking_on"), [])
        inviteShared = c.soft(MsKey("invite_shared"), false)
        inviteMembers = c.soft(MsKey("invite_members"), MsJSON.array([]))
        rule = c.soft(MsKey("rule"))
    }
}

enum MsPhase: String, Decodable {
    case idle, working, done, cancelled, failed
}

struct MsCreateDraft: Decodable, Equatable {
    var owners: [String]
    var members: [String: MsMemberInfo]
    var name: String
    var threshold: Int
    var chainId: Int?

    init(owners: [String], members: [String: MsMemberInfo], name: String, threshold: Int, chainId: Int?) {
        self.owners = owners
        self.members = members
        self.name = name
        self.threshold = threshold
        self.chainId = chainId
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        owners = c.soft(MsKey("owners"), [])
        members = c.soft(MsKey("members"), [:])
        name = c.soft(MsKey("name"), "")
        threshold = c.soft(MsKey("threshold"), 1)
        chainId = c.soft(MsKey("chainId"))
    }
}

struct MsCreateView: Decodable, Equatable {
    var phase: MsPhase = .idle
    var draftAddress: String?
    var draftError: String?
    var address: String?
    var refused: String?
    var error: String?
    /// `present` / `missing` / `unknown`, per chain id.
    var networks: [String: MsJSON] = [:]
    var otherSpace: [Int] = []
    var kept: MsCreateDraft?
    /// core-ms-2: the threshold, its note and its bounds (`MultisigThresholdView`).
    var threshold: MsJSON?

    init() {}
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        phase = c.soft(MsKey("phase"), .idle)
        draftAddress = c.soft(MsKey("draft_address"))
        draftError = c.soft(MsKey("draft_error"))
        address = c.soft(MsKey("address"))
        refused = c.soft(MsKey("refused"))
        error = c.soft(MsKey("error"))
        networks = c.soft(MsKey("networks"), [:])
        otherSpace = c.soft(MsKey("other_space"), [])
        kept = c.soft(MsKey("kept"))
        threshold = c.soft(MsKey("threshold"))
    }

    /// The Safe contracts on `chainId`: `present`, `missing`, `unknown` or `nil` (not asked).
    func contracts(on chainId: Int) -> String? { networks[String(chainId)]?.string ?? networks[String(chainId)]?.kind }
}

struct MsAddView: Decodable, Equatable {
    var phase: MsPhase = .idle
    var address: String?
    var outcome: MsJSON?
    /// r14-5: the outcome is a read that did not come back (the core's
    /// `AddOutcome::offers_retry`): Add says Try again.
    var retry = false
    var found: MsJSON?
    var ens: MsJSON?

    init() {}
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        phase = c.soft(MsKey("phase"), .idle)
        address = c.soft(MsKey("address"))
        outcome = c.soft(MsKey("outcome"))
        retry = c.soft(MsKey("retry"), false)
        found = c.soft(MsKey("found"))
        ens = c.soft(MsKey("ens"))
    }
}

struct MsImportView: Decodable, Equatable {
    var safes: [MsJSON] = []
    var error: String?
    var busy: String?
    var outcomes: [String: MsJSON] = [:]

    init() {}
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        safes = c.soft(MsKey("safes"), [])
        error = c.soft(MsKey("error"))
        busy = c.soft(MsKey("busy"))
        outcomes = c.soft(MsKey("outcomes"), [:])
    }
}

struct MsRenamePlan: Decodable, Equatable {
    var chainId: Int?
    var deployed: Bool?
    var blocked: String?
    var landsOn: Int?
    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id", deployed, blocked, landsOn = "lands_on"
    }
}

struct MsKindChoice: Decodable, Equatable {
    var kind: String?
    var suggested: String?
    var locked: Bool
    var note: String?

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        kind = c.soft(MsKey("kind"))
        suggested = c.soft(MsKey("suggested"))
        locked = c.soft(MsKey("locked"), false)
        note = c.soft(MsKey("note"))
    }
}

/// Where a link opened here stands (`MultisigLinkStand`).
struct MsLinkStand: Decodable, Equatable {
    var kind: String
    /// The link's own facts, as the core read them.
    var link: MsJSON?
    var text: String?
    var fault: String?
    var address: String?
    var keptBefore: Bool
    var kept: Bool
    var reason: MsJSON?
    /// §18 (c), round 14: a failed lookup that did not come back — the
    /// core offers Try again (`retry_link`).
    var retry: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        kind = c.soft(MsKey("kind"), "")
        retry = c.soft(MsKey("retry"), false)
        link = c.soft(MsKey("link"))
        text = c.soft(MsKey("text"))
        fault = c.soft(MsKey("fault"))
        address = c.soft(MsKey("address"))
        keptBefore = c.soft(MsKey("kept_before"), false)
        kept = c.soft(MsKey("kept"), false)
        reason = c.soft(MsKey("reason"))
    }

    var chainId: Int? { link?["chainId"]?.int }
    var multisig: String? { link?["multisig"]?.string }
    var safeTxHash: String? { link?["safeTxHash"]?.string }
    var chainIds: [Int] { link?["chainIds"]?.array.compactMap(\.int) ?? [] }
}

struct MsAccountsView: Decodable, Equatable {
    var loaded = false
    var accounts: [MsAccountRow] = []
    var active: String?
    var create = MsCreateView()
    var add = MsAddView()
    var importing = MsImportView()
    var backup: [String: String] = [:]
    var shareAgain: [String: MsPhase] = [:]
    var shareAgainFailure: [String: String] = [:]
    var rename: [String: MsRenamePlan] = [:]
    var serviceDown = false
    var link: MsLinkStand?
    var memberKinds: [String: MsKindChoice] = [:]
    var knownMembers: [String: MsMemberInfo] = [:]
    /// r9-12: ONE name per member, by lowercase address, by the core's rule
    /// (kept, then the passkey index, then this device's own name, then the
    /// short address). Drawn as it is; no shell picks a name.
    var memberLabels: [String: String] = [:]
    /// r9-12: the addresses the passkey index has not answered for yet
    /// (this device's own among them): looked up, answered `index_names`.
    var indexWanted: [String] = []
    /// core-ms-2: the invite the machine opened (`MultisigInviteView`).
    var invite: MsJSON?
    /// core-ms-2: the network hints, read by the core.
    var recentNetworks: [Int] = []
    var usedNetworks: [Int] = []

    init() {}
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        loaded = c.soft(MsKey("loaded"), false)
        accounts = c.soft(MsKey("accounts"), [])
        active = c.soft(MsKey("active"))
        create = c.soft(MsKey("create"), MsCreateView())
        add = c.soft(MsKey("add"), MsAddView())
        importing = c.soft(MsKey("import"), MsImportView())
        backup = c.soft(MsKey("backup"), [:])
        shareAgain = c.soft(MsKey("share_again"), [:])
        shareAgainFailure = c.soft(MsKey("share_again_failure"), [:])
        rename = c.soft(MsKey("rename"), [:])
        serviceDown = c.soft(MsKey("service_down"), false)
        link = c.soft(MsKey("link"))
        memberKinds = c.soft(MsKey("member_kinds"), [:])
        knownMembers = c.soft(MsKey("known_members"), [:])
        memberLabels = c.soft(MsKey("member_labels"), [:])
        indexWanted = c.soft(MsKey("index_wanted"), [])
        invite = c.soft(MsKey("invite"))
        recentNetworks = c.soft(MsKey("recent_networks"), [])
        usedNetworks = c.soft(MsKey("used_networks"), [])
    }

    func row(_ address: String?) -> MsAccountRow? {
        guard let address else { return nil }
        return accounts.first { $0.account.address.caseInsensitiveCompare(address) == .orderedSame }
    }

    /// The multisig shown, when one is.
    var activeRow: MsAccountRow? { row(active) }
}

// MARK: - Proposals

struct MsApproval: Decodable, Equatable {
    var owner: String
    /// Verified, and its owner is a member on this chain now (the core's).
    var counts: Bool
    var raw: MsJSON
    init(from decoder: Decoder) throws {
        raw = try MsJSON(from: decoder)
        owner = raw["owner"]?.string ?? ""
        counts = raw["counts"]?.bool ?? true
    }
}

struct MsProposalItem: Decodable, Equatable, Identifiable {
    var chainId: Int
    var safeTxHash: String
    var tx: MsJSON
    var review: MsJSON
    var nonce: String
    var proposer: String
    var approvals: [MsApproval]
    var approved: Int
    var threshold: Int
    var status: String
    var canApprove: [String]
    var canExecute: [String]
    var cancelledBy: String?
    var sameNonce: [String]
    var executing: Bool
    var createdMs: Double
    var unsent: Bool
    var renameUndeployed: Bool
    /// r9-9: every member of it on this device (the core's), whatever this
    /// transaction asks of them — `nil` from a core without the field.
    var membersHere: [String]?
    /// r11: every number (decimal) a queued transaction waits behind, the
    /// core's (`waits_for`, at most 20).
    var waitsFor: [String]

    var id: String { "\(chainId):\(safeTxHash.lowercased())" }
    var key: String { "\(chainId):\(safeTxHash)" }
    /// What it does, as the core read it (`Review.action`).
    var action: MsJSON? { review["action"].flatMap { $0.isNull ? nil : $0 } }
    var refusal: String? { review["refusal"]?.string }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        chainId = c.soft(MsKey("chainId"), 0)
        safeTxHash = c.soft(MsKey("safeTxHash"), "")
        tx = c.soft(MsKey("tx"), .null)
        review = c.soft(MsKey("review"), .null)
        nonce = (c.soft(MsKey("nonce")) as MsJSON?)?.string ?? "0"
        proposer = c.soft(MsKey("proposer"), "")
        approvals = c.soft(MsKey("approvals"), [])
        approved = c.soft(MsKey("approved"), 0)
        threshold = c.soft(MsKey("threshold"), 1)
        status = c.soft(MsKey("status"), "pending")
        canApprove = c.soft(MsKey("canApprove"), [])
        canExecute = c.soft(MsKey("canExecute"), [])
        cancelledBy = c.soft(MsKey("cancelledBy"))
        sameNonce = c.soft(MsKey("sameNonce"), [])
        executing = c.soft(MsKey("executing"), false)
        createdMs = c.soft(MsKey("createdMs"), 0)
        unsent = c.soft(MsKey("unsent"), false)
        renameUndeployed = c.soft(MsKey("renameUndeployed"), false)
        membersHere = c.soft(MsKey("membersHere"))
        waitsFor = c.soft(MsKey("waitsFor"), [])
    }
}

struct MsChainMembers: Decodable, Equatable {
    var deployed: Bool
    var owners: [String]
    var threshold: Int
    var nonce: String

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        deployed = c.soft(MsKey("deployed"), false)
        owners = c.soft(MsKey("owners"), [])
        threshold = c.soft(MsKey("threshold"), 1)
        nonce = (c.soft(MsKey("nonce")) as MsJSON?)?.string ?? "0"
    }
}

struct MsChainState: Decodable, Equatable {
    var chainId: Int
    var members: MsChainMembers?
    var items: [MsProposalItem]
    var mailbox: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        chainId = c.soft(MsKey("chainId"), 0)
        members = c.soft(MsKey("members"))
        items = c.soft(MsKey("items"), [])
        mailbox = c.soft(MsKey("mailbox"), false)
    }
}

/// A tagged value the screens switch on by `kind` and read fields from.
struct MsTagged: Decodable, Equatable {
    var kind: String
    var raw: MsJSON
    init(from decoder: Decoder) throws {
        raw = try MsJSON(from: decoder)
        kind = raw.kind ?? ""
    }
    subscript(key: String) -> MsJSON? { raw[key] }
}

struct MsDraft: Decodable, Equatable {
    var item: MsProposalItem
    var review: MsJSON
    /// §12: where `review` stands (`MultisigReviewState`: `loading`,
    /// `ready`, `failed {reason_key}`) — kept as the core wrote it for the
    /// link door's `review.door`; `nil` from a core without it (the door
    /// reads that as `loading`: nothing is signed).
    var reviewState: MsJSON?
    var membersHere: [String]
    var defaultMember: String?
    /// core-ms-2: ready on its proposer's approval — propose and execute.
    var executeNow: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        executeNow = c.soft(MsKey("executeNow"), false)
        item = try c.decode(MsProposalItem.self, forKey: MsKey("item"))
        review = c.soft(MsKey("review"), .null)
        reviewState = c.soft(MsKey("reviewState"))
        membersHere = c.soft(MsKey("membersHere"), [])
        defaultMember = c.soft(MsKey("defaultMember"))
    }
}

struct MsSigners: Decodable, Equatable {
    struct Row: Decodable, Equatable {
        var owner: String
        var kind: String?
    }
    var rows: [Row]
    var outside: [String]
    var inVela: [String]
    var either: [String]
    var contract: [String]
    var enough: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        rows = c.soft(MsKey("rows"), [])
        outside = c.soft(MsKey("outside"), [])
        inVela = c.soft(MsKey("in_vela"), [])
        either = c.soft(MsKey("either"), [])
        contract = c.soft(MsKey("contract"), [])
        enough = c.soft(MsKey("enough"), false)
    }
}

struct MsWaiting: Decodable, Equatable {
    var needsYou: Int
    var ready: Int
    var waitingPayer: Int
    var first: MsJSON?
    /// r11-1: the transactions `needsYou` / `ready` count, each
    /// `<chainId>:<safeTxHash lowercase>` — what a notification keys
    /// "already said" by (a count says nothing of WHICH).
    var needsYouKeys: [String]
    var readyKeys: [String]
    /// r11-1: when the list behind these was read on this page (ms); `nil`
    /// for one kept from an earlier page — never news.
    var readAt: Double?

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        needsYou = c.soft(MsKey("needs_you"), 0)
        ready = c.soft(MsKey("ready"), 0)
        waitingPayer = c.soft(MsKey("waiting_payer"), 0)
        first = c.soft(MsKey("first"))
        needsYouKeys = c.soft(MsKey("needs_you_keys"), [])
        readyKeys = c.soft(MsKey("ready_keys"), [])
        readAt = c.soft(MsKey("read_at"))
    }
}

struct MsLinkedView: Decodable, Equatable {
    var safe: String
    var chainId: Int
    var safeTxHash: String
    var preview: Bool
    var stand: String
    var item: MsProposalItem?
    var members: MsChainMembers?
    var stale: Bool
    /// r13-3: what a `failed` / `unreadable` / `notFound` stand says (the
    /// core's key), and whether it offers Try again.
    var reasonKey: String?
    var retry: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        reasonKey = c.soft(MsKey("reason_key"))
        retry = c.soft(MsKey("retry"), false)
        safe = c.soft(MsKey("safe"), "")
        chainId = c.soft(MsKey("chain_id"), 0)
        safeTxHash = c.soft(MsKey("safe_tx_hash"), "")
        preview = c.soft(MsKey("preview"), false)
        stand = c.soft(MsKey("stand"), "reading")
        item = c.soft(MsKey("item"))
        members = c.soft(MsKey("members"))
        stale = c.soft(MsKey("stale"), false)
    }
}

struct MsProposalsView: Decodable, Equatable {
    var followed: String?
    var chains: [MsChainState] = []
    var items: [MsProposalItem] = []
    var openCount = 0
    var waitingOnMe = 0
    var missing: [MsJSON] = []
    var cachedAt: Double?
    var unreadable: [Int] = []
    var historyUnreadable: [Int] = []
    /// r13-7: where the followed multisig's history stands on this page —
    /// `reading` (its first read is out: the home's Activity is loading,
    /// never "No activity yet"), `read`, or `unread` (said, with Try again).
    /// A view from a core that does not say it reads as `read`.
    var history = "read"
    var offline = false
    var readAt: Double?
    var loading = false
    var stale = false
    var unreachable = false
    var paceMs = 12_000
    var outbox: [String: MsJSON] = [:]
    var draft: MsDraft?
    var actions: [String: MsTagged] = [:]
    var reviews: [String: MsJSON] = [:]
    /// §12: where each review stands, per `<chainId>:<safeTxHash>` (the
    /// keys of `reviews`); missing — not asked yet — is `loading`.
    var reviewStates: [String: MsJSON] = [:]
    var previews: [String: MsJSON] = [:]
    var executions: [String: MsJSON] = [:]
    /// An executed transaction's facts as they were when it ran
    /// (`FrozenFacts`: executor, threshold, approvers, executorApproved,
    /// atMs) — read by `timeline` once it is done; never re-counted.
    var frozen: [String: MsJSON] = [:]
    var lastActor: String?
    var presence: [String: MsWaiting?] = [:]
    var othersWaiting = false
    var membersAnywhere: [String] = []
    var standings: [String: MsTagged] = [:]
    var waitingNote: MsTagged?
    var timelines: [String: [MsTagged]] = [:]
    var dapp: [String: MsJSON] = [:]
    var callsStatus: [String: MsJSON] = [:]
    var linked: MsLinkedView?
    var signers: [String: MsSigners] = [:]
    /// core-ms-2: the members here whose one press approves and executes.
    var approveExecute: [String: [String]] = [:]
    /// core-ms-2: the header chip, the waiting line's parts, the banner.
    var ruleChip: MsJSON?
    var waitingParts: [String: [MsJSON]] = [:]
    var banner: MsJSON?
    var setUp = false
    /// r8-2: the tokens the proposals shown move, per `chainId:token`
    /// (`TokenFacts`: symbol, decimals — either null when unread).
    var tokens: [String: MsJSON] = [:]

    init() {}
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: MsKey.self)
        followed = c.soft(MsKey("followed"))
        chains = c.soft(MsKey("chains"), [])
        items = c.soft(MsKey("items"), [])
        openCount = c.soft(MsKey("open_count"), 0)
        waitingOnMe = c.soft(MsKey("waiting_on_me"), 0)
        missing = c.soft(MsKey("missing"), [])
        cachedAt = c.soft(MsKey("cached_at"))
        unreadable = c.soft(MsKey("unreadable"), [])
        historyUnreadable = c.soft(MsKey("history_unreadable"), [])
        history = c.soft(MsKey("history"), "read")
        offline = c.soft(MsKey("offline"), false)
        readAt = c.soft(MsKey("read_at"))
        loading = c.soft(MsKey("loading"), false)
        stale = c.soft(MsKey("stale"), false)
        unreachable = c.soft(MsKey("unreachable"), false)
        paceMs = c.soft(MsKey("pace_ms"), 12_000)
        outbox = c.soft(MsKey("outbox"), [:])
        draft = c.soft(MsKey("draft"))
        actions = c.soft(MsKey("actions"), [:])
        reviews = c.soft(MsKey("reviews"), [:])
        reviewStates = c.soft(MsKey("review_states"), [:])
        previews = c.soft(MsKey("previews"), [:])
        executions = c.soft(MsKey("executions"), [:])
        frozen = c.soft(MsKey("frozen"), [:])
        lastActor = c.soft(MsKey("last_actor"))
        presence = c.soft(MsKey("presence"), [:])
        othersWaiting = c.soft(MsKey("others_waiting"), false)
        membersAnywhere = c.soft(MsKey("members_anywhere"), [])
        standings = c.soft(MsKey("standings"), [:])
        waitingNote = c.soft(MsKey("waiting_note"))
        timelines = c.soft(MsKey("timelines"), [:])
        dapp = c.soft(MsKey("dapp"), [:])
        callsStatus = c.soft(MsKey("calls_status"), [:])
        linked = c.soft(MsKey("linked"))
        signers = c.soft(MsKey("signers"), [:])
        approveExecute = c.soft(MsKey("approve_execute"), [:])
        ruleChip = c.soft(MsKey("rule_chip"))
        waitingParts = c.soft(MsKey("waiting_parts"), [:])
        banner = c.soft(MsKey("banner"))
        setUp = c.soft(MsKey("set_up"), false)
        tokens = c.soft(MsKey("tokens"), [:])
    }

    /// The members here whose one press approves and executes `item`.
    func approveExecute(_ item: MsProposalItem) -> [String] {
        lookup(approveExecute, chainId: item.chainId, hash: item.safeTxHash) ?? []
    }

    func item(chainId: Int, hash: String) -> MsProposalItem? {
        items.first { $0.chainId == chainId && $0.safeTxHash.caseInsensitiveCompare(hash) == .orderedSame }
            ?? chains.flatMap(\.items).first { $0.chainId == chainId && $0.safeTxHash.caseInsensitiveCompare(hash) == .orderedSame }
    }

    func chain(_ chainId: Int) -> MsChainState? { chains.first { $0.chainId == chainId } }

    /// `<chainId>:<hash>` maps are keyed as the core wrote the hash; look
    /// both spellings up.
    func lookup<T>(_ map: [String: T], chainId: Int, hash: String) -> T? {
        map["\(chainId):\(hash)"] ?? map["\(chainId):\(hash.lowercased())"]
            ?? map.first { $0.key.lowercased() == "\(chainId):\(hash.lowercased())" }?.value
    }

    func review(_ item: MsProposalItem) -> MsJSON? { lookup(reviews, chainId: item.chainId, hash: item.safeTxHash) }
    /// §12: where `item`'s review stands, as the core wrote it (`nil`: not
    /// asked yet).
    func reviewState(_ item: MsProposalItem) -> MsJSON? {
        lookup(reviewStates, chainId: item.chainId, hash: item.safeTxHash)
    }
    func standing(_ item: MsProposalItem) -> MsTagged? { lookup(standings, chainId: item.chainId, hash: item.safeTxHash) }
    func timeline(_ item: MsProposalItem) -> [MsTagged] { lookup(timelines, chainId: item.chainId, hash: item.safeTxHash) ?? [] }
    func signers(_ item: MsProposalItem) -> MsSigners? { lookup(signers, chainId: item.chainId, hash: item.safeTxHash) }
    func execution(_ item: MsProposalItem) -> MsJSON? { lookup(executions, chainId: item.chainId, hash: item.safeTxHash) }
    func frozen(_ item: MsProposalItem) -> MsJSON? {
        lookup(frozen, chainId: item.chainId, hash: item.safeTxHash).flatMap { $0.isNull ? nil : $0 }
    }
    func preview(_ item: MsProposalItem, executor: String) -> MsJSON? {
        let key = "\(item.chainId):\(item.safeTxHash.lowercased()):\(executor.lowercased())"
        return previews[key] ?? previews.first { $0.key.lowercased() == key }?.value
    }
    func action(_ key: String) -> MsTagged? {
        actions[key] ?? actions.first { $0.key.lowercased() == key.lowercased() }?.value
    }
}

// MARK: - Members

struct MsMembersView: Decodable, Equatable {
    var address: String?
    var raw: MsJSON = .null

    init() {}
    init(from decoder: Decoder) throws {
        raw = try MsJSON(from: decoder)
        address = raw["address"]?.string
    }
}

extension MultisigMachine {
    /// The accounts machine's view, typed.
    var accountsView: MsAccountsView { decoded(MsAccountsView.self) ?? MsAccountsView() }
    /// The proposals machine's view, typed.
    var proposalsView: MsProposalsView { decoded(MsProposalsView.self) ?? MsProposalsView() }
    /// The raw view as `MsJSON`.
    var json: MsJSON { MsJSON(any: view) }
}
