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
    static func detail(_ request: AgentRequest, graph: AppGraph, loc: Loc) -> String? {
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
        case "set_balance_privacy":
            return loc.t(action["hidden"] as? Bool == true ? "home.a11yHideBalance" : "home.a11yShowBalance")
        case "set_network_space":
            return (action["space"] as? String).map { loc.t("networkSpace.\($0)") }
        default:
            // The core's: the network, URL or record it changes — and, read
            // against the wallet, a site's networks and wallet, or the wallet
            // a switch leaves (spec 107).
            let detail = intentApprovalDetailIn(
                actionJson: request.actionJson, worldJson: AgentShellBridge.views(graph)
            )
            return (try? JSONSerialization.jsonObject(with: Data(detail.utf8), options: .fragmentsAllowed)) as? String
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
    /// Allow takes a tap only once the request has been on screen a moment:
    /// the next request is a new sheet (`sheet(item:)`, by its id), and a
    /// finger already on its way to Allow does not land on it.
    @State private var armed = false
    static let armMs = 700

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Its own words: this sheet has no host header to pin them in.
            SheetTitleWords(value: SheetTitleValue(title: loc.t("settings.agent.approveTitle")))
            Text(words)
                .typeRole(Typography.fieldLabel)
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.bottom, Tokens.Space.s24)
            // The safe answer is the easy one: Deny on top and filled, Allow
            // below it and quiet. Asked to tap Deny on the phone, the owner
            // twice hit the large red Allow that used to lead (2026-10-27).
            VelaButton(title: loc.t("settings.agent.deny"), kind: .primary) { onAnswer(false) }
                .padding(.bottom, Tokens.Space.s12)
                .accessibilityIdentifier("agent-deny")
            VelaButton(title: loc.t("settings.agent.allow"), kind: .secondary, enabled: armed) { onAnswer(true) }
                .accessibilityIdentifier("agent-allow")
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .presentationDetents([.medium])
        .presentationBackground(theme.bgRaised)
        .task(id: request.id) {
            armed = false
            try? await Task.sleep(for: .milliseconds(Self.armMs))
            armed = true
        }
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
    @Environment(\.openURL) private var openURL
    let agent: AgentStore
    let loc: Loc

    @State private var url: String?
    @State private var token: String?
    @FocusState private var tokenFocused: Bool
    /// Spec 106: a clientmcp.dev sign-in under way, and why the last did not connect.
    @State private var linking = false
    @State private var problem: AgentLinkProblem?
    /// "Use your own bridge" unfolded by the person (`nil`: unfolded when a
    /// token was entered by hand).
    @State private var ownOpen: Bool?
    /// The agent's address revealed (Show), and just copied.
    @State private var addressShown = false
    @State private var addressCopied = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s24) {
            Text(loc.t("settings.agent.description"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
                .contentShape(Rectangle())
                .onTapGesture { SettingsFocus.release() }
            if let linked = agent.config.linked {
                linkedSection(linked)
            } else {
                connectSection
                ownBridgeToggle
                if ownOpen ?? !agent.config.token.isEmpty {
                    ownBridge
                }
            }
            walletsSection
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

    // MARK: Spec 107 E8 — the wallets the agent may use

    /// A switch per wallet on this device; the agent sees and acts only in
    /// the ones on. Each change is the person's own `configure_agent`, the
    /// list whole.
    private var walletsSection: some View {
        let allowed = agent.config.wallets
        let accounts = agent.graph?.session.view.accounts ?? []
        return VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t("settings.agent.wallets"))
                .typeRole(Typography.rowTitle)
                .foregroundStyle(theme.fgBase)
            Text(loc.t("settings.agent.walletsHint"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
            if allowed.isEmpty {
                Text(loc.t("settings.agent.walletsNone"))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.warningBase)
                    .accessibilityIdentifier("agent-wallets-none")
            }
            ForEach(accounts, id: \.account.address) { row in
                let address = row.account.address
                let on = allowed.contains { $0.caseInsensitiveCompare(address) == .orderedSame }
                SettingsSwitchRow(
                    title: row.account.name,
                    detail: "\(address.prefix(6))…\(address.suffix(4))",
                    isOn: on,
                    onChange: { want in
                        var next = allowed.filter { $0.caseInsensitiveCompare(address) != .orderedSame }
                        if want { next.append(address) }
                        // The whole action: what stays as it is, said as such.
                        agent.person([
                            "type": "configure_agent",
                            "enabled": NSNull(),
                            "bridge_url": NSNull(),
                            "client_token": NSNull(),
                            "wallets": next,
                        ])
                    }
                )
                .accessibilityIdentifier("agent-wallet-\(address)")
            }
        }
    }

    // MARK: Spec 106 — connecting with clientmcp.dev

    private var enableRow: some View {
        SettingsSwitchRow(
            title: loc.t("settings.agent.enable"),
            detail: AgentWords.link(agent.link, loc: loc).words,
            isOn: agent.config.enabled,
            onChange: { on in configure(enabled: on) }
        )
        .accessibilityIdentifier("agent-enable")
    }

    /// The one button, its hint, and why the last attempt did not connect.
    private var connectSection: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            VelaButton(
                title: loc.t("settings.agent.connect"),
                kind: .secondary,
                loading: linking,
                busyTitle: loc.t("settings.agent.connecting"),
                action: connect
            )
            .accessibilityIdentifier("agent-connect")
            Text(loc.t("settings.agent.connectHint"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
            problemLine
        }
    }

    @ViewBuilder private var problemLine: some View {
        if let problem, problem != .cancelled {
            Text(problem.words(loc))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.errorBase)
                .accessibilityIdentifier("agent-link-problem")
        }
    }

    private var ownBridgeToggle: some View {
        let open = ownOpen ?? !agent.config.token.isEmpty
        return Button {
            ownOpen = !open
        } label: {
            HStack(spacing: Tokens.Space.s8) {
                Image(systemName: open ? "chevron.down" : "chevron.right")
                    .font(.system(size: Tokens.TextSize.t13, weight: .semibold))
                Text(loc.t("settings.agent.ownBridge"))
                    .typeRole(Typography.flowCaption)
            }
            .foregroundStyle(theme.fgMuted)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("agent-own-bridge")
    }

    /// Connected through clientmcp.dev: whose it is, where the person connects
    /// their agent, Reconnect and Disconnect.
    private func linkedSection(_ linked: AgentLinkedTo) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            Text(loc.t("settings.agent.connectedAs").replacingOccurrences(of: "{{login}}", with: linked.login))
                .typeRole(Typography.rowTitle)
                .foregroundStyle(theme.fgBase)
                .accessibilityIdentifier("agent-linked")
            Text(loc.t("settings.agent.clientId").replacingOccurrences(of: "{{id}}", with: linked.clientId))
                .font(Font.system(size: Tokens.TextSize.t13, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .textSelection(.enabled)
                .padding(.top, -Tokens.Space.s8)
                .accessibilityIdentifier("agent-client-id")
            if !agent.config.agentUrl.isEmpty {
                agentAddress(agent.config.agentUrl)
            }
            enableRow
            Button {
                if let account = URL(string: clientmcpAccountUrl(site: linked.site)) { openURL(account) }
            } label: {
                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    Text("\(loc.t("settings.agent.connectAgent")) →")
                        .typeRole(Typography.bodyStrong)
                        .foregroundStyle(theme.fgBase)
                    Text(loc.t("settings.agent.connectAgentHint"))
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Tokens.Space.s12)
                .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: 10))
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("agent-connect-agent")
            HStack(spacing: Tokens.Space.s12) {
                VelaButton(
                    title: loc.t("settings.agent.reconnect"),
                    kind: .secondary,
                    loading: linking,
                    busyTitle: loc.t("settings.agent.connecting"),
                    action: connect
                )
                .accessibilityIdentifier("agent-reconnect")
                VelaButton(title: loc.t("settings.agent.disconnect"), kind: .secondary) {
                    // Forgotten here; the client stays on clientmcp.dev until deleted there.
                    configure(enabled: false, url: "", token: "")
                }
                .accessibilityIdentifier("agent-disconnect")
            }
            problemLine
        }
    }

    /// The MCP server address for the person's AI agent, to copy from here:
    /// masked until Show — it is a credential.
    private func agentAddress(_ url: String) -> some View {
        let masked: String = {
            guard let slash = url.lastIndex(of: "/") else { return "••••••••••••" }
            return String(url[...slash]) + "••••••••••••"
        }()
        return VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(loc.t("settings.agent.agentAddress"))
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
            Text(addressShown ? url : masked)
                .font(Font.system(size: Tokens.TextSize.t13, design: .monospaced))
                .foregroundStyle(theme.fgBase)
                .textSelection(.enabled)
                .accessibilityIdentifier("agent-address-value")
            HStack(spacing: Tokens.Space.s16) {
                Spacer()
                Button(loc.t(addressShown ? "settings.agent.hide" : "settings.agent.show")) {
                    addressShown.toggle()
                }
                .accessibilityIdentifier("agent-address-show")
                ShareLink(item: url) { Text(loc.t("settings.agent.share")) }
                    .accessibilityIdentifier("agent-address-share")
                Button(loc.t(addressCopied ? "settings.agent.copied" : "settings.agent.copy")) {
                    UIPasteboard.general.string = url
                    VelaHaptic.select.play()
                    addressCopied = true
                    Task { @MainActor in
                        try? await Task.sleep(for: .seconds(1.8))
                        addressCopied = false
                    }
                }
                .fontWeight(.semibold)
                .accessibilityIdentifier("agent-address-copy")
            }
            .font(.system(size: Tokens.TextSize.t13))
            .foregroundStyle(theme.fgBase)
            Text(loc.t("settings.agent.agentAddressHint"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
        }
        .padding(Tokens.Space.s12)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("agent-address")
    }

    private func connect() {
        guard !linking else { return }
        linking = true
        problem = nil
        Task { @MainActor in
            problem = await agent.connectWithClientMCP()
            linking = false
        }
    }

    // MARK: Your own bridge (spec 103)

    @ViewBuilder private var ownBridge: some View {
        enableRow
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
