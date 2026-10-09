//
//  CorrectnessGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for PR 2 (the correctness batch), reached with
//  `VELA_PAGE=pr2` and `VELA_STATE=<board>`. Every board is drawn by the
//  PRODUCTION builders (`SigningLive.model`, `SendLive.confirm`/`receipt`)
//  over views in the cores' own JSON, and the signing footer is the core's
//  gate (`signConfirmState`) over those views:
//
//  - `held-sheet` — a site's transaction while this account's previous one
//    on the network is in flight: the confirm held with the one line
//    (`previous_pending`), ahead of the settled fee;
//  - `held-send` — the send confirm held the same way, the line under the CTA;
//  - `fee-provisional` / `fee-settled` — the fee switched to another coin and
//    measured again (the confirm held, the measuring sign on), then settled;
//  - `fee-chain-down` / `fee-internal` — the fee's own failure, the row and
//    the footer naming one cause: the chain out of reach, or Vela's own fault
//    (issue #483) — never "Can't reach <chain>" for the latter;
//  - `refused-sheet` / `refused-send` — the relay refused it, told by its
//    reason ("another transaction from this account went first").
//
//  The integration's core round (PR 2 notes 1/10, 9, 11, 13) — every fee view
//  here written by the real `fee_policy` core (`FeeCoreScene`), the home's
//  by the real `balance_dashboard` core (`BalanceCoreScene`):
//
//  - `fee-chain-down` / `fee-internal` / `fee-retrying` / `fee-tap` — the
//    sheet's fee row and the line under the held confirm saying ONE thing:
//    the core retrying by itself (reason, the dash, "Retrying…", never a
//    tap), its re-ask out (the reason kept, the measuring sign on), and a
//    failure only a tap retries ("Tap to retry" / "Tap it to retry");
//  - `send-fee-chain-down` / `send-fee-internal` / `send-fee-retrying` — the
//    Send form's row, the same state; `send-confirm-retrying` /
//    `send-confirm-tap` — the confirm's fee row and its footer line;
//  - `alert-chain-down` / `alert-internal` — Continue's estimate failed,
//    worded by its cause;
//  - `refused-held` / `refused-fee` — the sheet's failure says why the relay
//    did not take it (`failure_refusal_key`): at submit, the previous
//    transaction holding the nonce (Try again stays); after it, the fee;
//  - `home-internal` / `home-chain-down` — the home when Ethereum's read
//    failed inside Vela (never "Can't reach Ethereum"), and when Ethereum
//    really was out of reach; `home-internal-all` — every chain's read failed
//    inside Vela with nothing cached: the skeleton and Vela's own reason,
//    never a settled $0.00 or "Deposit your first asset".
//
//  Fixture data only: nothing here is quoted, signed or sent.
//

#if DEBUG

import SwiftUI
import VelaCore

struct CorrectnessGalleryScreen: View {
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "held-send":
            FlowHost(model: heldSendModel, sendCtaDisabled: true)
        case "refused-send":
            FlowHost(model: refusedSendModel)
        case "send-fee-chain-down", "send-fee-internal", "send-fee-retrying":
            // Continue is the core's `can_continue`, which no fee holds: the
            // estimate behind it is what says the cause (`alert-*`).
            FlowHost(model: sendFormModel, onRefreshFee: {})
        case "send-confirm-retrying", "send-confirm-tap":
            FlowHost(model: sendConfirmModel, sendCtaDisabled: true, onRefreshFee: {})
        case "alert-chain-down", "alert-internal":
            FlowHost(model: sendFormModel, alert: estimateAlert, onRefreshFee: {})
        case "home-internal", "home-chain-down", "home-internal-all":
            WalletScreen(model: homeModel, loc: loc)
        case "refused-held", "refused-fee":
            // The ending's own buttons: Done, and Try again where the core
            // says a retry can help (the held nonce).
            SigningSheet(model: signingModel, onClose: {}, onRefreshFee: {}, onRetry: {})
        default:
            SigningSheet(model: signingModel, onRefreshFee: {})
        }
    }

    // MARK: - The core round's fee scenes

    /// Which of the fee machine's views this board draws — `nil` for a board
    /// that is not about a failed fee.
    private var feeScene: (scene: FeeCoreScene, retrying: Bool)? {
        switch state {
        case "fee-chain-down", "send-fee-chain-down", "alert-chain-down": (.chainDown, false)
        case "fee-internal", "send-fee-internal", "alert-internal": (.internalFault, false)
        case "fee-retrying", "send-fee-retrying", "send-confirm-retrying": (.chainDown, true)
        case "fee-tap", "send-confirm-tap": (.missingKey, false)
        default: nil
        }
    }

    /// The fee view the core wrote for this board's scene.
    private var sceneJson: String? {
        guard let (scene, retrying) = feeScene, let views = scene.views() else { return nil }
        return retrying ? (views.retrying ?? views.failed) : views.failed
    }

    private static let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let recipient = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private static let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
    private static let op = "0x" + String(repeating: "c3", count: 32)

    // MARK: - The signing sheet

    private var request: SigningController.Incoming {
        let tx: [String: Any] = [
            "from": Self.address, "to": Self.recipient, "value": "0x38d7ea4c68000", "data": "0x",
        ]
        let params = String(decoding: (try? JSONSerialization.data(withJSONObject: [tx])) ?? Data(), as: UTF8.self)
        return SigningController.Incoming(
            id: "pr2", method: "eth_sendTransaction", paramsJson: params,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 8453
        )
    }

    /// The fee view the board is about, as the fee machine writes it.
    private var feeJson: String {
        let usdc = CoreJSON.string([
            "type": "erc20", "token": Self.usdc, "decimals": 6, "amount": "21000", "symbol": "USDC",
        ])
        let estimate = """
        {"chain_id":8453,"total_wei":"21000000000000","max_fee_per_gas":"1000000",\
        "network_fee_per_gas":"1000000","relayer_fee_per_gas":"0","bundler_gas_price":"1000000",\
        "in_band_gas_basis":"0","effective_gas_price":null,"max_gas_price":null,"total_gas":"21000",\
        "deployed":true,"tier":"standard","quoted":true,"fee_asset":\(usdc),"fee_recipient":null}
        """
        func view(busy: Bool, failed: String, fee: String, ready: Bool, provisional: Bool = false) -> String {
            """
            {"busy":\(busy),"failed":\(failed),"fee":\(fee),"stale":false,\
            "fee_token":"\(Self.usdc)","options":[],\
            "confirm_fee_ready":\(ready),"provisional":\(provisional)}
            """
        }
        if let sceneJson { return sceneJson }
        switch state {
        case "fee-provisional":
            return view(busy: true, failed: "null", fee: estimate, ready: false, provisional: true)
        default:
            return view(busy: false, failed: "null", fee: estimate, ready: true)
        }
    }

    private var fee: FeeViewWire? {
        try? CoreJSON.decoder.decode(FeeViewWire.self, from: Data(feeJson.utf8))
    }

    /// The sign machine's view with the request on the sheet: the gate open,
    /// or — held — shut on the previous transaction, as the core shuts it.
    private var signJson: String {
        var object = (try? CoreJSON.object(SignRequestCore().view())) ?? [:]
        object["surface"] = "sheet"
        object["confirm_gate_open"] = state != "held-sheet"
        object["confirm_block"] = state == "held-sheet" ? "previous_pending" : NSNull()
        return CoreJSON.string(object)
    }

    /// The other two machines' views, settled for a plain transfer.
    private var clearJson: String {
        var object = (try? CoreJSON.object(ClearSigningCore().view())) ?? [:]
        object["resolving"] = false
        object["resolved"] = true
        object["surface"] = "clear_sign"
        return CoreJSON.string(object)
    }

    private var guardJson: String {
        var object = (try? CoreJSON.object(ApprovalGuardCore().view())) ?? [:]
        object["confirm_allowed"] = true
        return CoreJSON.string(object)
    }

    /// The footer: the CORE's gate over the four views.
    private var gate: SignConfirmStateWire {
        SignConfirmStateWire.of(
            sign: signJson, guard: guardJson, clear: clearJson, fee: feeJson, speedTier: "standard"
        )
    }

    private var signingModel: SigningModel {
        let refused = state == "refused-sheet"
        // PR 2 note 9: the relay did not take it, and the sheet says why —
        // the core's `failure_refusal_key`, as `sign_request` sets it.
        let refusedWhy = state == "refused-held" || state == "refused-fee"
        var sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: refused ? Self.op : nil,
            error: refusedWhy ? SignErrorNoticeWire(kind: .submitFailed, detail: nil) : nil,
            funding: nil, confirmGateOpen: state != "held-sheet",
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
        switch state {
        case "refused-held":
            // At submit: the account's previous operation holds the nonce —
            // nothing was sent, and Try again stays (it is retryable).
            sign.failureRefused = true
            sign.failureRetryable = true
            sign.failureRefusalKey = "componentsUi.signing.confirmBlock.previousPending"
        case "refused-fee":
            // After it: the tracker's verdict, `fee_below_market`.
            sign.failureRefused = true
            sign.failureRefusalKey = "send.txRejectedFees"
        default:
            break
        }
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: SettingsLive.chainColor(8453), nativeSymbol: "ETH",
            walletName: "Everyday wallet", walletAddress: Self.address
        )
        if refused {
            // The tracker's verdict: the relay refused it, and why.
            var entry = TrackEntryWire(
                userOpHash: Self.op, chainId: 8453, recordIds: ["r"], status: "rejected",
                txHash: nil, polling: false, submittedAtMs: 1, outcome: "final"
            )
            entry.refusal = "nonce_used"
            entry.refusalKey = "componentsUi.signing.wentFirst"
            context.track = entry
        }
        let fee = self.fee
        let speed = HandoffFeeFixtures.speedView
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: .empty, guard: .empty, fee: fee, context: context,
            speed: speed.map { SendLive.SpeedInputs(view: $0, feeView: { _ in fee }) },
            gate: gate
        )
    }

    // MARK: - Send

    private static let usdcOnBase: [String: Any] = [
        "network": "base", "chain_id": 8453, "symbol": "USDC", "balance": "250",
        "decimals": 6, "token_address": usdc, "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    /// A Send to `recipient` of 25 USDC on Base, at `stage`; the fee machine
    /// is re-asking while `feeBusy` (the send machine's mirror of it).
    private func baseSendView(stage: String) -> SendViewWire? {
        sendView([
            "stage": stage, "selected_token": Self.usdcOnBase, "recipient": Self.recipient,
            "amount": "25", "token_amount": "25", "confirm_amount": "25",
            "fee_busy": fee?.busy ?? false,
        ])
    }

    /// The speed control's view, settled at the stored default: what makes
    /// the form draw its refresh and the reason line under the row.
    private var speedInputs: SendLive.SpeedInputs? {
        let fee = self.fee
        return HandoffFeeFixtures.speedView.map { SendLive.SpeedInputs(view: $0, feeView: { _ in fee }) }
    }

    /// The form's fee row over the fee machine's failure (PR 2 note 1).
    private var sendFormModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd2, loc: loc)
        if case .sendForm(let drawn) = model.base, let view = baseSendView(stage: "enter_details") {
            model.base = .sendForm(SendLive.form(
                view, fee: fee, display: .usd, on: drawn, loc: loc, speed: speedInputs
            ))
        }
        return model
    }

    /// The confirm's fee row and the one line under its held button.
    private var sendConfirmModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd3, loc: loc)
        if case .sendConfirm(let drawn) = model.base, let view = baseSendView(stage: "confirm") {
            model.base = .sendConfirm(SendLive.confirm(
                view, from: (Self.address, "Everyday wallet"), display: .usd, on: drawn, loc: loc,
                fee: fee, speed: HandoffFeeFixtures.speedView
            ))
        }
        return model
    }

    /// Continue's estimate failed, said by its cause (PR 2 note 13): the
    /// send machine's own `estimate_failed` alert, its `kind` the fee
    /// machine's failure passed through as it is.
    private var estimateAlert: FlowAlertModel? {
        guard let failed = fee?.failed else { return nil }
        let kind: [String: Any] = [
            "type": "estimate_failed", "kind": SendExecutor.estimateFailure(failed),
        ]
        let text = SendLive.alertText(kind, loc: loc, chain: "Base")
        return FlowAlertModel(title: text.title, message: text.body, dismiss: loc.t("common.gotIt"))
    }

    // MARK: - The home (PR 2 note 11)

    private var homeModel: WalletHomeModel {
        var model = WalletFixtures.buildMobileState(.h1, loc: loc)
        if let view = BalanceCoreScene.view(
            internalFault: state != "home-chain-down", everyChain: state == "home-internal-all"
        ) {
            model.balance = WalletLive.balance(view, fallback: model.balance, loc: loc)
            model.assetRows = WalletLive.assetRows(view)
            model.assetsSection = WalletLive.assetsSection(
                view, rows: model.assetRows, fallback: model.assetsSection
            )
        }
        return model
    }

    private func sendView(_ patch: [String: Any]) -> SendViewWire? {
        var object = (try? CoreJSON.object(SendCore().view())) ?? [:]
        for (key, value) in patch { object[key] = value }
        return try? CoreJSON.decode(SendViewWire.self, from: object)
    }

    private static let xdai: [String: Any] = [
        "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
        "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    private var heldSendModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd3, loc: loc)
        if case .sendConfirm(var confirm) = model.base,
           let view = sendView([
               "stage": "confirm",
               "previous_pending": [
                   "chain_id": 100, "user_op_hash": Self.op,
                   "key": "componentsUi.signing.confirmBlock.previousPending",
               ],
           ]) {
            // The production builder's line, from the core's key.
            confirm.heldNote = view.previousPending.map { loc.t($0.key) }
            model.base = .sendConfirm(confirm)
        }
        return model
    }

    private var refusedSendModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd4b, loc: loc)
        if case .sendReceipt(let drawn) = model.base,
           let view = sendView([
               "stage": "receipt", "user_op_hash": Self.op, "tx_status": "submitted",
               "recipient": Self.recipient,
               "selected_token": Self.xdai,
               "receipt": [
                   "status": "failed", "hold_reason": NSNull(), "kind": NSNull(), "transfers": [],
                   "amount": "0.001", "usd_value": 0.001, "typical_inclusion_s": 5,
                   "submitted_at_ms": NSNull(), "refusal_key": "componentsUi.signing.wentFirst",
               ] as [String: Any],
           ]) {
            model.base = .sendReceipt(SendLive.receipt(view, display: .usd, on: drawn, loc: loc))
        }
        return model
    }
}

#endif
