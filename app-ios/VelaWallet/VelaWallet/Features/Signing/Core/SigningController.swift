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
//  ## The confirm gate is the core's (spec 099 R7)
//
//  `sign_request`, `approval_guard`, `clear_signing` and `fee_policy` each
//  have a say, and the core's `signConfirmState` joins them over their views
//  as the core wrote them — with which part is shut and the line that says
//  so. Arming on fewer is how an unlimited approval gets past a guard that
//  had not finished reading the token, or how a person signs a transaction
//  whose fee nobody could quote. This file used to AND three of them itself,
//  without the off-chain and still-reading rules the sheet's own copy had;
//  both copies are gone (`confirmState`).
//
//  ## The confirm waits for the simulation's verdict (PR 3)
//
//  The gate looked at the request, the reading, the guard and the fee — not
//  at the simulation — so a person could confirm before the balance changes
//  were on the sheet, and those are the one part of it a site cannot write.
//  The wait is the core's (`sign_request`): this file only says when the
//  simulation of the request on the sheet has been SENT (`sim_started`, in
//  the step that opens the request) and when what stands in the verdict's
//  place is no longer "Checking…" (`sim_settled`) — a notice at once, a
//  checked answer once its tokens are judged (`simVerdict`). The deadline is
//  the core's too, run by `SignExecutor`'s timer. Nothing here shuts the
//  confirm, and nothing here keeps a clock.
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
        /// The address the site was shown (spec 070). `sign_request` signs
        /// from it, and never silently from another account.
        var grantedAddress: String? = nil
        /// The wallet asked this of itself (the key backup) — set where it
        /// raises one, `RootView.openEthereumBackup`, and nowhere else. A page
        /// never is, whatever bytes it submits.
        var firstParty: Bool = false
    }

    struct Ports {
        /// The answer, exactly once: the core's `SignResponsePayload`, and the
        /// user-operation hash when that hash IS the answer (the receipt did
        /// not land in time) — so the browser can translate the page's
        /// receipt polls for it.
        var respond: (_ transportId: String, _ id: String, _ payload: [String: Any], _ userOpHash: String?) -> Void
        = { _, _, _, _ in }
        /// The tracker follows the accepted operation to its verdict — or,
        /// `maybeSent`, the one whose submit reply was lost, from the head
        /// read before its POST (`submitBlock`, ruling 8).
        var trackSubmitted: (TrackSubmission) -> Void = { _ in }
        /// A write-ahead op the core proved never sent (spec 082 RJ1): the
        /// tracker drops it.
        var trackWithdrawn: (_ userOpHash: String, _ recordIds: [String]) -> Void = { _, _ in }
        var recordsPersisted: () -> Void = {}
        var nativeSymbol: (_ chainId: Int) -> String = { _ in "" }
        var knownChains: () -> [Int] = { [] }
        /// The descriptor endpoint base.
        var dataBase: () -> String = { "" }
        /// The simulated deltas, to the `token_trust` machine that judges them
        /// — the ONE entrance that may never admit a token (spec 017 ⑤) — and
        /// its judgment of THOSE deltas, once every token in them is named
        /// (`TokenTrustStore.simJudged`): the view the sheet draws its
        /// "Balance changes" from, and what the record keeps (083 F1, spec
        /// 093). `nil` back — or no judge at all — is a checked answer nobody
        /// judged: the sheet cannot report it, and says so. `@MainActor`: see
        /// `simulate`.
        var simJudged: (@MainActor (_ address: String, _ chainId: Int, _ deltas: [[String: Any]]) async
            -> TrustSimViewWire?)? = nil
        /// `eth_simulateV1` with these params, its answer as it came; `nil` =
        /// through the pool (the Android twin's `Ports.simulate`). What the
        /// answer MEANS stays the core's (`simOutcome`). `@MainActor`: the
        /// controller calls it there, and an unannotated async function type
        /// is a call to NULL on iOS 17 (087 F33).
        var simulate: (@MainActor (_ chainId: Int, _ params: [Any]) async -> RpcOutcome)? = nil
    }

    private(set) var sign: SignViewWire = .empty
    private(set) var clear: ClearSigningViewWire = .empty
    private(set) var guardView: GuardViewWire = .empty
    /// The three views as the core wrote them, for `signConfirmState`.
    private(set) var signJson: String?
    private(set) var clearJson: String?
    private(set) var guardJson: String?
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

    /// How the last Trusted Signer ceremony for this request ended without a
    /// signature. The core heard a cancelled ceremony and kept the request
    /// open; this is the sentence that says why, until the next confirm.
    private(set) var trustedSignerNotice: TrustedSignerNotice?

    /// The fee row's coin list (the web's `feeOpen`). Which coins pay, what
    /// each costs and which cannot are the fee machine's; the pick is a quote
    /// PARAMETER the core re-prices the operation in, and the approve carries
    /// the same view's `fee_token` (issue #262: an account holding USDT and no
    /// ETH was quoted in ETH with no way to choose the coin it has).
    private(set) var feeOpen = false

    /// A tap on the fee row — exactly what its words say (PR 2 polish). A
    /// failed quote's `tap`: `retry` asks again at once (the core's `requote`
    /// — it drops its own timer for the next re-ask; while a re-ask is out
    /// the core asks nothing more), `choose_coin` opens the coin list ("Pay
    /// with another coin" after `would_fail`), `nothing` does nothing. With
    /// no failure and more than one coin, the list opens or closes.
    func feeTapped() {
        guard let fee else { return }
        switch Self.feeTap(fee) {
        case .retry:
            // Measured again for real — the held readings dropped first.
            fees.refresh()
        case .open, .chooseCoin:
            feeOpen.toggle()
        case .nothing:
            break
        }
    }

    /// What a tap on the sheet's fee row does over `fee`: its failure's
    /// `tap`, else the coin list when there is more than one coin to choose.
    static func feeTap(_ fee: FeeViewWire) -> FeeRowTap {
        if let failure = fee.failure { return failure.rowTap }
        return fee.options.count > 1 ? .open : .nothing
    }

    /// A coin from the list, by its row id (`SigningLive.nativeFeeId` = the
    /// chain's own). A coin that cannot pay is refused by the core. Every
    /// speed is re-priced in it, so a speed picked next is still paid in the
    /// coin chosen (spec 069).
    func pickFee(_ id: String) {
        fees.chooseFeeToken(id == SigningLive.nativeFeeId ? nil : id)
        feeOpen = false
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
    private let spine: UserOpSpine
    /// Kept for its one stoppable clock (`elapseSimVerdictTimer`).
    private let signExecutor: SignExecutor

    /// Spec 079 (owner: one slide, not two) and 102: this account reviews and
    /// signs on a trusted page, whose own slide is the consent — so the sheet
    /// is the hand-off card and its confirm goes there. Read once per request,
    /// from the plan the spine signs by.
    private(set) var venuePlan: SigningPlanWire?
    var trustedSignerRoute: Bool { venuePage != nil }
    /// The page, when the venue is one.
    var venuePage: String? { venuePlan?.venue.pageUrl }
    /// "Confirm with …" — the plan's own name for the key (D-17).
    var venueKeyLabel: KeyLabelWire? { venuePlan?.keyLabel }
    private var ports: Ports

    /// Record ids already on disk, and the handoff waiting for them. The
    /// tracker is handed a hash only once every record it names exists —
    /// 052's ordering invariant, which is what makes a force-quit recoverable.
    private var persistedRecords: Set<String> = []
    private var pendingHandoff: SignTrackerHandoffWire?
    /// Handoffs already given to the tracker, by hash, records and the two
    /// facts that change between hand-offs of ONE op (spec 082 RJ1): the
    /// write-ahead hand-off (`maybeSent`) and the relay's acceptance
    /// (`admitted`) name the same hash and the same records, and keyed on
    /// those alone the second was never fed — an accepted op then read "may
    /// have been sent" and could end "not sent" over money that landed.
    private var handedOff: Set<String> = []
    /// Withdrawals already given to the tracker, once per value.
    private var withdrawn: Set<String> = []
    /// The tracker's last view, for the answer that follows it (RJ4).
    private var trackerView: TrackViewWire?
    /// The last entry state forwarded as `op_tracked`, so one state reaches
    /// the core once however often the tracker's view is rebuilt.
    private var lastTracked: String?
    private var answered = false
    /// The page behind this request is gone (`transportDropped`): nothing
    /// may be signed or sent for it any more (RB2).
    private var askerGone = false
    /// When the confirm was tapped — the start of the answer window (RA12).
    private var approvedAtMs: Double?
    /// The operation the relay accepted for this request, once it has.
    private var submittedHash: String?

    /// The page has its answer. The sheet may still be up (a submitted state
    /// waiting to be dismissed), but nothing more will be said to the page.
    var hasAnswered: Bool { answered }

    /// The operation this request handed the tracker, once it has — what
    /// the ending's `signEndingOf` names whether or not the answer is it.
    var submittedUserOp: String? { submittedHash }

    /// Past the point of no return: a passkey ceremony or a submit is under
    /// way. A controller in this state is not dropped when its page goes away
    /// — the operation may land, and its record must still be written.
    ///
    /// A failure on the sheet ends that (spec 097 N4): the core holds a
    /// refusal the tracker reached after the submit until the sheet closes,
    /// and with the page gone it drops it unanswered (the browser answered
    /// 4900) — nothing is left to land, record or answer.
    var committed: Bool {
        sign.isSigning || sign.isSubmitting || (submittedHash != nil && !answered && sign.error == nil)
    }
    /// Where the simulation for the request on screen has got to.
    ///
    /// THREE states, because they are three different sentences and a boolean
    /// could only carry two. "Not yet" is silence; "it ran" is the balance
    /// block; anything else is the CORE's notice (`simOutcome`, spec 082
    /// RG6): a revert is danger — "expected to fail", with its sanitised
    /// reason — and a node that could not or would not check is caution,
    /// "Vela couldn't check what this does". Which is which is not decided
    /// here.
    enum Simulation: Equatable {
        case pending
        case answered
        /// `risk` is a `ClearRisk` wire name, `key` a corpus key, `reason` the
        /// `{{reason}}` it takes.
        case notice(risk: String, key: String, reason: String?)
    }

    private(set) var simulation: Simulation = .pending

    /// THIS request's checked answer, judged: what the verdict's place draws
    /// once `simulation` is `.answered`. `nil` until the judgment is in — the
    /// place says "Checking…" meanwhile — and the request's own, never the
    /// judge's last session: a verdict for another transaction is worse than
    /// none.
    private(set) var simVerdict: TrustSimViewWire?

    /// The pool, kept for the simulation — the same endpoints, bans and
    /// cooldowns the rest of the wallet uses, never one a page named.
    private let pool: RpcPool

    /// The calls this request carries, kept for the re-quote.
    private var feeCalls: [[String: Any]] = []
    /// Whether the person could still choose, as last told to the speed core.
    private var lastOnForm: Bool?

    init(
        wallet: (address: String, credentialId: String),
        relay: RelayClient,
        accounts: UserOpSpine.AccountPort,
        spine: UserOpSpine,
        store: VelaStore,
        pool: RpcPool,
        preferredTier: @escaping () -> String = { "standard" },
        numberPreset: @escaping () -> String = { "comma_dot" },
        ports: Ports,
        timers: FeeStore.Timers = .wallClock
    ) {
        self.wallet = wallet
        self.relay = relay
        self.pool = pool
        self.ports = ports
        self.preferredTier = preferredTier
        self.numberPreset = numberPreset
        self.fees = FeeStore(
            relay: relay, accounts: accounts, measureCall: FeeExecutor.measuring(with: pool),
            timers: timers
        )

        self.spine = spine
        let signExecutor = SignExecutor(spine: spine, relay: relay, store: store, timers: timers)
        self.signExecutor = signExecutor
        let clearExecutor = ClearExecutor(dataBase: ports.dataBase, pool: pool)
        let guardExecutor = GuardExecutor(pool: pool)

        signCore = CoreStore(
            bridge: SignRequestCore(),
            perform: { await signExecutor.perform($0) },
            onView: { [weak self] view in self?.commitSign(view) },
            onFault: { VelaLog.failure(.sign, kind: "sign_request_fault", VelaLog.error($0)) },
            keepsJson: true
        )
        clearCore = CoreStore(
            bridge: ClearSigningCore(),
            perform: { await clearExecutor.perform($0) },
            onView: { [weak self] view in
                guard let self else { return }
                clear = view
                clearJson = clearCore.json
            },
            onFault: { VelaLog.failure(.sign, kind: "clear_signing_fault", VelaLog.error($0)) },
            keepsJson: true
        )
        guardCore = CoreStore(
            bridge: ApprovalGuardCore(),
            perform: { await guardExecutor.perform($0) },
            onView: { [weak self] view in
                guard let self else { return }
                guardView = view
                guardJson = guardCore.json
            },
            onFault: { VelaLog.failure(.sign, kind: "approval_guard_fault", VelaLog.error($0)) },
            keepsJson: true
        )
        // Each machine's idle view until it speaks: the gate is asked over
        // views, never over nothing.
        signJson = signCore.json
        clearJson = clearCore.json
        guardJson = guardCore.json

        signExecutor.ports = SignExecutor.Ports(
            respond: { [weak self] transportId, id, payload in
                guard let self else { return }
                // Mark first: the answer may make the browser forward its next
                // request at once, and that request must find this one done.
                markAnswered()
                ports.respond(transportId, id, payload, Self.opHashAnswer(payload, submitted: submittedHash))
            },
            opSubmitted: { [weak self] id, hash, maybeSent, submitBlock in
                self?.submittedHash = hash
                self?.dispatchSign([
                    "type": "op_submitted", "id": id, "user_op_hash": hash,
                    "now_ms": Date().timeIntervalSince1970 * 1000,
                    "maybe_sent": maybeSent,
                    "submit_block": submitBlock.map { $0 as Any } ?? NSNull(),
                ])
                // What the tracker already knows about this op, now that the
                // core will take it (RJ4).
                self?.forwardTracked()
            },
            // The record before the bytes (spec 082 RJ1): the core writes it
            // "may have been sent", hands it to the tracker, then clears.
            opSigned: { [weak self] id, hash, submitBlock in
                self?.dispatchSign([
                    "type": "op_signed", "id": id, "user_op_hash": hash,
                    "submit_block": submitBlock.map { $0 as Any } ?? NSNull(),
                    "now_ms": Date().timeIntervalSince1970 * 1000,
                ])
            },
            answered: { [weak self] in self?.answered ?? true },
            // The prompt's own bracket (RA9): the sheet says "awaiting your
            // signature" only while it is really up — never through the
            // network work before it.
            signingStarted: { [weak self] id in
                self?.dispatchSign(["type": "ceremony_started", "id": id])
            },
            ceremonyDone: { [weak self] id in
                self?.dispatchSign(["type": "ceremony_done", "id": id])
            },
            askerLive: { [weak self] in !(self?.askerGone ?? true) },
            approvedAtMs: { [weak self] in self?.approvedAtMs },
            recordsPersisted: { [weak self] in self?.ports.recordsPersisted() },
            recordWritten: { [weak self] id in self?.recordLanded(id) },
            switchAccount: { _ in true },
            nativeSymbol: ports.nativeSymbol,
            // The browser's fact about who asked. The wallet's own request is
            // not a site, and the page draws an empty origin as the wallet.
            origin: { [weak self] in
                guard let request = self?.request, !request.firstParty else { return "" }
                return request.origin
            },
            // A page in this app's browser — never the wallet's own requests
            // (the key backup): the browser saw that origin (spec 079).
            originSeenByBrowser: { [weak self] in
                guard let request = self?.request else { return false }
                return !request.firstParty
            },
            trustedSignerEnded: { [weak self] notice in self?.trustedSignerNotice = notice }
        )
    }

    // MARK: - Opening

    func open(_ incoming: Incoming) {
        request = incoming
        let nowMs = Date().timeIntervalSince1970 * 1000
        venuePlan = nil
        Task { [weak self, spine, wallet] in
            let plan = await spine.plan(account: wallet.address)
            guard let self, self.request == incoming else { return }
            self.venuePlan = plan
            // The card's line: checked now, unless a fresh ruling stands.
            if let page = plan?.venue.pageUrl { SignerPageChecks.shared.prime(page) }
        }
        // Each request starts at the stored defaults: a pick is one-shot.
        fees.resetSpeed()
        fees.configureSpeed(preferred: preferredTier(), number: numberPreset())
        trustedSignerNotice = nil

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
            "first_party": incoming.firstParty,
            "dedicated_transport": true,
            "per_request_chain": incoming.chainId,
            "dapp": NSNull(),
            "granted_address": incoming.grantedAddress.flatMap { $0.isEmpty ? nil : $0 } as Any? ?? NSNull(),
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
        // A message never gets here: it has no calls, starts no simulation,
        // and its confirm waits for none.
        simulate(id: incoming.id, chainId: incoming.chainId, calls: calls, feeCalls: feeCalls)
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
    ///
    /// Spec 083 fee, issue #411: deltas also go to the fee machine — to every
    /// session pricing `feeCalls` — before the tokens are judged, because which
    /// coin can pay does not wait on token names. A Uniswap swap on Polygon
    /// whose path named both stablecoins was paid in POL, held at 0, because
    /// nobody told the machine what the swap leaves. A revert, or a run
    /// nobody could check, tells it nothing.
    ///
    /// PR 3: the confirm waits for the verdict, and the core keeps that wait.
    /// It is told the simulation is out in THIS step (`sim_started` — after
    /// `request_arrived`, before anything is drawn, and only when one really
    /// is sent), and told it has settled in the very step the verdict's place
    /// stops saying "Checking…": a notice at once; a checked answer when its
    /// judgment is in, not when the node replied.
    private func simulate(id: String, chainId: Int, calls: [UserOpCall], feeCalls: [[String: Any]]) {
        simulation = .pending
        simVerdict = nil
        let legs = calls.map {
            SimDeltas.Call(to: $0.to, value: $0.value, data: $0.data)
        }
        guard let payload = SimDeltas.payload(from: wallet.address, calls: legs) else {
            // Nothing could be asked, so nothing is: the place says the
            // core's could-not-check line from its first frame, and the core
            // is told of no simulation — the confirm waits for none.
            simulation = Self.simulation(of: simOutcome(user: wallet.address, replyJson: #"{"unreachable":true}"#))
            return
        }
        dispatchSign(["type": "sim_started", "id": id])
        Task { [weak self] in
            guard let self else { return }
            let answer: RpcOutcome
            if let simulate = ports.simulate {
                answer = await simulate(chainId, payload)
            } else {
                answer = await pool.call(
                    chainId: chainId, method: "eth_simulateV1", params: payload, kind: "rpc"
                )
            }
            let outcome = simOutcome(user: wallet.address, replyJson: Self.simReply(answer))
            simulation = Self.simulation(of: outcome)
            if outcome.kind != "deltas" {
                VelaLog.failure(.sign, kind: "sim_\(outcome.kind)", "chain=\(chainId)")
                // The notice is the verdict, and it is on the sheet now.
                simSettled(id)
                return
            }
            // Checked: the person's own moves, to the machine that judges
            // them — the one entrance that may never admit a token.
            let parsed = (try? JSONSerialization.jsonObject(with: Data(outcome.deltasJson.utf8))) as? [[String: Any]]
            let deltas = parsed ?? []
            // What it moves, to the fee machine first: moves this build could
            // not read are no measurement, and it is told nothing.
            if let parsed, let changes = SimDeltas.feeBalanceChanges(parsed) {
                fees.balanceChanges(calls: feeCalls, changes: changes)
            }
            // The request left the sheet while the node was answering — it
            // was refused, or is already being signed: nobody is left to read
            // a verdict, and the judge's one session belongs to whichever
            // sheet is up now.
            guard sign.request?.id == id, !committed else { return }
            var judged: TrustSimViewWire?
            if let judge = ports.simJudged { judged = await judge(wallet.address, chainId, deltas) }
            guard let judged else {
                // Checked, and nobody judged it (a later simulation took the
                // judge's session, or this host has no judge): a check this
                // sheet cannot report. The core's could-not-check verdict,
                // now — never "Checking…" until the deadline.
                simulation = Self.simulation(
                    of: simOutcome(user: wallet.address, replyJson: #"{"unreachable":true}"#)
                )
                simSettled(id)
                return
            }
            // The verdict and the word that it is on the sheet, in one step:
            // no frame draws one without the other.
            simVerdict = judged
            simSettled(id)
        }
    }

    /// What stands in the verdict's place for request `id` is no longer
    /// "Checking…": the core stops holding the confirm for it, and takes the
    /// timed-out line back off the sheet if its deadline had passed.
    private func simSettled(_ id: String) {
        guard request?.id == id else { return }
        dispatchSign(["type": "sim_settled", "id": id])
    }

    /// A test's clock moving, for a controller built with `timers: .stopped`:
    /// the simulation's deadline (`sim_verdict_timer`) runs out now.
    func elapseSimVerdictTimer() { signExecutor.elapseSimVerdictTimer() }

    /// How many simulation deadlines a stopped clock holds (tests).
    var heldSimVerdictTimers: Int { signExecutor.heldSimVerdictTimers }

    /// The pool's answer, normalised into the reply `simOutcome` reads:
    /// `{"result": …}`, `{"error": {code, message}}` or `{"unreachable": true}`.
    /// Nothing is judged here — a revert inside a result, a node that cannot
    /// simulate and a node nobody reached are the core's to tell apart.
    static func simReply(_ answer: RpcOutcome) -> String {
        switch answer {
        case .ok(let body):
            return CoreJSON.string(["result": body ?? NSNull()])
        case .rpcError(let code, let message):
            return CoreJSON.string(["error": ["code": code.map { $0 as Any } ?? NSNull(), "message": message]])
        case .failed, .rangeCap:
            return #"{"unreachable":true}"#
        }
    }

    /// The sheet's state for the core's verdict.
    static func simulation(of outcome: SimOutcomeRecord) -> Simulation {
        guard outcome.kind != "deltas" else { return .answered }
        return .notice(
            risk: outcome.noticeRisk ?? "caution",
            key: outcome.noticeKey ?? "componentsUi.signing.simUnavailableWarning",
            reason: outcome.revertReason
        )
    }

    /// The stored default speed (spec 069): a dApp transaction is priced —
    /// and, through the quoted fee, submitted — at the speed Settings names,
    /// which is `standard` for everybody who never chose, until the sheet's own
    /// speed control picks another. The number preset writes each gas bid.
    private let preferredTier: () -> String
    private let numberPreset: () -> String

    /// Price the request. The account's deployment is read by the fee machine
    /// itself (`read_deployment`, issue #483): a read that fails is the fee's
    /// own failure — on the row, the footer (the core's gate sees it) and the
    /// retry alike — worded "can't reach the chain" only when the chain did not
    /// answer, and read again by every retry (`fresh`). The shell's own read
    /// and its "quote could not start" state are gone with it: the footer used
    /// to say "Working out the network fee…" under a row that said "Tap to
    /// retry", because the core's gate never saw a failure the shell kept.
    ///
    /// HOW FAST is the speed core's to say: the store asks at its tier. WHICH
    /// COIN is the fee machine's until the person taps one (spec 078): it pays
    /// in a coin that can, and the approve carries the fee view's `fee_token`
    /// — the very coin it picked — beside the amount from the same estimate
    /// (`approveOpts`), so what the sheet shows is what is signed. A tap
    /// re-asks with the pick turned off (`FeeStore.chooseFeeToken`).
    private func requestQuote(chainId: Int) {
        guard !feeCalls.isEmpty else { return }
        fees.ask(
            chainId: chainId, account: wallet.address, deployed: false,
            publicKeyAvailable: true, calls: feeCalls, feeToken: nil,
            autoFeeToken: true, readDeployment: true
        )
    }

    /// A test's clock moving, for a controller built with `timers:
    /// .stopped`: every held `timer` of the fee sessions runs out now.
    func elapseFeeTimer(_ timer: String) { fees.elapse(timer) }

    /// No fee read is out — only timers a stopped clock holds (tests).
    var feeIdle: Bool { fees.isIdle }

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

    /// The refresh control (spec 079 — it had no caller until then): measure
    /// again, the held readings dropped first — the account's deployment
    /// included (issue #483). Before any view (nothing asked yet), it asks.
    func refreshFee() {
        guard fees.view != nil else {
            if let request { requestQuote(chainId: request.chainId) }
            return
        }
        fees.refresh()
    }

    // MARK: - What the sheet does

    /// The confirm was tapped.
    func approve() {
        trustedSignerNotice = nil
        approvedAtMs = Date().timeIntervalSince1970 * 1000
        dispatchSign(["type": "approve_tapped", "opts": Self.approveOpts(
            fee: fee, clear: clear, guard: guardView,
            balanceChanges: Self.drawnChanges(simVerdict, simulation: simulation)
        )])
    }

    /// What the sheet drew under "Balance changes" as the confirm was tapped (083
    /// F1, spec 093): the core's judgments, exactly as `SigningLive.balanceBlocks`
    /// gates them — this request's simulation answered, its judgments ready
    /// and not empty. `nil` otherwise (a notice stood there, or nothing yet),
    /// and the record keeps none.
    static func drawnChanges(_ sim: TrustSimViewWire?, simulation: Simulation) -> [TrustSimJudgmentWire]? {
        guard simulation == .answered, let sim, sim.ready, !sim.judgments.isEmpty else { return nil }
        return sim.judgments
    }

    func reject() { dispatchSign(["type": "reject_tapped"]) }
    func dismiss() { dispatchSign(["type": "dismiss_tapped"]) }
    /// Spec 096 F8: Try again on a failure that sent nothing — the core takes
    /// the request back to review, still unanswered.
    func retry() { dispatchSign(["type": "retry_tapped"]) }

    /// The sheet's ✕ — since spec 079 its only close (no swipe, owner ruling).
    ///
    /// **Always this, never `reject`.** The core routes by phase: the funding
    /// view means cancel the funding; an error, a submitted or a submitting
    /// state means dismiss; anything earlier means refuse. A shell that picked
    /// one itself would answer a page 4001 for a transaction already on chain.
    func swipeDismissed() {
        closedByPerson = true
        dispatchSign(["type": "swipe_dismissed"])
    }

    /// The last view the core showed a sheet for, and its JSON.
    private var lastShown: SignViewWire?
    private var lastShownJson: String?

    /// What the sheet draws: the core's view, or — in the one turn between
    /// the core clearing the sheet to answer the page and that answer being
    /// sent (the answer is an effect, run a turn later) — the view it last
    /// showed. Spec 079: the sheet stays up across that gap and turns into
    /// the ending, instead of closing and reopening; presenting a sheet while
    /// the same one is still leaving is how iOS ends up showing neither.
    var shownSign: SignViewWire {
        if sign.isVisible || answered || closedByPerson { return sign }
        return lastShown ?? sign
    }

    /// `shownSign` as the core wrote it.
    var shownSignJson: String? {
        if sign.isVisible || answered || closedByPerson { return signJson }
        return lastShown == nil ? signJson : lastShownJson
    }

    /// The person closed this request's sheet themselves. After the approval
    /// that refuses nothing — the operation goes on and the page still gets
    /// its answer — but the ending is not brought back on screen: a sheet
    /// somebody closed does not reopen by itself (spec 079).
    private(set) var closedByPerson = false

    /// The page behind this request is gone and has already been answered
    /// (4900, by the browser core). The core clears the sheet; a pipeline
    /// already past the commitment keeps running so its record is written.
    func transportDropped() {
        guard let request else { return }
        askerGone = true
        dispatchSign(["type": "transport_dropped", "transport_id": request.transportId])
    }

    func fundingCancelled() { dispatchSign(["type": "funding_cancelled"]) }
    func fundingComplete() { dispatchSign(["type": "funding_complete_tapped"]) }

    func guardPreset(_ mode: String) {
        dispatch(guardCore, ["type": "preset_selected", "mode": mode])
    }

    func guardCustomAmount(_ text: String) {
        dispatch(guardCore, ["type": "custom_amount_changed", "text": text])
    }

    /// One batch leg's chip and field. The core ignores `preset_selected` /
    /// `custom_amount_changed` on a batch — they are the SINGLE approval's —
    /// so a leg card that sent them drew chips that did nothing.
    func guardLegPreset(_ index: Int, _ mode: String) {
        dispatch(guardCore, ["type": "leg_preset_selected", "index": index, "mode": mode])
    }

    func guardLegCustomAmount(_ index: Int, _ text: String) {
        dispatch(guardCore, ["type": "leg_custom_amount_changed", "index": index, "text": text])
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

    /// The one gate the sheet reads — the core's, over the views the sheet
    /// draws (`shownSign`) and the speed in force. See the file header.
    var confirmState: SignConfirmStateWire {
        SignConfirmStateWire.of(
            sign: shownSignJson, guard: guardJson, clear: clearJson,
            fee: fees.viewJson, speedTier: speed?.tier
        )
    }

    // MARK: - Plumbing

    private func dispatchSign(_ event: [String: Any]) { dispatch(signCore, event) }

    private func dispatch<V: Decodable>(_ core: CoreStore<V>, _ event: [String: Any]) {
        let json = CoreJSON.string(event)
        if !core.boot(json) { core.dispatch(json) }
    }

    private func commitSign(_ view: SignViewWire) {
        sign = view
        signJson = signCore.json
        if view.isVisible {
            lastShown = view
            lastShownJson = signJson
        }
        // A free upgrade is decided only while the person can still choose —
        // never under a confirm that has already gone.
        let onForm = view.surface == .sheet && !view.isSigning && !view.isSubmitting
        if onForm != lastOnForm {
            lastOnForm = onForm
            fees.speedStage(onForm: onForm)
        }

        // A withdrawal first: when the relay answered another hash, the core
        // takes the write-ahead op back and hands the relay's over in the
        // same view.
        if let withdraw = view.trackerWithdraw {
            let key = Self.withdrawKey(withdraw)
            if !withdrawn.contains(key) {
                withdrawn.insert(key)
                if let pending = pendingHandoff,
                   pending.userOpHash.caseInsensitiveCompare(withdraw.userOpHash) == .orderedSame {
                    // Never handed over, and now never will be.
                    pendingHandoff = nil
                }
                // Those records are being deleted: a hand-off that names the
                // same ids again (the relay's own hash) waits for its own write.
                persistedRecords.subtract(withdraw.recordIds)
                VelaLog.notice(.sign, "write-ahead withdrawn hash=\(VelaLog.short(withdraw.userOpHash))")
                ports.trackWithdrawn(withdraw.userOpHash, withdraw.recordIds)
            }
        }
        if let handoff = view.trackerHandoff {
            let key = Self.handoffKey(handoff)
            if !handedOff.contains(key) {
                handedOff.insert(key)
                pendingHandoff = handoff
                tryHandoff()
            }
        }
        if view.surface == .hidden, view.request == nil, request != nil, answered {
            closed = true
            endFees()
        }
        // The page left and the pipeline has stopped with nothing sent
        // (`asker_gone`, RB2): nobody will be answered, so nothing keeps this
        // request alive either.
        if askerGone, view.phase == .idle, !view.isSigning, !view.isSubmitting,
           submittedHash == nil, request != nil {
            closed = true
            endFees()
        }
    }

    /// The sheet is over (the page has its answer, or the controller is
    /// closed): its fee stops asking and answering — the core's own re-ask
    /// after a failure and its block-time tick both run on `start_ttl`, and a
    /// sheet nobody looks at must not keep reading the chain (PR 2 note 1).
    /// The last fee view stays drawn: the sheet turning into its ending keeps
    /// its row as it stood.
    private func endFees() {
        guard !feesEnded else { return }
        feesEnded = true
        fees.end(keepingView: true)
    }

    private var feesEnded = false

    /// `persist_record` wrote `recordId`: a hand-off naming it may go.
    private func recordLanded(_ recordId: String) {
        persistedRecords.insert(recordId)
        tryHandoff()
    }

    private func tryHandoff() {
        guard let handoff = pendingHandoff,
              handoff.recordIds.allSatisfy({ persistedRecords.contains($0) })
        else { return }
        pendingHandoff = nil
        ports.trackSubmitted(TrackSubmission(
            userOpHash: handoff.userOpHash, recordIds: handoff.recordIds, chainId: handoff.chainId,
            maybeSent: handoff.maybeSent, submitBlock: handoff.submitBlock,
            admitted: handoff.admitted, sender: handoff.sender
        ))
    }

    /// A handoff's identity (spec 082 RJ1, the core's contract §4): its op,
    /// its records, and whether it may have been sent or was admitted — so
    /// the write-ahead hand-off and the relay's acceptance of the SAME op are
    /// two hand-offs, each fed once.
    static func handoffKey(_ handoff: SignTrackerHandoffWire) -> String {
        [
            handoff.userOpHash.lowercased(),
            handoff.recordIds.sorted().joined(separator: ","),
            handoff.maybeSent ? "maybe_sent" : "-",
            handoff.admitted ? "admitted" : "-",
        ].joined(separator: "|")
    }

    /// A withdrawal's identity: its op and its records.
    static func withdrawKey(_ withdraw: SignTrackerWithdrawWire) -> String {
        withdraw.userOpHash.lowercased() + "|" + withdraw.recordIds.sorted().joined(separator: ",")
    }

    // MARK: - The answer follows the tracker (spec 082 RJ4)

    /// The tracker's view changed: its entry for this request's op goes to
    /// the core as `op_tracked` — which answers the page at once from a
    /// terminal verdict (the tx hash, or a refusal) instead of waiting out
    /// the receipt window. `RootView` feeds every view while the request lives.
    func trackerChanged(_ view: TrackViewWire) {
        trackerView = view
        forwardTracked()
    }

    /// The tracker's `in_flight_ops` (PR 2 §3), as the core computed it from
    /// its own view: the sheet's confirm is held — `previous_pending`, one
    /// line ahead of every fee block — while this account's previous op on
    /// the request's chain is in flight. Signatures never wait. Forwarded on
    /// every render; the machine dedupes an identical list.
    func inFlightChanged(_ opsJson: String) {
        guard let ops = (try? JSONSerialization.jsonObject(with: Data(opsJson.utf8))) as? [Any] else { return }
        dispatchSign(["type": "in_flight_ops", "ops": ops])
    }

    /// Only past `op_submitted` (the core takes it no earlier) and only while
    /// the page has no answer; once per entry state.
    private func forwardTracked() {
        guard !answered, let op = submittedHash,
              let entry = trackerView?.entry(userOpHash: op)
        else { return }
        let state = "\(entry.userOpHash.lowercased())|\(entry.status)|\(entry.txHash ?? "")|\(entry.refusal ?? "")"
        guard state != lastTracked else { return }
        lastTracked = state
        dispatchSign(Self.opTracked(entry, nowMs: Date().timeIntervalSince1970 * 1000))
    }

    /// The tracker entry as `op_tracked`: its status, the bundle it names,
    /// and — for a refusal — WHY, the entry's own `refusal` (PR 2 note 9).
    /// The sheet's failure then says the sentence the entry's `refusal_key`
    /// says (`SignView.failure_refusal_key`): one source, the core's
    /// `refusal_key(reason)`, for the sheet and its ending alike.
    static func opTracked(_ entry: TrackEntryWire, nowMs: Double) -> [String: Any] {
        var event: [String: Any] = [
            "type": "op_tracked",
            "user_op_hash": entry.userOpHash,
            "status": entry.status,
            "tx_hash": entry.txHash.map { $0 as Any } ?? NSNull(),
            "now_ms": nowMs,
        ]
        if let refusal = entry.refusal { event["refusal"] = refusal }
        return event
    }

    private func markAnswered() {
        answered = true
        endFees()
        if sign.surface == .hidden { closed = true }
    }

    // MARK: - The pure parts

    /// The chain's usual time to include an operation, in seconds — the core's
    /// table, the same number the send receipt counts against (spec 079);
    /// `nil` for a network Vela does not ship.
    static func typicalInclusionS(chainId: Int) -> Int? {
        networkTypicalInclusionS(chainId: UInt32(clamping: chainId)).map { Int($0) }
    }

    /// The user-operation hash when it is what the page is being answered
    /// with — `receipt_pending` answers `ok` with the op hash; a landed one
    /// answers with the TRANSACTION hash and needs no translation. A batch
    /// answered `{ id }` (EIP-5792 2.0.0, spec 097 G) names none here: the
    /// browser core knows a batch by its method, with no op named.
    static func opHashAnswer(_ payload: [String: Any], submitted: String?) -> String? {
        guard payload["type"] as? String == "ok",
              let result = payload["result"] as? String,
              let submitted, !submitted.isEmpty,
              result.caseInsensitiveCompare(submitted) == .orderedSame
        else { return nil }
        return submitted
    }

    /// What the confirm signs: the fee as quoted, the guard's rewrite,
    /// the intent.
    ///
    /// `params_override_json` is the load-bearing one. When the guard rewrote
    /// an approval, **these** params are what gets signed, submitted and
    /// recorded — never the original request.
    ///
    /// Spec 093: the verb the record keeps is the core's `record_intent`, the
    /// approval surface's token is copied as `token_meta`, and the sheet's
    /// judged balance changes ride as `balance_changes` — each copied, none
    /// decided here.
    static func approveOpts(
        fee: FeeViewWire?, clear: ClearSigningViewWire, guard guardView: GuardViewWire,
        balanceChanges: [TrustSimJudgmentWire]? = nil
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
            "intent": clear.recordIntent as Any? ?? NSNull(),
            // The guard showed an unbounded amount and it was kept as the site
            // asked — the submit guard's only waiver, copied, never decided.
            "unlimited_approved": guardView.unlimitedConsented,
            "token_meta": guardView.meta.wire,
            "balance_changes": balanceChanges.map { $0.map(\.wire) as Any } ?? NSNull(),
            // Spec 097: what the reading named, copied verbatim.
            "reading": clear.recordReading?.wire as Any? ?? NSNull(),
        ]
    }

    /// The fee the sheet displayed, as signed: amount in the paying coin's
    /// base units, and where it goes. No recipient → no quoted fee (the send
    /// core's `submit_user_op` rule).
    static func quotedFee(_ estimate: FeeEstimateWire?) -> [String: Any]? {
        guard let estimate, let recipient = estimate.feeRecipient else { return nil }
        let amount: String
        switch estimate.feeAsset {
        case .erc20(_, _, let erc20Amount, _): amount = erc20Amount
        case .native: amount = estimate.totalWei
        }
        // …with the speed this very estimate was priced at, named on the wire
        // beside the amount (spec 069); the core drops a `rapid`.
        return ["amount": amount, "recipient": recipient, "tier": estimate.tier]
    }

    /// The first call of a request: `to`, `data`, `value`. Given the
    /// `method`, chosen BY it, the way the submit path chooses what it sends:
    /// an `eth_sendTransaction` is its own top-level call — never a stray
    /// `calls` key beside it, which once had a harmless leg described while a
    /// malicious call was signed (the desktop's 083 review).
    static func firstCall(
        paramsJson: String, method: String? = nil
    ) -> (to: String?, data: String?, value: String?)? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              let first = params.first as? [String: Any]
        else { return nil }
        let call = method == "eth_sendTransaction"
            ? first
            : ((first["calls"] as? [[String: Any]])?.first) ?? first
        func field(_ name: String) -> String? {
            (call[name] as? String).flatMap { $0.isEmpty ? nil : $0 }
        }
        return (to: field("to"), data: field("data"), value: valueText(call["value"]))
    }

    /// The first call's `value` as the core must see it (spec 082 RC6): a
    /// string as written, a JSON number AS TEXT — which the core refuses to
    /// print (RC4), so the sheet shows the blind rung rather than a calm
    /// "0" — and absent or JSON null as nothing (= 0).
    static func valueText(_ raw: Any?) -> String? {
        switch raw {
        case let text as String: return text.isEmpty ? nil : text
        case let number as NSNumber: return number.stringValue
        default: return nil
        }
    }

    static func clearKickoff(
        method: String, paramsJson: String, chainId: Int, origin: String?
    ) -> [String: Any]? {
        // 089 S1: a batch goes over whole — the core reads EVERY call, so the
        // sheet can never describe call 1 while signing them all.
        if method == "wallet_sendCalls" {
            return [
                "type": "resolve_batch",
                "params_json": paramsJson,
                "chain_id": chainId,
                "locale": defaultLocale,
            ]
        }
        if method == "eth_sendTransaction" {
            let call = firstCall(paramsJson: paramsJson, method: method)
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
                "typed_data_json": typedDataOf(method: method, paramsJson: paramsJson),
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

    /// The ONE document the request is read as — the core's
    /// `typed_data_document`, the same bytes the passkey's digest covers. The
    /// audit of 2026-10-01: reading `params[1]` here previewed the benign half
    /// of a legacy `[malicious, benign]` while the passkey signed the
    /// malicious one. Empty when the request is not a valid typed-data
    /// request — the core refuses it before a sheet.
    static func typedDataOf(method: String, paramsJson: String) -> String {
        typedDataDocument(method: method, paramsJson: paramsJson) ?? ""
    }

    static func stringParams(paramsJson: String) -> [String] {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else { return [] }
        return params.compactMap { $0 as? String }
    }
}
