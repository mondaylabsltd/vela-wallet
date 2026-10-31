//
//  MultisigHomeCard.swift
//  VelaWallet
//
//  The multisig's own lines on its home (the web's `MultisigHomeCard` and
//  Activity's waiting group, `MultisigRows`):
//
//  - its rule ("Needs 2 of 3 members") and what it is here (not shared yet,
//    read-only — no member on this device);
//  - what waits for this device ("1 needs your approval · 1 ready"), then
//    every open transaction with the core's one word for it, a tap away;
//  - the group's one note (offline, the service away, a chain unreachable,
//    the last known list) with Retry where the core says one helps;
//  - once created here, its first steps — invite, add funds, what its first
//    transfer out costs — until dismissed or it is set up.
//

import SwiftUI

struct MultisigHomeCard: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let row: MsAccountRow
    let words: MsWords
    /// The multisig holds something on its first network (the balance read).
    var funded = false
    var onOpen: (MultisigSheet) -> Void
    var onFund: () -> Void

    private var view: MsProposalsView { hub.proposalsView }
    private var account: MsAccount { row.account }
    private var following: Bool { view.followed?.lowercased() == account.address.lowercased() }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            header
            // core-ms-2: the banner — said only when something waits and there
            // is one to open (`banner {parts, first}`).
            if following, let banner = view.banner, !banner.isNull,
               let line = Self.waitingLine(banner["parts"]?.array ?? [], words: words) {
                Button {
                    let first = banner["first"]
                    if let chain = first?["chain_id"]?.int, let hash = first?["safe_tx_hash"]?.string {
                        onOpen(.proposal(chainId: chain, safeTxHash: hash))
                    }
                } label: {
                    HStack {
                        Text(verbatim: line)
                            .typeRole(Typography.bodyStrong.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                        Spacer()
                        LucideIcon(.chevronRight, size: LucideIconSize.smallChevron).foregroundStyle(theme.fgSubtle)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("multisig.home.waiting")
            }
            if let note = waitingNote {
                HStack(alignment: .top) {
                    MsNote(text: note.text, tone: .warning, identifier: "multisig.home.note")
                    if note.retry {
                        Button(words.t("service.retry")) { hub.sendProposal(["type": "retry"]) }
                            .font(Typography.actionLabel.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                            .accessibilityIdentifier("multisig.home.retry")
                    }
                }
            }
            if following, !openItems.isEmpty {
                MsSection(title: words.t("panel.pending")) {
                    VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                        ForEach(openItems) { item in
                            proposalRow(item)
                            MsDivider()
                        }
                    }
                }
            }
            ForEach(view.missing.indices, id: \.self) { index in
                let missing = view.missing[index]
                MsNote(text: missingLine(missing), tone: .warning, identifier: "multisig.home.missingNetwork")
            }
            if account.firstRun == true, !setUp {
                firstRun
            }
        }
        .padding(.vertical, Tokens.Space.s12)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("multisig.home")
    }

    // MARK: - Header

    private var header: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            HStack(spacing: Tokens.Space.s8) {
                MsChip(text: ruleChip).accessibilityIdentifier("multisig.home.rule")
                if !row.shared {
                    MsChip(text: words.t("switcher.notShared"), tone: .warning)
                }
                if !row.highRiskOn.isEmpty {
                    MsChip(text: words.t("authority.riskFlag"), tone: .danger)
                }
            }
            if row.localMembers.isEmpty {
                MsNote(text: words.t("panel.readOnly"), identifier: "multisig.home.readOnly")
            } else if !row.shared {
                MsNote(text: words.t("panel.notShared"), tone: .warning, identifier: "multisig.home.notShared")
            }
            ForEach(row.refusedOn, id: \.self) { chainId in
                MsNote(text: words.t("authority.refusedOn", ["network": words.network(chainId)]), tone: .danger)
            }
            MsNavRow(
                title: words.t("panel.members"),
                subtitle: words.ruleTitle(m: account.threshold, n: account.owners.count),
                icon: .usersRound, identifier: "multisig.home.settings"
            ) { onOpen(.settings(address: account.address)) }
        }
    }

    /// The rule in force (core-ms-2's `rule_chip` for the multisig followed:
    /// its members now where it is deployed, its original ones where not).
    private var ruleChip: String {
        guard following, let chip = view.ruleChip, !chip.isNull else {
            return words.ruleChip(m: account.threshold, n: account.owners.count)
        }
        if chip.kind == "differs" { return words.t("presence.ruleDiffers") }
        let rule = chip["rule"]
        return words.ruleChip(m: rule?["m"]?.int ?? account.threshold, n: rule?["n"]?.int ?? account.owners.count)
    }

    /// The waiting line's parts as the core ordered them, in words.
    static func waitingLine(_ parts: [MsJSON], words: MsWords) -> String? {
        let said = parts.compactMap { part -> String? in
            let count = part["count"]?.int ?? 0
            guard count > 0 else { return nil }
            switch part.kind {
            case "needsYou": return words.t("presence.needsYou", count: count, ["count": String(count)])
            case "ready": return words.t("presence.ready", count: count, ["count": String(count)])
            case "waitingPayer": return words.t("presence.waitingPayer", count: count, ["count": String(count)])
            default: return nil
            }
        }
        return said.isEmpty ? nil : said.joined(separator: " · ")
    }

    // MARK: - The waiting list

    private var openItems: [MsProposalItem] {
        view.items.filter { $0.status != "done" }
    }

    private func proposalRow(_ item: MsProposalItem) -> some View {
        Button { onOpen(.proposal(chainId: item.chainId, safeTxHash: item.safeTxHash)) } label: {
            HStack(spacing: Tokens.Space.s12) {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: words.actionTitleNow(chainId: item.chainId, action: item.action, nonce: item.nonce))
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(2)
                    Text(verbatim: "\(words.standing(view.standing(item), item: item)) · \(words.network(item.chainId))")
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(view.standing(item)?.kind == "yourTurn" ? theme.fgBase : theme.fgSubtle)
                        .lineLimit(2)
                }
                Spacer(minLength: Tokens.Space.s8)
                Text(verbatim: "\(item.approved)/\(item.threshold)")
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron).foregroundStyle(theme.fgSubtle)
            }
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("multisig.home.proposal")
    }

    /// The group's one note (`waiting_note`), and whether Retry goes with it.
    private var waitingNote: (text: String, retry: Bool)? {
        guard following, let note = view.waitingNote else { return nil }
        let networks = { (note["chain_ids"]?.array ?? []).compactMap(\.int).map(words.network).joined(separator: ", ") }
        switch note.kind {
        case "offline":
            if let since = note["since"]?.int {
                let time = Date(timeIntervalSince1970: Double(since) / 1000).formatted(date: .omitted, time: .shortened)
                return (words.t("service.offlineSince", ["time": time]), false)
            }
            return (words.t("service.offline"), false)
        case "unreachable": return (words.t("service.unreachableNote"), true)
        case "chainUnreachable": return (words.t("service.chainUnreachable", ["network": networks()]), true)
        case "lastKnown":
            let time = Date(timeIntervalSince1970: Double(note["at"]?.int ?? 0) / 1000).formatted(date: .omitted, time: .shortened)
            return (words.t("panel.lastKnown", ["time": time]), false)
        case "stale": return (words.t("panel.stale"), false)
        case "historyUnreadable": return (words.t("panel.historyUnreadable", ["network": networks()]), true)
        case "checking": return (words.t("panel.checking"), false)
        default: return nil
        }
    }

    private func missingLine(_ missing: MsJSON) -> String {
        let chainId = missing["chainId"]?.int ?? 0
        let count = missing["count"]?.int ?? 0
        let head = words.t("proposal.missingNetwork", ["chainId": String(chainId)])
        return "\(head) · \(words.t("proposal.missingCount", ["n": String(count)]))"
    }

    // MARK: - First steps

    /// Set up on a network already (core-ms-2's `set_up`, read from the chain).
    private var setUp: Bool { following && view.setUp }

    private var firstRun: some View {
        let network = account.chainIds.first
        let solo = account.owners.count == 1
        return VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: words.t("create.firstRunTitle"))
                .typeRole(Typography.rowTitle.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .accessibilityAddTraits(.isHeader)
            if !solo {
                step(
                    title: words.t("create.firstRunInvite"), note: words.t("create.firstRunInviteNote"),
                    done: row.inviteShared, icon: .qrCode, id: "multisig.home.firstRun.invite"
                ) { onOpen(.invite(address: account.address)) }
            }
            step(
                title: words.t("create.firstRunFund"),
                note: network.map { words.t("create.firstRunFundNote", ["network": words.network($0)]) },
                done: funded, icon: .arrowDownLeft, id: "multisig.home.firstRun.fund", action: onFund
            )
            if let network {
                MsNote(text: words.t("create.firstRunCost", ["network": words.network(network)]))
            }
            Button(words.t("create.firstRunDismiss")) {
                hub.send(["type": "set_first_run", "address": account.address, "on": false])
            }
            .font(Typography.actionLabel.scaled(textScale).font)
            .foregroundStyle(theme.fgMuted)
            .accessibilityIdentifier("multisig.home.firstRun.dismiss")
        }
        .padding(Tokens.Space.s16)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r16).fill(theme.bgSunken))
    }

    private func step(title: String, note: String?, done: Bool, icon: LucideGlyph, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: Tokens.Space.s12) {
                LucideIcon(done ? .check : icon, size: LucideIconSize.action)
                    .foregroundStyle(done ? theme.successBase : theme.fgBase)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: title)
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    if let note {
                        Text(verbatim: note)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron).foregroundStyle(theme.fgSubtle)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityValue(done ? Text(verbatim: "✓") : Text(verbatim: ""))
        .accessibilityIdentifier(id)
    }
}

/// The switcher's multisig section: every multisig here, with its rule and
/// what waits on it; Create and Add.
struct MultisigSwitcherSection: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    var onSelect: (String) -> Void
    var onCreate: () -> Void
    var onAdd: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: words.t("switcher.section"))
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .padding(.top, Tokens.Space.s16)
                .accessibilityAddTraits(.isHeader)
            ForEach(hub.accountsView.accounts) { row in
                rowView(row)
                MsDivider()
            }
            MsNavRow(title: words.t("switcher.create"), subtitle: words.t("switcher.createMultisigNote"), icon: .usersRound, identifier: "multisig.switcher.create", action: onCreate)
            MsNavRow(title: words.t("switcher.add"), subtitle: words.t("switcher.addMultisigNote"), icon: .plus, identifier: "multisig.switcher.add", action: onAdd)
        }
    }

    private func rowView(_ row: MsAccountRow) -> some View {
        let selected = hub.accountsView.active?.lowercased() == row.account.address.lowercased()
        // core-ms-2: what waits on it, as the core counts it (`waiting_parts`).
        let parts = hub.proposalsView.waitingParts[row.account.address.lowercased()] ?? []
        let count = parts.filter { $0.kind != "waitingPayer" }.reduce(0) { $0 + ($1["count"]?.int ?? 0) }
        var tags: [String] = [words.ruleChip(m: row.account.threshold, n: row.account.owners.count)]
        if row.localMembers.isEmpty { tags.append(words.t("switcher.readOnly")) }
        if !row.shared { tags.append(words.t("switcher.notShared")) }
        return Button { onSelect(row.account.address) } label: {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: row.account.address, size: 40, tappable: false)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: words.name(of: row.account, rowName: row.name))
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .fontWeight(.semibold)
                        .foregroundStyle(selected ? theme.accentBase : theme.fgBase)
                    Text(verbatim: tags.joined(separator: " · "))
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    Text(verbatim: MsWords.short(row.account.address))
                        .typeRole(Typography.monoSmall.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer(minLength: Tokens.Space.s8)
                if count > 0 {
                    Text(verbatim: String(count))
                        .typeRole(Typography.tabCount.scaled(textScale))
                        .foregroundStyle(theme.fgInverse)
                        .padding(.horizontal, Tokens.Space.s8)
                        .padding(.vertical, Tokens.Space.s2)
                        .background(Capsule().fill(theme.fgBase))
                        .accessibilityLabel(words.t("presence.needsYou", count: count, ["count": String(count)]))
                }
                if selected {
                    LucideIcon(.check, size: LucideIconSize.action).foregroundStyle(theme.accentBase)
                }
            }
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityIdentifier("multisig.switcher.row")
    }
}
