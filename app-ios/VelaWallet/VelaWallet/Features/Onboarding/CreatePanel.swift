//
//  CreatePanel.swift
//  VelaWallet
//
//  The five screens of the v2 create journey, and the shell that frames them.
//
//  Spec 014 put this flow in a bottom sheet. The v2 design makes it the whole
//  screen at every step and keeps a sheet for FAILURES only, where an
//  interruption genuinely is modal — a form someone is halfway through is not an
//  interruption, and putting it behind a scrim said it was.
//
//  Nothing here holds flow state. Every screen renders `CreateView` and sends
//  events back; the whole mapping from view to screen is `screenFor`.
//

import SwiftUI

// MARK: - The shell

/// A back affordance, and nothing else: every screen inside decides its own
/// content.
///
/// The three-segment bar and the flow's name that used to sit here are gone
/// (founder call, 2026-08-25): a meter over a journey whose every screen
/// already says what it is measured decoration rather than progress, and the
/// label repeated the heading directly under it.
struct FlowShell<Content: View>: View {
    @Environment(\.theme) private var theme
    let backLabel: String
    let canGoBack: Bool
    let onBack: () -> Void
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The row keeps its height with or without the affordance, so the
            // screen below never jumps when back disappears.
            HStack {
                if canGoBack {
                    Button(action: onBack) {
                        HStack(spacing: Tokens.Space.s4) {
                            Image(systemName: "chevron.left")
                            Text(backLabel).typeRole(Typography.body)
                        }
                        .foregroundStyle(theme.fgMuted)
                    }
                    .accessibilityLabel(backLabel)
                }
                Spacer()
            }
            .frame(minHeight: Tokens.Layout.hitTarget)

            content
                .padding(.top, Tokens.Space.s24)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(theme.bgBase.ignoresSafeArea())
    }
}

// MARK: - Name

/// Name the wallet, and accept the three gates.
///
/// Three checkboxes, matching the core's `ACK_COUNT`, and every one of them a
/// FACT ABOUT WHERE SOMETHING ENDS UP: the public key and the name go into the
/// on-chain contract, the private key stays in the device's password manager or
/// on a security key, and the legal assent. Together they are the whole custody
/// story, and **none arrives pre-ticked** — a box that is already ticked records
/// nothing about what the person read.
///
/// The recovery assurance that used to sit between them is gone. It described a
/// BENEFIT, and mixing one of those into a list of consequences teaches people
/// to skim the list.
///
/// The field has no label and no helper line: the heading directly above it
/// already says "name your wallet", and what the helper said (the name is
/// stored on-chain) is now `ack0`, where a person has to look at it.
///
/// The gates sit at the BOTTOM, against the button they gate, and OUTSIDE the
/// ScrollView — a `Spacer` inside one does not expand, so a checklist left in
/// the scroll lands in the middle of the screen. The field above scrolls
/// instead: it is one row, and the gates are the part that has to be seen.
struct NameScreen: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let view: CreateView
    let statusText: String?
    @Binding var name: String
    let onToggleAck: (Int) -> Void
    let onSubmit: () -> Void
    let onStartOver: () -> Void
    let onLink: (ActionId) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s24) {
                    Text(loc.t(I18nKeys.Create.nameTitle))
                        .typeRole(Typography.display)
                        .foregroundStyle(theme.fgBase)

                    NameField(
                        label: "",
                        placeholder: loc.t(I18nKeys.Create.accountNamePlaceholder),
                        helper: "",
                        tooLongText: loc.t(I18nKeys.Create.nameTooLong),
                        text: $name,
                        tooLong: view.nameTooLong
                    )
                    .disabled(!view.nameEditable)
                }
                .padding(.bottom, Tokens.Space.s32)
            }
            .scrollBounceBehavior(.basedOnSize)

            VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                AckRow(
                    segments: [AckSegment(text: loc.t(I18nKeys.Create.ack0))],
                    checked: binding(for: 0)
                )

                AckRow(
                    segments: [AckSegment(text: loc.t(I18nKeys.Create.ack1))],
                    checked: binding(for: 1)
                )

                // Unlike Android, iOS can put the two links INSIDE the consent
                // sentence: `AckRow` disables row-wide toggling when a row has
                // links, so the link tap and the checkbox tap do not compete.
                // Android's row is one touch target and needs its links on a
                // line of their own.
                AckRow(
                    segments: [
                        AckSegment(text: loc.t(I18nKeys.Create.ack2)),
                        AckSegment(
                            text: loc.t(I18nKeys.Create.ack2PrivacyPolicy),
                            action: .openPrivacyPolicy
                        ),
                        AckSegment(text: loc.t(I18nKeys.Create.ack2And)),
                        AckSegment(text: loc.t(I18nKeys.Create.ack2Terms), action: .openTerms),
                        AckSegment(text: loc.t(I18nKeys.Create.ack2Period)),
                    ],
                    checked: binding(for: 2),
                    onLink: onLink
                )

                if let statusText {
                    Text(statusText)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                        .accessibilityAddTraits(.updatesFrequently)
                }
            }
            .padding(.bottom, Tokens.Space.s24)

            VelaButton(
                title: loc.t(submitLabelToI18n(view.submitLabel)),
                kind: .primary,
                enabled: view.canSubmit,
                loading: view.busy,
                action: onSubmit
            )

            if view.showStartOver {
                Button(action: onStartOver) {
                    Text(loc.t(I18nKeys.Create.startOverBtn))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                }
                .frame(maxWidth: .infinity)
                .padding(.top, Tokens.Space.s12)
            }
        }
        .padding(.bottom, Tokens.Space.s16)
    }

    /// The core owns the ack state; this binding only reports a tap.
    ///
    /// A local `@State` mirror would drift the instant the core refused a
    /// toggle — and refusing one is exactly what `StartOver` does.
    private func binding(for index: Int) -> Binding<Bool> {
        Binding(
            get: { view.acks.indices.contains(index) ? view.acks[index] : false },
            set: { _ in onToggleAck(index) }
        )
    }
}

// MARK: - Keys

/// The founding key list — the screen spec 014 never had, and the only place a
/// multi-key wallet can be assembled.
///
/// Everything on it is a rendering of `CreateView`; nothing here decides. The
/// three gates the core enforces (at most seven keys, every key confirmed, a
/// sole key must be backed up) surface as a disabled control with a stated
/// reason rather than as a tap that quietly does nothing.
struct KeysScreen: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let view: CreateView
    let onAddKey: (KeyMethod) -> Void
    let onConfirmKey: (Int) -> Void
    let onRemoveKey: (Int) -> Void
    let onFinish: () -> Void
    /// Spec 102, "Use a trusted signing page": Vela's own sheet and every page
    /// this device trusts, the chosen one marked. Empty hides the entry.
    var pageChoices: [SigningPageChoiceModel] = []
    /// A page was chosen (`nil`: Vela's own) — `signing_page_chosen`.
    var onChoosePage: ((String?) -> Void)?
    /// "Add a page" in the picker, and the core's refusal of the last one.
    var onAddPage: ((String) -> Void)?
    var pageAddError: String?
    /// The picker is on screen: check the pages it lists.
    var onPagesShown: () -> Void = {}

    @State private var pickerOpen = false
    @State private var pagePickerOpen = false

    /// The chosen page, as the picker draws it.
    private var chosenPage: SigningPageChoiceModel? {
        guard let page = view.signingPage else { return nil }
        return pageChoices.first { $0.url.map(SignerPageChecks.key) == SignerPageChecks.key(page) }
            ?? SigningPageChoiceModel(
                url: page, title: SigningPageNames.host(page), subtitle: SigningPageNames.host(page),
                domainLine: SigningPageNames.keysOnLine(url: page, domain: view.signingDomain, loc: loc),
                line: nil, selected: true
            )
    }

    /// The entry is offered only before the first key, and only where there
    /// is somewhere to send the choice.
    private var offersSigningPage: Bool {
        view.canChoosePage && view.signingPage == nil && onChoosePage != nil && !pageChoices.isEmpty
    }

    private var full: Bool { view.keys.count >= maxKeys }

    /// The heading over the three places, chosen by the core from the count
    /// (issue #475): "Add a passkey" with none, "Add another" after the
    /// first, "Limit of 7 reached" when full. An absent key (a core from
    /// before #475) keeps the old label.
    private var addHeading: String {
        loc.t(view.addHeadingKey.isEmpty ? I18nKeys.Create.addMethodLabel : view.addHeadingKey)
    }

    /// The three places are on screen: pinned open by the core while there is
    /// no key yet (the list is the only way forward), else unfolded by hand.
    private var methodsShown: Bool {
        view.methodsPinned || (pickerOpen && view.canAddKey)
    }

    /// The add section in its three states.
    ///
    /// - **Pinned** (no key yet): a plain heading over the open list.
    /// - **Foldable** (a key or more, room for another): the heading IS the
    ///   disclosure row; a tap unfolds the list under it.
    /// - **Closed** (the set is full, or a ceremony is running): the heading
    ///   alone, dimmed — nothing to tap, and it says why.
    @ViewBuilder private var addSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            if view.methodsPinned {
                Text(addHeading)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgMuted)
                    .padding(.vertical, Tokens.Space.s8)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("create.addHeading")
            } else {
                Button {
                    pickerOpen.toggle()
                } label: {
                    HStack(spacing: Tokens.Space.s12) {
                        if view.canAddKey {
                            LucideIcon(.plus, size: LucideIconSize.ghostPlus)
                        }
                        Text(addHeading)
                            .typeRole(Typography.body)
                        Spacer(minLength: Tokens.Space.s8)
                        if view.canAddKey {
                            LucideIcon(.chevronDown, size: LucideIconSize.rowGlyph)
                                .rotationEffect(.degrees(pickerOpen ? 180 : 0))
                                .foregroundStyle(theme.fgSubtle)
                        }
                    }
                    .foregroundStyle(view.canAddKey ? theme.accentBase : theme.fgMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .frame(minHeight: KeyMethodRows.rowHeight)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(!view.canAddKey)
                .opacity(view.canAddKey ? 1 : Tokens.Opacity.disabled)
                .accessibilityIdentifier("create.addHeading")
                if methodsShown { SettingsDivider() }
            }

            if methodsShown {
                KeyMethodRows(loc: loc, chooser: .create, allowed: view.addMethods) { method in
                    pickerOpen = false
                    onAddKey(method)
                }
                // Not a fourth place: where this wallet reviews and signs.
                // Offered before the first key only — the first key commits
                // the set to one domain — and set apart from the three
                // places by a section gap and a hairline.
                if offersSigningPage {
                    SettingsDivider().padding(.top, Tokens.Space.s16)
                    SigningPageEntry(loc: loc) { pagePickerOpen = true }
                }
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                    Text(loc.t(view.needsSecondKey
                        ? I18nKeys.Create.keysTitleBlocked
                        : I18nKeys.Create.keysTitle))
                        .typeRole(Typography.display)
                        .foregroundStyle(theme.fgBase)

                    Text(loc.t(subtitleKey))
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgMuted)

                    // Spec 102: where this wallet will review and sign, when
                    // a page was chosen — and the domain its keys belong to.
                    if let chosen = chosenPage {
                        ChosenSigningPageCard(
                            loc: loc, choice: chosen,
                            onChange: view.canChoosePage && onChoosePage != nil ? { pagePickerOpen = true } : nil
                        )
                    }

                    if view.needsSecondKey {
                        HStack(alignment: .top, spacing: Tokens.Space.s12) {
                            Image(systemName: "exclamationmark.triangle")
                                .foregroundStyle(theme.accentBase)
                            Text(loc.t(I18nKeys.Create.needSecondKeyHint))
                                .typeRole(Typography.flowCaption)
                                .foregroundStyle(theme.fgBase)
                        }
                        .padding(Tokens.Space.s12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(theme.accentSoft, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                    }

                    // "Added 0 / 7" over an empty list said nothing the title
                    // had not (issue #475: the screen was wordy); the count and
                    // its list appear with the first key.
                    if !view.keys.isEmpty {
                        HStack {
                            Text(loc.t(I18nKeys.Create.keysLabel))
                                .typeRole(Typography.label)
                                .foregroundStyle(theme.fgMuted)
                            Spacer()
                            // Mono: it is a count, and a count that jitters in width
                            // as it changes reads as the layout moving rather than
                            // the number.
                            Text(loc.t(
                                I18nKeys.Create.keyCount,
                                vars: ["current": "\(view.keys.count)", "max": "\(maxKeys)"]
                            ))
                            .typeRole(Typography.monoSmall)
                            .foregroundStyle(theme.fgMuted)
                        }
                        .padding(.top, Tokens.Space.s8)

                        VStack(spacing: 0) {
                            ForEach(Array(view.keys.enumerated()), id: \.offset) { index, key in
                                if index > 0 {
                                    Divider().overlay(theme.borderBase)
                                }
                                KeyRow(
                                    loc: loc,
                                    key: key,
                                    busy: view.busy,
                                    // Row 0 is the pinned key: not removable, and
                                    // its name IS the wallet name. Removing it is
                                    // `start over`, not a row action.
                                    removable: index > 0,
                                    onConfirm: { onConfirmKey(index) },
                                    onRemove: { onRemoveKey(index) }
                                )
                            }
                            Divider().overlay(theme.borderBase)
                        }
                    }

                    // Issue #475: ONE way to add — the three places under the
                    // core's heading. The separate "+ Add a passkey" row said
                    // the same thing twice.
                    addSection

                    Text(loc.t(I18nKeys.Create.keysHint))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                        .padding(.top, Tokens.Space.s16)
                }
                .padding(.bottom, Tokens.Space.s24)
            }
            .scrollBounceBehavior(.basedOnSize)

            VelaButton(
                title: loc.t(view.needsSecondKey
                    ? I18nKeys.Create.addSecondKeyBtn
                    : I18nKeys.Create.createWalletBtn),
                kind: .primary,
                enabled: view.canFinish,
                loading: view.busy,
                action: onFinish
            )
        }
        .padding(.bottom, Tokens.Space.s16)
        .sheet(isPresented: $pagePickerOpen) {
            SigningPagePicker(
                loc: loc,
                choices: pageChoices,
                onPick: { url in
                    pagePickerOpen = false
                    onChoosePage?(url)
                },
                onAdd: onAddPage,
                addError: pageAddError,
                onClose: { pagePickerOpen = false }
            )
            .task { onPagesShown() }
            .presentationDetents([.large])
            .presentationDragIndicator(.visible)
        }
        .task(id: view.signingPage) { if view.signingPage != nil { onPagesShown() } }
    }

    private var subtitleKey: String {
        if view.needsSecondKey { return I18nKeys.Create.keysSubtitleBlocked }
        return full ? I18nKeys.Create.keysSubtitleFull : I18nKeys.Create.keysSubtitle
    }
}

private struct KeyRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var colorScheme
    let loc: Loc
    let key: CreateKeyRow
    let busy: Bool
    let removable: Bool
    let onConfirm: () -> Void
    let onRemove: () -> Void

    /// Who is holding this key: the compiled catalog's name, then the
    /// directory's for a model no catalog carries, and nothing when neither
    /// knows — the row then says what it always said about the METHOD.
    private var holder: String? {
        if !key.providerName.isEmpty { return key.providerName }
        guard !key.aaguid.isEmpty else { return nil }
        return PasskeyDirectory.shared
            .entry(aaguid: key.aaguid, dark: colorScheme == .dark)?
            .name
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            // Who is holding this key, when the core's AAGUID catalog knows:
            // the vault's own mark and its own name. When it does not — a
            // hardware key, an authenticator that reported nothing — the row
            // says where the key LIVES, from the authenticator's report (#207).
            PasskeyProviderMark(
                key: key,
                label: holder ?? loc.t(providerLineFor(key.kind)),
                glyphFallback: true
            )

            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(key.name).typeRole(Typography.rowTitle).foregroundStyle(theme.fgBase)
                Text(holder ?? loc.t(providerLineFor(key.kind)))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgMuted)
            }
            Spacer()

            // One trailing slot, as the design draws it. A key that has not
            // confirmed its membership has no status to show yet, so the retry
            // TAKES that slot rather than crowding in beside it. A key whose
            // attestation nobody could read has no badge either: the slot stays
            // empty rather than claiming a backup (#207).
            if key.confirmed {
                if let badge = keyBadge(key) {
                    Text(loc.t(badge.text))
                        .typeRole(Typography.label)
                        .foregroundStyle(badge.synced ? theme.successBase : theme.fgMuted)
                        .padding(.horizontal, Tokens.Space.s8)
                        .padding(.vertical, Tokens.Space.s4)
                        .background(
                            badge.synced ? theme.successSoft : theme.bgSunken,
                            in: Capsule()
                        )
                }
            } else {
                Button(action: onConfirm) {
                    Text(loc.t(I18nKeys.Create.confirmKeyBtn))
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.accentBase)
                }
                .disabled(busy)
            }

            if removable {
                Button(action: onRemove) {
                    Image(systemName: "xmark").foregroundStyle(theme.fgSubtle)
                }
                .disabled(busy)
                .accessibilityLabel(loc.t(I18nKeys.Create.removeKeyBtn))
            }
        }
        .padding(.vertical, Tokens.Space.s12)
    }
}

/// The three places a key can live — this device, a phone or tablet by scan,
/// a USB security key (spec 102: three, no fourth) — as the create screen and
/// the sign-in sheet both list them.
///
/// Unlike the browser, this client OWNS the picker, so the person's selection
/// here is honoured at the ceremony rather than merely recorded.
///
/// Issue #475: Settings metrics. A hairline between rows, 52pt at least, 4pt
/// between a title and the line under it, and that line ONE line — the rows
/// were 44pt with nothing between them, and a wrapped line nearly touched the
/// next row's title, so it was hard to tell whose it was. The line shrinks a
/// little before it is ever cut (fr / de / it / es run to 52–55 characters on
/// a 375pt phone); at the accessibility text sizes it wraps instead.
struct KeyMethodRows: View {
    /// A Settings row's height (`SettingsRow`).
    static let rowHeight: CGFloat = 52

    @Environment(\.theme) private var theme
    @Environment(\.dynamicTypeSize) private var typeSize
    let loc: Loc
    let chooser: KeyChooser
    /// The places a key may be picked from; the rest draw dimmed.
    var allowed: [KeyMethod] = KeyMethod.allCases
    let onPick: (KeyMethod) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(KeyMethod.allCases.enumerated()), id: \.element) { index, method in
                let available = allowed.contains(method)
                let copy = methodCopy(method, chooser: chooser, loc: loc)
                if index > 0 { SettingsDivider() }
                Button { onPick(method) } label: {
                    HStack(spacing: Tokens.Space.s12) {
                        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                            Text(copy.title)
                                .typeRole(Typography.rowTitle)
                                .foregroundStyle(theme.fgBase)
                                .lineLimit(2)
                                .multilineTextAlignment(.leading)
                            if !copy.body.isEmpty {
                                Text(copy.body)
                                    .typeRole(Typography.flowCaption)
                                    .foregroundStyle(theme.fgMuted)
                                    .multilineTextAlignment(.leading)
                                    .oneLineSubtitle(typeSize)
                            }
                        }
                        Spacer(minLength: Tokens.Space.s8)
                        if available {
                            LucideIcon(.chevronRight, size: LucideIconSize.rowGlyph)
                                .foregroundStyle(theme.fgSubtle)
                        }
                    }
                    .frame(minHeight: Self.rowHeight)
                    .padding(.vertical, Tokens.Space.s4)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(!available)
                .opacity(available ? 1 : Tokens.Opacity.disabled)
            }
        }
    }
}

extension View {
    /// A row's second line, held to ONE line (issue #475): it shrinks to 80%
    /// before it is cut, and at the accessibility text sizes — where one line
    /// of a sentence is a few words — it wraps instead.
    @ViewBuilder func oneLineSubtitle(_ typeSize: DynamicTypeSize) -> some View {
        if typeSize.isAccessibilitySize {
            fixedSize(horizontal: false, vertical: true)
        } else {
            lineLimit(1).minimumScaleFactor(0.8)
        }
    }
}

// MARK: - Progress

/// Deriving the address.
///
/// Three task rows, driven by the stage the core reported — never by elapsed
/// time. This is why spec 014's elapsed-seconds ring is gone from the create
/// flow.
///
/// The percentage meter that used to head them is gone too (founder call,
/// 2026-08-25, and the desktop had already reached the same conclusion): it
/// LOOKED measured and was not — the same three statuses the rows below name,
/// divided by three — and its label named one phase while another was running.
/// What is left is honest: what finished, what is running, what has not
/// started, with the running one spinning because it is waiting on a network.
struct ProgressScreen: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let position: ProgressPosition
    let keyCount: Int

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s32) {
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                Text(loc.t(I18nKeys.Create.progressTitle))
                    .typeRole(Typography.display)
                    .foregroundStyle(theme.fgBase)
                Text(loc.t(I18nKeys.Create.progressSubtitle, vars: ["count": "\(keyCount)"]))
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
            }

            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(progressTasks.enumerated()), id: \.offset) { index, task in
                    let done = index < position.activeTask
                    let active = index == position.activeTask
                    HStack(spacing: Tokens.Space.s12) {
                        Group {
                            if done {
                                Image(systemName: "checkmark").foregroundStyle(theme.successBase)
                            } else if active {
                                // A spinner, not a dot: this row is waiting on
                                // a network round trip, and a still dot beside
                                // "writing the key index" says nothing about
                                // whether anything is happening (founder call,
                                // 2026-08-25).
                                ProgressView()
                                    .progressViewStyle(.circular)
                                    .controlSize(.small)
                                    .tint(theme.accentBase)
                            } else {
                                Circle()
                                    .fill(theme.borderStrong)
                                    .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                            }
                        }
                        .frame(width: Tokens.Space.s20)
                        // The row before the live one is finished, the row after
                        // has not started. Neither is emphasised: only what is
                        // happening right now is.
                        Text(loc.t(task))
                            .typeRole(active ? Typography.rowTitle : Typography.body)
                            .foregroundStyle(active ? theme.fgBase : theme.fgMuted)
                        Spacer()
                    }
                    .frame(minHeight: Tokens.Layout.hitTarget)
                }
            }
            Spacer()
        }
    }
}

/// The done card's address: the WHOLE line is the copy target, and the
/// confirmation replaces it in place.
///
/// Not `AddressStrip`: the v2 card draws the address as bare text under a
/// rule, because it is the only 0x string on the screen and a sunken well
/// around it made the card look like a form.
private struct DoneAddressLine: View {
    @Environment(\.theme) private var theme
    let address: String
    let copyLabel: String
    let copiedLabel: String

    @State private var copied = false

    var body: some View {
        Button(action: copy) {
            Text(copied ? copiedLabel : address)
                .typeRole(Typography.mono)
                .foregroundStyle(copied ? theme.successBase : theme.fgMuted)
                .multilineTextAlignment(.leading)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.top, Tokens.Space.s12)
                .overlay(alignment: .top) {
                    Rectangle()
                        .fill(theme.borderBase)
                        .frame(height: Tokens.BorderWidth.hairline)
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .animation(.easeOut(duration: Tokens.Motion.fast), value: copied)
        .accessibilityLabel(copyLabel)
        .accessibilityValue(Text(verbatim: address))
    }

    private func copy() {
        velaCopy(address)
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            copied = false
        }
    }
}

// MARK: - Retry

/// The keys were minted; the group never landed.
///
/// Nothing is lost and nothing is re-minted. The core keeps the whole founding
/// set and a pending record it wrote BEFORE the first publish attempt, so a
/// retry resumes at the publish — which is why this screen offers a retry rather
/// than starting over, and why "start over" is the quiet secondary.
struct RetryScreen: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    /// The publish's own error, forwarded verbatim — it goes into the bug
    /// report, so prettifying it here would lose the only detail worth filing.
    let detail: String?
    let busy: Bool
    let onRetry: () -> Void
    let onStartOver: () -> Void
    let onEditEndpoint: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                    Text(loc.t(I18nKeys.Create.syncFailedTitle))
                        .typeRole(Typography.display)
                        .foregroundStyle(theme.fgBase)
                    Text(loc.t(I18nKeys.Create.syncFailedMessage))
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgMuted)
                    Text(loc.t(I18nKeys.Create.syncFailedHint))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)

                    if let detail {
                        TechDetailsDisclosure(
                            label: loc.t(I18nKeys.Create.technicalDetails),
                            details: TechDetails(
                                code: detail,
                                context: loc.t(I18nKeys.Create.statusSyncingKey)
                            ),
                            onToggle: { _ in }
                        )
                        .padding(.top, Tokens.Space.s16)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.bottom, Tokens.Space.s24)
            }
            .scrollBounceBehavior(.basedOnSize)

            VStack(spacing: Tokens.Space.s12) {
                VelaButton(
                    title: loc.t(I18nKeys.Create.retryUploadBtn),
                    kind: .primary,
                    loading: busy,
                    action: onRetry
                )
                VelaButton(
                    title: loc.t(I18nKeys.Flow.editIndexEndpoint),
                    kind: .secondary,
                    enabled: !busy,
                    action: onEditEndpoint
                )
                VelaButton(
                    title: loc.t(I18nKeys.Create.startOverBtn),
                    kind: .secondary,
                    enabled: !busy,
                    action: onStartOver
                )
            }
        }
        .padding(.bottom, Tokens.Space.s16)
    }
}

// MARK: - Done

/// The wallet exists.
///
/// This is the first moment an address is shown, and that ordering is a rule
/// rather than a layout choice: the core withholds `address` until the group has
/// landed and the account is saved, because an address shown earlier is one
/// somebody can fund before the wallet is reachable.
struct DoneScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var colorScheme
    let loc: Loc
    let address: String
    let walletName: String
    let keys: [CreateKeyRow]
    let onEnter: () -> Void

    /// Who is holding this key: the compiled catalog first, the directory
    /// second, nothing when neither knows.
    private func doneHolder(_ key: CreateKeyRow) -> String? {
        if !key.providerName.isEmpty { return key.providerName }
        guard !key.aaguid.isEmpty else { return nil }
        return PasskeyDirectory.shared
            .entry(aaguid: key.aaguid, dark: colorScheme == .dark)?
            .name
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s24) {
                    VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                        HStack(spacing: Tokens.Space.s12) {
                            // The tick is a badge, not a glyph beside the words:
                            // this is the one screen in the flow that reports an
                            // outcome, and the design gives it a disc.
                            Text(verbatim: "✓")
                                .typeRole(Typography.title)
                                .foregroundStyle(theme.successBase)
                                .frame(width: FlowMetrics.doneCheck, height: FlowMetrics.doneCheck)
                                .background(theme.successSoft, in: Circle())
                            Text(loc.t(I18nKeys.Create.successTitle))
                                .typeRole(Typography.display)
                                .foregroundStyle(theme.fgBase)
                        }
                        // Plural (issue #409): "Your key can sign in on its
                        // own" for one key, "Any of your 2 keys…" for several.
                        Text(loc.t(I18nKeys.Create.successMessage, count: keys.count))
                            .typeRole(Typography.body)
                            .foregroundStyle(theme.fgMuted)
                    }

                    // Avatar beside the name, then the address under a rule.
                    // The caption that used to sit here DESCRIBED the identicon
                    // ("an identity pattern generated from the address") — a
                    // sentence narrating a picture that is right next to it —
                    // and the "wallet address" label went for the same reason:
                    // a 42-character 0x string in mono under a wallet's name is
                    // not mistakable for anything else.
                    VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                        HStack(spacing: Tokens.Space.s12) {
                            // Rendered from the address by the same core that
                            // derived it, so what the person memorises here is
                            // what every other client draws.
                            IdenticonAvatar(seed: address, size: FlowMetrics.identicon)
                            Text(walletName)
                                .typeRole(Typography.title)
                                .foregroundStyle(theme.fgBase)
                        }

                        DoneAddressLine(
                            address: address,
                            copyLabel: loc.t(I18nKeys.Flow.copyAddress),
                            copiedLabel: loc.t(I18nKeys.Flow.copied)
                        )
                    }
                    .padding(Tokens.Space.s16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(theme.bgRaised, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))

                    VStack(spacing: 0) {
                        ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
                            HStack(spacing: Tokens.Space.s12) {
                                PasskeyProviderMark(
                                    key: key,
                                    label: doneHolder(key) ?? "",
                                    size: Tokens.Space.s20
                                )
                                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                                    Text(key.name)
                                        .typeRole(Typography.body)
                                        .foregroundStyle(theme.fgBase)
                                    // Where the key lives, under the name it was
                                    // given. Quieter than the name: one is the
                                    // person's word, the other the system's.
                                    if let holder = doneHolder(key) {
                                        Text(holder)
                                            .typeRole(Typography.flowCaption)
                                            .foregroundStyle(theme.fgSubtle)
                                    }
                                }
                                Spacer()
                                if let badge = keyBadge(key) {
                                    Text(loc.t(badge.text))
                                        .typeRole(Typography.label)
                                        .foregroundStyle(badge.synced ? theme.successBase : theme.fgMuted)
                                }
                            }
                            .padding(.vertical, Tokens.Space.s8)
                        }
                    }

                    Text(loc.t(I18nKeys.Create.verifyHint))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
                .padding(.bottom, Tokens.Space.s24)
            }
            .scrollBounceBehavior(.basedOnSize)

            VelaButton(title: loc.t(I18nKeys.Create.enterWalletBtn), kind: .primary, action: onEnter)
        }
        .padding(.bottom, Tokens.Space.s16)
    }
}
