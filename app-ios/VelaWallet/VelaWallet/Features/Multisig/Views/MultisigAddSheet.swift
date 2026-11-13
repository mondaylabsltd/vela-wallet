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
    /// r14-5: what is said is a read that did not come back — Add says Try again.
    @State private var retry = false
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
            Text(prose: words.t("add.intro"))
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
                title: retry && failure != nil ? words.t("review.state.tryAgain") : words.t("add.add"), kind: .ink,
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
                Text(prose: words.t("add.invitedMembers"))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                ForEach(Array(members.enumerated()), id: \.offset) { _, member in
                    let address = member["address"]?.string
                    let kind = member["kind"]?.string.map { $0 == "wallet" ? words.t("add.kindWallet") : words.t("add.kindPasskey") }
                    let you = address.flatMap { words.localName($0) } != nil ? words.t("add.thatsYou") : nil
                    let line = [member["name"]?.string, kind, you].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        if !line.isEmpty {
                            Text(prose: line).typeRole(Typography.rowSub.scaled(textScale)).foregroundStyle(theme.fgBase)
                        }
                        if let address {
                            AddressTextView(MsRules.eip55(address), role: Typography.monoSmall.scaled(textScale))
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
                if chosen == nil { Text(prose: words.t("create.networkNeeded")).tag(-1) }
                ForEach(offered, id: \.self) { id in Text(prose: words.network(id)).tag(id) }
            }
            .pickerStyle(.menu)
            .tint(theme.fgBase)
            .accessibilityIdentifier("multisig.add.network")
        }
    }

    // MARK: - Add

    private func add(acceptRisks: Bool) {
        let typed = address.trimmingCharacters(in: .whitespaces)
        retry = false
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
            retry = failure != nil && hub.accountsView.add.retry
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
        case "busy":
            // r14-5: the network's nodes were rate-limiting — named, with Try again.
            failure = words.t("add.busy", ["network": words.network(outcome["chain_id"]?.int ?? 0)])
        // A closed list of kinds with words (`add.<kind>`): never a key built
        // from a kind the corpus has no words for (r17-17).
        case "notFound", "unavailable", "unsupported", "notMultisig", "unchecked": failure = words.t("add.\(outcome.kind ?? "unavailable")")
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
                Text(prose: words.t("import.pick"))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                ForEach(Array(safes.enumerated()), id: \.offset) { _, safe in
                    safeRow(safe)
                }
                // r15-8: a Safe already here (imported a moment ago) is not counted
                // again — a press that would import nothing is never offered.
                let toImport = picked.filter { view.row($0) == nil }
                VelaButton(
                    title: words.t("import.importSome", ["total": String(toImport.count)]), kind: .ink,
                    enabled: !toImport.isEmpty, loading: view.importing.busy != nil
                ) { importPicked() }
                    .accessibilityIdentifier("multisig.import.go")
            }
        }
    }

    private func safeRow(_ safe: MsJSON) -> some View {
        let address = safe["address"]?.string ?? ""
        let outcome = view.importing.outcomes[address.lowercased()] ?? view.importing.outcomes[address]
        // r15-8: a Safe this import added (by its row's Try again, say) says
        // "Imported", never "Already added".
        let here = view.row(address) != nil
        let already = Self.importAlready(here: here, outcome: outcome)
        let deployed = (safe["chains"]?.array ?? []).contains { $0["deployed"]?.bool == true }
        let status = Self.importStatus(outcome, already: already, words: words)
        // A read that did not come back (the core's `import.retry`): said in
        // full under the row, with Try again — the same `import` asked again.
        let retry = !here && view.importing.offersRetry(address)
        let flags = [MsWords.short(address)] + (deployed ? [] : [words.t("import.undeployed")]) + (retry ? [] : [status].compactMap { $0 })
        return VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Toggle(isOn: Binding(
                get: { picked.contains(address) },
                set: { on in if on { picked.insert(address) } else { picked.remove(address) } }
            )) {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(prose: safe["name"]?.string ?? MsWords.short(address))
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                    Text(prose: flags.joined(separator: " · "))
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            .disabled(here)
            .tint(theme.fgBase)
            .accessibilityIdentifier("multisig.import.safe")
            if retry, let status {
                MsNote(text: status, tone: .warning, identifier: "multisig.import.failed")
                VelaButton(title: words.t("review.state.tryAgain"), kind: .secondary, loading: view.importing.busy?.caseInsensitiveCompare(address) == .orderedSame) {
                    hub.send(Self.importEvent(address))
                }
                .accessibilityIdentifier("multisig.import.retry")
            }
        }
    }

    /// r15-8 (the web's `here`): "Already added" is for a Safe that was here
    /// before this import said anything of it; one this import added says
    /// what the import came to.
    static func importAlready(here: Bool, outcome: MsJSON?) -> Bool { here && outcome == nil }

    /// What a Safe's row says it came to — the add's words for the same
    /// outcomes (`said`): a rate-limited import is busy with its network
    /// named, an unchecked contract names its network, never "unavailable".
    static func importStatus(_ outcome: MsJSON?, already: Bool, words: MsWords) -> String? {
        already ? words.t("import.already") : outcome.map { o -> String in
            let network = words.network(o["chain_id"]?.int ?? 0)
            switch o.kind {
            case "imported": return words.t("import.imported")
            case "notDeployed": return words.t("import.notDeployed")
            case "refused": return words.t("authority.refusedFlag")
            case "highRisk": return words.t("authority.riskFlag")
            case "busy": return words.t("add.busy", ["network": network])
            case "unverified": return words.t("authority.unverified", ["network": network])
            case "notFound", "unavailable", "unsupported": return words.t("add.\(o.kind ?? "unavailable")")
            default: return words.t("add.unavailable")
            }
        }
    }

    /// Importing one exported Safe.
    static func importEvent(_ address: String) -> [String: Any] {
        ["type": "import", "address": address, "accept_risks": false]
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
            hub.send(Self.importEvent(address))
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
                MsNote(text: Self.brokenLine(stand, words: words), tone: .warning, identifier: "multisig.link.broken")
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
                // §18 (c): a lookup that did not come back — the core says
                // whether to offer Try again; it opens the same link again.
                if stand?.retry == true {
                    // r17-20: busy the moment it is pressed, until the lookup
                    // says something new (Opening, or its answer).
                    MsRetryButton(title: words.t("review.state.tryAgain"), said: failedLine,
                                  identifier: "multisig.link.retry") { hub.send(["type": "retry_link"]) }
                }
            case "kept":
                MsNote(text: words.t("proposal.previewNote"), identifier: "multisig.link.kept")
            default:
                HStack(spacing: Tokens.Space.s8) {
                    ProgressView()
                    Text(prose: words.t("proposal.opening"))
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

    private var failedLine: String { Self.failedLine(stand?.reason, words: words) }

    /// r17-17: a link whose lookup failed, in the catalog's words for every
    /// `AddOutcome` kind — never a key built from a kind with no words
    /// (`add.refused`, `add.highRisk` and `add.unverified` are not in the
    /// corpus, so they were drawn as raw keys).
    static func failedLine(_ reason: MsJSON?, words: MsWords) -> String {
        guard let reason else { return words.t("proposal.notFound") }
        let network = words.network(reason["chain_id"]?.int ?? 0)
        switch reason.kind {
        case "notFound": return words.t("proposal.notFound")
        case "busy": return words.t("add.busy", ["network": network])
        case "unverified": return words.t("authority.unverified", ["network": network])
        case "refused": return words.t("authority.refusedTitle")
        case "highRisk": return words.t("authority.riskTitle")
        case "unavailable": return words.t("add.unavailable")
        case "unsupported": return words.t("add.unsupported")
        case "notMultisig": return words.t("add.notMultisig")
        case "unchecked": return words.t("add.unchecked")
        case "chooseNetwork": return words.t("add.chooseNetwork")
        default: return words.t("add.unavailable")
        }
    }

    /// r17-17: a damaged link, in words — never what it held drawn as the
    /// sentence (the zh tester saw "5042002.0x2746…0x8496…" under 多签交易):
    /// a proposal link's "incomplete or damaged", an invite's own fault.
    static func brokenLine(_ stand: MsLinkStand?, words: MsWords) -> String {
        guard stand?.link?.string == "add" else { return words.t("proposal.badLink") }
        return words.t(stand?.fault == "checksum" ? "add.badChecksum" : "add.badInvite")
    }

    private func close() {
        if stand?.kind == "missingNetwork" { hub.send(["type": "drop_link"]) } else { hub.send(["type": "close_link"]) }
        onClose()
    }
}
