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
        default:
            SigningSheet(model: signingModel, onRefreshFee: {})
        }
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
        switch state {
        case "fee-provisional":
            return view(busy: true, failed: "null", fee: estimate, ready: false, provisional: true)
        case "fee-chain-down":
            return view(busy: false, failed: #"{"chain_read":{"rate_limited":false}}"#, fee: "null", ready: false)
        case "fee-internal":
            return view(busy: false, failed: #""internal""#, fee: "null", ready: false)
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
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: refused ? Self.op : nil,
            error: nil, funding: nil, confirmGateOpen: state != "held-sheet",
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
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
