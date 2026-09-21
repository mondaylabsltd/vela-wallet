//
//  FlowBodies.swift
//  VelaWallet
//
//  The bodies of the wallet flows (spec 021).
//
//  Every one takes its model and nothing else, so the same body serves the
//  pushed screen, the sheet, and the gallery. Chrome — the scaffold, the
//  sheet — is the caller's business.
//

import SwiftUI

// MARK: - Receive

/// R1 — the receive network list.
///
/// The subtitle is the whole idea: one address, every network. The list under
/// it is not eight addresses, it is eight ways of saying the same one — which
/// is why every row shows the same characters, and why the copy button is on
/// each row rather than once at the top.
struct ReceiveListBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ReceiveListModel
    var onQr: (Int) -> Void = { _ in }

    @State private var query = ""
    @State private var copiedIndex: Int?

    private var shown: [(offset: Int, element: NetworkRowModel)] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        let all = Array(model.rows.enumerated())
        guard !trimmed.isEmpty else { return all.map { ($0.offset, $0.element) } }
        return all
            .filter { $0.element.name.localizedCaseInsensitiveContains(trimmed) }
            .map { ($0.offset, $0.element) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            // R1 prints this directly under the title: the list below is not
            // eight addresses, and the sentence is the only thing that says so.
            Text(verbatim: model.subtitle)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            FlowSearchField(placeholder: model.searchPlaceholder, text: $query)
            if shown.isEmpty {
                Text(verbatim: model.emptyText.replacingOccurrences(
                    of: "{{query}}",
                    with: query.trimmingCharacters(in: .whitespaces)
                ))
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Tokens.Space.s32)
            } else {
                VStack(spacing: Tokens.Space.s0) {
                    ForEach(Array(shown.enumerated()), id: \.element.offset) { position, entry in
                        if position > 0 { FlowDivider() }
                        NetworkRowView(
                            row: entry.element,
                            copied: copiedIndex == entry.offset,
                            onCopy: { copy(entry.offset) },
                            onQr: { onQr(entry.offset) }
                        )
                    }
                }
            }
        }
    }

    /// The tick holds and goes back (SPEC 动效 · 收款). Long enough to register,
    /// short enough that copying three networks in a row never leaves a person
    /// wondering which tick is the live one.
    private func copy(_ index: Int) {
        copiedIndex = index
        Task {
            try? await Task.sleep(for: .seconds(Tokens.Motion.fast))
            if copiedIndex == index { copiedIndex = nil }
        }
    }
}

/// R2 / R3 — the address, as a code.
struct ReceiveQrBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ReceiveQrModel
    var onSave: () -> Void = {}
    var onExplorer: () -> Void = {}

    @State private var copied: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: model.title)
                .typeRole(Typography.rowTitle.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)

            if let contract = model.contract {
                HStack(spacing: Tokens.Space.s4) {
                    Text(verbatim: contract.label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    Text(verbatim: contract.value)
                        .monoRole(Typography.monoAddress.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Button {
                        // It copies. Until 058 this button showed a checkmark
                        // and left the clipboard exactly as it was — a person
                        // pasted whatever was there before, into a payment.
                        velaCopy(contract.copyValue ?? contract.value)
                        copied = "contract"
                    } label: {
                        // The same copy affordance the address row carries, one
                        // size down: a contract is a detail ABOUT the code
                        // below, not the thing being received.
                        LucideIcon(copied == "contract" ? .check : .copy, size: LucideIconSize.checkmark)
                            .foregroundStyle(copied == "contract" ? theme.successBase : theme.fgSubtle)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(contract.copyLabel)
                }
            }

            AddressCardView(
                account: model.account,
                copied: copied == "address",
                // The lines ARE the address, split for the card — the same
                // reconstruction Android's receive sheet does.
                onCopy: {
                    velaCopy(model.account.lines.joined())
                    copied = "address"
                }
            )

            QrCardView(label: model.title, modules: model.modules) {
                // The asset's own logo in the middle of its code (058); the
                // coloured disc with its ticker is the fallback.
                RemoteLogoView(urls: model.centre.logoURLs, size: WalletFlowGeometry.qrCentre) {
                    Circle()
                        .fill(model.centre.badgeColor)
                        .overlay {
                            Text(verbatim: model.centre.ticker)
                                .typeRole(Typography.tokenGlyph)
                                .foregroundStyle(theme.onAccent)
                        }
                }
            }
            .frame(maxWidth: .infinity)

            Text(verbatim: model.warning)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.top, Tokens.Space.s8)

            VelaButton(title: model.saveImage, kind: .secondary, action: onSave)
            VelaButton(title: model.viewOnExplorer, kind: .secondary, action: onExplorer)
        }
    }
}

// MARK: - Activity

/// A1 — the full history.
///
/// The wallet home shows the last three; this shows all of them, grouped by
/// day. Same `ActivityRowView` as the home, same day headings — the difference
/// is the network filter in the header and that the list does not stop.
struct HistoryBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: HistoryModel
    var onSelect: (Int, Int) -> Void = { _, _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            if model.mode == .empty {
                // A filtered-empty history is a narrowing, not a problem: one
                // quiet line rather than the illustrated empty state the home
                // uses for a wallet that has genuinely never done anything.
                Text(verbatim: model.emptyText)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, Tokens.Space.s48)
            } else {
                ForEach(Array(model.groups.enumerated()), id: \.element.id) { groupIndex, group in
                    Text(verbatim: group.label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .padding(.vertical, Tokens.Space.s4)
                    ForEach(Array(group.rows.enumerated()), id: \.element.id) { rowIndex, row in
                        Button { onSelect(groupIndex, rowIndex) } label: {
                            ActivityRowView(model: row).contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }
}

/// A2 / A3 — one transaction.
///
/// A2 is a received ERC-20 and A3 a sent native coin; the difference between
/// them is entirely in the fact list (a native coin has no contract row), so
/// this takes the facts as data rather than branching on a kind.
struct TxDetailBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: TxDetailModel
    var onExplorer: () -> Void = {}
    /// 删除记录. Absent in the gallery, where nothing is real enough to remove.
    var onDelete: (() -> Void)?

    @State private var copiedIndex: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: model.title)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                StatusChipView(chip: model.status)
            }
            AmountHeroView(amount: model.amount, fiat: model.fiat, positive: model.positive)
            FlowDivider()
            ForEach(Array(model.facts.enumerated()), id: \.element.id) { index, fact in
                if index > 0 { FlowDivider() }
                FactRowView(
                    fact: fact,
                    copied: copiedIndex == index,
                    onCopy: { copiedIndex = index }
                )
            }
            VelaButton(title: model.viewOnExplorer, kind: .secondary, action: onExplorer)
                .padding(.top, Tokens.Space.s16)
            if let label = model.deleteLabel, let onDelete {
                VelaButton(title: label, kind: .danger, action: onDelete)
                    .padding(.top, Tokens.Space.s8)
            }
        }
    }
}

// MARK: - Assets

/// T1 / T4 — everything the wallet holds.
///
/// T4 is the same screen with nothing in it, and it does more than say so: an
/// empty asset list usually means either "you haven't received anything yet"
/// or "you have, and we can't see it". The guidance card answers the second,
/// because the person in that case is the one who needs help.
struct AssetsBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: AssetsModel
    var onSelect: (Int) -> Void = { _ in }
    var onAdd: () -> Void = {}
    var onReceive: () -> Void = {}

    @State private var query = ""

    private var shown: [(offset: Int, element: AssetRowModel)] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        let all = Array(model.rows.enumerated())
        guard !trimmed.isEmpty else { return all.map { ($0.offset, $0.element) } }
        return all
            .filter { "\($0.element.ticker) \($0.element.chain)".localizedCaseInsensitiveContains(trimmed) }
            .map { ($0.offset, $0.element) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            FlowSearchField(placeholder: model.searchPlaceholder, text: $query)
            if let empty = model.empty {
                // The empty state is tappable: its caption says "tap here to
                // see your address", so it had better be the thing that does.
                Button(action: onReceive) {
                    WalletEmptyState(
                        icon: .creditCard,
                        model: SectionEmptyModel(title: empty.title, caption: empty.caption)
                    )
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                HintCardView(title: empty.hintTitle, body_: empty.hintBody) {
                    VelaButton(title: empty.cta, kind: .secondary, action: onAdd)
                }
            } else {
                VStack(spacing: Tokens.Space.s0) {
                    ForEach(Array(shown.enumerated()), id: \.element.offset) { position, entry in
                        if position > 0 { FlowDivider() }
                        Button { onSelect(entry.offset) } label: {
                            AssetRowView(model: entry.element).contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
                Button(action: onAdd) {
                    // A link, not a button: adding a token by hand is the rare
                    // path out of a list that normally fills itself.
                    Text(verbatim: model.addByAddress)
                        .typeRole(Typography.body.scaled(textScale))
                        // T1 sets this quiet and centred, not as a link: it is
                        // the way out of "my token is missing", not an action
                        // competing with the rows above it.
                        .foregroundStyle(theme.fgMuted)
                        .frame(maxWidth: .infinity)
                        .padding(Tokens.Space.s8)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// T2 — one token: what you hold, what it is, and what it has done.
struct TokenDetailBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: TokenDetailModel
    var onReceive: () -> Void = {}
    var onSend: () -> Void = {}
    var onExplorer: () -> Void = {}

    @State private var copiedIndex: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            HStack(spacing: Tokens.Space.s12) {
                TokenIconView(mark: model.mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: model.symbol)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Text(verbatim: model.chain)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
            }
            .padding(.bottom, Tokens.Space.s12)

            Text(verbatim: model.balance)
                .typeRole(Typography.display.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .minimumScaleFactor(WalletGeometry.heroMinScale)
                .lineLimit(1)
            Text(verbatim: model.fiat)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)

            // Receive and Send sit under the balance because they are the two
            // reasons anyone opens this sheet. Everything below is reference.
            HStack(spacing: Tokens.Space.s8) {
                VelaButton(title: model.receive, kind: .secondary, action: onReceive)
                VelaButton(title: model.send, kind: .secondary, action: onSend)
            }
            .padding(.vertical, Tokens.Space.s16)

            FlowDivider()
            ForEach(Array(model.facts.enumerated()), id: \.element.id) { index, fact in
                if index > 0 { FlowDivider() }
                FactRowView(
                    fact: fact,
                    copied: copiedIndex == index,
                    onCopy: { copiedIndex = index }
                )
            }

            Text(verbatim: model.transactionsTitle)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .padding(.top, Tokens.Space.s16)
            ForEach(model.rows) { ActivityRowView(model: $0) }

            Button(action: onExplorer) {
                Text(verbatim: model.viewOnExplorer)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .frame(maxWidth: .infinity)
                    .padding(Tokens.Space.s12)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }
}

/// T3 / T3b / T5 / T5b — adding a token, or the network one lives on.
///
/// Two tabs over one shape: a field, a result card, a CTA. Every failure state
/// in T5 and T5b is a variant of the same two elements — the field's error,
/// and what the result card holds — so they are model states, not screens.
struct AddTokenBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: AddTokenModel
    var onTab: (String) -> Void = { _ in }
    var onNetwork: () -> Void = {}
    var onSubmit: () -> Void = {}
    /// Present only when a machine owns the field. The gallery passes nothing
    /// and keeps its picture.
    var input: Binding<String>?
    /// Shown under the CTA when a save failed — the mock has no alert, and a
    /// tap that changes nothing has told the person nothing.
    var errorText: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            FlowSegmentedToggle(
                options: [
                    (id: AddTokenTab.erc20.rawValue, label: model.tabErc20),
                    (id: AddTokenTab.native.rawValue, label: model.tabNative),
                ],
                selectedId: model.tab.rawValue,
                onSelect: onTab
            )

            if let network = model.network {
                Button(action: onNetwork) {
                    HStack(spacing: Tokens.Space.s8) {
                        InlineTokenMark(mark: network.mark)
                        Text(verbatim: network.name)
                            .typeRole(Typography.rowTitle.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                        Spacer(minLength: Tokens.Space.s8)
                        LucideIcon(.chevronDown, size: LucideIconSize.nameChevron)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .padding(Tokens.Space.s12)
                    .background(
                        RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised)
                    )
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(network.pickLabel)
            }

            if let input {
                FlowMonoInput(
                    value: input,
                    label: model.fieldLabel,
                    placeholder: model.fieldPlaceholder,
                    error: model.fieldError
                )
            } else {
                FlowMonoField(
                    value: model.fieldValue,
                    label: model.fieldLabel,
                    error: model.fieldError
                )
            }

            result

            VelaButton(
                title: model.cta,
                kind: .primary,
                enabled: !model.ctaDisabled,
                action: onSubmit
            )
            .padding(.top, Tokens.Space.s4)

            if let errorText {
                Text(verbatim: errorText)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
            }
        }
    }

    @ViewBuilder private var result: some View {
        switch model.result {
        case .none:
            EmptyView()
        case .searching(let text), .notFound(let text):
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
        case .token(let mark, let name, let detail, let chip):
            HStack(spacing: Tokens.Space.s12) {
                TokenIconView(mark: mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    Text(verbatim: detail)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: Tokens.Space.s8)
                if let chip { StatusChipView(chip: chip) }
            }
            .padding(Tokens.Space.s12)
            .overlay(resultOutline)
        case .network(let mark, let name, let chip, let facts, let link):
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                HStack(spacing: Tokens.Space.s12) {
                    TokenIconView(mark: mark)
                    Text(verbatim: name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Spacer(minLength: Tokens.Space.s8)
                    StatusChipView(chip: chip)
                }
                if let link {
                    Text(verbatim: link)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .padding(.top, Tokens.Space.s8)
                }
                ForEach(facts) { FactRowView(fact: $0) }
            }
            .padding(Tokens.Space.s12)
            .overlay(resultOutline)
        }
    }

    /// Card outline, in one place so the two result cards cannot drift apart.
    private var resultOutline: some View {
        RoundedRectangle(cornerRadius: Tokens.Radius.r12)
            .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
    }
}

// MARK: - Send

/// SD1 / SD1b — which token (or tokens) to send.
///
/// SD1b is the same list in multi-select. Once the first token is chosen the
/// network is decided, and rows on other chains grey out rather than
/// disappearing — the person still owns them, and a list that silently
/// shortened would read as a bug.
struct SendPickBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: SendPickModel
    var onFilter: (String) -> Void = { _ in }
    var onSelect: (Int) -> Void = { _ in }
    /// **The rows that are on screen**, by index. A search narrows the list,
    /// and "select all valuable" must not sweep holdings a person cannot see.
    var onSelectAll: ([Int]) -> Void = { _ in }
    var onCta: () -> Void = {}

    @State private var query = ""

    private var shown: [(offset: Int, element: AssetRowModel)] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        let all = Array(model.rows.enumerated())
        guard !trimmed.isEmpty else { return all.map { ($0.offset, $0.element) } }
        return all
            .filter { "\($0.element.ticker) \($0.element.chain)".localizedCaseInsensitiveContains(trimmed) }
            .map { ($0.offset, $0.element) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            FlowSearchField(placeholder: model.searchPlaceholder, text: $query)
            FlowFilterChips(options: model.filters, onSelect: onFilter)
            if let notice = model.notice {
                NoticeBannerView(text: notice.text, mark: notice.mark)
            }
            VStack(spacing: Tokens.Space.s0) {
                ForEach(Array(shown.enumerated()), id: \.element.offset) { position, entry in
                    if position > 0 { FlowDivider() }
                    let dimmed = model.selection?.dimmed[safe: entry.offset] ?? false
                    let selected = model.selection?.selected[safe: entry.offset] ?? false
                    Button { onSelect(entry.offset) } label: {
                        AssetRowView(model: entry.element)
                            .padding(.horizontal, selected ? Tokens.Space.s8 : Tokens.Space.s0)
                            .background(
                                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                                    .fill(selected ? theme.bgRaised : .clear)
                            )
                            .opacity(dimmed ? Tokens.Opacity.disabled : 1)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(dimmed)
                }
            }
            if let selection = model.selection {
                Button { onSelectAll(shown.map(\.offset)) } label: {
                    Text(verbatim: selection.selectAll)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            VelaButton(
                title: model.cta.label,
                kind: model.cta.accent ? .primary : .secondary,
                action: onCta
            )
            .padding(.top, Tokens.Space.s4)
        }
    }
}

/// SD2 / SD2b / SD2d — the send form, in its three modes.
///
/// One component, because the three ARE one form: single is a token, an amount
/// and a person; split is the same token to several people; sweep is several
/// tokens to one person. The SPEC sheet makes them mutually exclusive, so they
/// share a mode rather than living in three screens that would each need their
/// own fee row, summary line and CTA.
struct SendFormBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: SendFormModel
    var onPickRecipient: () -> Void = {}
    var onScan: () -> Void = {}
    var onRecipientAction: (RecipientAction) -> Void = { _ in }
    var onRemoveRecipient: (Int) -> Void = { _ in }
    var onFee: () -> Void = {}
    var onDenom: () -> Void = {}
    var onMax: (Int) -> Void = { _ in }
    var onAddRecipient: () -> Void = {}
    var onContinue: () -> Void = {}
    /// The two live fields. Absent everywhere the form is a picture.
    var amountText: Binding<String>?
    var recipientText: Binding<String>?
    /// One binding pair per split row, by the core's row id. Absent in the
    /// gallery, where the rows are a picture of a list.
    var rowText: ((String) -> (address: Binding<String>, amount: Binding<String>))?
    /// The core's live refusal, under the fields.
    var warning: String?
    var ctaDisabled = false
    /// Spec 069: measure the fee again; fold or unfold the speed control; a
    /// one-shot pick. Absent in the gallery.
    var onRefreshFee: (() -> Void)? = nil
    var onToggleSpeed: () -> Void = {}
    var onPickSpeed: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if let token = model.token {
                TokenHeaderCardView(token: token, onMax: { onMax(0) })
            }
            if let summary = model.sweepSummary {
                Text(verbatim: summary)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            ForEach(Array(model.sweepRows.enumerated()), id: \.element.id) { index, row in
                AssetRowView(model: AssetRowModel(
                    ticker: row.symbol,
                    chain: row.balanceLabel,
                    badgeColor: row.mark.badgeColor,
                    balance: row.amount,
                    fiat: .none,
                    masked: false
                ))
                .overlay(alignment: .trailing) {
                    // An empty label means no chip. A sweep moves the MAXIMUM
                    // of every row by definition — the core already computes
                    // each one net of gas — so the live form has nothing for
                    // this control to do, and `tap_max` would move the amount
                    // of whichever single token was selected before the sweep
                    // began. The drawing keeps it; the live screen does not.
                    if !row.max.isEmpty {
                        Button { onMax(index) } label: {
                            Text(verbatim: row.max)
                                .typeRole(Typography.chip.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                                .padding(.horizontal, Tokens.Space.s8)
                                .padding(.vertical, Tokens.Space.s2)
                                .background(Capsule().fill(theme.bgSunken))
                                .contentShape(Capsule())
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, Tokens.Space.s12)
                .background(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised)
                )
            }
            if let amount = model.amount {
                AmountInputView(amount: amount, onDenom: onDenom, text: amountText)
            }
            if let recipient = model.recipient {
                RecipientFieldView(
                    field: recipient, onPick: onPickRecipient, onScan: onScan,
                    text: recipientText
                )
            }
            // The core's sentence, where the person is looking when it becomes
            // true — not in an alert they have to dismiss to get back to it.
            // A split's is said at the foot instead, beside its total.
            if let warning, model.mode != .split {
                Text(verbatim: warning)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.warningBase)
            }
            if let add = model.addRecipient {
                Button(action: onAddRecipient) {
                    // The door from a single send into a split. Quiet on
                    // purpose: most sends have one recipient.
                    Text(verbatim: "+  \(add)")
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            ForEach(Array(model.recipients.enumerated()), id: \.element.id) { index, recipient in
                let live = rowText.map { $0(recipient.rowId) }
                RecipientCardView(
                    recipient: recipient,
                    onRemove: { onRemoveRecipient(index) },
                    address: live?.address,
                    amount: live?.amount
                )
            }
            if !model.recipientActions.isEmpty {
                GhostPillRowView(items: model.recipientActions, onSelect: onRecipientAction)
            }
            if let summary = model.summary {
                SummaryLineView(summary: summary)
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                FeeRowView(fee: model.fee, onOpen: onFee, onRefresh: onRefreshFee)
                if let speed = model.speed {
                    FeeSpeedControlView(speed: speed, onToggle: onToggleSpeed, onPick: onPickSpeed)
                }
            }
            // A split's total, its refusal and Continue travel together: the
            // refusal first, else which row is unfinished and why the button
            // is dark.
            if model.mode == .split, let line = warning ?? model.hint {
                Text(verbatim: line)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(warning != nil ? theme.warningBase : theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VelaButton(title: model.cta, kind: .primary, action: onContinue)
                .disabled(ctaDisabled)
                .opacity(ctaDisabled ? Tokens.Opacity.disabled : 1)
                .padding(.top, Tokens.Space.s4)
        }
    }
}

/// SD2e — choosing who gets the money.
///
/// Scan sits at the top, above the saved people. Most sends go to someone
/// already in the book, but the ones that don't are the ones where a person is
/// holding a phone in one hand and an address in the other — so the escape
/// hatch is the first thing, not the last.
struct ContactPickBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ContactPickModel
    var onScan: () -> Void = {}
    var onGroup: (Int) -> Void = { _ in }
    var onSelect: (Int) -> Void = { _ in }

    @State private var query = ""

    private var shown: [(offset: Int, element: ContactEntryModel)] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        let all = Array(model.contacts.enumerated())
        guard !trimmed.isEmpty else { return all.map { ($0.offset, $0.element) } }
        return all
            .filter { "\($0.element.name) \($0.element.addressDisplay)".localizedCaseInsensitiveContains(trimmed) }
            .map { ($0.offset, $0.element) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            FlowSearchField(placeholder: model.searchPlaceholder, text: $query)
            Button(action: onScan) {
                HStack(spacing: Tokens.Space.s8) {
                    LucideIcon(.qrCode, size: LucideIconSize.flowRowAction)
                        .foregroundStyle(theme.fgSubtle)
                    Text(verbatim: model.scanRow)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Spacer(minLength: Tokens.Space.s8)
                    LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                        .foregroundStyle(theme.fgSubtle)
                }
                .padding(Tokens.Space.s12)
                .background(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if !model.groups.isEmpty && query.isEmpty {
                sectionCaption(model.groupsTitle)
                ForEach(Array(model.groups.enumerated()), id: \.element.id) { index, group in
                    Button { onGroup(index) } label: {
                        HStack(spacing: Tokens.Space.s12) {
                            // Two overlapping discs stand for "several people"
                            // without drawing any of them — a group has no
                            // single face to show.
                            HStack(spacing: -WalletGeometry.pillDotOverlap) {
                                ForEach(Array(group.colors.enumerated()), id: \.offset) { _, color in
                                    Circle()
                                        .fill(color)
                                        .frame(
                                            width: WalletGeometry.rowIcon,
                                            height: WalletGeometry.rowIcon
                                        )
                                }
                            }
                            Text(verbatim: group.name)
                                .typeRole(Typography.rowTitle.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                            Spacer(minLength: Tokens.Space.s8)
                            Text(verbatim: group.count)
                                .typeRole(Typography.rowSub.scaled(textScale))
                                .foregroundStyle(theme.fgSubtle)
                            LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                                .foregroundStyle(theme.fgSubtle)
                        }
                        .frame(minHeight: WalletGeometry.rowMinHeight)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }

            sectionCaption(model.contactsTitle)
            ForEach(shown, id: \.element.id) { entry in
                ContactPickRowView(
                    contact: entry.element,
                    onSelect: { onSelect(entry.offset) }
                )
            }
        }
    }

    private func sectionCaption(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub.scaled(textScale))
            .foregroundStyle(theme.fgSubtle)
            .padding(.top, Tokens.Space.s12)
    }
}

/// SD2f — which coin pays the network fee.
///
/// The hint above the list is doing real work: paying gas in a stablecoin is
/// unusual enough that a person seeing USDC offered as a fee token will wonder
/// whether they are being asked to send it. Saying what the choice is for,
/// once, above the rows, is cheaper than a tooltip on each.
struct FeeTokenBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: FeeTokenPickModel
    var onSelect: (Int) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: model.hint)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
                FeeTokenRowView(
                    row: row,
                    estimateLabel: model.estimateLabel,
                    onSelect: { onSelect(index) }
                )
            }
        }
    }
}

/// SD2c — pasting or importing a list of recipients.
///
/// The screen's real subject is the rate, not the paste box. Someone importing
/// a payroll sheet has amounts in their own currency, and the question that
/// decides whether the transfer is right is what those become in the token.
///
/// Bad rows are marked and skipped, never silently dropped, and the CTA counts
/// only the good ones — a button that says "Import 3" and imports 2 is how
/// someone underpays a contractor.
struct BatchImportBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: BatchImportModel
    var onUnit: (String) -> Void = { _ in }
    var onFile: () -> Void = {}
    var onTemplate: () -> Void = {}
    var onApply: () -> Void = {}
    /// Add to the rows on the form, or replace them — the other of the two.
    var onMerge: () -> Void = {}
    /// The live fields. `nil` renders exactly as drawn, so the gallery and the
    /// screenshot sweep stay pixel-identical.
    var pasteText: Binding<String>?
    var rateText: Binding<String>?
    var onResetRate: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            FlowSegmentedToggle(
                options: [
                    (id: BatchUnit.fiat.rawValue, label: model.unitFiat),
                    (id: BatchUnit.token.rawValue, label: model.unitToken),
                ],
                selectedId: model.unit.rawValue,
                onSelect: onUnit
            )
            if let pasteText {
                // A real field: pasting a list is the whole point of this
                // sheet, and a `Text` cannot be pasted into.
                TextEditor(text: pasteText)
                    .font(Typography.monoAddress.scaled(textScale).font)
                    .foregroundStyle(theme.fgBase)
                    .scrollContentBackground(.hidden)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .frame(minHeight: WalletGeometry.batchPasteHeight)
                    .padding(Tokens.Space.s8)
                    .overlay(
                        RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                            .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
                    )
                    .overlay(alignment: .topLeading) {
                        if pasteText.wrappedValue.isEmpty {
                            Text(verbatim: model.pastePlaceholder)
                                .monoRole(Typography.monoAddress.scaled(textScale))
                                .foregroundStyle(theme.fgSubtle)
                                .padding(Tokens.Space.s12)
                                .allowsHitTesting(false)
                        }
                    }
                    .accessibilityIdentifier("send.batchPaste")
            } else {
                FlowMonoField(value: model.pasteValue, lineLimit: 4)
            }

            HStack(spacing: Tokens.Space.s4) {
                Spacer(minLength: Tokens.Space.s0)
                Button(action: onFile) {
                    HStack(spacing: Tokens.Space.s2) {
                        LucideIcon(.fileText, size: LucideIconSize.checkmark)
                        Text(verbatim: model.importFile)
                            .typeRole(Typography.rowSub.scaled(textScale))
                    }
                    .foregroundStyle(theme.fgMuted)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                Text(verbatim: " · ")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                Button(action: onTemplate) {
                    Text(verbatim: model.template)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                Spacer(minLength: Tokens.Space.s0)
            }

            if model.priced {
            FlowDivider()
            HStack(spacing: Tokens.Space.s4) {
                Text(verbatim: model.rateSection)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                Spacer(minLength: Tokens.Space.s8)
                Text(verbatim: model.rateLabel)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                if let rateText {
                    // Somebody's own rate, when the wallet could not price the
                    // currency — or when they disagree with the price it found.
                    TextField(model.rateValue, text: rateText)
                        .font(Typography.body.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                        .multilineTextAlignment(.trailing)
                        .keyboardType(.decimalPad)
                        .frame(maxWidth: WalletGeometry.splitAmountWidth)
                        .accessibilityIdentifier("send.batchRate")
                } else {
                    Text(verbatim: model.rateValue)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                }
                if model.rateEdited, !model.rateReset.isEmpty {
                    Button(action: onResetRate) {
                        Text(verbatim: model.rateReset)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.accentBase)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                } else {
                    LucideIcon(.pencil, size: LucideIconSize.checkmark)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            .padding(.vertical, Tokens.Space.s8)
            Text(verbatim: model.rateHint)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .fixedSize(horizontal: false, vertical: true)
            }

            Text(verbatim: model.parsedLabel)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .padding(.top, Tokens.Space.s8)
            ForEach(model.rows) { row in
                HStack(spacing: Tokens.Space.s8) {
                    LucideIcon(row.ok ? .check : .close, size: LucideIconSize.checkmark)
                        .foregroundStyle(row.ok ? theme.successBase : theme.errorBase)
                    Text(verbatim: row.address)
                        .monoRole(Typography.monoAddress.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    Spacer(minLength: Tokens.Space.s8)
                    Text(verbatim: row.conversion)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
                .padding(.vertical, Tokens.Space.s8)
            }
            if let rejected = model.rejectedText {
                Text(verbatim: rejected)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
            }
            if let note = model.note {
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(model.noteIsError ? theme.errorBase : theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }

            if let total = model.total {
                // What the import comes to, and what it is paid from — the
                // number somebody checks before they press the button.
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    HStack(spacing: Tokens.Space.s8) {
                        Text(verbatim: total.label)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                        Spacer(minLength: Tokens.Space.s8)
                        Text(verbatim: total.value)
                            .typeRole(Typography.rowTitle.scaled(textScale))
                            .foregroundStyle(total.over ? theme.errorBase : theme.fgBase)
                    }
                    HStack(spacing: Tokens.Space.s8) {
                        Text(verbatim: total.balance)
                            .typeRole(Typography.rowSub.scaled(textScale))
                        Spacer(minLength: Tokens.Space.s8)
                        if let detail = total.detail {
                            Text(verbatim: detail)
                                .typeRole(Typography.rowSub.scaled(textScale))
                        }
                    }
                    .foregroundStyle(theme.fgSubtle)
                }
                .padding(.top, Tokens.Space.s8)
            }
            if let merge = model.merge {
                // Adding is the default; replacing is one tap away and says so.
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: merge.note)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                    Button(action: onMerge) {
                        Text(verbatim: merge.action)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.accentBase)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("send.batchMerge")
                }
            }

            VelaButton(
                title: model.cta,
                kind: .primary,
                enabled: !model.ctaDisabled,
                action: onApply
            )
            .padding(.top, Tokens.Space.s8)
        }
    }
}

/// SD3 — the last screen before the money moves.
///
/// Two blocks and nothing else: what is being sent, and the four facts that
/// decide whether that is right. A split or a sweep adds a second card listing
/// the parts. Per the SPEC sheet this is the ONE accent CTA in the whole send
/// journey — every other button on the way here is an outline.
struct SendConfirmBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: SendConfirmModel
    var onConfirm: () -> Void = {}
    var onNoticeAction: () -> Void = {}
    var onNoticeSecondary: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            VStack(spacing: Tokens.Space.s2) {
                if let mark = model.mark {
                    TokenIconView(mark: mark)
                    .padding(.bottom, Tokens.Space.s8)
                }
                Text(verbatim: model.amount)
                    .typeRole(Typography.display.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                    .lineLimit(1)
                Text(verbatim: model.subline)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Tokens.Space.s16)

            VStack(spacing: Tokens.Space.s0) {
                ForEach(Array(model.facts.enumerated()), id: \.element.id) { index, fact in
                    if index > 0 { FlowDivider() }
                    FactRowView(fact: fact)
                }
            }
            .padding(.horizontal, Tokens.Space.s12)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))

            if let notice = model.notice {
                VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                    NoticeBannerView(text: notice)
                    if model.noticeAction != nil || model.noticeSecondary != nil {
                        HStack(spacing: Tokens.Space.s8) {
                            if let secondary = model.noticeSecondary {
                                Button(action: onNoticeSecondary) {
                                    Text(verbatim: secondary)
                                        .typeRole(Typography.rowSub.scaled(textScale))
                                        .foregroundStyle(theme.fgMuted)
                                        .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                            }
                            Spacer(minLength: Tokens.Space.s8)
                            if let action = model.noticeAction {
                                Button(action: onNoticeAction) {
                                    Text(verbatim: action)
                                        .typeRole(Typography.rowSub.scaled(textScale))
                                        .foregroundStyle(theme.accentBase)
                                        .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
            }

            if let note = model.repeatNote {
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.warningBase)
                    .fixedSize(horizontal: false, vertical: true)
            }

            if !model.breakdown.isEmpty {
                VStack(spacing: Tokens.Space.s0) {
                    ForEach(model.breakdown) { item in
                        HStack(spacing: Tokens.Space.s8) {
                            if let lead = item.lead { InlineTokenMark(mark: lead) }
                            if let seed = item.identiconSeed {
                                IdenticonAvatar(seed: seed, size: WalletFlowGeometry.inlineMark)
                            }
                            Text(verbatim: item.label)
                                .typeRole(Typography.body.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                                .lineLimit(1)
                            Spacer(minLength: Tokens.Space.s8)
                            Text(verbatim: item.value)
                                .typeRole(Typography.body.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                        }
                        .padding(.vertical, Tokens.Space.s8)
                    }
                }
                .padding(.horizontal, Tokens.Space.s12)
                .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
            }
        }
    }
}

/// SD4 — the receipt, in whichever state it is in.
///
/// The SPEC sheet calls these "三态" and means it: submitting, submitted,
/// confirmed. One screen that changes, not three that replace each other —
/// which is why the disc, the title and the button keep their positions and
/// only their contents move.
///
/// "Close · keep running" is load-bearing copy. The transaction does not
/// depend on this screen staying open, and a person who thinks it does will
/// sit here watching a spinner.
struct SendReceiptBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: SendReceiptModel
    var onExplorer: () -> Void = {}

    @State private var copied = false

    var body: some View {
        VStack(spacing: Tokens.Space.s8) {
            // Spec 038 #D3 / issue 199: while the relay has the op, the screen
            // counts — one second is the right grain. The sentences arrive in
            // the model; only the number is this screen's.
            if let eta = model.eta, model.stage == .submitted {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    let nowMs = context.date.timeIntervalSince1970 * 1000
                    StatusHeroView(stage: model.stage, title: model.title,
                                   captions: model.captions + eta.lines(nowMs: nowMs),
                                   progress: eta.progress(nowMs: nowMs))
                }
            } else {
                StatusHeroView(stage: model.stage, title: model.title, captions: model.captions,
                               progress: model.stage == .confirmed ? 1 : nil)
            }
            if !model.breakdown.isEmpty {
                ReceiptBreakdownView(title: model.breakdownTitle, rows: model.breakdown)
                    .padding(.bottom, Tokens.Space.s8)
            }
            if let hash = model.hash {
                HStack(spacing: Tokens.Space.s4) {
                    Text(verbatim: hash.label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    Text(verbatim: hash.value)
                        .monoRole(Typography.monoAddress.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Button { copied = true } label: {
                        LucideIcon(copied ? .check : .copy, size: LucideIconSize.checkmark)
                            .foregroundStyle(copied ? theme.successBase : theme.fgSubtle)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(hash.copyLabel)
                }
            }
            if let explorer = model.viewOnExplorer {
                VelaButton(title: explorer, kind: .secondary, action: onExplorer)
            }
        }
    }
}

/// A split's people on the receipt (#261): who got what, as the confirm
/// listed them — from the core's frozen transfers.
private struct ReceiptBreakdownView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String?
    let rows: [BreakdownRowModel]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            if let title {
                Text(verbatim: title)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            VStack(spacing: Tokens.Space.s0) {
                ForEach(rows) { row in
                    HStack(spacing: Tokens.Space.s8) {
                        if let seed = row.identiconSeed {
                            IdenticonAvatar(seed: seed, size: WalletFlowGeometry.inlineMark)
                        }
                        Text(verbatim: row.label)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .lineLimit(1)
                        Spacer(minLength: Tokens.Space.s8)
                        Text(verbatim: row.value)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                    }
                    .padding(.vertical, Tokens.Space.s8)
                }
            }
            .padding(.horizontal, Tokens.Space.s12)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Shared

extension Array {
    /// Bounds-safe read for the selection masks, which are sized to the
    /// unfiltered row list while the view may be showing a filtered slice.
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
