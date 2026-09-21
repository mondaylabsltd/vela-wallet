//
//  SettingsSheet.swift
//  VelaWallet
//
//  Every overlay the phone draws as a bottom sheet (spec 023). One `.sheet`
//  whose CONTENT swaps, not one per overlay: presenting a second sheet while a
//  first is dismissing fails silently on iOS (the nesting bug the founder hit
//  on iPhone, 2026-08-27), and the settings screen can move between an account
//  switcher and a sign-out confirm in one tap.
//

import SwiftUI

struct SettingsSheet: View {
    @Environment(\.theme) private var theme
    let model: SettingsScreenModel
    let overlay: SettingsOverlay
    let onDismiss: () -> Void
    let onSignOut: () -> Void
    /// A row picked in one of the five select sheets. Absent in the gallery,
    /// where the sheets are pictures of a choice already made.
    var onPick: ((SettingsOverlay, String) -> Void)?
    /// 全部清除, and the erase this app will not run without a person. The
    /// erase answers whether it happened: `false` keeps this sheet up, its
    /// callout saying what did not go (spec 072).
    var onClearCaches: (() -> Void)?
    var onErase: (() -> Bool)?
    /// An account row tapped in the switcher.
    var onSelectAccount: ((String) -> Void)?
    var onAccountCreate: (() -> Void)?
    var onAccountSignIn: (() -> Void)?
    /// SR2's field and its commit (058). Absent in the gallery, where the
    /// endpoint is a picture of one already typed.
    var rpcDraft: Binding<String>?
    var onCommitRpc: (() -> Void)?
    /// SR3's 立即重试, per chain id.
    var onRetryChain: ((String) -> Void)?
    /// The destructive action waiting on an answer — a storage row's 清除, a
    /// network's bin, "reset to defaults" — and its "yes".
    var pendingConfirm: ConfirmSheetModel?
    var onConfirmPending: (() -> Void)?
    /// The Clear Signer page's Save (spec 071): `true` when the core took the
    /// address, which is what closes the sheet. Absent in the gallery.
    var onSaveSignerUrl: ((String) -> Bool)?
    var onResetSignerUrl: (() -> Void)?
    /// The language sheet's "suggest a fix". Absent in the gallery.
    var onOpenLink: ((String) -> Void)?

    /// Where "suggest a fix" goes — the issue tracker the web links (its
    /// `SelectSheetBody`), since the corpus lives in that repository.
    static let contributeUrl = "https://github.com/mondaylabsltd/vela-wallet/issues"

    var body: some View {
        // The ✕ sits in the host, not in each body: every sheet opens with a
        // SheetTitle, so one overlay pinned top-trailing lands on the title
        // line for all of them — and none of them can forget it. The drag
        // indicator alone is not an affordance a first-time reader recognises.
        ZStack(alignment: .topTrailing) {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                switch overlay {
                case .accounts:
                    AccountsSheetBody(
                        sheet: model.accountsSheet,
                        onSelect: { address in onSelectAccount?(address) },
                        onCreate: onAccountCreate,
                        onSignIn: onAccountSignIn
                    )
                case .signOut:
                    ConfirmSheetBody(sheet: model.signOutSheet,
                                     onConfirm: onSignOut, onCancel: onDismiss)
                case .language:
                    SelectSheetBody(
                        sheet: model.languageSheet,
                        onPick: { id in onPick?(.language, id) },
                        onFooterLink: onOpenLink.map { open in { open(Self.contributeUrl) } }
                    )
                case .currency:
                    SelectSheetBody(
                        sheet: model.currencySheet,
                        onPick: { id in onPick?(.currency, id) }
                    )
                case .feeSpeed:
                    SelectSheetBody(
                        sheet: model.feeSpeedSheet,
                        onPick: { id in onPick?(.feeSpeed, id) }
                    )
                case .signWith:
                    SelectSheetBody(
                        sheet: model.signWithSheet,
                        onPick: { id in onPick?(.signWith, id) }
                    )
                case .signerPage:
                    if let page = model.signerPage {
                        SignerPageSheetBody(
                            model: page,
                            onSave: { text in
                                if onSaveSignerUrl?(text) == true { onDismiss() }
                            },
                            onReset: onResetSignerUrl.map { reset in
                                {
                                    reset()
                                    onDismiss()
                                }
                            }
                        )
                    }
                case .numberFormat:
                    SelectSheetBody(
                        sheet: model.numberSheet,
                        onPick: { id in onPick?(.numberFormat, id) }
                    )
                case .dateFormat:
                    SelectSheetBody(
                        sheet: model.dateSheet,
                        onPick: { id in onPick?(.dateFormat, id) }
                    )
                case .timeFormat:
                    SelectSheetBody(
                        sheet: model.timeSheet,
                        onPick: { id in onPick?(.timeFormat, id) }
                    )
                case .clearStorageItem, .removeNetwork, .resetEndpoints:
                    // Built from what the page already says — a row's own
                    // label and action word, the network's name, the fields
                    // "reset" replaces. The screen holds the question and what
                    // "yes" does; this only asks it.
                    if let confirm = pendingConfirm {
                        ConfirmSheetBody(
                            sheet: confirm,
                            onConfirm: { onConfirmPending?(); onDismiss() },
                            onCancel: onDismiss
                        )
                    }
                case .clearCaches:
                    ConfirmSheetBody(
                        sheet: model.clearCachesSheet,
                        onConfirm: { onClearCaches?(); onDismiss() },
                        onCancel: onDismiss
                    )
                case .eraseDevice:
                    ConfirmSheetBody(
                        sheet: model.eraseSheet,
                        // Wired, and **never run on the founder's phone**. The
                        // confirm is drawn and the action exists; verifying it
                        // means reading the code and the simulator, not erasing
                        // a device with a real wallet on it. An erase that
                        // left something behind keeps the sheet up — its
                        // callout says so and the button is still there.
                        onConfirm: { if onErase?() != false { onDismiss() } },
                        onCancel: onDismiss
                    )
                case .feedback:
                    FeedbackSheetBody(model: model.feedback)
                case .rpcFix:
                    RpcFixSheetBody(
                        model: model.rpcFix,
                        draft: rpcDraft,
                        onPrimary: {
                            // Save, THEN close: the primary is 保存 while the
                            // endpoint is unproven and 完成 once it answered.
                            onCommitRpc?()
                            onDismiss()
                        }
                    )
                case .balanceDetail:
                    BalanceDetailSheetBody(model: model.balanceDetail, onRetry: onRetryChain)
                case .relayer:
                    RelayerSheetBody(model: model.relayer, onPrimary: onDismiss)
                case .none:
                    EmptyView()
                }
            }
            .padding(.horizontal, Tokens.Space.s24)
            .padding(.vertical, Tokens.Space.s24)
        }
        .background(theme.bgBase)

            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: 32, height: 32)
                    .background(theme.bgRaised, in: Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.closeLabel)
            .padding(.trailing, Tokens.Space.s24)
            .padding(.top, Tokens.Space.s24)
        }
        .background(theme.bgBase)
        .presentationDragIndicator(.visible)
    }
}

private struct SheetTitle: View {
    @Environment(\.theme) private var theme
    let title: String
    var subtitle: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(title)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
            if let subtitle {
                Text(subtitle)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.bottom, Tokens.Space.s12)
    }
}

private struct SelectSheetBody: View {
    @Environment(\.theme) private var theme
    let sheet: SelectSheetModel
    var onPick: ((String) -> Void)?
    /// The footer link's destination. `nil` draws it as the label it was.
    var onFooterLink: (() -> Void)?

    var body: some View {
        SheetTitle(title: sheet.title, subtitle: sheet.subtitle)
        if let placeholder = sheet.searchPlaceholder {
            SettingsUrlField(field: UrlFieldModel(id: "search", label: "", value: "",
                                                  placeholder: placeholder))
                .padding(.bottom, Tokens.Space.s12)
        }
        ForEach(sheet.rows) { row in
            if let onPick {
                Button { onPick(row.id) } label: { SelectRow(row: row) }
                    .buttonStyle(.plain)
            } else {
                SelectRow(row: row)
            }
        }
        if let note = sheet.footerNote {
            Text(note)
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
                .padding(.top, Tokens.Space.s16)
        }
        if let link = sheet.footerLink, let onFooterLink {
            Button(action: onFooterLink) {
                Text(link)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.infoBase)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isLink)
            .padding(.top, Tokens.Space.s8)
        } else if let link = sheet.footerLink {
            Text(link)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.infoBase)
                .padding(.top, Tokens.Space.s8)
        }
    }
}

/// The Clear Signer page (spec 071): the address, Save, and the way back to
/// the official page. What is under the field is the core's to say — a refused
/// address, and that a page off `getvela.app` cannot use this wallet's
/// passkeys.
private struct SignerPageSheetBody: View {
    @Environment(\.theme) private var theme
    let model: SignerPageModel
    let onSave: (String) -> Void
    var onReset: (() -> Void)?

    /// Local, seeded from the address in force: what is half-typed is nobody
    /// else's business until Save hands it to the core.
    @State private var text: String

    init(model: SignerPageModel, onSave: @escaping (String) -> Void, onReset: (() -> Void)?) {
        self.model = model
        self.onSave = onSave
        self.onReset = onReset
        _text = State(initialValue: model.field.value)
    }

    var body: some View {
        SheetTitle(title: model.title, subtitle: model.subtitle)
        SettingsUrlField(field: model.field, text: $text, onCommit: { onSave(text) })
            .padding(.bottom, Tokens.Space.s8)
        if let error = model.error {
            Text(error)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.errorBase)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.bottom, Tokens.Space.s8)
        }
        if let foreign = model.foreign {
            SettingsCallout(callout: CalloutModel(tone: .warning, text: foreign))
                .padding(.vertical, Tokens.Space.s8)
        }
        VelaButton(title: model.save, kind: .primary) { onSave(text) }
            .padding(.top, Tokens.Space.s8)
            .padding(.bottom, Tokens.Space.s12)
        if let reset = model.reset, let onReset {
            VelaButton(title: reset, kind: .secondary, action: onReset)
        }
    }
}

private struct ConfirmSheetBody: View {
    @Environment(\.theme) private var theme
    let sheet: ConfirmSheetModel
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        SheetTitle(title: sheet.title)
        Text(sheet.body)
            .typeRole(Typography.fieldLabel)
            .foregroundStyle(theme.fgBase)
            .padding(.bottom, Tokens.Space.s16)
        if let note = sheet.note {
            Text(note)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .padding(.bottom, Tokens.Space.s16)
        }
        if let callout = sheet.callout {
            SettingsCallout(callout: callout)
                .padding(.bottom, Tokens.Space.s16)
        }
        // The tone picks the CTA's colour, so "清除缓存" is accent and "全部清除"
        // is red without either screen owning a button of its own.
        VelaButton(title: sheet.confirm, kind: sheet.danger ? .danger : .primary,
                   action: onConfirm)
            .padding(.bottom, Tokens.Space.s12)
        VelaButton(title: sheet.cancel, kind: .secondary, action: onCancel)
    }
}

private struct AccountsSheetBody: View {
    @Environment(\.theme) private var theme
    let sheet: AccountsSheetModel
    var onSelect: ((String) -> Void)?
    /// The two ways on from here. Absent = a fixture board, where they do
    /// nothing on purpose; present = the live screen, where they must.
    var onCreate: (() -> Void)?
    var onSignIn: (() -> Void)?

    var body: some View {
        SheetTitle(title: sheet.title)
        Text(sheet.summary)
            .typeRole(Typography.flowCaption)
            .foregroundStyle(theme.fgSubtle)
            .padding(.bottom, Tokens.Space.s12)
        // Keyed by POSITION, not by address: the address is not unique. Two
        // records can derive the same Safe (one wallet signed into with a
        // second passkey), and `Identifiable` on `addressFull` then hands
        // `ForEach` a duplicate id — the web's switcher threw outright on that
        // pair (issue 214 follow-up). The core refuses to hold the pair now,
        // and this stops the shell from depending on it.
        ForEach(Array(sheet.rows.enumerated()), id: \.offset) { _, row in
            Button { onSelect?(row.addressFull) } label: {
            VStack(spacing: 0) {
                HStack(spacing: Tokens.Space.s12) {
                    IdenticonAvatar(seed: row.addressFull, size: 40, name: row.name)
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(row.name)
                            .typeRole(Typography.fieldLabel)
                            .fontWeight(.semibold)
                            .foregroundStyle(row.selected ? theme.accentBase : theme.fgBase)
                        Text(row.addressDisplay)
                            .typeRole(Typography.monoSmall)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer(minLength: Tokens.Space.s8)
                    Text(row.amount)
                        .typeRole(Typography.fieldLabel)
                        .foregroundStyle(theme.fgBase)
                    if row.selected {
                        LucideIcon(.check, size: LucideIconSize.action)
                            .foregroundStyle(theme.accentBase)
                    }
                }
                .padding(.vertical, Tokens.Space.s12)
                SettingsDivider()
            }
            .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(onSelect == nil)
        }
        // Both closures were EMPTY: 创建新账户 and 登录已有账户 drew, took the
        // tap, and did nothing (founder, 2026-09-16). They are the only two
        // ways on from this sheet, so an empty closure is the sheet's dead end.
        VelaButton(title: sheet.primary, kind: .primary) { onCreate?() }
            .padding(.top, Tokens.Space.s24)
            .padding(.bottom, Tokens.Space.s12)
        VelaButton(title: sheet.secondary, kind: .secondary) { onSignIn?() }
    }
}

private struct FeedbackSheetBody: View {
    @Environment(\.theme) private var theme
    let model: FeedbackModel

    var body: some View {
        SheetTitle(title: model.title, subtitle: model.subtitle)
        SettingsUrlField(field: UrlFieldModel(id: "what", label: "", value: "",
                                              placeholder: model.placeholder))
        Text(model.addSteps)
            .typeRole(Typography.flowCaption)
            .foregroundStyle(theme.infoBase)
            .padding(.vertical, Tokens.Space.s12)
        // Open by default: the point of the disclosure is that somebody can see
        // what is about to leave their device before pressing send, and a
        // closed box would be a promise instead of a showing.
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(model.previewToggle)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
            ForEach(model.previewLines, id: \.self) { line in
                Text(line)
                    .typeRole(Typography.monoSmall)
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Space.s12)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        SettingsCallout(callout: CalloutModel(tone: .info, text: model.consent))
            .padding(.vertical, Tokens.Space.s16)
        VelaButton(title: model.send, kind: .primary) {}
        Text(model.githubLink)
            .typeRole(Typography.flowCaption)
            .foregroundStyle(theme.infoBase)
            .frame(maxWidth: .infinity)
            .padding(.top, Tokens.Space.s12)
    }
}

private struct RpcFixSheetBody: View {
    @Environment(\.theme) private var theme
    let model: RpcFixModel
    /// The URL being typed. `nil` keeps the drawn, uneditable box.
    var draft: Binding<String>?
    let onPrimary: () -> Void

    var body: some View {
        SheetTitle(title: model.title)
        HStack(spacing: Tokens.Space.s12) {
            ChainMark(mark: model.mark)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(model.name)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                Text(model.meta)
                    .typeRole(Typography.monoSmall)
                    .foregroundStyle(theme.fgSubtle)
            }
            Spacer()
            StatusPill(pill: model.badge)
        }
        .padding(.bottom, Tokens.Space.s16)
        SettingsCallout(callout: model.callout)
            .padding(.bottom, Tokens.Space.s16)
        SettingsUrlField(field: model.field, text: draft, onCommit: onPrimary)
            .padding(.bottom, Tokens.Space.s16)
        VelaButton(title: model.primary, kind: .primary, action: onPrimary)
        if let label = model.providersLabel {
            Text(label)
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
                .padding(.top, Tokens.Space.s16)
                .padding(.bottom, Tokens.Space.s8)
            HStack(spacing: Tokens.Space.s8) {
                ForEach(model.providers, id: \.self) { name in
                    Text(name)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgBase)
                        .padding(.horizontal, Tokens.Space.s12)
                        .padding(.vertical, Tokens.Space.s8)
                        .background(theme.bgRaised,
                                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r8))
                }
            }
        }
        if let report = model.report {
            Text(report)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.infoBase)
                .padding(.top, Tokens.Space.s16)
        }
    }
}

private struct BalanceDetailSheetBody: View {
    @Environment(\.theme) private var theme
    let model: BalanceDetailModel
    /// 立即重试 on an unreachable chain. Absent in the gallery.
    var onRetry: ((String) -> Void)?

    var body: some View {
        SheetTitle(title: model.title)
        Text(model.summary)
            .typeRole(Typography.flowCaption)
            .foregroundStyle(theme.fgSubtle)
            .padding(.bottom, Tokens.Space.s16)
        Text(model.sectionPending)
            .typeRole(Typography.flowCaption)
            .fontWeight(.semibold)
            .foregroundStyle(theme.fgBase)
        Text(model.pendingNote)
            .typeRole(Typography.label)
            .foregroundStyle(theme.fgSubtle)
            .padding(.vertical, Tokens.Space.s8)
        ForEach(model.pending) { row(model: $0) }
        Text(model.sectionDone)
            .typeRole(Typography.flowCaption)
            .fontWeight(.semibold)
            .foregroundStyle(theme.fgBase)
            .padding(.top, Tokens.Space.s16)
        ForEach(model.done) { row(model: $0) }
    }

    @ViewBuilder private func row(model row: BalanceDetailRowModel) -> some View {
        HStack(spacing: Tokens.Space.s12) {
            ChainMark(mark: row.mark)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(row.name)
                    .typeRole(Typography.fieldLabel)
                    .foregroundStyle(theme.fgBase)
                if let status = row.status {
                    // Rate-limiting gets a grey line and no button because it
                    // resolves itself; a dead RPC gets red and 立即重试.
                    Text(status)
                        .typeRole(Typography.label)
                        .foregroundStyle(row.tone == .error ? theme.errorBase : theme.fgSubtle)
                }
            }
            Spacer(minLength: Tokens.Space.s8)
            if let action = row.action, let onRetry {
                Button { onRetry(row.id) } label: {
                    Text(action)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.infoBase)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            } else if let action = row.action {
                Text(action)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.infoBase)
            }
            if let amount = row.amount {
                Text(amount)
                    .typeRole(Typography.fieldLabel)
                    .foregroundStyle(theme.fgBase)
            }
        }
        .padding(.vertical, Tokens.Space.s12)
    }
}

private struct RelayerSheetBody: View {
    @Environment(\.theme) private var theme
    let model: RelayerModel
    let onPrimary: () -> Void

    var body: some View {
        SheetTitle(title: model.title)
        Text(model.lead)
            .typeRole(Typography.flowCaption)
            .foregroundStyle(theme.fgMuted)
            .padding(.bottom, Tokens.Space.s16)
        HStack(spacing: Tokens.Space.s12) {
            ChainMark(mark: model.mark)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(model.name)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                Text(model.amountHint)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgSubtle)
            }
            Spacer()
        }
        .padding(.bottom, Tokens.Space.s16)
        QRPlaceholder(caption: model.qrCaption)
            .frame(maxWidth: .infinity)
            .padding(.bottom, Tokens.Space.s16)
        Text(model.addressDisplay)
            .typeRole(Typography.monoSmall)
            .foregroundStyle(theme.fgBase)
            .frame(maxWidth: .infinity)
            .padding(Tokens.Space.s12)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .padding(.bottom, Tokens.Space.s16)
        // Non-refundable, and it goes to the bundler operator rather than to
        // Vela or to this transaction — which is why the note sits between the
        // address and the CTA rather than under it.
        SettingsCallout(callout: model.callout)
            .padding(.bottom, Tokens.Space.s16)
        VelaButton(title: model.primary, kind: .primary, action: onPrimary)
    }
}
