//
//  SendStore.swift
//  VelaWallet
//
//  The resident `send` machine.
//
//  Resident rather than per-screen for the reason every machine here is: the
//  core's model IS the app's state. A send that died with its screen would lose
//  a submitted operation the moment somebody swiped back, and money in flight
//  is the one thing that must outlive a view.
//

import Foundation
import Observation
import VelaCore

extension SendCore: CoreBridge {}

@MainActor
@Observable
final class SendStore {

    private(set) var view: SendViewWire?
    /// The alert the core raised, waiting to be shown. The screen clears it.
    var alert: [String: Any]?
    /// How the last Trusted Signer ceremony ended without a signature (spec
    /// 071), for the confirmation's notice. The core only heard "cancelled";
    /// the sentence is this screen's, until the next attempt or leaving.
    private(set) var trustedSignerNotice: TrustedSignerNotice?

    private var core: CoreStore<SendViewWire>!
    private let executor: SendExecutor

    /// No effect in flight: nothing but a new event can change `view` —
    /// `CoreDriver.isIdle`. What a test waits on instead of a clock.
    var isIdle: Bool { core.isIdle }
    /// Whether `Open` has been sent for the journey currently on screen.
    private var entered = false
    /// How many journeys the CORE has closed — its `back` from the picker, or
    /// `done`. The flow host watches it and takes the send screens away, so
    /// the machine's close and the screen's close are one event (087 F27):
    /// Android's `sendClosed`, counted so that every close is a change.
    private(set) var closes = 0
    /// Whose money the journey on screen is about (lower-cased) — the only
    /// account whose asset-list rounds it follows.
    private var account: String?
    /// The asset-list round this journey last heard of, so one round is
    /// handed over once however often the screen re-renders.
    private var heardRound: Int?

    init(executor: SendExecutor) {
        self.executor = executor
        self.core = CoreStore(
            bridge: SendCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { VelaLog.failure(.sign, kind: "send_fault", VelaLog.error($0)) }
        )
        // Two ports close over this store, so they are installed after it
        // exists rather than passed into the executor's initialiser.
        //
        // `signingStarted` is a CHECKPOINT, not a notification: the core marks
        // the attempt as signing and a `cancel_signing` after it ends that
        // attempt. Without it, a cancel has nothing to cancel and the next tap
        // raises a second prompt — the defect FR-009 counts.
        executor.ports.signingStarted = { [weak self] in
            self?.trustedSignerNotice = nil
            self?.dispatch(["type": "signing_started"])
        }
        // The op is signed and nothing has left (spec 082 RJ1): the core
        // writes every recipient's record ahead, hands the op to the tracker
        // and only then clears the POST.
        executor.ports.opSigned = { [weak self] hash, block in
            self?.dispatch([
                "type": "op_signed", "user_op_hash": hash,
                "submit_block": block.map { $0 as Any } ?? NSNull(),
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])
        }
        executor.ports.alert = { [weak self] kind in self?.alert = kind }
        executor.ports.trustedSignerEnded = { [weak self] notice in self?.trustedSignerNotice = notice }
        // What the asset list has so far, while the first fetch waits for its
        // round. Display-only: the core takes it only while that load is out.
        executor.ports.tokensPartial = { [weak self] tokens in
            self?.dispatch(["type": "tokens_partial", "tokens": tokens])
        }
        // Leaving re-arms `Open`, and it is the CORE's leaving that counts —
        // not a view disappearing. SwiftUI tears a view down and rebuilds it
        // for reasons that have nothing to do with the journey, and an
        // `onDisappear` here re-armed the door mid-flow: the next rebuild sent
        // `Open` again and the form bounced back to the picker.
        let leaving = executor.ports.closed
        executor.ports.closed = { [weak self] in
            leaving()
            self?.entered = false
            self?.account = nil
            self?.heardRound = nil
            self?.trustedSignerNotice = nil
            self?.closes += 1
        }
    }

    /// The person left the journey by a door the core never heard about — a
    /// 转账 from somewhere else, a pay link, a contact's 转账 — so the next
    /// `open` starts a NEW journey instead of resuming this one (087 F27).
    ///
    /// Only the core's own close re-armed `open` (`ports.closed` above), and
    /// the guard in `open` then dropped every later 转账 while an abandoned
    /// journey was alive: a new send came up on the last code's recipient and
    /// its network scope. Every entry into Send calls this first.
    ///
    /// The machine forgets the journey NOW, with an `Open` for nobody — the
    /// real `Open` waits on the account store, and the screen entering in the
    /// meantime must not draw the journey just left. Dropped before the
    /// machine's first boot, when there is nothing to forget.
    func leave() {
        core.dispatch(CoreJSON.string([
            "type": "open",
            "account": NSNull(),
            "params": Self.noParams,
            "display": Self.display(code: "USD", rate: nil, decimals: 2),
        ]))
        entered = false
        account = nil
        heardRound = nil
        trustedSignerNotice = nil
        alert = nil
    }

    /// An open with nothing handed in. A pay link or a scanned code arrives
    /// afterwards, through `scanned`, exactly as a code on the form does.
    private static let noParams: [String: Any] = [
        "preselected_symbol": NSNull(), "preselected_network": NSNull(),
        "prefilled_recipient": NSNull(), "prefilled_chain_id": NSNull(),
        "prefilled_token_address": NSNull(), "prefilled_amount_base": NSNull(),
        "locked": false, "preselected_multi": NSNull(),
    ]

    /// Open the flow for an account. Idempotent — a second open re-reads the
    /// holdings rather than starting a new machine.
    ///
    /// `params` is how a locked request arrives (a pay-link, a scanned code).
    /// Empty here: 052 opens the flow from the home's 转账 button and nothing
    /// else. The pay-link is 056 and the scanner is 055.
    ///
    /// **A second call is IGNORED until the flow is left** — closed by the
    /// core, or left through `leave()`. `Open` means
    /// "enter the flow" and resets the machine to the picker, so a caller that
    /// fires it again mid-journey throws the person's work away. SwiftUI will
    /// re-run a `.task` whenever the view it is attached to is rebuilt, which
    /// is often — a device run watched the form appear and bounce straight back
    /// to the picker, twice, before this guard existed. Idempotence belongs
    /// here rather than in each call site, because every call site is one
    /// rebuild away from being wrong.
    func open(
        accountId: String,
        address: String,
        name: String?,
        displayCode: String,
        displayRate: Double?,
        fiatDecimals: Int
    ) {
        let event = CoreJSON.string([
            "type": "open",
            "account": [
                "id": accountId, "address": address,
                "name": name.map { $0 as Any } ?? NSNull(),
            ] as [String: Any],
            "params": Self.noParams,
            "display": Self.display(code: displayCode, rate: displayRate, decimals: fiatDecimals),
        ])
        guard !entered else { return }
        entered = true
        account = address.lowercased()
        heardRound = nil
        if !core.boot(event) { core.dispatch(event) }
    }

    /// The asset list settled a round while this journey is open (spec 078).
    ///
    /// Send shows the SAME holdings the home does — one source, so a refresh
    /// started from here, a poll, or a transfer confirming moves this screen
    /// too. Handed over whole, mapped exactly as `fetch_tokens` is answered;
    /// what follows is the core's (`holdings_updated`): the picker always, the
    /// form's selected balance and a Max behind it on the form, never a
    /// confirm page. Another account's round, or a round already heard, is
    /// not handed over.
    func holdingsUpdated(_ balance: BalanceViewWire, round: Int) {
        guard entered, let account, SendExecutor.sameAccount(balance, account),
              heardRound != round
        else { return }
        heardRound = round
        dispatch(["type": "holdings_updated", "tokens": SendExecutor.sendTokens(balance)])
    }

    private static func display(code: String, rate: Double?, decimals: Int) -> [String: Any] {
        [
            "code": code,
            "rate": rate.map { $0 as Any } ?? NSNull(),
            "fiat_decimals": decimals,
        ]
    }

    func dispatch(_ event: [String: Any]) { core.dispatch(CoreJSON.string(event)) }

    // MARK: - What the drawn screens send

    func selectToken(id: String) { dispatch(["type": "select_token", "token_id": id]) }
    func setRecipient(_ value: String) { dispatch(["type": "set_recipient", "recipient": value]) }
    func setAmount(_ value: String) { dispatch(["type": "set_amount", "amount": value]) }
    func tapMax() { dispatch(["type": "tap_max"]) }
    func toggleFiatInput() { dispatch(["type": "toggle_fiat_input"]) }
    func openContactPicker(target: String?) {
        dispatch(["type": "open_contact_picker", "target": target.map { $0 as Any } ?? NSNull()])
    }
    func closeContactPicker() { dispatch(["type": "close_contact_picker"]) }
    func pickedAddress(_ address: String) {
        dispatch(["type": "picked_address", "address": address])
    }
    // MARK: - Split (spec 054)

    /// Turn a single send into a list of them.
    func enterSplitMode() { dispatch(["type": "enter_split_mode"]) }

    /// The **whole list**, every time. The core reconciles it — ids, names and
    /// identities included — so a shell that sent a delta would be deciding
    /// which parts of its own state the core is allowed to trust.
    func recipientsChanged(_ rows: [SplitRows.Draft]) {
        dispatch(["type": "recipients_changed", "recipients": rows.map(\.json)])
    }

    /// Seed the list from somewhere else — a batch file, a whole group.
    func seedSplitRecipients(_ rows: [[String: Any]]) {
        dispatch(["type": "seed_split_recipients", "recipients": rows])
    }

    /// The same rows ADDED to whoever is already on the form. A list brought
    /// to a form that has people on it is, nearly always, more people — the
    /// seed above replaces them (web #265). Also shuts the picker and the
    /// importer, in the core.
    func appendSplitRecipients(_ rows: [[String: Any]]) {
        dispatch(["type": "append_split_recipients", "recipients": rows])
    }

    // MARK: - Sweep

    /// The events one tap on the picker becomes, in the order the core needs
    /// them: the pin before the tick, because the pin is what the tick is
    /// judged against.
    func sweepTap(_ events: [[String: Any]]) {
        for event in events { dispatch(event) }
    }

    func confirmMultiSelection() { dispatch(["type": "confirm_multi_selection"]) }

    // MARK: - Batch

    // MARK: - The scanner (spec 055)

    /// The viewfinder opens because the CORE says so — the flag is
    /// `show_scanner`, and a shell that pushed its own screen would show a
    /// scanner the machine does not know is open.
    func openScanner() { dispatch(["type": "open_scanner"]) }
    func closeScanner() { dispatch(["type": "close_scanner"]) }

    /// A decoded code. The shell tokenises (it owns the grammar — Hermes has no
    /// WebAssembly, so every client parses its own), and the CORE decides what
    /// the result means: which token, which chain, whether the send locks.
    func scanned(_ text: String) {
        dispatch(["type": "scan_resolved", "scan": Eip681.scan(of: text)])
    }

    func openBatchImport() { dispatch(["type": "open_batch_import"]) }
    func closeBatchImport() { dispatch(["type": "close_batch_import"]) }

    func advance() { dispatch(["type": "continue"]) }
    func back() { dispatch(["type": "back"]) }
    func editAmount() { dispatch(["type": "edit_amount"]) }
    func chooseFeeToken(_ token: String?) {
        dispatch(["type": "choose_fee_token", "token": token.map { $0 as Any } ?? NSNull()])
    }
    /// The fee session settled a quote. The two machines are bridged in the
    /// shell, and this is the half that tells `send` what `fee_policy` decided.
    ///
    /// `estimate` is NOT optional in the core's event: there is nothing to
    /// report until there is an estimate, and a busy or failed session says so
    /// through `feeBusyChanged` and the form's own warning instead.
    func feeUpdated(_ estimate: FeeEstimateWire) {
        dispatch(["type": "fee_updated", "estimate": estimate.coreJSON])
    }
    func feeBusyChanged(_ busy: Bool) {
        dispatch(["type": "fee_busy_changed", "busy": busy])
    }
    func slideConfirm() { dispatch(["type": "slide_confirm"]) }
    func cancelSigning() { dispatch(["type": "cancel_signing"]) }
    func dismissTreasurySheet() { dispatch(["type": "dismiss_treasury_sheet"]) }
    func retryAfterError() { dispatch(["type": "retry_after_error"]) }

    // MARK: - The TRACKER's verdict, back to the send machine (spec 056)
    //
    // The receipt screen and the tracker are two machines watching one
    // operation, and only the tracker polls. Without this the receipt sat on
    // "submitted" while the notification said "confirmed" — two answers about
    // the same money, on one phone. Since 082 the mapping from the tracker's
    // entry to the receipt's verdict is the core's alone (`trackerChanged`);
    // the two hand-built verdicts this file kept — one of them stamping every
    // failure "sent" — are gone with it.

    /// The last receipt verdict handed over, by op — so one verdict reaches
    /// the machine once however often the tracker's view is rebuilt.
    private var heardOutcome: [String: String] = [:]

    /// The tracker's last view, whatever op it was about.
    ///
    /// Since the write-ahead (spec 082 RJ1) the tracker has the op BEFORE its
    /// POST, so it can reach its verdict while the relay's reply is still out
    /// — the chain check finds the landed op while that reply is being lost.
    /// This journey learns which op is its own only when the reply (or its
    /// loss) comes back, and by then the tracker may never change again: a
    /// terminal entry stops its clock, and a may-have-been-sent verdict hands
    /// it nothing new. So the verdict it already has is read at that moment
    /// (`commit`), not waited for — or the receipt says "may have been sent"
    /// over money the tracker saw land (RJ4, G37 on the Send screen).
    private var lastTracker: TrackViewWire?

    /// The core's view. When it first names this journey's op, the tracker's
    /// verdict so far is handed over at once.
    private func commit(_ next: SendViewWire) {
        let before = view?.userOpHash
        view = next
        guard let op = next.userOpHash, !op.isEmpty,
              op.caseInsensitiveCompare(before ?? "") != .orderedSame,
              let lastTracker
        else { return }
        trackerChanged(lastTracker)
    }

    /// The TRACKER's view, as the receipt's verdict (spec 082): the core's
    /// one mapping (`sendReceiptOutcomeOf`) — confirmed, failed (a revert, a
    /// fee rejection, or a may-have-been-sent op the relay never had), held
    /// for fees, or acknowledged — for the op this journey submitted. Deduped
    /// on the verdict itself, not on the status alone: a fee hold and an
    /// acknowledgement are both "pending" to the tracker.
    func trackerChanged(_ view: TrackViewWire) {
        lastTracker = view
        guard let op = self.view?.userOpHash, !op.isEmpty,
              let entry = view.entry(userOpHash: op),
              let json = try? sendReceiptOutcomeOf(trackEntryJson: entry.coreJSON),
              heardOutcome[op.lowercased()] != json,
              let outcome = try? CoreJSON.object(json)
        else { return }
        heardOutcome[op.lowercased()] = json
        dispatch(["type": "receipt_update", "user_op_hash": op, "outcome": outcome])
    }
    func retryAfterBootstrap() { dispatch(["type": "retry_after_bootstrap"]) }
    func done() { dispatch(["type": "done"]) }
    func refreshTokens() { dispatch(["type": "refresh_tokens"]) }
    /// The display currency changed under a screen that already has a figure on
    /// it. The core re-denominates rather than relabelling.
    func displayChanged(code: String, rate: Double?, fiatDecimals: Int) {
        dispatch([
            "type": "display_changed",
            "display": Self.display(code: code, rate: rate, decimals: fiatDecimals),
        ])
    }
}
