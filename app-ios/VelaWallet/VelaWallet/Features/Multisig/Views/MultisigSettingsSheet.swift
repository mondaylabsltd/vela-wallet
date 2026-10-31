//
//  MultisigSettingsSheet.swift
//  VelaWallet
//
//  A multisig's settings (M1, M3) — the web's `MultisigSettings`: its name on
//  this device and its shared name (a rename is one of its own transactions,
//  said honestly while its record is on its way — r7-10), its members per
//  network as each chain holds them (the members machine), the invite, its
//  record (shared, or "share it again" on one passkey), its setup on
//  Ethereum, and removing it from this device. "Change members" opens the
//  members machine's composer.
//

import SwiftUI

struct MultisigSettingsSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    let address: String
    var onClose: () -> Void
    var onOpen: (MultisigSheet) -> Void
    var onRemoved: () -> Void

    @State private var label = ""
    @State private var newName = ""
    @State private var removing = false
    @State private var renaming = false

    private var row: MsAccountRow? { hub.accountsView.row(address) }
    private var members: MsJSON { hub.membersView }

    var body: some View {
        MsSheet(title: row.map { words.name(of: $0.account, rowName: $0.name) } ?? words.t("switcher.section"),
                closeLabel: words.loc.t("common.done"), tall: true, onClose: onClose) {
            if let row {
                identity(row)
                membersSection(row)
                MsNavRow(title: words.t("members.title"), icon: .usersRound, identifier: "multisig.settings.members") {
                    onOpen(.members(address: address))
                }
                if row.account.owners.count > 1 {
                    MsNavRow(title: words.t("panel.invite"), icon: .qrCode, identifier: "multisig.settings.invite") {
                        onOpen(.invite(address: address))
                    }
                }
                recordSection(row)
                renameSection(row)
                backupSection(row)
                Button(words.t("settings.remove"), role: .destructive) { removing = true }
                    .font(Typography.actionLabel.scaled(textScale).font)
                    .padding(.top, Tokens.Space.s16)
                    .accessibilityIdentifier("multisig.settings.remove")
                    .alert(words.t("settings.remove"), isPresented: $removing) {
                        Button(words.t("settings.remove"), role: .destructive) {
                            hub.send(["type": "remove", "address": address])
                            onRemoved()
                        }
                        Button(words.loc.t("common.cancel"), role: .cancel) {}
                    } message: {
                        Text(verbatim: words.t("switcher.removeBody"))
                    }
            }
        }
        .task(id: address) {
            await hub.showMembers(address)
            hub.send(["type": "plan_rename", "address": address])
            hub.send(["type": "check_backup", "address": address])
        }
    }

    // MARK: - Identity

    private func identity(_ row: MsAccountRow) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: row.account.address, size: 40, tappable: false)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: words.ruleTitle(m: row.account.threshold, n: row.account.owners.count))
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Text(verbatim: row.account.chainIds.map(words.network).joined(separator: ", "))
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            MsRow(label: words.t("create.address"), value: row.account.address, mono: true, copyable: true)
        }
    }

    // MARK: - Members per network

    @ViewBuilder private func membersSection(_ row: MsAccountRow) -> some View {
        let original = members["original"]
        let originalMembers = original?["members"]?.array ?? []
        MsSection(title: words.t("settings.originalMembers")) {
            ForEach(Array((originalMembers.isEmpty ? row.account.owners.map { MsJSON.object(["address": .string($0)]) } : originalMembers).enumerated()), id: \.offset) { _, member in
                memberLine(member)
            }
            MsNote(text: words.t("settings.originalNote"))
        }
        ForEach(Array((members["networks"]?.array ?? []).enumerated()), id: \.offset) { _, network in
            let chainId = network["chain_id"]?.int ?? 0
            let state = network["state"]
            switch state?.kind {
            case "deployed":
                MsSection(title: words.t("settings.currentOn", ["network": words.network(chainId)])) {
                    if state?["same"]?.bool == true {
                        MsNote(text: words.t("settings.sameAsOriginal"))
                    } else {
                        ForEach(Array((state?["members"]?.array ?? []).enumerated()), id: \.offset) { _, member in
                            memberLine(member, added: member["added"]?.bool == true)
                        }
                        ForEach(Array((state?["removed"]?.array ?? []).enumerated()), id: \.offset) { _, member in
                            memberLine(member, removed: true)
                        }
                        if state?["threshold_was"]?.int != state?["threshold_is"]?.int {
                            MsNote(text: words.t("settings.thresholdChanged", [
                                "was": state?["threshold_was"]?.string ?? "", "is": state?["threshold_is"]?.string ?? "",
                            ]))
                        }
                    }
                }
                .accessibilityIdentifier("multisig.settings.network.\(chainId)")
            case "undeployed":
                MsNote(text: words.t("settings.undeployedOn", ["network": words.network(chainId)]), identifier: "multisig.settings.undeployed")
            case "unreadable":
                MsNote(text: words.t("proposal.cantCheck", ["network": words.network(chainId)]), tone: .warning)
            default:
                EmptyView()
            }
        }
    }

    private func memberLine(_ member: MsJSON, added: Bool = false, removed: Bool = false) -> some View {
        let address = member["address"]?.string ?? ""
        let name = member["name"]?.string ?? words.localName(address) ?? words.keptName(address)
        var tags: [String] = []
        if member["on_device"]?.bool == true || words.localName(address) != nil { tags.append(words.t("panel.thisDevice")) }
        if added { tags.append(words.t("settings.added")) }
        if removed { tags.append(words.t("settings.removed")) }
        return HStack(spacing: Tokens.Space.s12) {
            IdenticonAvatar(seed: address, size: 28, tappable: false)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: name ?? MsWords.short(address))
                    .typeRole(Typography.fieldLabel.scaled(textScale))
                    .foregroundStyle(removed ? theme.fgSubtle : theme.fgBase)
                    .strikethrough(removed)
                Text(verbatim: ([MsWords.short(address)] + tags).joined(separator: " · "))
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.vertical, Tokens.Space.s4)
        .accessibilityElement(children: .combine)
    }

    // MARK: - The record

    @ViewBuilder private func recordSection(_ row: MsAccountRow) -> some View {
        if !row.shared {
            MsSection(title: words.t("settings.shareAgain")) {
                MsNote(text: words.t(row.recordWaiting ? "panel.notShared" : "settings.shareAgainNote"), tone: .warning)
                if let failure = hub.accountsView.shareAgainFailure[address.lowercased()] ?? hub.accountsView.shareAgainFailure[address] {
                    MsNote(text: words.failureText(failure), tone: .danger)
                }
                if let owner = row.localMembers.first, !row.recordWaiting {
                    VelaButton(
                        title: words.t("settings.shareAgain"), kind: .ink,
                        loading: (hub.accountsView.shareAgain[address.lowercased()] ?? hub.accountsView.shareAgain[address]) == .working
                    ) {
                        hub.send(["type": "share_again", "address": address, "owner": owner])
                    }
                    .accessibilityIdentifier("multisig.settings.shareAgain")
                }
            }
        }
    }

    // MARK: - Shared name (r7-10: the record's state said honestly)

    @ViewBuilder private func renameSection(_ row: MsAccountRow) -> some View {
        let plan = hub.accountsView.rename[address.lowercased()] ?? hub.accountsView.rename[address]
        MsSection(title: words.t("settings.renameTitle")) {
            if let plan {
                switch plan.blocked {
                case "unrecorded": MsNote(text: words.t("settings.renameNotRecorded"), identifier: "multisig.settings.rename.blocked")
                case "pending":
                    MsNote(text: words.t("settings.renamePending", ["network": words.network(plan.landsOn ?? plan.chainId ?? 100)]), identifier: "multisig.settings.rename.pending")
                case "unreachable", "elsewhere": MsNote(text: words.t("settings.renameUnavailable"), identifier: "multisig.settings.rename.blocked")
                default:
                    if let chainId = plan.chainId {
                        if plan.deployed == false {
                            MsNote(text: words.t("settings.renameNotDeployed", ["network": words.network(chainId)]))
                        } else {
                            MsNote(text: words.t("settings.renameNote", ["network": words.network(chainId)]))
                            if renaming {
                                MsField(label: words.t("create.name"), placeholder: words.name(of: row.account), text: $newName, identifier: "multisig.settings.rename.field")
                                VelaButton(title: words.t("settings.renameAction"), kind: .primary, enabled: MsWords.validName(newName.trimmingCharacters(in: .whitespaces))) {
                                    let name = newName.trimmingCharacters(in: .whitespaces)
                                    renaming = false
                                    Task { @MainActor in
                                        _ = await hub.propose(chainId: chainId, proposal: ["kind": "rename", "name": name], address: address)
                                    }
                                }
                                .accessibilityIdentifier("multisig.settings.rename.propose")
                            } else {
                                Button(words.t("settings.renameAction")) { renaming = true; newName = words.name(of: row.account) }
                                    .font(Typography.actionLabel.scaled(textScale).font)
                                    .foregroundStyle(theme.fgBase)
                                    .accessibilityIdentifier("multisig.settings.rename")
                            }
                        }
                    }
                }
            } else {
                ProgressView()
            }
        }
    }

    // MARK: - Setup on Ethereum

    @ViewBuilder private func backupSection(_ row: MsAccountRow) -> some View {
        let state = hub.accountsView.backup[address.lowercased()] ?? hub.accountsView.backup[address]
        if let state, state != "unavailable" {
            MsSection(title: words.t("settings.backupTitle")) {
                let line: String = switch state {
                case "backedUp": words.t("settings.backedUp")
                case "notBackedUp", "failed": words.t("settings.notBackedUp")
                case "sent": words.t("proposal.executing")
                case "checking": words.t("panel.checking")
                default: words.t("settings.couldNotCheck")
                }
                MsNote(text: line, tone: state == "backedUp" ? .positive : .neutral, identifier: "multisig.settings.backup")
                MsNote(text: words.t("settings.backupNote"))
                if state == "notBackedUp" || state == "failed", let from = row.localMembers.first {
                    VelaButton(title: words.t("settings.backupTitle"), kind: .ink) {
                        hub.send(["type": "save_to_ethereum", "address": address, "from": from])
                    }
                }
                if state == "couldNotCheck" {
                    Button(words.t("service.retry")) { hub.send(["type": "check_backup", "address": address]) }
                        .font(Typography.actionLabel.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                }
            }
        }
    }
}

/// Changing who the members are and how many must approve (M3) — the web's
/// `MemberChanges`, on the members machine: Add member, Replace and Remove
/// on each member, the approvals stepper. Each action is ONE proposal per
/// network the multisig is deployed on, built by the core against that
/// network's members; a refusal is said for that network before anything is
/// signed. The rule each leaves ("Approvals: 2 of 3 → 2 of 4") is shown.
struct MultisigMembersSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    let address: String
    var onClose: () -> Void

    @State private var entry = ""
    @State private var proposing: [Int: String] = [:]

    private var view: MsJSON { hub.membersView }

    var body: some View {
        MsSheet(title: words.t("members.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: close) {
            if view["none_deployed"]?.bool == true {
                MsNote(text: words.t("members.noneDeployed"), tone: .warning, identifier: "multisig.members.noneDeployed")
            }
            if let recovery = view["recovery"], !recovery.isNull {
                MsNote(text: recovery.kind == "every_member" ? words.t("members.recoveryAll")
                    : words.t("members.recovery", ["m": recovery["m"]?.string ?? "", "n": recovery["n"]?.string ?? ""]))
            }
            groups
            compose
        }
        .task(id: address) { await hub.showMembers(address) }
    }

    // MARK: - Changes waiting

    @ViewBuilder private var groups: some View {
        let list = view["groups"]?.array ?? []
        if !list.isEmpty {
            MsSection(title: words.t("members.waitingTitle")) {
                ForEach(Array(list.enumerated()), id: \.offset) { _, group in
                    let parts = (group["parts"]?.array ?? []).map {
                        "\(words.network($0["chain_id"]?.int ?? 0)) \($0["approved"]?.int ?? 0)/\($0["threshold"]?.int ?? 0)"
                    }.joined(separator: " · ")
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(verbatim: words.actionTitleNow(chainId: 0, action: group["action"], nonce: ""))
                            .typeRole(Typography.fieldLabel.scaled(textScale))
                        Text(verbatim: parts).typeRole(Typography.rowSub.scaled(textScale)).foregroundStyle(theme.fgSubtle)
                    }
                    .accessibilityElement(children: .combine)
                }
            }
        }
    }

    // MARK: - Composing a change

    @ViewBuilder private var compose: some View {
        let composing = view["compose"]
        let rows = view["rows"]?.array ?? []
        if composing == nil || composing?.isNull == true {
            MsSection(title: words.t("panel.members")) {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    let address = row["address"]?.string ?? ""
                    let name = row["name"]?.string ?? words.label(address)
                    HStack {
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            Text(verbatim: name).typeRole(Typography.fieldLabel.scaled(textScale))
                            Text(verbatim: MsWords.short(address)).typeRole(Typography.monoSmall.scaled(textScale)).foregroundStyle(theme.fgSubtle)
                        }
                        Spacer()
                        Menu {
                            Button(words.t("members.replaceLabel", ["name": name])) {
                                hub.sendMembers(["type": "compose", "compose": ["kind": "replace", "owner": address]])
                            }
                            Button(words.t("members.removeLabel", ["name": name]), role: .destructive) {
                                hub.sendMembers(["type": "compose", "compose": ["kind": "remove", "owner": address]])
                            }
                        } label: {
                            LucideIcon(.ellipsis, size: LucideIconSize.action).foregroundStyle(theme.fgMuted)
                        }
                        .accessibilityLabel(words.t("members.replaceLabel", ["name": name]))
                        .accessibilityIdentifier("multisig.members.menu")
                    }
                    .padding(.vertical, Tokens.Space.s4)
                }
                HStack(spacing: Tokens.Space.s16) {
                    Button(words.t("members.add")) { hub.sendMembers(["type": "compose", "compose": ["kind": "add"]]) }
                        .accessibilityIdentifier("multisig.members.add")
                    Button(words.t("members.thresholdTitle")) { hub.sendMembers(["type": "compose", "compose": ["kind": "threshold"]]) }
                        .accessibilityIdentifier("multisig.members.threshold")
                }
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                MsNote(text: words.t("members.lostPasskey"))
            }
        } else if let composing {
            composer(composing)
        }
    }

    @ViewBuilder private func composer(_ composing: MsJSON) -> some View {
        let kind = composing.kind
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if kind == "replace" {
                MsNote(text: words.t("members.replaceIntro", ["name": words.label(composing["owner"]?.string ?? "")]))
            } else if kind == "remove" {
                MsNote(text: words.t("members.removeIntro", ["name": words.label(composing["owner"]?.string ?? "")]))
            }
            if kind == "add" || kind == "replace" { entryField }
            if let stepper = view["stepper"], !stepper.isNull {
                HStack {
                    Text(verbatim: kind == "threshold" ? words.t("members.thresholdTitle") : words.t("members.thresholdAfter"))
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                    Spacer()
                    Button { hub.sendMembers(["type": "step", "by": -1]) } label: { Text(verbatim: "−") }
                        .disabled(stepper["can_fewer"]?.bool != true)
                        .accessibilityLabel(words.t("create.fewer"))
                    Text(verbatim: words.t("members.ruleValue", ["m": stepper["value"]?.string ?? "", "n": stepper["n"]?.string ?? ""]))
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .accessibilityIdentifier("multisig.members.rule")
                    Button { hub.sendMembers(["type": "step", "by": 1]) } label: { Text(verbatim: "+") }
                        .disabled(stepper["can_more"]?.bool != true)
                        .accessibilityLabel(words.t("create.more"))
                }
                .font(Typography.rowTitle.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
            }
            plan
            VelaButton(title: words.t("send.propose"), kind: .primary, enabled: view["can_propose"]?.bool == true, haptic: .confirm) {
                propose()
            }
            .accessibilityIdentifier("multisig.members.propose")
            VelaButton(title: words.loc.t("common.cancel"), kind: .secondary) {
                hub.sendMembers(["type": "compose", "compose": NSNull()])
            }
        }
    }

    private var entryField: some View {
        let entryView = view["entry"]
        return VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            MsField(
                label: words.t("members.entryLabel"), placeholder: words.t("members.entryPlaceholder"),
                text: Binding(get: { entry }, set: { entry = $0; hub.sendMembers(["type": "entry", "text": $0]) }),
                mono: true, error: entryNote(entryView?["note"]), identifier: "multisig.members.entry"
            )
            .onSubmit { hub.sendMembers(["type": "read_entry"]) }
            if entryView?["resolving"]?.bool == true {
                MsNote(text: words.t("members.resolving", ["name": entry]))
            }
            ForEach(Array((entryView?["suggestions"]?.array ?? []).prefix(5).enumerated()), id: \.offset) { _, suggestion in
                let address = suggestion["address"]?.string ?? ""
                Button {
                    entry = address
                    hub.sendMembers(["type": "pick", "address": address])
                } label: {
                    Text(verbatim: "\(suggestion["name"]?.string ?? MsWords.short(address)) · \(MsWords.short(address))")
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                }
            }
            if entryView?["entry"]?.isNull == false {
                MsField(
                    label: words.t("members.memberName"), placeholder: words.t("members.memberNamePlaceholder"),
                    text: Binding(get: { entryView?["name"]?.string ?? "" }, set: { hub.sendMembers(["type": "entry_name", "name": $0]) }),
                    identifier: "multisig.members.name"
                )
                MsNote(text: words.t("members.nameNote"))
            }
        }
    }

    private func entryNote(_ note: MsJSON?) -> String? {
        switch note?.kind {
        case "invalid_address": words.t("members.refuse.notAnAddress")
        case "invalid_checksum": words.t("send.invalidChecksum")
        case "no_one": words.t("members.refuse.noOne")
        case "name_not_found": words.t("members.nameNotFound", ["name": note?["name"]?.string ?? ""])
        case "name_unavailable": words.t("members.nameUnavailable", ["name": note?["name"]?.string ?? ""])
        default: nil
        }
    }

    /// One proposal per network, each with the rule it leaves or why not.
    @ViewBuilder private var plan: some View {
        let networks = view["plan"]?["networks"]?.array ?? []
        if !networks.isEmpty {
            MsSection(title: words.t("members.perNetwork")) {
                ForEach(Array(networks.enumerated()), id: \.offset) { _, network in
                    let chainId = network["chain_id"]?.int ?? 0
                    let verdict = network["verdict"]
                    if let state = proposing[chainId] {
                        MsNote(text: state, identifier: "multisig.members.proposing.\(chainId)")
                    } else if verdict?.kind == "refused" || network["proposal"]?.isNull != false {
                        MsNote(text: words.t("members.failedOn", ["network": words.network(chainId), "reason": refusal(verdict)]), tone: .warning)
                    } else {
                        let before = verdict?["before"], after = verdict?["after"]
                        MsNote(text: words.t("members.onNetwork", [
                            "network": words.network(chainId),
                            "before": words.ruleText(m: before?["m"]?.int ?? before?["threshold"]?.int ?? 0, n: before?["n"]?.int ?? before?["owners"]?.int ?? 0),
                            "after": words.ruleText(m: after?["m"]?.int ?? after?["threshold"]?.int ?? 0, n: after?["n"]?.int ?? after?["owners"]?.int ?? 0),
                        ]), identifier: "multisig.members.plan.\(chainId)")
                    }
                }
            }
        }
    }

    private func refusal(_ verdict: MsJSON?) -> String {
        let reason = verdict?["reason"] ?? verdict
        switch reason?.kind {
        case "alreadyMember": return words.t("members.refuse.alreadyMember")
        case "notMember": return words.t("members.refuse.notMember")
        case "lastMember": return words.t("members.refuse.lastMember")
        case "theMultisig": return words.t("members.refuse.theMultisig")
        case "thresholdZero": return words.t("members.refuse.thresholdZero")
        case "unchanged": return words.t("members.refuse.unchanged")
        case "changeWaiting": return words.t("members.refuse.changeWaiting")
        case "thresholdTooHigh":
            return words.t("members.refuse.thresholdTooHigh", ["owners": reason?["owners"]?.string ?? "", "threshold": reason?["threshold"]?.string ?? ""])
        case "belowThreshold":
            return words.t("members.refuse.belowThreshold", ["owners": reason?["owners"]?.string ?? "", "threshold": reason?["threshold"]?.string ?? ""])
        case "tooMany": return words.t("members.refuse.tooMany", ["max": reason?["max"]?.string ?? ""])
        default: return words.t("members.refuse.noOne")
        }
    }

    /// Each network's proposal through the proposals machine, one passkey each.
    private func propose() {
        let networks = (view["plan"]?["networks"]?.array ?? []).filter { $0["proposal"]?.isNull == false }
        hub.sendMembers(["type": "proposing"])
        Task { @MainActor in
            for network in networks {
                let chainId = network["chain_id"]?.int ?? 0
                guard let proposal = network["proposal"]?.any as? [String: Any] else { continue }
                proposing[chainId] = words.t("members.proposing", ["network": words.network(chainId)])
                let settled = await hub.propose(chainId: chainId, proposal: proposal, address: address)
                proposing[chainId] = settled?.kind == "done"
                    ? words.t("members.proposed", ["network": words.network(chainId)])
                    : words.t("members.failedOn", [
                        "network": words.network(chainId),
                        "reason": words.actionFailure(settled, chainId: chainId) ?? words.t("proposal.reason.unknown"),
                    ])
            }
            hub.sendMembers(["type": "compose", "compose": NSNull()])
        }
    }

    private func close() {
        hub.sendMembers(["type": "compose", "compose": NSNull()])
        onClose()
    }
}

/// The invite (M1): a code and a link that add the multisig in Vela with its
/// network and the members' names (the core's `inviteLink`), handed on by
/// share sheet or copy — the home's first step ticks (`invite_shared`).
struct MultisigInviteSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    let address: String
    var onClose: () -> Void

    @State private var copied = false

    private var link: String? {
        guard let row = hub.accountsView.row(address) else { return nil }
        let known = row.account.members.reduce(into: [String: Any]()) { out, pair in
            out[pair.key] = ["name": pair.value.name.map { $0 as Any } ?? NSNull(), "kind": pair.value.kind.map { $0 as Any } ?? NSNull()]
        }
        return MsRules.inviteLink(
            origin: MsRules.string("shareOrigin", ["origin": "app://ios"]) ?? "https://getvela.app",
            locale: MultisigServiceURL.locale, address: row.account.address, owners: row.account.owners,
            chainIds: row.account.chainIds, now: hub.proposalsView.membersAnywhere, known: known
        )
    }

    var body: some View {
        MsSheet(title: words.t("panel.invite"), closeLabel: words.loc.t("common.done"), onClose: onClose) {
            MsNote(text: words.t("invite.body"))
            if let link {
                if let modules = QrCode.modules(link) {
                    MsQrView(modules: modules)
                        .frame(maxWidth: .infinity)
                        .aspectRatio(1, contentMode: .fit)
                        .accessibilityLabel(words.t("invite.qrLabel"))
                        .accessibilityIdentifier("multisig.invite.qr")
                }
                Text(verbatim: link)
                    .typeRole(Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("multisig.invite.link")
                HStack(spacing: Tokens.Space.s12) {
                    ShareLink(item: link, message: Text(verbatim: words.t("invite.shareText"))) {
                        Label(words.t("proposal.share"), systemImage: "square.and.arrow.up")
                    }
                    .simultaneousGesture(TapGesture().onEnded { shared() })
                    .accessibilityIdentifier("multisig.invite.share")
                    Button(copied ? words.t("proposal.copied") : words.t("create.copyInvite")) {
                        UIPasteboard.general.string = link
                        copied = true
                        VelaHaptic.select.play()
                        shared()
                    }
                    .accessibilityIdentifier("multisig.invite.copy")
                }
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
            }
            MsNote(text: words.t("invite.outsideHint"))
        }
        .onAppear { shared() }
    }

    /// The invite was handed on (shown): the home's first step ticks.
    private func shared() {
        hub.send(["type": "invite_shared", "address": address])
    }
}

/// A QR code's modules on a white card (scanners want contrast, not palette).
struct MsQrView: View {
    let modules: [[Bool]]

    var body: some View {
        Canvas { context, size in
            let width = modules.count
            guard width > 0 else { return }
            let quiet = 2
            let cell = min(size.width, size.height) / CGFloat(width + quiet * 2)
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
            for (row, line) in modules.enumerated() {
                for (col, on) in line.enumerated() where on {
                    let rect = CGRect(x: CGFloat(col + quiet) * cell, y: CGFloat(row + quiet) * cell, width: cell, height: cell)
                    context.fill(Path(rect), with: .color(.black))
                }
            }
        }
    }
}
