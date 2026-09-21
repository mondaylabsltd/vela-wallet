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
    private(set) var fee: FeeViewWire?

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

    /// The fee row's coin list (the web's `feeOpen`). Which coins pay, what
    /// each costs and which cannot are the fee machine's; the pick is a quote
    /// PARAMETER the core re-prices the operation in, and the approve carries
    /// the same view's `fee_token` (issue #262: an account holding USDT and no
    /// ETH was quoted in ETH with no way to choose the coin it has).
    private(set) var feeOpen = false

    /// A tap on the fee row: a failed quote is asked again; with more than one
    /// coin, the list opens or closes.
    func feeTapped() {
        guard let fee else { return }
        if fee.failed != nil {
            dispatch(feeCore, ["type": "requote"])
        } else if fee.options.count > 1 {
            feeOpen.toggle()
        }
    }

    /// A coin from the list, by its row id (`SigningLive.nativeFeeId` = the
    /// chain's own). A coin that cannot pay is refused by the core.
    func pickFee(_ id: String) {
        dispatch(feeCore, [
            "type": "select_fee_asset",
            "token": id == SigningLive.nativeFeeId ? NSNull() : id as Any,
        ])
        feeOpen = false
    }

    private var signCore: CoreStore<SignViewWire>!
    private var clearCore: CoreStore<ClearSigningViewWire>!
    private var guardCore: CoreStore<GuardViewWire>!
    private var feeCore: CoreStore<FeeViewWire>!

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

    init(
        wallet: (address: String, credentialId: String),
        relay: RelayClient,
        accounts: UserOpSpine.AccountPort,
        spine: UserOpSpine,
        store: VelaStore,
        pool: RpcPool,
        ports: Ports
    ) {
        self.wallet = wallet
        self.relay = relay
        self.pool = pool
        self.ports = ports

        let signExecutor = SignExecutor(spine: spine, relay: relay, store: store)
        let clearExecutor = ClearExecutor(dataBase: ports.dataBase, pool: pool)
        let guardExecutor = GuardExecutor(pool: pool)
        let feeExecutor = FeeExecutor(
            relay: relay, accounts: accounts, measureCall: FeeExecutor.measuring(with: pool)
        )

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
        feeCore = CoreStore(
            bridge: FeePolicyCore(),
            perform: { await feeExecutor.perform($0) },
            onView: { [weak self] view in self?.commitFee(view) },
            onFault: { print("[vela-wallet] fee_policy fault: \($0)") }
        )

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

    private func requestQuote(chainId: Int) {
        guard !feeCalls.isEmpty else { return }
        Task { [weak self] in
            guard let self else { return }
            guard let deployed = await relay.isDeployed(chainId: chainId, address: wallet.address)
            else { return }
            dispatch(feeCore, [
                "type": "quote_requested",
                "chain_id": chainId,
                "account": wallet.address,
                "deployed": deployed,
                "public_key_available": true,
                "tier": "fast",
                "calls": feeCalls,
                "fee_token": NSNull(),
            ])
        }
    }

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
        fee = view
        // A quote goes stale while somebody reads. While the sheet is up and
        // nothing is signing, ask again — otherwise the slide shuts with no
        // way to reopen it, which is what Android's phase 5 watched happen.
        guard view.stale, !view.busy, !answered, !requoting,
              sign.surface == .sheet, !sign.isSigning, !sign.isSubmitting,
              let chainId = request?.chainId
        else { return }
        requoting = true
        requestQuote(chainId: chainId)
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
            // The coin the person picked pays, and the amount signed is in
            // THAT coin — the send core's own rule (`submit_user_op`): an
            // ERC-20 fee's amount rides in `fee_asset`; `total_wei` is the
            // native figure and never what an ERC-20 leg moves.
            "gas_fee_token": fee?.feeToken as Any? ?? NSNull(),
            "quoted_fee": quotedFee(fee?.fee) as Any? ?? NSNull(),
            "fee_collector": NSNull(),
            "params_override_json": guardView.rewrittenParamsJson as Any? ?? NSNull(),
            "intent": clear.result?.intent as Any? ?? NSNull(),
        ]
    }

    /// The fee the slide displayed, as signed: amount in the paying coin's
    /// base units, and where it goes. No recipient → no quoted fee (the send
    /// core's `submit_user_op` rule).
    static func quotedFee(_ estimate: FeeEstimateWire?) -> [String: Any]? {
        guard let estimate, let recipient = estimate.feeRecipient else { return nil }
        let amount: String
        switch estimate.feeAsset {
        case .erc20(_, _, let erc20Amount, _): amount = erc20Amount
        case .native: amount = estimate.totalWei
        }
        return ["amount": amount, "recipient": recipient]
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
