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

    private var core: CoreStore<SendViewWire>!
    private let executor: SendExecutor
    /// Whether `Open` has been sent for the journey currently on screen.
    private var entered = false

    init(executor: SendExecutor) {
        self.executor = executor
        self.core = CoreStore(
            bridge: SendCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view },
            onFault: { print("[vela-wallet] send fault: \($0)") }
        )
        // Two ports close over this store, so they are installed after it
        // exists rather than passed into the executor's initialiser.
        //
        // `signingStarted` is a CHECKPOINT, not a notification: the core marks
        // the attempt as signing and a `cancel_signing` after it ends that
        // attempt. Without it, a cancel has nothing to cancel and the next tap
        // raises a second prompt — the defect FR-009 counts.
        executor.ports.signingStarted = { [weak self] in
            self?.dispatch(["type": "signing_started"])
        }
        executor.ports.alert = { [weak self] kind in self?.alert = kind }
        // Leaving re-arms `Open`, and it is the CORE's leaving that counts —
        // not a view disappearing. SwiftUI tears a view down and rebuilds it
        // for reasons that have nothing to do with the journey, and an
        // `onDisappear` here re-armed the door mid-flow: the next rebuild sent
        // `Open` again and the form bounced back to the picker.
        let leaving = executor.ports.closed
        executor.ports.closed = { [weak self] in
            leaving()
            self?.entered = false
        }
    }

    /// Open the flow for an account. Idempotent — a second open re-reads the
    /// holdings rather than starting a new machine.
    ///
    /// `params` is how a locked request arrives (a pay-link, a scanned code).
    /// Empty here: 052 opens the flow from the home's 转账 button and nothing
    /// else. The pay-link is 056 and the scanner is 055.
    ///
    /// **A second call is IGNORED until the flow is left.** `Open` means
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
            "params": [
                "preselected_symbol": NSNull(), "preselected_network": NSNull(),
                "prefilled_recipient": NSNull(), "prefilled_chain_id": NSNull(),
                "prefilled_token_address": NSNull(), "prefilled_amount_base": NSNull(),
                "locked": false, "preselected_multi": NSNull(),
            ] as [String: Any],
            "display": Self.display(code: displayCode, rate: displayRate, decimals: fiatDecimals),
        ])
        guard !entered else { return }
        entered = true
        if !core.boot(event) { core.dispatch(event) }
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
        dispatch(["type": "fee_updated", "estimate": Self.estimateEvent(estimate)])
    }
    func feeBusyChanged(_ busy: Bool) {
        dispatch(["type": "fee_busy_changed", "busy": busy])
    }
    func slideConfirm() { dispatch(["type": "slide_confirm"]) }
    func cancelSigning() { dispatch(["type": "cancel_signing"]) }
    func dismissTreasurySheet() { dispatch(["type": "dismiss_treasury_sheet"]) }
    func retryAfterError() { dispatch(["type": "retry_after_error"]) }

    /// The TRACKER's verdict, back to the send machine (spec 056).
    ///
    /// The receipt screen and the tracker are two machines watching one
    /// operation, and only the tracker polls. Without this the receipt sat on
    /// "submitted" while the notification said "confirmed" — two answers about
    /// the same money, on one phone.
    func receiptConfirmed(userOpHash: String, txHash: String) {
        dispatch([
            "type": "receipt_update",
            "user_op_hash": userOpHash,
            "outcome": ["type": "confirmed", "tx_hash": txHash],
        ])
    }

    func receiptFailed(userOpHash: String, rejected: Bool) {
        dispatch([
            "type": "receipt_update",
            "user_op_hash": userOpHash,
            "outcome": ["type": "failed", "rejected": rejected],
        ])
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

    /// `FeeEstimateView` as `send`'s `FeeUpdated` carries it.
    private static func estimateEvent(_ estimate: FeeEstimateWire) -> [String: Any] {
        var asset: [String: Any]
        switch estimate.feeAsset {
        case .native:
            asset = ["type": "native"]
        case .erc20(let token, let decimals, let amount, let symbol):
            asset = [
                "type": "erc20", "token": token, "decimals": decimals, "amount": amount,
                "symbol": symbol.map { $0 as Any } ?? NSNull(),
            ]
        }
        return [
            "chain_id": estimate.chainId,
            "total_wei": estimate.totalWei,
            "max_fee_per_gas": estimate.maxFeePerGas,
            "network_fee_per_gas": "0",
            "relayer_fee_per_gas": "0",
            "bundler_gas_price": "0",
            "in_band_gas_basis": "0",
            "total_gas": estimate.totalGas,
            "deployed": estimate.deployed,
            "tier": "fast",
            "quoted": estimate.quoted,
            "fee_asset": asset,
            "fee_recipient": estimate.feeRecipient.map { $0 as Any } ?? NSNull(),
        ]
    }
}
