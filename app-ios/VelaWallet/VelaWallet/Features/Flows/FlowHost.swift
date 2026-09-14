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

import SwiftUI

/// Where a screen can go next. Names match the web host's navigation intents.
enum FlowStep {
    case receiveQr
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
    var sendWarning: String?
    var sendCtaDisabled = false
    /// Which row of the picker was tapped. The index travels because the core
    /// keys tokens by id and the screen only knows positions — the same defect
    /// the receive sheet and the network list each had: a tap that forgets what
    /// it tapped opens the first one.
    var onSelectToken: ((Int) -> Void)?
    var onMax: (() -> Void)?
    /// The confirm page's CTA, and the receipt's exit. Absent where the flow is
    /// a picture, where the CTA still just navigates.
    var onConfirm: (() -> Void)?
    var onReceiptDone: (() -> Void)?
    /// The form's 继续. The CORE owns the step, so this dispatches rather than
    /// navigates — pushing a screen the machine has not moved to renders the
    /// one it is still on, which looks exactly like a dead button.
    var onContinueSend: (() -> Void)?

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
            .environment(\.walletTextScale, model.textScale)
            .sheet(isPresented: $pickingChain) {
                if let chainSheet {
                    ChainSelectSheet(model: chainSheet, onSelect: { chainId in
                        onPickChain?(chainId)
                        pickingChain = false
                    })
                    .environment(\.walletTextScale, model.textScale)
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
                        addTokenInput: addTokenInput,
                        onAddToken: onAddToken,
                        addTokenError: addTokenError,
                        onExplorer: onExplorer,
                        onSaveCard: onSaveCard,
                        alert: alert,
                        onDismissAlert: onDismissAlert
                    )
                        .environment(\.walletTextScale, model.textScale)
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
            ScanSurfaceView(model: m, onClose: onBack)
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
                    onSelect: { index in
                        if let onSelectToken { onSelectToken(index) }
                        else { onNavigate(.sendForm) }
                    },
                    onCta: { onNavigate(.sendMulti) }
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
                    onFee: { onNavigate(.feeToken) },
                    onMax: { _ in onMax?() },
                    onAddRecipient: { onNavigate(.addRecipient) },
                    onContinue: {
                        if let onContinueSend { onContinueSend() }
                        else { onNavigate(.sendConfirm) }
                    },
                    amountText: sendAmount,
                    recipientText: sendRecipient,
                    warning: sendWarning,
                    ctaDisabled: sendCtaDisabled
                )
            }
        case .sendConfirm(let m):
            FlowScaffold(header: m.header, onBack: onBack) {
                SendConfirmBody(model: m)
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
                        onReceiptDone?()
                        onNavigate(.done)
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
    var addTokenInput: Binding<String>?
    var onAddToken: (() -> Void)?
    var addTokenError: String?
    var onExplorer: (() -> Void)?
    var onSaveCard: (() -> Void)?
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
            TxDetailBody(model: m, onExplorer: { onExplorer?() })
        case .tokenDetail(let m):
            TokenDetailBody(
                model: m,
                onReceive: { onNavigate(.receive) },
                onSend: { onNavigate(.sendForm) },
                onExplorer: { onExplorer?() }
            )
        case .addToken(let m):
            AddTokenBody(
                model: m,
                onSubmit: { onAddToken?() },
                input: addTokenInput,
                errorText: addTokenError
            )
        case .contactPick(let m): ContactPickBody(model: m, onScan: { onNavigate(.scan) })
        case .feeToken(let m): FeeTokenBody(model: m)
        case .batchImport(let m): BatchImportBody(model: m)
        }
    }
}
