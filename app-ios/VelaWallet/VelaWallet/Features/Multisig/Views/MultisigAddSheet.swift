//
//  MultisigAddSheet.swift
//  VelaWallet
//
//  Adding a multisig (M1) — the web's `AddMultisigSheet`, on the accounts
//  machine: by its address (all another member needs), from an invite (its
//  networks and its members' labels ride along; nothing is added until the
//  person's Add), or imported from a Safe{Wallet} export (read by the core,
//  on this device).
//
//  Who can act for it is the chain's, or the core's counterfactual rule
//  (§3.5): a genesis the core refuses is said with every reason and nothing
//  adds it; one something besides its members can act for is shown with
//  what that is, and added only on the person's explicit tap.
//

import SwiftUI
import UniformTypeIdentifiers
import VelaCore

struct MultisigAddSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    /// The invite it was opened from (`MultisigLinkStand.invite`), if any.
    var invite: MsLinkStand?
    var prefill: String?
    var onClose: () -> Void
    var onAdded: (String) -> Void
    var onAddNetwork: ((Int) -> Void)?

    @State private var address = ""
    @State private var failure: String?
    @State private var choosing = false
    @State private var chosen: Int?
    @State private var refused: [MsJSON]?
    @State private var risky: [MsJSON]?
    @State private var busy = false
    @State private var importing = false
    @State private var picked: Set<String> = []
    @State private var results: [String: String] = [:]

    private var view: MsAccountsView { hub.accountsView }
    private var linkChains: [Int] {
        (invite?.chainIds.isEmpty == false ? invite?.chainIds : nil) ?? hub.accountsView.invite?["link"]?["chainIds"]?.array.compactMap(\.int) ?? []
    }
    private var fromLink: Bool {
        guard let multisig = invite?.multisig else { return false }
        return address.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(multisig) == .orderedSame
    }

    var body: some View {
        MsSheet(title: words.t("add.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: onClose) {
            Text(verbatim: words.t("add.intro"))
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            if let invite { inviteFacts(invite) }
            MsField(label: words.t("add.title"), placeholder: words.t("add.placeholder"), text: $address, mono: true, error: failure, identifier: "multisig.add.address")
            if choosing { networkChoice }
            if let refused {
                MsSection(title: words.t("authority.refusedTitle")) {
                    MsNote(text: words.t("authority.refusedBody"), tone: .danger)
                    ForEach(Array(refused.enumerated()), id: \.offset) { _, finding in
                        MsNote(text: words.finding(finding), tone: .danger)
                    }
                }
                .accessibilityIdentifier("multisig.add.refused")
            }
            if let risky {
                MsSection(title: words.t("authority.riskTitle")) {
                    MsNote(text: words.t("authority.riskBody"), tone: .danger)
                    ForEach(Array(risky.enumerated()), id: \.offset) { _, finding in
                        MsNote(text: words.finding(finding), tone: .danger)
                    }
                    VelaButton(title: words.t("authority.riskConfirm"), kind: .danger) { add(acceptRisks: true) }
                        .accessibilityIdentifier("multisig.add.acceptRisk")
                }
            }
            VelaButton(
                title: words.t("add.add"), kind: .ink,
                enabled: !address.trimmingCharacters(in: .whitespaces).isEmpty && !(choosing && chosen == nil),
                loading: busy
            ) { add(acceptRisks: false) }
                .accessibilityIdentifier("multisig.add.add")
            importSection
        }
        .onAppear {
            if address.isEmpty { address = invite?.multisig ?? prefill ?? "" }
            if let fault = invite?.fault {
                failure = words.t(fault == "checksum" ? "add.badChecksum" : "add.badInvite")
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json, .text]) { result in
            readExport(result)
        }
    }

    // MARK: - An invite's facts

    private func inviteFacts(_ invite: MsLinkStand) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            if !linkChains.isEmpty {
                MsNote(text: words.t("add.usedOn", ["networks": words.loc.list(linkChains.map(networkName))]))
            }
            // core-ms-2: an invite naming a network this device lacks.
            if let missing = hub.accountsView.invite?["missing"]?.int {
                let name = hub.accountsView.invite?["missing_name"]?.string ?? networkName(missing)
                MsNote(text: words.t("add.networkMissing", ["network": name]), tone: .warning, identifier: "multisig.add.networkMissing")
                if let onAddNetwork {
                    Button(words.t("proposal.addNetwork")) { onAddNetwork(missing) }
                        .font(Typography.actionLabel.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                }
            }
            let members = invite.link?["members"]?.array ?? []
            if !members.isEmpty {
                Text(verbatim: words.t("add.invitedMembers"))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                ForEach(Array(members.enumerated()), id: \.offset) { _, member in
                    let address = member["address"]?.string
                    let kind = member["kind"]?.string.map { $0 == "wallet" ? words.t("add.kindWallet") : words.t("add.kindPasskey") }
                    let you = address.flatMap { words.localName($0) } != nil ? words.t("add.thatsYou") : nil
                    let line = [member["name"]?.string, kind, you].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        if !line.isEmpty {
                            Text(verbatim: line).typeRole(Typography.rowSub.scaled(textScale)).foregroundStyle(theme.fgBase)
                        }
                        if let address {
                            AddressTextView(MsRules.eip55(address))
                                .typeRole(Typography.monoSmall.scaled(textScale))
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
                    .accessibilityElement(children: .combine)
                }
                MsNote(text: words.t("add.labelsNote"))
            }
        }
        .accessibilityIdentifier("multisig.add.invite")
    }

    /// A network's name: this device's, else the core's catalog
    /// (`networks.name`), else its id.
    private func networkName(_ chainId: Int) -> String {
        ChainCatalog.meta(chainId)?.displayName
            ?? MsRules.string("networks.name", ["chainId": chainId])
            ?? words.network(chainId)
    }

    // MARK: - The network, when nothing says

    private var networkChoice: some View {
        let offered = MsRules.likelyNetworks(["networks": NetworkSpace.chainIds(store: VelaStore()), "hints": ["named": fromLink ? linkChains : []]])
        return VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            MsNote(text: words.t("add.chooseNetwork"), tone: .warning)
            Picker(words.t("create.network"), selection: Binding(get: { chosen ?? -1 }, set: { chosen = $0 })) {
                if chosen == nil { Text(verbatim: words.t("create.networkNeeded")).tag(-1) }
                ForEach(offered, id: \.self) { id in Text(verbatim: words.network(id)).tag(id) }
            }
            .pickerStyle(.menu)
            .tint(theme.fgBase)
            .accessibilityIdentifier("multisig.add.network")
        }
    }

    // MARK: - Add

    private func add(acceptRisks: Bool) {
        let typed = address.trimmingCharacters(in: .whitespaces)
        switch addressEntry(address: typed) {
        case "valid": break
        case "checksum": failure = words.t("send.invalidChecksum"); return
        default: failure = words.t("send.invalidAddress"); return
        }
        busy = true
        failure = nil
        refused = nil
        if !acceptRisks { risky = nil }
        // core-ms-2: only the person's pick; an invite's networks are the
        // machine's to apply after Add (`view.invite`).
        let named: Int? = choosing ? chosen : nil
        hub.send(["type": "add", "address": typed, "network": named.map { $0 as Any } ?? NSNull(), "accept_risks": acceptRisks])
        Task { @MainActor in
            let outcome = await hub.accounts.until(timeout: 120) { view -> MsJSON? in
                guard let add = MultisigMachine.decode(MsAccountsView.self, from: view)?.add,
                      add.phase == .done || add.phase == .failed,
                      add.address?.caseInsensitiveCompare(typed) == .orderedSame
                else { return nil }
                return add.outcome ?? .null
            }
            busy = false
            said(outcome ?? .null)
        }
    }

    private func said(_ outcome: MsJSON) {
        switch outcome.kind {
        case "added":
            let added = outcome["address"]?.string ?? address
            VelaHaptic.success.play()
            onAdded(added)
        case "chooseNetwork": choosing = true
        case "refused": refused = outcome["findings"]?.array ?? []
        case "highRisk": risky = outcome["findings"]?.array ?? []
        case "unverified":
            failure = words.t("authority.unverified", ["network": words.network(outcome["chain_id"]?.int ?? 0)])
        case "notFound", "unavailable", "unsupported", "notMultisig", "unchecked":
            failure = words.t("add.\(outcome.kind ?? "unavailable")")
        default:
            failure = words.t("add.unavailable")
        }
    }

    // MARK: - Import from Safe{Wallet}

    @ViewBuilder private var importSection: some View {
        MsSection(title: words.t("import.or")) {
            MsNote(text: words.t("import.body"))
            Button(words.t("import.choose")) { importing = true }
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                .accessibilityIdentifier("multisig.import.choose")
            if let error = view.importing.error {
                MsNote(text: words.t(error == "empty" ? "import.empty" : "import.notExport"), tone: .warning)
            }
            let safes = view.importing.safes
            if !safes.isEmpty {
                Text(verbatim: words.t("import.pick"))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                ForEach(Array(safes.enumerated()), id: \.offset) { _, safe in
                    safeRow(safe)
                }
                VelaButton(
                    title: words.t("import.importSome", ["total": String(picked.count)]), kind: .ink,
                    enabled: !picked.isEmpty, loading: view.importing.busy != nil
                ) { importPicked() }
                    .accessibilityIdentifier("multisig.import.go")
            }
        }
    }

    private func safeRow(_ safe: MsJSON) -> some View {
        let address = safe["address"]?.string ?? ""
        let already = view.row(address) != nil
        let outcome = view.importing.outcomes[address.lowercased()] ?? view.importing.outcomes[address]
        let deployed = (safe["chains"]?.array ?? []).contains { $0["deployed"]?.bool == true }
        let status: String? = already ? words.t("import.already") : outcome.map { o -> String in
            switch o.kind {
            case "imported": return words.t("import.imported")
            case "notDeployed": return words.t("import.notDeployed")
            case "refused": return words.t("authority.refusedFlag")
            case "highRisk": return words.t("authority.riskFlag")
            default: return words.t("add.\(o.kind ?? "unavailable")")
            }
        }
        return Toggle(isOn: Binding(
            get: { picked.contains(address) },
            set: { on in if on { picked.insert(address) } else { picked.remove(address) } }
        )) {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: safe["name"]?.string ?? MsWords.short(address))
                    .typeRole(Typography.fieldLabel.scaled(textScale))
                Text(verbatim: ([MsWords.short(address)] + (deployed ? [] : [words.t("import.undeployed")]) + [status].compactMap { $0 }).joined(separator: " · "))
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .disabled(already)
        .tint(theme.fgBase)
        .accessibilityIdentifier("multisig.import.safe")
    }

    private func readExport(_ result: Result<URL, Error>) {
        guard case .success(let url) = result else { return }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let data = try? Data(contentsOf: url) else { return }
        picked = []
        hub.send(["type": "parse_export", "json": String(decoding: data, as: UTF8.self)])
    }

    private func importPicked() {
        for address in picked where view.row(address) == nil {
            hub.send(["type": "import", "address": address, "accept_risks": false])
        }
    }
}

/// A multisig link the app was opened with, where it stands on the accounts
/// machine's one link flow (`MultisigLinkStand`): opening, a network this
/// device lacks, a member here (its transaction opens), a preview (no member
/// here — read only, ms-21), failed, damaged, or kept until there is an
/// account. Replaces "opens in the web wallet for now": the native flow.
struct MultisigLinkStandSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    var onClose: () -> Void
    var onAddNetwork: ((Int) -> Void)?

    var body: some View {
        MsSheet(title: title, closeLabel: words.loc.t("common.done"), onClose: close) {
            switch stand?.kind {
            case "broken":
                MsNote(text: stand?.text ?? words.t("proposal.badLink"), tone: .warning, identifier: "multisig.link.broken")
            case "missingNetwork":
                let chainId = stand?.chainId ?? 0
                MsNote(text: words.t("proposal.missingNetwork", ["chainId": String(chainId)]), tone: .warning, identifier: "multisig.link.missingNetwork")
                MsNote(text: words.t("proposal.missingNetworkBody"))
                if let onAddNetwork {
                    VelaButton(title: words.t("proposal.addNetwork"), kind: .ink) { onAddNetwork(chainId) }
                        .accessibilityIdentifier("multisig.link.addNetwork")
                }
            case "failed":
                MsNote(text: failedLine, tone: .warning, identifier: "multisig.link.failed")
            case "kept":
                MsNote(text: words.t("proposal.previewNote"), identifier: "multisig.link.kept")
            default:
                HStack(spacing: Tokens.Space.s8) {
                    ProgressView()
                    Text(verbatim: words.t("proposal.opening"))
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
                .accessibilityIdentifier("multisig.link.opening")
            }
        }
    }

    private var stand: MsLinkStand? { hub.accountsView.link }

    private var title: String {
        stand?.kind == "invite" ? words.t("add.title") : words.t("proposal.title")
    }

    private var failedLine: String {
        guard let reason = stand?.reason else { return words.t("proposal.notFound") }
        switch reason.kind {
        case "notFound": return words.t("proposal.notFound")
        case "unavailable": return words.t("add.unavailable")
        default: return words.t("add.\(reason.kind ?? "unavailable")")
        }
    }

    private func close() {
        if stand?.kind == "missingNetwork" { hub.send(["type": "drop_link"]) } else { hub.send(["type": "close_link"]) }
        onClose()
    }
}
