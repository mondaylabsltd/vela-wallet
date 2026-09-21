//
//  FlowHost.swift
//  VelaWallet
//
//  The phone host: one flow state, rendered.
//
//  Takes a FlowScreenModel and draws its base screen plus, where the state
//  has one, the sheet over it. Every screen in the four journeys goes
//  through here, so the gallery and the real app render the same thing by
//  construction rather than by discipline — there is no second code path
//  for either to drift down.
//
//  The 1.35× text scale rides through `walletTextScale`, exactly as spec
//  015's H7x does, so one mechanism serves both features.
//

import AVFoundation
import SwiftUI

/// Where a screen can go next. Names match the web host's navigation intents.
enum FlowStep {
    case receiveQr
    /// R3 — the same sheet, for ONE asset: a token's own page asking to be
    /// paid in that token. The distinction is the contract line and the title,
    /// and it is the reason this is a separate step rather than a flag.
    case receiveQrAsset
    case txDetail
    case tokenDetail
    case addToken
    case sendForm
    case sendConfirm
    case sendReceipt
    case contactPick
    case feeToken
    case batchImport
    case sendMulti
    case addRecipient
    case scan
    case receive
    case chains
    case done
}

/// Everything the live scanner needs, as one value.
struct ScanInputs {
    var session: AVCaptureSession?
    var refusal: String?
    var refusalAction: (label: String, act: () -> Void)?
    var torchOn = false
    var onTool: (ScanTool) -> Void = { _ in }
}

struct FlowHost: View {
    @Environment(\.theme) private var theme

    let model: FlowScreenModel
    var onBack: () -> Void = {}
    var onNavigate: (FlowStep) -> Void = { _ in }
    /// The live add-token sheet's field and CTA. Absent everywhere the sheet is
    /// a fixture, which is the gallery and the screenshot sweep.
    var addTokenInput: Binding<String>?
    var onAddToken: (() -> Void)?
    var addTokenError: String?
    /// Which network row opened the code. The drawn sheet names a chain and
    /// draws its mark, and it can only name the RIGHT one if the row that was
    /// tapped travels with the navigation.
    var onReceiveNetwork: ((Int) -> Void)?
    /// Which history row opened the transaction sheet, as (group, row).
    var onSelectActivity: ((Int, Int) -> Void)?
    /// Which assets row opened the token sheet.
    var onSelectAsset: ((Int) -> Void)?
    /// 转账 from a token's own sheet, with that token preselected.
    var onSendToken: (() -> Void)?
    var onReceiveToken: (() -> Void)?
    /// 删除记录 on the open transaction (058). Absent in the gallery.
    var onDeleteTx: (() -> Void)?
    /// The network filter behind the header pill. Absent where the screen is a
    /// fixture, which is the gallery and the screenshot sweep.
    var chainSheet: ChainSheetModel?
    var onPickChain: ((Int?) -> Void)?
    /// 在区块浏览器中查看, on whichever sheet is open. Absent where there is
    /// nothing real to look up — the gallery, and a chain with no explorer.
    var onExplorer: (() -> Void)?
    /// 保存图片 on the receive sheet. Absent in the gallery, where there is no
    /// address to put on a card.
    var onSaveCard: (() -> Void)?
    /// What that action had to say. Presented **inside the sheet**, so saving
    /// a card does not close the code somebody was showing.
    var alert: FlowAlertModel?
    var onDismissAlert: (() -> Void)?
    /// The send journey's live half. Absent everywhere the flow is a picture,
    /// which is the gallery and the screenshot sweep.
    var sendAmount: Binding<String>?
    var sendRecipient: Binding<String>?
    /// One binding pair per split row, by the core's row id.
    var sendRow: ((String) -> (address: Binding<String>, amount: Binding<String>))?
    var sendWarning: String?
    var sendCtaDisabled = false
    /// Which row of the picker was tapped. The index travels because the core
    /// keys tokens by id and the screen only knows positions — the same defect
    /// the receive sheet and the network list each had: a tap that forgets what
    /// it tapped opens the first one.
    var onSelectToken: ((Int) -> Void)?
    /// Sweep: "select all valuable", scoped to the rows on screen.
    var onSelectAllTokens: (([Int]) -> Void)?
    /// 稳定币 / Gas / 其他 — the picker's class chips, drawn since 021 with
    /// nothing behind them.
    var onSendFilter: ((String) -> Void)?
    /// The picker's CTA. Entering the sweep pick from a single pick, or
    /// confirming a sweep once one is under way — the screen cannot tell those
    /// apart, and the shell's own picking flag can.
    var onPickCta: (() -> Void)?
    var onMax: (() -> Void)?
    /// Split: a row was removed, or a row was added.
    var onRemoveRecipient: ((Int) -> Void)?
    var onAddRecipient: (() -> Void)?
    /// The confirm page's CTA, and the receipt's exit. Absent where the flow is
    /// a picture, where the CTA still just navigates.
    var onConfirm: (() -> Void)?
    var onReceiptDone: (() -> Void)?
    /// The form's 继续. The CORE owns the step, so this dispatches rather than
    /// navigates — pushing a screen the machine has not moved to renders the
    /// one it is still on, which looks exactly like a dead button.
    var onContinueSend: (() -> Void)?
    /// The two sheets the send flow raises. Both bodies have always had an
    /// `onSelect`; the host simply never passed one, which is the same shape
    /// as the token picker before phase 3 — a list nobody can pick from.
    /// The confirm page's notice buttons: retry what stopped it, or 暂不.
    var onNoticeAction: (() -> Void)?
    var onNoticeSecondary: (() -> Void)?
    var onPickFeeToken: ((Int) -> Void)?
    var onPickContact: ((Int) -> Void)?
    /// A whole group from the picker, added to the split.
    var onPickGroup: ((Int) -> Void)?
    /// The batch importer's four live edges — the unit toggle, the file
    /// picker, the template and the apply — plus its two fields. Absent
    /// everywhere the sheet is a picture, which is the gallery and the
    /// screenshot sweep.
    var batchPaste: Binding<String>?
    var batchRate: Binding<String>?
    var onBatchUnit: ((BatchUnit) -> Void)?
    var onBatchFile: (() -> Void)?
    var onBatchTemplate: (() -> Void)?
    var onBatchResetRate: (() -> Void)?
    var onBatchApply: (() -> Void)?
    /// "Replace them instead" / "Add to them instead" under the importer:
    /// which way an import meets the rows already on the form.
    var onBatchMerge: (() -> Void)?

    /// The live viewfinder and what it has to say. Absent in the gallery, where
    /// the scanner is a picture of a frame.
    ///
    /// ONE value rather than five arguments, because this host's parameter list
    /// is the thing that has timed out Swift's type checker three times in this
    /// program — and five more expressions to solve is how it happens a fourth.
    var scan: ScanInputs?

    /// Whether the network picker is up. Local, because which sheet is showing
    /// is render-domain state no machine needs to know.
    @State private var pickingChain = false

    /// The sheet's own dismissal. A new state means a new sheet, so the flag
    /// is keyed on the model's state — closing one must not suppress the next.
    @State private var sheetDismissed: FlowStateId?

    private var sheetShown: Bool {
        model.sheet != nil && sheetDismissed != model.state
    }

    var body: some View {
        base
            .walletTextScale(model.textScale)
            // The refusal surface belongs to the SCREEN, not only to the sheet.
            //
            // Spec 051 put it inside `FlowSheetHost` because the thing that
            // needed it was saving a receive card, which is a sheet. The send
            // form is a base screen, so the core's refusals — an amount over
            // the balance, an address that is not one — were raised into a
            // surface that did not exist: 继续 was armed, pressing it said
            // nothing at all, and the device found it.
            //
            // Presented only when no sheet is up, so a refusal cannot appear
            // twice at once.
            .alert(
                alert?.title ?? "",
                isPresented: Binding(
                    get: { alert != nil && !sheetShown },
                    set: { if !$0 { onDismissAlert?() } }
                )
            ) {
                Button("OK") { onDismissAlert?() }
            } message: {
                Text(verbatim: alert?.message ?? "")
            }
            .sheet(isPresented: $pickingChain) {
                if let chainSheet {
                    ChainSelectSheet(model: chainSheet, onSelect: { chainId in
                        onPickChain?(chainId)
                        pickingChain = false
                    })
                    .walletTextScale(model.textScale)
                    .presentationDetents([.medium])
                    .presentationDragIndicator(.hidden)
                    .presentationCornerRadius(Tokens.Radius.r20)
                    .presentationBackground(theme.bgBase)
                }
            }
            .sheet(
                isPresented: Binding(
                    get: { sheetShown },
                    set: { if !$0 { sheetDismissed = model.state } }
                )
            ) {
                if let sheet = model.sheet {
                    FlowSheetHost(
                        sheet: sheet,
                        onNavigate: onNavigate,
                        onPickFeeToken: onPickFeeToken,
                        onPickContact: onPickContact,
                        onPickGroup: onPickGroup,
                        // The importer's fields and its four edges. Declaring
                        // them on both hosts and forwarding NEITHER is what put
                        // a read-only paste box on a live sheet — the drawn
                        // field renders when the binding is absent, so the
                        // omission looks exactly like the drawing.
                        batchPaste: batchPaste,
                        batchRate: batchRate,
                        onBatchUnit: onBatchUnit,
                        onBatchFile: onBatchFile,
                        onBatchTemplate: onBatchTemplate,
                        onBatchResetRate: onBatchResetRate,
                        onBatchApply: onBatchApply,
                        onBatchMerge: onBatchMerge,
                        addTokenInput: addTokenInput,
                        onAddToken: onAddToken,
                        addTokenError: addTokenError,
                        onExplorer: onExplorer,
                        onSaveCard: onSaveCard,
                        onSendToken: onSendToken,
                        onReceiveToken: onReceiveToken,
                        onDeleteTx: onDeleteTx,
                        alert: alert,
                        onDismissAlert: onDismissAlert
                    )
                        .walletTextScale(model.textScale)
                        .presentationDragIndicator(.hidden)
                        .presentationCornerRadius(Tokens.Radius.r20)
                        .presentationBackground(theme.bgBase)
                }
            }
    }

    /// The pill's tap. With nothing wired it still reports the intent, so a
    /// screen that has no picker yet behaves exactly as it did.
    private func openChainPicker() {
        if chainSheet != nil { pickingChain = true } else { onNavigate(.chains) }
    }

    @ViewBuilder private var base: some View {
        switch model.base {
        case .scan(let m):
            ScanSurfaceView(
                model: m,
                onClose: onBack,
                onTool: { tool in scan?.onTool(tool) },
                session: scan?.session,
                refusalText: scan?.refusal,
                refusalAction: scan?.refusalAction,
                torchOn: scan?.torchOn ?? false
            )
        case .share(let m):
            // Not a screen: the saved image, shown on its own so the gallery
            // and the save path render the very same artwork.
            ScrollView {
                ShareCardArtwork(model: m).padding(.vertical, Tokens.Space.s24)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(theme.bgSunken.ignoresSafeArea())
        case .receive(let m):
            FlowScaffold(header: m.header, onBack: onBack) {
                ReceiveListBody(model: m, onQr: { index in
                    onReceiveNetwork?(index)
                    onNavigate(.receiveQr)
                })
            }
        case .history(let m):
            FlowScaffold(header: m.header, onBack: onBack, onPill: { openChainPicker() }) {
                HistoryBody(model: m, onSelect: { group, row in
                    onSelectActivity?(group, row)
                    onNavigate(.txDetail)
                })
            }
        case .assets(let m):
            FlowScaffold(
                header: m.header,
                onBack: onBack,
                onAction: { onNavigate(.addToken) },
                onPill: { openChainPicker() }
            ) {
                AssetsBody(
                    model: m,
                    onSelect: { index in
                        onSelectAsset?(index)
                        onNavigate(.tokenDetail)
                    },
                    onAdd: { onNavigate(.addToken) },
                    onReceive: { onNavigate(.receive) }
                )
            }
        case .sendPick(let m):
            FlowScaffold(header: m.header, onBack: onBack, onPill: { onNavigate(.chains) }) {
                SendPickBody(
                    model: m,
                    onFilter: { id in onSendFilter?(id) },
                    onSelect: { index in
                        if let onSelectToken { onSelectToken(index) }
                        else { onNavigate(.sendForm) }
                    },
                    onSelectAll: { visible in onSelectAllTokens?(visible) },
                    onCta: {
                        if let onPickCta { onPickCta() }
                        else { onNavigate(.sendMulti) }
                    }
                )
            }
        case .sendForm(let m):
            FlowScaffold(header: m.header, onBack: onBack) {
                SendFormBody(
                    model: m,
                    onPickRecipient: { onNavigate(.contactPick) },
                    onScan: { onNavigate(.scan) },
                    onRecipientAction: { action in
                        switch action {
                        case .importList: onNavigate(.batchImport)
                        case .contacts: onNavigate(.contactPick)
                        case .add: onNavigate(.addRecipient)
                        }
                    },
                    onRemoveRecipient: { index in onRemoveRecipient?(index) },
                    onFee: { onNavigate(.feeToken) },
                    onMax: { _ in onMax?() },
                    onAddRecipient: {
                        if let onAddRecipient { onAddRecipient() }
                        else { onNavigate(.addRecipient) }
                    },
                    onContinue: {
                        if let onContinueSend { onContinueSend() }
                        else { onNavigate(.sendConfirm) }
                    },
                    amountText: sendAmount,
                    recipientText: sendRecipient,
                    rowText: sendRow,
                    warning: sendWarning,
                    ctaDisabled: sendCtaDisabled
                )
            }
        case .sendConfirm(let m):
            FlowScaffold(header: m.header, onBack: onBack) {
                SendConfirmBody(
                    model: m,
                    onNoticeAction: { onNoticeAction?() },
                    onNoticeSecondary: { onNoticeSecondary?() }
                )
            } footer: {
                FlowFooter {
                    // A BUTTON, not a slider: the slider belongs to the signing
                    // sheet, and Android 045 recorded the difference after
                    // building the wrong one.
                    VelaButton(title: m.cta, kind: .primary) {
                        if let onConfirm { onConfirm() } else { onNavigate(.sendReceipt) }
                    }
                    .disabled(sendCtaDisabled)
                    .opacity(sendCtaDisabled ? Tokens.Opacity.disabled : 1)
                }
            }
        case .sendReceipt(let m):
            FlowScaffold(header: m.header, onBack: onBack) {
                SendReceiptBody(model: m)
            } footer: {
                FlowFooter {
                    VelaButton(
                        title: m.cta,
                        kind: m.ctaAccent ? .primary : .secondary
                    ) {
                        // 关闭 · 后台继续 and 完成 are the same button in two
                        // states, and both mean "I am done looking". The core
                        // decides what that does to a send still in flight.
                        //
                        // 取消 is the third state and it is not an exit: the
                        // passkey prompt is up, the core's checkpoint answers
                        // it, and the screen stays to show what happened.
                        onReceiptDone?()
                        if !m.ctaCancels { onNavigate(.done) }
                    }
                }
            }
        }
    }
}

/// The pinned bottom bar. Confirming and the receipt's exit are the two
/// screens whose CTA the mocks anchor to the bottom of the frame rather than
/// letting it ride under the content.
struct FlowFooter<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.vertical, Tokens.Space.s12)
    }
}

/// The sheets, over whichever screen raised them.
private struct FlowSheetHost: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let sheet: WalletFlowSheet
    var onNavigate: (FlowStep) -> Void = { _ in }
    /// The confirm page's notice buttons: retry what stopped it, or 暂不.
    var onNoticeAction: (() -> Void)?
    var onNoticeSecondary: (() -> Void)?
    var onPickFeeToken: ((Int) -> Void)?
    var onPickContact: ((Int) -> Void)?
    /// A whole group from the picker, added to the split.
    var onPickGroup: ((Int) -> Void)?
    /// The batch importer's four live edges — the unit toggle, the file
    /// picker, the template and the apply — plus its two fields. Absent
    /// everywhere the sheet is a picture, which is the gallery and the
    /// screenshot sweep.
    var batchPaste: Binding<String>?
    var batchRate: Binding<String>?
    var onBatchUnit: ((BatchUnit) -> Void)?
    var onBatchFile: (() -> Void)?
    var onBatchTemplate: (() -> Void)?
    var onBatchResetRate: (() -> Void)?
    var onBatchApply: (() -> Void)?
    /// "Replace them instead" / "Add to them instead" under the importer:
    /// which way an import meets the rows already on the form.
    var onBatchMerge: (() -> Void)?
    var addTokenInput: Binding<String>?
    var onAddToken: (() -> Void)?
    var addTokenError: String?
    var onExplorer: (() -> Void)?
    var onSaveCard: (() -> Void)?
    var onSendToken: (() -> Void)?
    var onReceiveToken: (() -> Void)?
    /// 删除记录 on the open transaction (058). Absent in the gallery.
    var onDeleteTx: (() -> Void)?
    var alert: FlowAlertModel?
    var onDismissAlert: (() -> Void)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                titleRow
                body_
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.bottom, Tokens.Space.s32)
        }
        .background(theme.bgBase)
        .alert(
            alert?.title ?? "",
            isPresented: Binding(
                get: { alert != nil },
                set: { if !$0 { onDismissAlert?() } }
            )
        ) {
            Button("OK") { onDismissAlert?() }
        } message: {
            Text(verbatim: alert?.message ?? "")
        }
    }

    /// The sheet's own title row.
    ///
    /// The × is always there even though the grabber already dismisses by
    /// drag: a sheet reached mid-transfer needs a way out that does not depend
    /// on knowing a gesture.
    private var titleRow: some View {
        HStack(spacing: Tokens.Space.s8) {
            if let title = sheet.chromeTitle {
                Text(verbatim: title)
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
            Spacer(minLength: Tokens.Space.s8)
            Button { dismiss() } label: {
                LucideIcon(.close, size: LucideIconSize.flowBack)
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(sheet.closeLabel)
        }
        .padding(.top, Tokens.Space.s16)
    }

    @ViewBuilder private var body_: some View {
        switch sheet {
        case .receiveQr(let m):
            ReceiveQrBody(
                model: m,
                onSave: { onSaveCard?() },
                onExplorer: { onExplorer?() }
            )
        case .txDetail(let m):
            TxDetailBody(model: m, onExplorer: { onExplorer?() }, onDelete: onDeleteTx)
        case .tokenDetail(let m):
            TokenDetailBody(
                model: m,
                // 收款 on a token's page means "pay me THIS" — the asset-limited
                // code, not the network list. Android has done it this way
                // since 048; iOS drew the sheet and could not reach it.
                onReceive: { onReceiveToken?() ?? onNavigate(.receive) },
                // The send opens on THIS token. Pushing the form without
                // preselecting it opened the picker instead, which is the
                // dead-button shape once more: the button worked and landed
                // somewhere that looked like nothing had happened.
                onSend: { onSendToken?() ?? onNavigate(.sendForm) },
                onExplorer: { onExplorer?() }
            )
        case .addToken(let m):
            AddTokenBody(
                model: m,
                onSubmit: { onAddToken?() },
                input: addTokenInput,
                errorText: addTokenError
            )
        case .contactPick(let m):
            ContactPickBody(
                model: m,
                onScan: { onNavigate(.scan) },
                onGroup: { index in onPickGroup?(index) },
                onSelect: { index in onPickContact?(index) }
            )
        case .feeToken(let m):
            FeeTokenBody(model: m, onSelect: { index in onPickFeeToken?(index) })
        case .batchImport(let m):
            BatchImportBody(
                model: m,
                onUnit: { raw in onBatchUnit?(BatchUnit(rawValue: raw) ?? .token) },
                onFile: { onBatchFile?() },
                onTemplate: { onBatchTemplate?() },
                onApply: { onBatchApply?() },
                onMerge: { onBatchMerge?() },
                pasteText: batchPaste,
                rateText: batchRate,
                onResetRate: { onBatchResetRate?() }
            )
        }
    }
}
