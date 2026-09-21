//
//  SigningController.swift
//  VelaWallet
//
//  The signing sheet's journey — four machines, one sheet.
//
//  `sign_request` owns the request's life; `clear_signing` says what the
//  transaction DOES; `approval_guard` decides what an approval may be edited
//  to; `fee_policy` prices it. The sheet reads all four, which is why they are
//  born and die together here rather than living as residents: a request that
//  ended must not leave a decoded intent or a half-edited allowance behind for
//  the next one to inherit.
//
//  Ported from `app-android/.../feature/signing/core/SigningController.kt`
//  (spec 044 T032), which is the desktop's `wallet/signing_host.rs`.
//
//  ## The confirm gate is three machines ANDed
//
//  `sign_request.confirmGateOpen` AND `approval_guard.confirmAllowed` AND
//  `fee_policy.confirmFeeReady`. Arming on one of the three is how an
//  unlimited approval gets past a guard that had not finished reading the
//  token, or how a person signs a transaction whose fee nobody could quote.
//
//  ## The order the machine is told things
//
//  `networks_changed` and `accounts_changed` BEFORE `request_arrived`. A
//  request that names a chain nobody vouched for is refused 4902 — which is
//  correct, and looks exactly like a broken chain when the reason is that the
//  shell spoke out of order.
//

import Foundation
import Observation
import VelaCore

extension SignRequestCore: CoreBridge {}
extension ClearSigningCore: CoreBridge {}
extension ApprovalGuardCore: CoreBridge {}

@MainActor
@Observable
final class SigningController {

    /// One request from a page, with the shell's facts attached.
    struct Incoming: Equatable {
        let id: String
        let method: String
        let paramsJson: String
        /// The origin the browser observed — never the page's claim.
        let origin: String
        /// The tab that asked. The answer goes there and nowhere else.
        let transportId: String
        let chainId: Int
    }

    struct Ports {
        var respond: (_ transportId: String, _ id: String, _ json: [String: Any]) -> Void = { _, _, _ in }
        /// The tracker follows the accepted operation to its verdict.
        var trackSubmitted: (_ userOpHash: String, _ recordIds: [String], _ chainId: Int) -> Void
        = { _, _, _ in }
        var recordsPersisted: () -> Void = {}
        var nativeSymbol: (_ chainId: Int) -> String = { _ in "" }
        var knownChains: () -> [Int] = { [] }
        /// The descriptor endpoint base.
        var dataBase: () -> String = { "" }
        /// The simulated deltas, to the `token_trust` machine that judges them
        /// — the ONE entrance that may never admit a token (spec 017 ⑤).
        var simDeltas: (_ address: String, _ chainId: Int, _ deltas: [[String: Any]]) -> Void
        = { _, _, _ in }
    }

    private(set) var sign: SignViewWire = .empty
    private(set) var clear: ClearSigningViewWire = .empty
    private(set) var guardView: GuardViewWire = .empty
    /// The fee in force — whichever session prices the tier in force now.
    var fee: FeeViewWire? { fees.view }
    /// The speed control, as the `fee_speed` core decided it (spec 069).
    var speed: FeeSpeedViewWire? { fees.speed }

    /// The fee view of the session pricing `tier`, for that option's line.
    func feeView(of tier: String) -> FeeViewWire? { fees.view(of: tier) }

    private(set) var request: Incoming?
    /// The page has its answer and the core has cleared the sheet. The
    /// container may drop this controller.
    private(set) var closed = false

    /// "Sign with": WHERE the passkey that signs this request is. This
    /// controller lives for one request, so the choice cannot outlive the
    /// question it was made for. `auto` is the wallet's stored route.
    private(set) var signMethod = "auto"
    private(set) var signWithOpen = false

    /// `nil` toggles the list; an id picks a method and closes it.
    func signWith(_ id: String?) {
        guard let id else {
            signWithOpen.toggle()
            return
        }
        if ["auto", "platform", "hybrid", "security_key"].contains(id) { signMethod = id }
        signWithOpen = false
    }

    private var signCore: CoreStore<SignViewWire>!
    private var clearCore: CoreStore<ClearSigningViewWire>!
    private var guardCore: CoreStore<GuardViewWire>!
    /// The fee sessions and the speed control — the very store the send form
    /// runs (spec 069), so the sheet's speeds, previews, free upgrade and "the
    /// price you tap is the price you get" cannot drift from the form's.
    private let fees: FeeStore

    private let wallet: (address: String, credentialId: String)
    private let relay: RelayClient
    private var ports: Ports

    /// Record ids already on disk, and the handoff waiting for them. The
    /// tracker is handed a hash only once every record it names exists —
    /// 052's ordering invariant, which is what makes a force-quit recoverable.
    private var persistedRecords: Set<String> = []
    private var pendingHandoff: SignTrackerHandoffWire?
    private var handedOff = false
    private var answered = false
    /// Where the simulation for the request on screen has got to.
    ///
    /// THREE states, because they are three different sentences and a boolean
    /// could only carry two. "Not yet" is silence; "the node refused" is a
    /// warning; "it ran" is the balance block.
    enum Simulation { case pending, answered, unavailable }

    private(set) var simulation: Simulation = .pending

    /// The pool, kept for the simulation — the same endpoints, bans and
    /// cooldowns the rest of the wallet uses, never one a page named.
    private let pool: RpcPool

    /// The calls this request carries, kept for the re-quote.
    private var feeCalls: [[String: Any]] = []
    private var requoting = false
    /// Whether the person could still choose, as last told to the speed core.
    private var lastOnForm: Bool?

    init(
        wallet: (address: String, credentialId: String),
        relay: RelayClient,
        accounts: UserOpSpine.AccountPort,
        spine: UserOpSpine,
        store: VelaStore,
        pool: RpcPool,
        preferredTier: @escaping () -> String = { "fast" },
        numberPreset: @escaping () -> String = { "comma_dot" },
        ports: Ports
    ) {
        self.wallet = wallet
        self.relay = relay
        self.pool = pool
        self.ports = ports
        self.preferredTier = preferredTier
        self.numberPreset = numberPreset
        self.fees = FeeStore(relay: relay, accounts: accounts)

        let signExecutor = SignExecutor(spine: spine, relay: relay, store: store)
        let clearExecutor = ClearExecutor(dataBase: ports.dataBase, pool: pool)
        let guardExecutor = GuardExecutor(pool: pool)

        signCore = CoreStore(
            bridge: SignRequestCore(),
            perform: { await signExecutor.perform($0) },
            onView: { [weak self] view in self?.commitSign(view) },
            onFault: { print("[vela-wallet] sign_request fault: \($0)") }
        )
        clearCore = CoreStore(
            bridge: ClearSigningCore(),
            perform: { await clearExecutor.perform($0) },
            onView: { [weak self] view in self?.clear = view },
            onFault: { print("[vela-wallet] clear_signing fault: \($0)") }
        )
        guardCore = CoreStore(
            bridge: ApprovalGuardCore(),
            perform: { await guardExecutor.perform($0) },
            onView: { [weak self] view in self?.guardView = view },
            onFault: { print("[vela-wallet] approval_guard fault: \($0)") }
        )
        fees.onInForce = { [weak self] view in self?.commitFee(view) }

        signExecutor.ports = SignExecutor.Ports(
            respond: { [weak self] transportId, id, json in
                self?.ports.respond(transportId, id, json)
                // The page has its answer; the sheet may go once the core has
                // cleared it.
                self?.markAnswered()
            },
            opSubmitted: { [weak self] id, hash in
                self?.dispatchSign([
                    "type": "op_submitted", "id": id, "user_op_hash": hash,
                    "now_ms": Date().timeIntervalSince1970 * 1000,
                ])
            },
            signingStarted: {},
            recordsPersisted: { [weak self] in
                self?.ports.recordsPersisted()
                self?.recordLanded()
            },
            switchAccount: { _ in true },
            nativeSymbol: ports.nativeSymbol
        )
    }

    // MARK: - Opening

    func open(_ incoming: Incoming) {
        request = incoming
        let nowMs = Date().timeIntervalSince1970 * 1000
        // Each request starts at the stored default: a pick is one-shot.
        fees.resetSpeed()
        fees.configureSpeed(preferred: preferredTier(), number: numberPreset())

        // The world first. A machine told nothing refuses a request that names
        // a chain, and the refusal is indistinguishable from a broken network.
        dispatchSign(["type": "networks_changed", "chain_ids": ports.knownChains()])
        dispatchSign([
            "type": "accounts_changed",
            "accounts": [["address": wallet.address, "credential_id": wallet.credentialId]],
            "active_index": 0,
        ])
        dispatchSign([
            "type": "request_arrived",
            "id": incoming.id,
            "method": incoming.method,
            "params_json": incoming.paramsJson,
            "origin": incoming.origin,
            "transport_id": incoming.transportId,
            "dedicated_transport": true,
            "per_request_chain": incoming.chainId,
            "dapp": NSNull(),
            "granted_address": NSNull(),
            "requested_address": NSNull(),
            "request_ts_ms": NSNull(),
            "now_ms": nowMs,
        ])

        // What it does, in words. The BROWSER's fact about who is asking,
        // never the page's claim.
        if let kickoff = Self.clearKickoff(
            method: incoming.method, paramsJson: incoming.paramsJson,
            chainId: incoming.chainId, origin: incoming.origin.isEmpty ? nil : incoming.origin
        ) {
            dispatch(clearCore, kickoff)
        }

        dispatch(guardCore, [
            "type": "approval_detected",
            "method": incoming.method,
            "params_json": incoming.paramsJson,
            "chain_id": incoming.chainId,
            "wallet_address": wallet.address,
            "read_only": false,
            "now_ms": nowMs,
        ])

        guard let calls = SignExecutor.callsOf(method: incoming.method, paramsJson: incoming.paramsJson)
        else { return }
        feeCalls = calls.map { ["to": $0.to, "value": $0.value, "data": $0.data] }
        requestQuote(chainId: incoming.chainId)
        simulate(chainId: incoming.chainId, calls: calls)
    }

    /// Ask the chain what these calls WOULD do, and hand the answer to the core
    /// that judges it.
    ///
    /// Three outcomes and they are not the same fact:
    ///
    /// - deltas → the balance block;
    /// - an empty list → "checked, nothing moves";
    /// - no answer at all → `simulated` stays false, and the sheet says it
    ///   could not look.
    ///
    /// The last one is the one that matters. A wallet that says nothing when it
    /// could not check teaches people that silence means safe.
    private func simulate(chainId: Int, calls: [UserOpCall]) {
        simulation = .pending
        let legs = calls.map {
            SimDeltas.Call(to: $0.to, value: $0.value, data: $0.data)
        }
        guard let payload = SimDeltas.payload(from: wallet.address, calls: legs) else {
            simulation = .unavailable
            return
        }
        Task { [weak self] in
            guard let self else { return }
            let answer = await pool.call(
                chainId: chainId, method: "eth_simulateV1", params: payload, kind: "rpc"
            )
            guard case .ok(let body) = answer,
                  let logs = SimDeltas.logsOf(["result": body ?? NSNull()])
            else {
                // The node has told us NOTHING — no `eth_simulateV1`, or it
                // errored. Not the same as "nothing moves", and the sheet says
                // which of the two this is.
                simulation = .unavailable
                return
            }
            simulation = .answered
            ports.simDeltas(
                wallet.address, chainId,
                SimDeltas.deriveDeltas(logs: logs, user: wallet.address)
            )
        }
    }

    /// The stored default speed (spec 069): a dApp transaction is priced —
    /// and, through the quoted fee, submitted — at the speed Settings names,
    /// which is `fast` for everybody who never chose, until the sheet's own
    /// speed control picks another. The number preset writes each gas bid.
    private let preferredTier: () -> String
    private let numberPreset: () -> String

    private func requestQuote(chainId: Int) {
        guard !feeCalls.isEmpty else { return }
        Task { [weak self] in
            guard let self else { return }
            guard let deployed = await relay.isDeployed(chainId: chainId, address: wallet.address)
            else { return }
            // HOW FAST is the speed core's to say: the store asks at its tier.
            fees.ask(
                chainId: chainId, account: wallet.address, deployed: deployed,
                publicKeyAvailable: true, calls: feeCalls, feeToken: nil
            )
        }
    }

    // MARK: - The speed control (spec 069)

    /// `nil` folds or unfolds the control; a tier is a one-shot pick, never
    /// the stored preference.
    func speed(_ tier: String?) {
        guard let tier else {
            fees.toggleSpeed()
            return
        }
        fees.pickSpeed(tier)
    }

    /// The refresh control: measure again, the held readings dropped first.
    func refreshFee() { fees.refresh() }

    // MARK: - What the sheet does

    /// The slide fired.
    func approve() {
        dispatchSign(["type": "approve_tapped", "opts": Self.approveOpts(
            fee: fee, clear: clear, guard: guardView
        )])
    }

    func reject() { dispatchSign(["type": "reject_tapped"]) }
    func dismiss() { dispatchSign(["type": "dismiss_tapped"]) }

    /// The sheet was swiped away.
    ///
    /// **Always this, never `reject`.** The core routes by phase: the funding
    /// view means cancel the funding; an error, a submitted or a submitting
    /// state means dismiss; anything earlier means refuse. A shell that picked
    /// one itself would answer a page 4001 for a transaction already on chain.
    func swipeDismissed() { dispatchSign(["type": "swipe_dismissed"]) }

    func fundingCancelled() { dispatchSign(["type": "funding_cancelled"]) }
    func fundingComplete() { dispatchSign(["type": "funding_complete_tapped"]) }

    func guardPreset(_ mode: String) {
        dispatch(guardCore, ["type": "preset_selected", "mode": mode])
    }

    func guardCustomAmount(_ text: String) {
        dispatch(guardCore, ["type": "custom_amount_changed", "text": text])
    }

    /// The BOOLEAN card's two deliberate answers — `setApprovalForAll`, a DAI
    /// permit — where there is no amount to cap and the choice is yes or no.
    ///
    /// **Not the editor's chips.** 撤销 on the amount editor is
    /// `preset_selected { mode: "revoke" }`; sending `revoke_chosen` from
    /// there is an event the editor does not answer, and the chip does
    /// nothing at all (device-found, 053 phase 5).
    ///
    /// Neither has a control on the drawn sheet yet: the boolean card is
    /// `SigningBlock`'s vocabulary and nothing builds one. Recorded rather
    /// than deleted — the core's rule is that a grant-all is never
    /// preselected and must be tapped deliberately, and that surface is owed.
    func guardRevoke() { dispatch(guardCore, ["type": "revoke_chosen"]) }
    func guardGrant() { dispatch(guardCore, ["type": "grant_deliberately_chosen"]) }

    /// The one gate the sheet reads. See the file header.
    var confirmEnabled: Bool {
        sign.confirmGateOpen
            && guardView.confirmAllowed
            && (fee?.confirmFeeReady ?? false)
            && !SigningLive.feeOfAnotherTier(fee, speedTier: speed?.tier)
            && !sign.isSigning
            && !sign.isSubmitting
    }

    // MARK: - Plumbing

    private func dispatchSign(_ event: [String: Any]) { dispatch(signCore, event) }

    private func dispatch<V: Decodable>(_ core: CoreStore<V>, _ event: [String: Any]) {
        let json = CoreJSON.string(event)
        if !core.boot(json) { core.dispatch(json) }
    }

    private func commitSign(_ view: SignViewWire) {
        sign = view
        // A free upgrade is decided only while the person can still choose —
        // never under a slide that has already gone.
        let onForm = view.surface == .sheet && !view.isSigning && !view.isSubmitting
        if onForm != lastOnForm {
            lastOnForm = onForm
            fees.speedStage(onForm: onForm)
        }

        if let handoff = view.trackerHandoff, !handedOff {
            handedOff = true
            pendingHandoff = handoff
            tryHandoff()
        }
        if view.surface == .hidden, view.request == nil, request != nil, answered {
            closed = true
        }
    }

    private func commitFee(_ view: FeeViewWire) {
        // A quote goes stale while somebody reads. While the sheet is up and
        // nothing is signing, ask again — otherwise the slide shuts with no
        // way to reopen it, which is what Android's phase 5 watched happen.
        // The core keeps the request it priced; `requote` re-runs THAT one.
        guard view.stale, !view.busy, !answered, !requoting,
              sign.surface == .sheet, !sign.isSigning, !sign.isSubmitting
        else { return }
        requoting = true
        fees.requote()
        Task { @MainActor [weak self] in self?.requoting = false }
    }

    private func recordLanded() {
        guard let handoff = pendingHandoff else { return }
        // `persist_record` answered; the ids it wrote are the handoff's.
        persistedRecords.formUnion(handoff.recordIds)
        tryHandoff()
    }

    private func tryHandoff() {
        guard let handoff = pendingHandoff,
              handoff.recordIds.allSatisfy({ persistedRecords.contains($0) })
        else { return }
        pendingHandoff = nil
        ports.trackSubmitted(handoff.userOpHash, handoff.recordIds, handoff.chainId)
    }

    private func markAnswered() {
        answered = true
        if sign.surface == .hidden { closed = true }
    }

    // MARK: - The pure parts

    /// What the confirm slides into: the fee as quoted, the guard's rewrite,
    /// the intent.
    ///
    /// `params_override_json` is the load-bearing one. When the guard rewrote
    /// an approval, **these** params are what gets signed, submitted and
    /// recorded — never the original request.
    static func approveOpts(
        fee: FeeViewWire?, clear: ClearSigningViewWire, guard guardView: GuardViewWire
    ) -> [String: Any] {
        [
            "max_fee_per_gas": fee?.fee?.maxFeePerGas as Any? ?? NSNull(),
            "bundler_cost_wei": NSNull(),
            "gas_fee_token": NSNull(),
            // …with the speed this very estimate was priced at, named on the
            // wire beside the amount (spec 069); the core drops a `rapid`.
            "quoted_fee": fee?.fee.map { estimate in
                ["amount": estimate.totalWei, "recipient": estimate.feeRecipient ?? "", "tier": estimate.tier]
                    as [String: Any]
            } as Any? ?? NSNull(),
            "fee_collector": NSNull(),
            "params_override_json": guardView.rewrittenParamsJson as Any? ?? NSNull(),
            "intent": clear.result?.intent as Any? ?? NSNull(),
        ]
    }

    /// The first call of a request: `to`, `data`, `value`.
    static func firstCall(paramsJson: String) -> (to: String?, data: String?, value: String?)? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              let first = params.first as? [String: Any]
        else { return nil }
        // A batch's first leg is what the sheet leads with.
        let call = ((first["calls"] as? [[String: Any]])?.first) ?? first
        func field(_ name: String) -> String? {
            (call[name] as? String).flatMap { $0.isEmpty ? nil : $0 }
        }
        return (to: field("to"), data: field("data"), value: field("value"))
    }

    static func clearKickoff(
        method: String, paramsJson: String, chainId: Int, origin: String?
    ) -> [String: Any]? {
        if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
            let call = firstCall(paramsJson: paramsJson)
            return [
                "type": "resolve_transaction",
                "to": call?.to as Any? ?? NSNull(),
                "data": call?.data as Any? ?? NSNull(),
                "value": call?.value as Any? ?? NSNull(),
                "chain_id": chainId,
                "locale": defaultLocale,
            ]
        }
        if method.contains("signTypedData") {
            return [
                "type": "resolve_typed_data",
                "typed_data_json": typedDataOf(paramsJson: paramsJson),
                "chain_id": chainId,
                "locale": defaultLocale,
            ]
        }
        if method == "personal_sign" || method == "eth_sign" {
            return [
                "type": "message_presented",
                "method": method == "eth_sign" ? "eth_sign" : "personal_sign",
                "params": stringParams(paramsJson: paramsJson),
                "request_origin": origin as Any? ?? NSNull(),
            ]
        }
        return nil
    }

    /// How numbers and dates are written inside a clear-signing panel.
    ///
    /// **The person's own presets** (spec 056). 053 shipped the defaults here
    /// and recorded it: a signing sheet printing a date in a format the rest of
    /// the app does not use is a small wrongness, and this is the cut that
    /// removes it. The core does the formatting inside the panel, so what it
    /// needs is the RESOLVED preset — never the word "auto", which only a shell
    /// can turn into a convention.
    static var defaultLocale: [String: Any] {
        [
            "number_format": Formats.resolve(Formats.current.number).rawValue,
            "date_format": Formats.resolve(Formats.current.date).rawValue,
            "time_format": Formats.resolve(Formats.current.time).rawValue,
            "tz_offset_minutes": TimeZone.current.secondsFromGMT() / 60,
        ]
    }

    /// `eth_signTypedData_v4`'s payload is `[address, json]` — the JSON is the
    /// **second** parameter.
    static func typedDataOf(paramsJson: String) -> String {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              params.count > 1
        else { return "" }
        if let text = params[1] as? String { return text }
        guard let encoded = try? JSONSerialization.data(withJSONObject: params[1]),
              let text = String(data: encoded, encoding: .utf8)
        else { return "" }
        return text
    }

    static func stringParams(paramsJson: String) -> [String] {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else { return [] }
        return params.compactMap { $0 as? String }
    }
}
