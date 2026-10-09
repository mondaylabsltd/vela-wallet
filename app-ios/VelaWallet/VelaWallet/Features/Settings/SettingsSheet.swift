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

import PhotosUI
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
    var onErase: (() -> Void)?
    /// An account row tapped in the switcher.
    var onSelectAccount: ((String) -> Void)?
    var onAccountCreate: (() -> Void)?
    var onAccountSignIn: (() -> Void)?
    /// Taking ONE wallet off this device (2026-09-23), by its position in the
    /// session's own list.
    var onRemoveAccount: ((Int) -> Void)?
    /// SR2's field and its commit (058). Absent in the gallery, where the
    /// endpoint is a picture of one already typed.
    var rpcDraft: Binding<String>?
    var onCommitRpc: (() -> Void)?
    /// SR3's 立即重试, per chain id.
    var onRetryChain: ((String) -> Void)?
    /// SR6's per-network fix (spec 092): opens that chain's SR2.
    var onFixChain: ((Int) -> Void)?
    /// The destructive action waiting on an answer — a storage row's 清除, a
    /// network's bin, "reset to defaults" — and its "yes".
    var pendingConfirm: ConfirmSheetModel?
    var onConfirmPending: (() -> Void)?
    /// The relay's Save and reset (spec 075).
    var onSaveTunnelUrl: ((String) -> Bool)?
    var onResetTunnelUrl: (() -> Void)?
    /// The language sheet's "suggest a fix". Absent in the gallery.
    var onOpenLink: ((String) -> Void)?
    /// The report's sender, owned by the settings page so a report whose
    /// sheet is closed mid-send still lands and is still told (2026-09-27).
    /// Absent in the gallery, where the sheet makes its own.
    var feedbackSender: FeedbackSender?
    /// A report the app started (issue #466: a relay stop's "Report this"):
    /// the sheet opens on the core's words, and files under the core's area
    /// and fingerprint. Absent, the sheet is the blank one Settings opens.
    var feedbackSeed: BugReport.Seed?

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
                        onSignIn: onAccountSignIn,
                        onRemove: onRemoveAccount
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
                        // means reading the code, not erasing a device with a
                        // real wallet on it.
                        //
                        // Spec 081 FR-017: the sheet does NOT dismiss on
                        // confirm. The erase verifies before it succeeds, and a
                        // failed one has to say so where the person is looking;
                        // a success signs out and leaves this screen entirely,
                        // so the sheet goes with it either way.
                        onConfirm: { onErase?() },
                        onCancel: onDismiss
                    )
                case .feedback:
                    FeedbackSheetBody(model: model.feedback, sender: feedbackSender,
                                      seed: feedbackSeed, onDone: onDismiss)
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
                case .unreachable:
                    UnreachableSheetBody(model: model.unreachable, onFix: onFixChain)
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
    /// Taking ONE wallet off this device (2026-09-23); absent draws nothing.
    /// The index is the position in the ORIGINAL list, which is what the core
    /// removes by — never the address, which two records can share.
    var onRemove: ((Int) -> Void)?

    /// The row a confirmation is open for.
    @State private var removing: Int?

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
        ForEach(Array(sheet.rows.enumerated()), id: \.offset) { offset, row in
            Button { onSelect?(row.addressFull) } label: {
            VStack(spacing: 0) {
                HStack(spacing: Tokens.Space.s12) {
                    IdenticonAvatar(seed: row.addressFull, size: 40, tappable: false)
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
                    if onRemove != nil, !sheet.remove.isEmpty {
                        Button { removing = offset } label: {
                            LucideIcon(.close, size: LucideIconSize.action)
                                .foregroundStyle(theme.fgSubtle)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(sheet.remove)
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
            // Asked before it happens: the row it takes is the one under a
            // finger that was aiming to switch.
            .alert(
                removing.flatMap { sheet.rows[safe: $0]?.name } ?? "",
                isPresented: Binding(
                    get: { removing != nil },
                    set: { open in if !open { removing = nil } }
                ),
                presenting: removing
            ) { index in
                Button(sheet.remove, role: .destructive) {
                    removing = nil
                    onRemove?(index)
                }
                Button(sheet.removeCancel, role: .cancel) { removing = nil }
            } message: { _ in
                Text(sheet.removeBody)
            }
    }
}

/// ST15 — the one-click report (round 3; screenshots 2026-09-26; v2 after the
/// design review). The "what will be sent" rows are the report's
/// `environment`, line for line; the consent note sits directly above the
/// button it is a promise about. Filed says which issue it became; a report
/// the endpoint refused offers the prefilled GitHub form — the only remaining
/// road, never an apology.
struct FeedbackSheetBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.openURL) private var openURL
    /// The small glyphs beside words (eye, info, warning, image-plus) grow
    /// with Dynamic Type as the words do (v2 A8); the app's own text size
    /// multiplies on top, as `typeRole` does for the words.
    @ScaledMetric(relativeTo: .footnote) private var smallGlyph = LucideIconSize.statusIcon
    @ScaledMetric(relativeTo: .body) private var addGlyph = LucideIconSize.action
    @ScaledMetric(relativeTo: .footnote) private var chevronGlyph = LucideIconSize.smallChevron
    let model: FeedbackModel
    var onDone: () -> Void = {}
    /// A report the app started (issue #466): its words fill the form, and
    /// its area and fingerprint ride with the send.
    private let seed: BugReport.Seed?

    @State private var sender: FeedbackSender
    @State private var what: String
    @State private var steps: String
    @State private var stepsOpen: Bool
    @State private var previewOpen = true
    @State private var picked: [PhotosPickerItem] = []
    /// Where VoiceOver goes when the outcome changes (v3 B6/B7): a fallback's
    /// title, the success title.
    @AccessibilityFocusState private var focus: FocusTarget?
    /// Which field has the keyboard. Let go when Send is pressed: the fields
    /// are disabled while sending, and on the iPhone a field that was focused
    /// took the keyboard BACK when it re-enabled — straight over the fallback
    /// block and its "Open GitHub form" button (device run, 2026-09-27).
    @FocusState private var typing: String?
    /// The screenshot viewer while it is up (spec C), and the picture on
    /// screen — whose tile hides under it, as in Photos, so the picture flies
    /// out of an empty slot and back into it.
    @State private var viewer: ScreenshotViewer.Launch?
    @State private var viewing: UUID?
    /// Where the opened tile was, and its place in the row — the fallback for
    /// finding a tile to fly back to if its probe cannot say.
    @State private var viewerAnchor: ViewerAnchor?
    @State private var tileProbes = TileProbes()

    private enum FocusTarget: Hashable {
        case fallback, success
        /// Back from the viewer (C4): the tile of the picture it closed on,
        /// or the add target when none is left.
        case tile(UUID), add
    }

    private struct ViewerAnchor {
        let frame: CGRect
        let index: Int
    }
    /// The fallback block's scroll anchor.
    private static let fallbackAnchor = "feedback.fallbackBlock"

    /// `sender` and `what` are seams for a render of a given state; the sheet
    /// itself passes neither. `seed` starts the form on a report the app
    /// wrote — editable, and still filed under the seed's fingerprint.
    init(model: FeedbackModel, sender: FeedbackSender? = nil, what: String = "",
         seed: BugReport.Seed? = nil, onDone: @escaping () -> Void = {}) {
        self.model = model
        self.onDone = onDone
        self.seed = seed
        _sender = State(initialValue: sender ?? FeedbackSender())
        _what = State(initialValue: seed?.what ?? what)
        _steps = State(initialValue: seed?.steps ?? "")
        _stepsOpen = State(initialValue: !(seed?.steps ?? "").isEmpty)
    }

    var body: some View {
        ScrollViewReader { proxy in
            VStack(alignment: .leading, spacing: 0) {
                if case .filed(let number, let url, let deduped, let dropped) = sender.state {
                    OpticalCentre {
                        filed(number: number, url: url, deduped: deduped, dropped: dropped)
                    }
                    .containerRelativeFrame(.vertical) { height, _ in height - Tokens.Space.s32 }
                } else {
                    SheetTitle(title: model.title, subtitle: model.subtitle)
                    form
                }
            }
            .onChange(of: sender.state) { _, state in
                switch state {
                case .filed:
                    typing = nil
                    VelaHaptic.success.play()
                    // Said, and focused (v3 B7): the form the person was in
                    // is gone, and VoiceOver must not be left on nothing.
                    UIAccessibility.post(notification: .screenChanged, argument: nil)
                    focusSoon(.success)
                case .fallback:
                    // Never stranded below the fold (v3 B6): the keyboard stays
                    // down, the block and its "Open GitHub form" button come
                    // into view, and VoiceOver reads the block. The scroll waits
                    // a beat so it measures the page without a keyboard.
                    typing = nil
                    DispatchQueue.main.asyncAfter(deadline: .now() + FeedbackGeometry.focusDelay) {
                        withAnimation(.easeOut(duration: FeedbackGeometry.tileAnimation * 2)) {
                            proxy.scrollTo(Self.fallbackAnchor, anchor: .top)
                        }
                    }
                    focusSoon(.fallback)
                default:
                    break
                }
            }
        }
        .onChange(of: picked) { _, items in
            guard !items.isEmpty else { return }
            let loaders: [FeedbackSender.Loader] = items.map { item in
                { try? await item.loadTransferable(type: Data.self) }
            }
            picked = []
            sender.attach(loaders)
        }
        .fullScreenCover(item: $viewer) { launch in
            ScreenshotViewer(
                launch: launch,
                sender: sender,
                words: ScreenshotViewer.Words(viewScreenshot: model.viewScreenshot,
                                              close: model.closeViewer,
                                              remove: model.removeFromViewer),
                tileFrame: { id in tileFrame(id) },
                onCurrent: { id in viewing = id },
                onRemove: { id in removeTile(id) },
                onClosed: { id in viewerClosed(on: id) }
            )
            // Over the sheet, not instead of it: the sheet shows through as
            // the black fades under a swipe down.
            .presentationBackground(.clear)
        }
    }

    private func glyph(_ size: CGFloat) -> CGFloat { size * textScale }

    /// Move VoiceOver once the new view is laid out.
    private func focusSoon(_ target: FocusTarget) {
        DispatchQueue.main.asyncAfter(deadline: .now() + FeedbackGeometry.focusDelay) { focus = target }
    }

    /// Nothing on the form moves while the report is on its way (v3 B9).
    private var inert: Bool { sender.sending }

    @ViewBuilder private var form: some View {
        field(text: $what, placeholder: model.placeholder, id: "feedback.what")
            .disabled(inert)
        if stepsOpen {
            field(text: $steps, placeholder: model.stepsPlaceholder, id: "feedback.steps")
                .disabled(inert)
                .padding(.top, Tokens.Space.s12)
        } else {
            Button { stepsOpen = true } label: {
                Text(model.addSteps)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.infoBase)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(inert)
            .padding(.vertical, Tokens.Space.s12)
        }
        screenshots
            .padding(.top, stepsOpen ? Tokens.Space.s16 : Tokens.Space.s4)
        preview
            .padding(.top, Tokens.Space.s24)
        if case .fallback(_, let carried) = sender.state {
            fallbackBlock(carried: carried)
                .id(Self.fallbackAnchor)
                .padding(.top, Tokens.Space.s16)
        }
        consent
            .padding(.top, Tokens.Space.s16)
            .padding(.bottom, Tokens.Space.s12)
        if case .fallback(let url, _) = sender.state {
            VelaButton(title: model.openGithub, kind: .primary) {
                if let link = URL(string: url) { openURL(link) }
            }
            .accessibilityIdentifier("feedback.openGithub")
            .padding(.bottom, Tokens.Space.s12)
        }
        // Busy is the spinner AND "Sending…", never a dimmed button; dimmed
        // only while there is nothing to send. Screenshots never block it.
        // After a fallback it is a retry, and says so. The gallery's picture
        // stays at full emphasis, as the mock draws it.
        VelaButton(
            title: isFallback ? model.tryAgain : model.send,
            kind: isFallback ? .secondary : .primary,
            enabled: !model.live || FeedbackSender.ready(what),
            loading: sender.sending,
            busyTitle: model.sending
        ) {
            guard model.live else { return }
            // The keyboard goes down with the press, and stays down.
            typing = nil
            let lines = model.previewLines
            let typed = (what, steps)
            let seed = seed
            Task {
                await sender.send(what: typed.0, steps: typed.1, previewLines: lines,
                                  version: BuildInfo.version, seed: seed)
            }
        }
        .accessibilityIdentifier("feedback.send")
        // The fallback block already offers the form; a second way to the
        // same place under it is noise.
        if !isFallback { githubLink }
    }

    private var isFallback: Bool {
        if case .fallback = sender.state { return true }
        return false
    }

    // MARK: - What will be sent

    /// Plain rows, label and value, no box and no monospace (v2 A4): read as
    /// sentences about this phone, not as a log. Open by default — the point
    /// is to see what leaves the device before pressing send.
    private var preview: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Button { previewOpen.toggle() } label: {
                HStack(spacing: Tokens.Space.s4) {
                    Text(model.previewToggle)
                        .typeRole(Typography.rowSub)
                    LucideIcon(.chevronDown, size: glyph(chevronGlyph))
                        .rotationEffect(.degrees(previewOpen ? 180 : 0))
                }
                .foregroundStyle(theme.fgMuted)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(previewOpen ? [.isSelected] : [])
            if previewOpen {
                // Two columns and no colon (v3 B2): a half-width ": " after
                // Chinese read as a typo. The PAYLOAD keeps "label: value".
                Grid(alignment: .leadingFirstTextBaseline,
                     horizontalSpacing: Tokens.Space.s12, verticalSpacing: Tokens.Space.s4) {
                    ForEach(model.previewLines, id: \.self) { line in
                        let parts = Self.split(line)
                        GridRow {
                            Text(parts.label ?? "")
                                .typeRole(Typography.rowSub)
                                .foregroundStyle(theme.fgMuted)
                                .fixedSize()
                            // A long value wraps under ITSELF, not under the label.
                            Text(parts.value)
                                .typeRole(Typography.rowSub)
                                .foregroundStyle(theme.fgBase)
                                .fixedSize(horizontal: false, vertical: true)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// A preview line at its first ": " — the label and the value.
    static func split(_ line: String) -> (label: String?, value: String) {
        guard let range = line.range(of: ": ") else { return (nil, line) }
        return (String(line[..<range.lowerBound]), String(line[range.upperBound...]))
    }

    /// De-boxed and quiet (v2 A4): the promise, in muted text beside an info
    /// mark, right above the button it is about. It was blue on blue — below
    /// 4.5:1, and the loudest thing on the sheet.
    private var consent: some View {
        HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
            LucideIcon(.info, size: glyph(smallGlyph))
                .foregroundStyle(theme.infoBase)
            Text(model.consent)
                .typeRole(Typography.rowSub)
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Three parts, never glued (v2 A2): what happened, what to do, and — when
    /// images were attached — where they go instead.
    private func fallbackBlock(carried: Int) -> some View {
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(.triangleAlert, size: glyph(smallGlyph))
                .foregroundStyle(theme.warningBase)
                .padding(.top, Tokens.Space.s2)
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(model.fallbackTitle)
                    .typeRole(Typography.body)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                Text(model.fallbackBody)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgMuted)
                if carried > 0 {
                    Text(model.fallbackScreenshots)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgMuted)
                        .padding(.top, Tokens.Space.s8)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(Tokens.Space.s16)
        .background(theme.warningSoft, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
        .accessibilityFocused($focus, equals: .fallback)
        .accessibilityIdentifier("feedback.fallback")
    }

    // MARK: - Screenshots

    /// Header, the add target or the tiles, and the lines under them.
    private var screenshots: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                Text(model.screenshotsLabel)
                    .typeRole(Typography.label)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                Spacer(minLength: Tokens.Space.s8)
                Text(sender.shots.isEmpty
                     ? model.screenshotsHint
                     : "\(sender.shots.count) / \(ScreenshotPrep.maxCount)")
                    .monospacedDigit()
                    .typeRole(Typography.rowSub)
                    // fg-muted, not fg-subtle (v3 B8): subtle failed 4.5:1.
                    .foregroundStyle(theme.fgMuted)
            }
            if sender.shots.isEmpty {
                picker { emptyTarget }
            } else {
                // ONE row of five equal squares (v2 A6) — a 4 + 1 wrap read as
                // a layout mistake.
                // The row fills the column, so the fifth tile's edge lines up
                // with the field and the counter; the last remove area reaches
                // into the screen margin instead of taking room from the row.
                FiveColumns(gap: FeedbackGeometry.tileGap, maxSide: FeedbackGeometry.tileMax) {
                    ForEach(Array(sender.shots.enumerated()), id: \.element.id) { index, shot in
                        tile(shot, index: index)
                            .transition(.scale.combined(with: .opacity))
                    }
                    if sender.room > 0 {
                        picker { addTile }
                            .transition(.opacity)
                    }
                }
                // Room for the badges that overlap the row's top corners.
                .padding(.top, FeedbackGeometry.badgeOverlap)
            }
            noticeLines
        }
        .animation(.easeOut(duration: FeedbackGeometry.tileAnimation), value: sender.shots)
    }

    private func picker<Label: View>(@ViewBuilder _ label: () -> Label) -> some View {
        PhotosPicker(
            selection: $picked,
            maxSelectionCount: max(1, sender.room),
            matching: .images
        ) { label() }
        .buttonStyle(.plain)
        .disabled(inert)
        .accessibilityLabel(model.addScreenshots)
        .accessibilityIdentifier("feedback.addScreenshots")
        .accessibilityFocused($focus, equals: .add)
    }

    private var dashed: some View {
        RoundedRectangle(cornerRadius: Tokens.Radius.r12)
            .strokeBorder(theme.borderStrong, style: StrokeStyle(
                lineWidth: Tokens.BorderWidth.hairline, dash: FeedbackGeometry.dash
            ))
    }

    private var emptyTarget: some View {
        HStack(spacing: Tokens.Space.s8) {
            LucideIcon(.imagePlus, size: glyph(addGlyph))
            Text(model.addScreenshots)
                .typeRole(Typography.flowCaption)
        }
        .foregroundStyle(theme.fgMuted)
        .frame(maxWidth: .infinity, minHeight: FeedbackGeometry.addTargetHeight)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(dashed)
        .contentShape(Rectangle())
    }

    private var addTile: some View {
        LucideIcon(.imagePlus, size: glyph(addGlyph))
            .foregroundStyle(theme.fgMuted)
            .frame(minWidth: 0, maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(dashed)
            .contentShape(Rectangle())
    }

    private func tile(_ shot: FeedbackSender.Shot, index: Int) -> some View {
        TileBody(
            ready: shot.prepared != nil,
            name: model.viewScreenshot.replacingOccurrences(of: "{{index}}", with: String(index + 1)),
            number: index + 1,
            enabled: !inert,
            open: { openViewer(shot.id) }
        ) {
            tileFace(shot)
        }
        // Outside the button, so the press's shrink never skews where the
        // picture flies from.
        .background(TileFrameProbe(id: shot.id, probes: tileProbes))
        .accessibilityFocused($focus, equals: .tile(shot.id))
        .overlay(alignment: .topTrailing) { removeBadge(shot, index: index) }
        // Under the viewer, the slot is empty — the picture is up there.
        .opacity(viewing == shot.id ? 0 : 1)
    }

    /// The square itself: the processed picture, or a spinner while it is
    /// being prepared.
    private func tileFace(_ shot: FeedbackSender.Shot) -> some View {
        ZStack {
            // The PROCESSED pixels — what will be sent (C5), decoded once and
            // shared with the viewer so the flight starts on the same image.
            if shot.prepared != nil, let image = ScreenshotImages.image(for: shot) {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
            } else {
                // Still being decoded and re-encoded, off the main thread.
                theme.bgSunken
                ProgressView().controlSize(.small)
            }
        }
        // Both bounds, so the tile IS the square it is offered: with a max
        // alone a frame takes its child's size, and a portrait screenshot
        // filled to the width grew the tile down past its row.
        .frame(minWidth: 0, maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
        .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
        )
        .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
    }

    private func removeBadge(_ shot: FeedbackSender.Shot, index: Int) -> some View {
        // A TAP, not a Button (device run, 2026-09-27): the 44 grows out
        // into the gap above the row — where a thumb starts a scroll — and
        // a Button there still fired when the touch became a scroll, so
        // scrolling the sheet quietly removed a screenshot (it happened to
        // the real report). A tap gesture fails as soon as the finger
        // moves; VoiceOver and UI tests still see a button.
        Group {
            // Opaque (v3 B1): a translucent disc read two-toned and let
            // the screenshot show through. Near-black on light; in dark a
            // solid neutral lighter than the page — `fg.base` / `border.strong`.
            LucideIcon(.close, size: FeedbackGeometry.badgeGlyph)
                .foregroundStyle(Color.white)
                .frame(width: FeedbackGeometry.badge, height: FeedbackGeometry.badge)
                .background(Circle().fill(theme.scheme == .dark ? theme.borderStrong : theme.fgBase))
                .overlay(Circle().stroke(theme.bgBase, lineWidth: FeedbackGeometry.badgeRing))
                // Small to see, 44 to hit — and the 44 grows OUTWARD
                // (v3 B10): centred on the tile's corner, it reaches into
                // the picture only as far as the badge's own size, never
                // half a small tile. The badge sits in its inner corner,
                // overhanging the tile by `badgeOverlap`.
                .padding(.top, FeedbackGeometry.removeReach - FeedbackGeometry.badgeOverlap)
                .padding(.trailing, FeedbackGeometry.removeReach - FeedbackGeometry.badgeOverlap)
                .frame(width: FeedbackGeometry.badgeTarget, height: FeedbackGeometry.badgeTarget,
                       alignment: .topTrailing)
                .contentShape(Rectangle())
        }
        .onTapGesture { removeTile(shot.id) }
        .allowsHitTesting(!inert)
        .offset(x: FeedbackGeometry.removeReach, y: -FeedbackGeometry.removeReach)
        .accessibilityElement(children: .ignore)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel(model.removeScreenshot.replacingOccurrences(of: "{{index}}", with: String(index + 1)))
        .accessibilityAction { removeTile(shot.id) }
        .accessibilityIdentifier("feedback.removeScreenshot.\(index + 1)")
    }

    private func removeTile(_ id: UUID) {
        guard !inert else { return }
        VelaHaptic.select.play()
        sender.remove(id)
    }

    // MARK: - The viewer

    /// Open the viewer on a processed tile — never a processing one, never
    /// while the report is on its way (C1, C5). With the keyboard up it goes
    /// down first, and the viewer opens once the sheet has settled, so the
    /// picture flies from where the tile really is.
    private func openViewer(_ id: UUID) {
        guard viewer == nil,
              ScreenshotViewerState(opening: id, shots: sender.shots, sending: inert) != nil
        else { return }
        let present = {
            guard viewer == nil,
                  let state = ScreenshotViewerState(opening: id, shots: sender.shots, sending: inert)
            else { return }
            let from = tileProbes.frameInWindow(id)
            viewerAnchor = from.flatMap { frame in
                sender.shots.firstIndex { $0.id == id }.map { ViewerAnchor(frame: frame, index: $0) }
            }
            // No slide-up: the viewer draws its own opening, from the tile.
            var still = Transaction()
            still.disablesAnimations = true
            withTransaction(still) {
                viewer = ScreenshotViewer.Launch(state: state, from: from)
            }
        }
        if typing != nil {
            typing = nil
            DispatchQueue.main.asyncAfter(deadline: .now() + ScreenshotViewerGeometry.keyboardSettle, execute: present)
        } else {
            present()
        }
    }

    /// A tile's frame on screen now: its probe, or — should the probe be out
    /// of the window — worked out from the opened tile and its column.
    private func tileFrame(_ id: UUID) -> CGRect? {
        if let frame = tileProbes.frameInWindow(id) { return frame }
        guard let anchor = viewerAnchor, let index = sender.shots.firstIndex(where: { $0.id == id }) else { return nil }
        return anchor.frame.offsetBy(dx: CGFloat(index - anchor.index) * (anchor.frame.width + FeedbackGeometry.tileGap),
                                     dy: 0)
    }

    /// The viewer has landed (or faded). The tile shows again under the
    /// landed picture first, and the cover goes a beat later — never a frame
    /// with neither. Focus goes back to the tile of the picture it closed on,
    /// or to the add target when none is left (C4).
    private func viewerClosed(on id: UUID?) {
        viewing = nil
        DispatchQueue.main.async {
            var still = Transaction()
            still.disablesAnimations = true
            withTransaction(still) { viewer = nil }
            viewerAnchor = nil
            switch ScreenshotViewerState.returnFocus(closingOn: id, shots: sender.shots) {
            case .tile(let tile): focusSoon(.tile(tile))
            case .add: focusSoon(.add)
            }
        }
    }

    /// A refusal, when there is one, ABOVE the public warning — never in its
    /// place (v2 A1): the warning is the founder's ruling and stays visible
    /// before Send whenever anything is attached.
    @ViewBuilder private var noticeLines: some View {
        if let notice = sender.notice {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                LucideIcon(.triangleAlert, size: glyph(smallGlyph))
                Text(notice == .limit ? model.screenshotsLimit : model.screenshotUnsupported)
                    .typeRole(Typography.rowSub)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(theme.warningBase)
            .accessibilityIdentifier("feedback.screenshotsNotice")
        }
        if !sender.shots.isEmpty {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                LucideIcon(.eye, size: glyph(smallGlyph))
                Text(model.screenshotsPublic)
                    .typeRole(Typography.rowSub)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(theme.fgMuted)
            .accessibilityIdentifier("feedback.screenshotsPublic")
        }
    }

    /// The tracker itself — a real link, for somebody who would rather write
    /// the issue there.
    private var githubLink: some View {
        Link(destination: URL(string: BugReport.issueForm)!) {
            Text(model.githubLink)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.infoBase)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Tokens.Space.s12)
                .contentShape(Rectangle())
        }
        .accessibilityIdentifier("feedback.github")
    }

    // MARK: - Filed

    /// A centred column: the check, what it became, and the way back to it.
    /// No accent unless screenshots were dropped — then the issue page is
    /// where they get added, and that button leads (v2 A7).
    private func filed(number: Int, url: String, deduped: Bool, dropped: Int) -> some View {
        VStack(spacing: Tokens.Space.s12) {
            LucideIcon(.check, size: FeedbackGeometry.successCheck)
                .foregroundStyle(theme.successBase)
                .frame(width: FeedbackGeometry.successDisc, height: FeedbackGeometry.successDisc)
                .background(Circle().fill(theme.successSoft))
                .overlay(Circle().stroke(
                    theme.successBase.opacity(FeedbackGeometry.successRingOpacity),
                    lineWidth: FeedbackGeometry.successRing
                ))
                .padding(.bottom, Tokens.Space.s8)
            Text(model.successTitle)
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .accessibilityFocused($focus, equals: .success)
                .accessibilityIdentifier("feedback.successTitle")
            // The number is the whole point: a person who reported something
            // is owed a way back to it.
            Text((deduped ? model.successBodyDeduped : model.successBodyNew)
                .replacingOccurrences(of: "{{number}}", with: String(number)))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: FeedbackGeometry.successBodyWidth)
            if dropped > 0 {
                Text(model.screenshotsDropped)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.warningBase)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: FeedbackGeometry.successBodyWidth)
                    .accessibilityIdentifier("feedback.screenshotsDropped")
            }
            // A hierarchy (v3 B3): the issue page outlined — primary only when
            // screenshots were dropped, since that is where they get added —
            // and Done as plain text. Two equal pills said nothing.
            VStack(spacing: Tokens.Space.s8) {
                if let link = URL(string: url) {
                    VelaButton(title: model.viewIssue, kind: dropped > 0 ? .primary : .secondary) { openURL(link) }
                        .accessibilityIdentifier("feedback.viewIssue")
                }
                Button(action: onDone) {
                    Text(model.done)
                        .typeRole(Typography.button)
                        .foregroundStyle(theme.fgMuted)
                        .frame(maxWidth: .infinity, minHeight: Tokens.Control.lg)
                        .contentShape(Rectangle())
                }
                .buttonStyle(PlainTextButtonStyle())
                .accessibilityIdentifier("feedback.done")
            }
            .padding(.top, Tokens.Space.s16)
        }
        .frame(maxWidth: .infinity)
    }

    /// A multi-line field: what the person saw, or the steps behind it.
    private func field(text: Binding<String>, placeholder: String, id: String) -> some View {
        TextEditor(text: text)
            .font(Typography.body.scaled(textScale).font)
            .foregroundStyle(theme.fgBase)
            .scrollContentBackground(.hidden)
            .frame(minHeight: WalletGeometry.batchPasteHeight)
            .padding(Tokens.Space.s8)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
            )
            .overlay(alignment: .topLeading) {
                if text.wrappedValue.isEmpty {
                    Text(placeholder)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgSubtle)
                        .padding(Tokens.Space.s12)
                        .padding(.top, Tokens.Space.s2)
                        .allowsHitTesting(false)
                }
            }
            .focused($typing, equals: id)
            .accessibilityLabel(placeholder)
            .accessibilityIdentifier(id)
    }
}

/// A processed tile is a button named "View screenshot n" (C1) that answers
/// the finger like every other one — it shrinks to 0.97 and ticks while
/// pressed (the founder's rule) — but is NOT a SwiftUI `Button`: in this
/// sheet's ScrollView a Button fired when the finger lifted after a scroll
/// that had started on it, opening the viewer mid-scroll (device run,
/// 2026-09-27, recorded; the remove badge hit the same thing). So:
///
/// - the ACTION is a tap, which fails the moment the finger moves — a
///   scroll that starts on a tile never opens it;
/// - the PRESS is a separate press recogniser that also lets go after 10 pt,
///   shown after `pressDelay` — UIKit's own delay for a control in a scroll
///   view — so a finger that is about to scroll does not tick; held still
///   for `holdToOpen` it opens too, so a slow press is never a dead one.
///
/// One still processing is only what it shows — a spinner — and does nothing.
private struct TileBody<Face: View>: View {
    let ready: Bool
    let name: String
    let number: Int
    /// Inert while the report is sending (v3 B9): no press, no tick, no open.
    let enabled: Bool
    let open: () -> Void
    @ViewBuilder let face: () -> Face

    @State private var pressed = false
    @State private var pending: DispatchWorkItem?

    private static var pressDelay: TimeInterval { 0.15 }
    private static var holdToOpen: TimeInterval { 0.5 }
    private static var pressSlop: CGFloat { 10 }

    var body: some View {
        if ready {
            face()
                .scaleEffect(pressed ? Interaction.pressScaleButton : 1)
                .animation(Interaction.pressSpring, value: pressed)
                .onTapGesture {
                    guard enabled else { return }
                    // A quick tap never showed the press: it still ticks.
                    if !pressed { VelaHaptic.press.play() }
                    open()
                }
                .onLongPressGesture(minimumDuration: Self.holdToOpen, maximumDistance: Self.pressSlop,
                                    perform: { if enabled { open() } }, onPressingChanged: pressing)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(name)
                .accessibilityAddTraits(.isButton)
                .accessibilityAction { if enabled { open() } }
                .accessibilityIdentifier("feedback.viewScreenshot.\(number)")
        } else {
            face()
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("feedback.processingScreenshot.\(number)")
        }
    }

    private func pressing(_ down: Bool) {
        pending?.cancel()
        pending = nil
        guard down, enabled else {
            pressed = false
            return
        }
        let show = DispatchWorkItem {
            pressed = true
            VelaHaptic.press.play()
        }
        pending = show
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.pressDelay, execute: show)
    }
}

/// Up to five equal squares in one row: each side `min(maxSide, (width −
/// 4·gap) / 5)`, laid from the leading edge (v2 A6).
struct FiveColumns: Layout {
    let gap: CGFloat
    let maxSide: CGFloat
    /// Kept clear after the last column — room the last remove area grows
    /// into, so it never hangs past the column (v3 B10).
    var trailingRoom: CGFloat = 0

    private func side(_ width: CGFloat?) -> CGFloat {
        let columns = CGFloat(FeedbackGeometry.columns)
        guard let width else { return maxSide }
        return max(0, min(maxSide, (width - trailingRoom - (columns - 1) * gap) / columns))
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let edge = side(proposal.width)
        return CGSize(width: proposal.width ?? (edge * CGFloat(FeedbackGeometry.columns)), height: edge)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let edge = side(bounds.width)
        for (index, subview) in subviews.prefix(FeedbackGeometry.columns).enumerated() {
            subview.place(
                at: CGPoint(x: bounds.minX + CGFloat(index) * (edge + gap), y: bounds.minY),
                proposal: ProposedViewSize(width: edge, height: edge)
            )
        }
    }
}

/// One child at the optical centre of the height offered: its top gap is
/// `FeedbackGeometry.opticalTop` of the free space — about 2 : 3 above to
/// below, where the eye puts "the middle" (v2 A7).
struct OpticalCentre: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let child = subviews.first?.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil)) ?? .zero
        return CGSize(width: proposal.width ?? child.width, height: max(child.height, proposal.height ?? child.height))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard let child = subviews.first else { return }
        let size = child.sizeThatFits(ProposedViewSize(width: bounds.width, height: nil))
        let free = max(0, bounds.height - size.height)
        child.place(
            at: CGPoint(x: bounds.minX, y: bounds.minY + free * FeedbackGeometry.opticalTop),
            proposal: ProposedViewSize(width: bounds.width, height: size.height)
        )
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
            // 087 F07: one word per chip, whole — a full row wraps the next
            // chip onto a new line instead of breaking "Chainlist" in two
            // (seen on Android; an HStack squeezes the same way here).
            PillFlow(spacing: Tokens.Space.s8) {
                ForEach(model.providers, id: \.self) { name in
                    Text(name)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                        .fixedSize()
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

/// SR6 (spec 092): every network the wallet cannot reach, one row each — what
/// was last read there, and the network's RPC fix. The rows follow the live
/// view, so one that comes back leaves while the sheet is open.
private struct UnreachableSheetBody: View {
    @Environment(\.theme) private var theme
    let model: UnreachableModel
    /// A row's fix. Absent in the gallery.
    var onFix: ((Int) -> Void)?

    var body: some View {
        SheetTitle(title: model.title)
        if let summary = model.summary {
            Text(summary)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .padding(.bottom, Tokens.Space.s8)
        }
        ForEach(model.rows) { row in
            HStack(spacing: Tokens.Space.s12) {
                ChainMark(mark: row.mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(row.name)
                        .typeRole(Typography.fieldLabel)
                        .foregroundStyle(theme.fgBase)
                    Text(row.line)
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgSubtle)
                        .monospacedDigit()
                }
                Spacer(minLength: Tokens.Space.s8)
                if let onFix {
                    Button { onFix(row.chainId) } label: {
                        Text(row.action)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.infoBase)
                            .padding(.vertical, Tokens.Space.s8)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(PlainTextButtonStyle())
                } else {
                    Text(row.action)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.infoBase)
                }
            }
            .padding(.vertical, Tokens.Space.s12)
            .overlay(alignment: .bottom) {
                Rectangle().fill(theme.borderBase).frame(height: Tokens.BorderWidth.hairline)
            }
        }
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

/// A text-only button that still answers the finger: dims and gives a press
/// haptic, like every other button here (the founder's rule), with no box.
struct PlainTextButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? Interaction.pressedOpacity : 1)
            .scaleEffect(configuration.isPressed ? Interaction.pressScaleButton : 1)
            .animation(Interaction.pressSpring, value: configuration.isPressed)
            .onChange(of: configuration.isPressed) { _, pressed in
                if pressed { VelaHaptic.press.play() }
            }
    }
}
