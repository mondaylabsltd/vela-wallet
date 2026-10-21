//
//  AgentViews.swift
//  VelaWallet
//
//  The AI agent on screen (spec 103): the capsule over the wallet, the
//  request it puts in front of the person, and Settings → AI agent. The words
//  are the core catalog's `settings.agent.*`, the web's and the desktop's.
//

import SwiftUI
import UIKit
import VelaCore

/// The agent's words, as the person reads them.
enum AgentWords {
    /// What an action is called: `settings.agent.action.<camelCase type>`,
    /// or its own name where the catalog has none.
    static func action(_ kind: String, loc: Loc) -> String {
        let parts = kind.split(separator: "_").map(String.init)
        let camel = (parts.first ?? "") + parts.dropFirst().map { $0.prefix(1).uppercased() + $0.dropFirst() }.joined()
        let key = "settings.agent.action.\(camel)"
        let words = loc.t(key)
        return words == key ? kind : words
    }

    /// How the connection stands, and in which tone.
    static func link(_ link: AgentLink, loc: Loc) -> (tone: SettingsTone, words: String) {
        switch link {
        case .off: (.neutral, loc.t("settings.agent.statusOff"))
        case .connecting: (.warn, loc.t("settings.agent.statusConnecting"))
        case .online: (.ok, loc.t("settings.agent.statusOnline"))
        case .offline: (.warn, loc.t("settings.agent.statusOffline"))
        case .closed: (.error, loc.t("settings.agent.statusRefused"))
        }
    }

    /// What a request names, as the person knows it: a contact's or a
    /// wallet's name, the store, the preference.
    @MainActor
    static func detail(_ request: AgentRequest, graph: AppGraph) -> String? {
        let action = request.action
        let same = { (a: String, b: String) in a.caseInsensitiveCompare(b) == .orderedSame }
        switch request.kind {
        case "delete_contact":
            let address = action["address"] as? String ?? ""
            let known = graph.contacts.view?.contacts.first { same($0.address, address) }
            return known?.name.flatMap { $0.isEmpty ? nil : $0 } ?? address
        case "delete_group":
            let id = action["id"] as? String ?? ""
            return graph.contacts.view?.groups.first { $0.id == id }?.name ?? id
        case "remove_wallet":
            let address = action["address"] as? String ?? ""
            let row = graph.session.view.accounts.first { same($0.account.address, address) }
            return row.map { "\($0.account.name) · \(address)" } ?? address
        case "clear_storage_item":
            return action["item"] as? String
        case "set_preference":
            let preference = action["preference"] as? [String: Any] ?? [:]
            return "\(preference["kind"] ?? ""): \(preference["value"] ?? "")"
        default:
            return nil
        }
    }
}

/// The agent's capsule over the wallet, as the web's floats over its page:
/// whether it is connected — a dot in the link's tone — what it did last,
/// and Stop, which refuses everything of its still waiting and switches it
/// off. Shown while the agent is set up, or a request waits on the person.
struct AgentCapsule: View {
    @Environment(\.theme) private var theme
    let agent: AgentStore
    let loc: Loc

    /// How high it floats over the bottom of the safe area: clear of the tab
    /// bar, and of a flow's own button at the foot of its page.
    static let clearance: CGFloat = 84

    var body: some View {
        if agent.showsCapsule {
            HStack(spacing: Tokens.Space.s12) {
                Circle().fill(dot).frame(width: 8, height: 8)
                Text(loc.t("settings.agent.title"))
                    .typeRole(Typography.label)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                    .fixedSize()
                Text(what)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Button {
                    VelaHaptic.press.play()
                    agent.stop()
                } label: {
                    Text(loc.t("settings.agent.stop"))
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.errorBase)
                        .padding(.horizontal, Tokens.Space.s12)
                        .frame(height: 28)
                        .background(theme.errorSoft, in: Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("agent-stop")
            }
            .padding(.leading, Tokens.Space.s16)
            .padding(.trailing, Tokens.Space.s4)
            .padding(.vertical, Tokens.Space.s4)
            .background(theme.bgRaised, in: Capsule())
            .overlay(Capsule().stroke(theme.borderBase, lineWidth: 1))
            .shadow(color: .black.opacity(0.12), radius: 12, y: 4)
            .padding(.horizontal, Tokens.Space.s16)
            .padding(.vertical, Tokens.Space.s4)
            .frame(maxWidth: .infinity)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("agent-capsule")
        }
    }

    private var busy: Bool { agent.activity.first?.stage != nil }

    private var dot: Color {
        switch agent.link {
        case .online: busy ? theme.infoBase : theme.successBase
        case .closed: theme.errorBase
        case .offline, .connecting, .off: theme.warningBase
        }
    }

    /// Online, what it did last; otherwise how the link stands.
    private var what: String {
        switch agent.link {
        case .online:
            agent.activity.first.map { AgentWords.action($0.kind, loc: loc) }
                ?? loc.t("settings.agent.statusOnline")
        case .off: loc.t("settings.agent.statusConnecting")
        default: AgentWords.link(agent.link, loc: loc).words
        }
    }
}

/// The agent's manage-level request, asked here in the wallet — not in the
/// agent's chat. Dismissing it is a no.
struct AgentApprovalSheet: View {
    @Environment(\.theme) private var theme
    let request: AgentRequest
    let detail: String?
    let loc: Loc
    let onAnswer: (Bool) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Its own words: this sheet has no host header to pin them in.
            SheetTitleWords(value: SheetTitleValue(title: loc.t("settings.agent.approveTitle")))
            Text(words)
                .typeRole(Typography.fieldLabel)
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.bottom, Tokens.Space.s24)
            VelaButton(title: loc.t("settings.agent.allow"), kind: .danger) { onAnswer(true) }
                .padding(.bottom, Tokens.Space.s12)
                .accessibilityIdentifier("agent-allow")
            VelaButton(title: loc.t("settings.agent.deny"), kind: .secondary) { onAnswer(false) }
                .accessibilityIdentifier("agent-deny")
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .presentationDetents([.medium])
        .presentationBackground(theme.bgRaised)
    }

    private var words: String {
        let name = AgentWords.action(request.kind, loc: loc)
        let hint = loc.t("settings.agent.approveHint")
        return detail.map { "\(name) — \($0). \(hint)" } ?? "\(name). \(hint)"
    }
}

/// Settings → AI agent: whether this wallet answers a remote AI agent
/// through a ClientMCP bridge, and which bridge (spec 103 D1). Every change
/// is the person's own, through the `configure_agent` intent — an agent can
/// never make it.
struct AgentSettingsBody: View {
    @Environment(\.theme) private var theme
    let agent: AgentStore
    let loc: Loc

    @State private var url: String?
    @State private var token: String?
    @FocusState private var tokenFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s24) {
            Text(loc.t("settings.agent.description"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
                .contentShape(Rectangle())
                .onTapGesture { SettingsFocus.release() }
            SettingsSwitchRow(
                title: loc.t("settings.agent.enable"),
                detail: AgentWords.link(agent.link, loc: loc).words,
                isOn: agent.config.enabled,
                onChange: { on in configure(enabled: on) }
            )
            .accessibilityIdentifier("agent-enable")
            SettingsUrlField(
                field: UrlFieldModel(
                    id: "agent-bridge",
                    label: loc.t("settings.agent.bridge"),
                    value: agent.config.url,
                    placeholder: "wss://bridge.clientmcp.dev/connect",
                    hint: loc.t(agent.urlProblem == nil ? "settings.agent.bridgeHint" : "settings.agent.bridgeInvalid"),
                    tone: agent.urlProblem == nil ? nil : .error
                ),
                text: Binding(get: { url ?? agent.config.url }, set: { url = $0 }),
                onCommit: { if let url { configure(url: url) } },
                commitsOnBlur: true
            )
            .accessibilityIdentifier("agent-bridge")
            tokenField
            if let endpoint = agent.endpoint {
                VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                    Text(loc.t("settings.agent.endpoint"))
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgSubtle)
                    Button {
                        UIPasteboard.general.string = endpoint
                        VelaHaptic.select.play()
                    } label: {
                        Text(endpoint)
                            .font(Font.system(size: Tokens.TextSize.t13, design: .monospaced))
                            .foregroundStyle(theme.fgBase)
                            .multilineTextAlignment(.leading)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(Tokens.Space.s12)
                            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: 10))
                    }
                    .buttonStyle(.plain)
                    Text(loc.t("settings.agent.endpointHint"))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            if let latest = agent.activity.first {
                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    Text(loc.t("settings.agent.lastAction"))
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgSubtle)
                    Text("\(AgentWords.action(latest.kind, loc: loc)) · \(latest.code ?? latest.status ?? latest.stage ?? "")")
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                }
            }
        }
        // The page ends above the capsule that floats over it.
        .padding(.bottom, AgentCapsule.clearance)
    }

    /// The client token: a credential, so it is not shown once entered.
    private var tokenField: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(loc.t("settings.agent.token"))
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
            SecureField(loc.t("settings.agent.tokenHint"), text: Binding(
                get: { token ?? agent.config.token },
                set: { token = $0 }
            ))
            .font(Font.system(size: Tokens.TextSize.t13, design: .monospaced))
            .foregroundStyle(theme.fgBase)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(.done)
            .focused($tokenFocused)
            .onSubmit { if let token { configure(token: token) } }
            .onChange(of: tokenFocused) { was, now in
                if was, !now, let token { configure(token: token) }
            }
            .padding(Tokens.Space.s12)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(theme.borderBase, lineWidth: 1))
            .accessibilityIdentifier("agent-token")
            Text(loc.t("settings.agent.tokenHint"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
        }
    }

    /// The person's own change, through the intents.
    private func configure(enabled: Bool? = nil, url: String? = nil, token: String? = nil) {
        var action: [String: Any] = ["type": "configure_agent"]
        if let enabled { action["enabled"] = enabled }
        if let url { action["bridge_url"] = url }
        if let token { action["client_token"] = token }
        agent.person(action)
    }
}
