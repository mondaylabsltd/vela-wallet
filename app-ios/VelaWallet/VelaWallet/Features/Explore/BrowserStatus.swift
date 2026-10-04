//
//  BrowserStatus.swift
//  VelaWallet
//
//  The tab's status (spec 099 FR-014): one quiet line under the address bar
//  when the tab in front has something to say, and the panel behind it with
//  the tab's whole request record.
//
//  A dApp tab is six layers deep — the page, the wallet offered to it, the
//  wallet's own rules, the chain's RPC, Vela's relay, the passkey — and when
//  something does not work the first question is which one stopped. The
//  browser machine keeps the record and names the layer and the reason for
//  every request (`dapp_record`); this file only draws it, in the core's
//  words, and copies the core's own plain-text report.
//
//  The line says, in this order: the page was reloaded because its tab had
//  been let go to save memory; the page loaded and the wallet was not offered
//  to it; the latest request that ended in trouble. ✕ hides it until it would
//  say something else.
//

import SwiftUI

/// What the status line says for one tab.
struct BrowserStatusLine: Equatable {
    /// Which thing it says — a dismissed line stays away until this changes.
    let seen: String
    let text: String
    /// Trouble, drawn with the warning mark; the reload is news, not trouble.
    let warning: Bool
}

enum BrowserStatusLive {

    /// The line for tab `tabId`, from the core's facts about it and the tab
    /// the controller woke from suspension, or `nil` when there is nothing to
    /// say.
    static func line(tabId: String, tab: DbrTabViewWire?, reloadedTab: String?, loc: Loc) -> BrowserStatusLine? {
        if reloadedTab == tabId {
            return BrowserStatusLine(
                seen: "reloaded", text: loc.t("componentsUi.browserStatus.reloaded"), warning: false
            )
        }
        guard let tab else { return nil }
        if tab.page == "ready", tab.provider == "insecure_origin" || tab.provider == "no_hello",
           let key = DbrRecordWords.provider(tab.provider) {
            return BrowserStatusLine(seen: "provider:\(tab.provider)", text: loc.t(key), warning: true)
        }
        guard let note = tab.lastFailure else { return nil }
        return BrowserStatusLine(
            seen: "\(tab.failedRecent):\(note.method):\(note.reason)",
            text: "\(loc.t(note.key)) · \(note.method)",
            warning: true
        )
    }

    /// How one row of the record ended, in words: "…" while open, "✓ 120 ms"
    /// answered, the reason's line when it failed (its code when this build
    /// does not know the reason).
    static func outcome(_ row: DbrRequestRowWire, loc: Loc) -> String {
        switch row.outcome {
        case "answered":
            return row.durationMs.map { "✓ \(Int($0.rounded())) ms" } ?? "✓"
        case "failed":
            if let key = DbrRecordWords.reason(row.reason) { return loc.t(key) }
            return row.code.map(String.init) ?? "✕"
        default:
            return "…"
        }
    }
}

/// The line under the address bar — the chain notice's quiet style.
struct BrowserStatusLineView: View {
    @Environment(\.theme) private var theme

    let line: BrowserStatusLine
    let detailsLabel: String
    let dismissLabel: String
    let onDetails: () -> Void
    let onDismiss: () -> Void

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            LucideIcon(line.warning ? .triangleAlert : .refreshCw, size: LucideIconSize.addressLock)
                .foregroundStyle(line.warning ? theme.warningBase : theme.fgSubtle)
                .accessibilityHidden(true)
            Text(verbatim: line.text)
                .typeRole(Typography.rowSub)
                .foregroundStyle(theme.fgMuted)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button(action: onDetails) {
                Text(verbatim: detailsLabel)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.accentBase)
                    .padding(.vertical, Tokens.Space.s8)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("explore.status.details")
            Button(action: onDismiss) {
                LucideIcon(.close, size: LucideIconSize.disclosure)
                    .foregroundStyle(theme.fgSubtle)
                    .padding(Tokens.Space.s8)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(dismissLabel)
            .accessibilityIdentifier("explore.status.dismiss")
        }
        .padding(.leading, Tokens.Space.s16)
        .padding(.trailing, Tokens.Space.s8)
        .background(theme.bgSunken)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("explore.status")
    }
}

/// The tab's whole record: its page, the wallet on it, every request with how
/// it ended — newest first — and Copy, which copies the core's report.
struct BrowserInspectorView: View {
    @Environment(\.theme) private var theme

    let inspector: DbrInspectorViewWire?
    let loc: Loc
    let onClose: () -> Void

    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            HStack {
                Text(verbatim: loc.t("componentsUi.browserStatus.title"))
                    .typeRole(Typography.rowTitle)
                    .foregroundStyle(theme.fgBase)
                Spacer()
                Button(action: onClose) {
                    LucideIcon(.close, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                        .padding(Tokens.Space.s8)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(loc.t("explore.close"))
            }
            if let inspector {
                record(inspector)
            }
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.top, Tokens.Space.s16)
        .accessibilityIdentifier("explore.status.panel")
    }

    @ViewBuilder
    private func record(_ inspector: DbrInspectorViewWire) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            if let origin = inspector.origin {
                Text(verbatim: origin)
                    .typeRole(Typography.rowSub)
                    .foregroundStyle(theme.fgBase)
            }
            if let key = DbrRecordWords.page(inspector.page) {
                Text(verbatim: loc.t(key))
                    .typeRole(Typography.rowSub)
                    .foregroundStyle(theme.fgSubtle)
            }
            if let key = DbrRecordWords.provider(inspector.provider) {
                Text(verbatim: loc.t(key))
                    .typeRole(Typography.rowSub)
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        Text(verbatim: loc.t("componentsUi.browserStatus.requests"))
            .typeRole(Typography.label)
            .foregroundStyle(theme.fgBase)
            .padding(.top, Tokens.Space.s8)
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Tokens.Space.s8) {
                if inspector.rows.isEmpty {
                    Text(verbatim: loc.t("componentsUi.browserStatus.noRequests"))
                        .typeRole(Typography.rowSub)
                        .foregroundStyle(theme.fgSubtle)
                }
                // Newest first. A page's ids repeat across its documents, so
                // the row's place is its identity here.
                ForEach(Array(inspector.rows.reversed().enumerated()), id: \.offset) { _, row in
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(verbatim: row.method)
                            .typeRole(Typography.monoSmall)
                            .foregroundStyle(theme.fgBase)
                        Text(verbatim: BrowserStatusLive.outcome(row, loc: loc))
                            .typeRole(Typography.rowSub)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        VelaButton(
            title: loc.t(copied ? "componentsUi.browserStatus.copied" : "componentsUi.browserStatus.copy"),
            kind: .secondary
        ) {
            velaCopy(inspector.report)
            copied = true
        }
        .padding(.bottom, Tokens.Space.s16)
        .accessibilityIdentifier("explore.status.copy")
    }
}
