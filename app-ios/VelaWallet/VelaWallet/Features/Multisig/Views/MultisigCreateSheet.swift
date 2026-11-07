//
//  MultisigCreateSheet.swift
//  VelaWallet
//
//  Creating a multisig (M1) — the web's `CreateMultisigSheet`, on the
//  accounts machine: its name and members (the order is part of the
//  address; the name is a label written with its record), how many must
//  approve, a review that shows the address before anything is signed, then
//  ONE passkey — the creator's proof, with which the service records it.
//  Nothing is deployed and no fee is paid until its first transfer out.
//
//  The machine does it: `new_draft` (the salt, fixed per draft), `draft`
//  (the address the review shows), `check_networks`, `read_kinds` (what each
//  member is), `create` (one passkey, the account kept, then its record).
//  The form is kept by the machine (`keep_draft`) until created or
//  discarded, and the sheet opens on it (r7-9). The networks of the other
//  space are said, with the way there (r7-9).
//

import SwiftUI
import UIKit
import VelaCore

struct MultisigCreateSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    /// This device's Vela accounts (address, name), the one shown first.
    let accounts: [(address: String, name: String)]
    /// The contacts that may be members (address, name).
    let contacts: [(address: String, name: String)]
    /// The space in front's networks, in the wallet's order.
    let spaceNetworks: [Int]
    var onClose: () -> Void
    var onCreated: (String) -> Void
    var onInvite: (String) -> Void

    enum Step { case members, threshold, review, created }

    @State private var step: Step = .members
    @State private var owners: [String] = []
    @State private var names: [String: String] = [:]
    @State private var name = ""
    @State private var nameAsked = false
    @State private var typed = ""
    @State private var typedName = ""
    @State private var typedError: String?
    @State private var threshold = 1
    @State private var thresholdFor = 0
    @State private var chainId: Int?
    @State private var chose = false
    @State private var creator: String?
    @State private var started = false
    @State private var resumed = false
    @State private var created: String?
    @State private var pickingContact = false
    @State private var ensAsked: String?

    private var sheet: MsCreateView { hub.accountsView.create }

    var body: some View {
        MsSheet(title: words.t("create.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: close, footer: stepFooter) {
            switch step {
            case .members: membersStep
            case .threshold: thresholdStep
            case .review: reviewStep
            case .created: createdStep
            }
        }
        .interactiveDismissDisabled(sheet.phase == .working)
        .onAppear(perform: start)
        .onChange(of: draftKey) { _, _ in keep() }
        .onChange(of: draftAddressKey) { _, _ in askAddress() }
        .onChange(of: owners) { _, _ in readKinds() }
        .onChange(of: hub.accountsView.add.ens) { _, ens in ensArrived(ens) }
        .onChange(of: sheet.threshold?["value"]?.int) { _, value in
            if let value { threshold = value }
        }
    }

    // MARK: - Start, the kept draft

    private func start() {
        guard !started else { return }
        started = true
        hub.send(["type": "new_draft"])
        hub.send(["type": "check_networks", "chain_ids": spaceNetworks])
        if let kept = sheet.kept, !kept.owners.isEmpty {
            owners = kept.owners
            names = kept.members.compactMapValues { $0.name }.reduce(into: [:]) { $0[$1.key.lowercased()] = $1.value }
            name = kept.name
            threshold = kept.threshold
            thresholdFor = kept.owners.count
            if let chain = kept.chainId { chainId = chain; chose = true }
            resumed = true
        } else if let first = accounts.first {
            owners = [first.address]
        }
        if chainId == nil { chainId = firstDefault }
    }

    private var draftKey: String {
        "\(owners)|\(names)|\(name)|\(threshold)|\(chose ? chainId ?? 0 : 0)"
    }

    /// Every change is kept by the machine (a blank sheet keeps nothing).
    private func keep() {
        guard started, step != .created else { return }
        let members = owners.reduce(into: [String: Any]()) { out, owner in
            if let said = names[owner.lowercased()], !said.isEmpty { out[owner] = ["name": said] }
        }
        hub.send(["type": "keep_draft", "draft": [
            "owners": owners, "members": members, "name": name, "threshold": threshold,
            "chainId": chose ? (chainId.map { $0 as Any } ?? NSNull()) : NSNull(),
        ]])
    }

    private func discard() {
        hub.send(["type": "discard_draft"])
        owners = accounts.first.map { [$0.address] } ?? []
        names = [:]
        name = ""
        threshold = 1
        thresholdFor = 0
        nameAsked = false
        chose = false
        chainId = firstDefault
        resumed = false
    }

    // MARK: - Name

    private var cleanName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var nameBytes: Int { cleanName.utf8.count }
    private static let nameLimit = 64
    private var nameOk: Bool { !cleanName.isEmpty && MsWords.validName(cleanName) }
    private var nameError: String? {
        if cleanName.isEmpty { return nameAsked ? words.t("create.nameNeeded") : nil }
        if nameBytes > Self.nameLimit {
            return words.t("create.nameTooLong", ["bytes": String(nameBytes), "limit": String(Self.nameLimit)])
        }
        return nameOk ? nil : words.t("create.nameInvalid")
    }

    // MARK: - Members

    private var local: [String] {
        owners.filter { owner in accounts.contains { $0.address.caseInsensitiveCompare(owner) == .orderedSame } }
    }

    private func label(_ address: String) -> String {
        accounts.first { $0.address.caseInsensitiveCompare(address) == .orderedSame }?.name
            ?? names[address.lowercased()]
            ?? words.label(address)
    }

    private var membersStep: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            Text(prose: words.t("create.intro"))
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            if resumed {
                HStack {
                    MsNote(text: words.t("create.draftKept"))
                    Button(words.t("create.discardDraft"), action: discard)
                        .font(Typography.actionLabel.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                        .accessibilityIdentifier("multisig.create.discard")
                }
            }
            MsField(
                label: nameBytes >= Self.nameLimit - 8
                    ? "\(words.t("create.name")) · \(words.t("create.nameBytes", ["bytes": String(nameBytes), "limit": String(Self.nameLimit)]))"
                    : words.t("create.name"),
                placeholder: words.t("create.namePlaceholder"), text: $name, error: nameError,
                identifier: "multisig.create.name"
            )
            if nameError == nil {
                MsNote(text: words.t("create.nameHint"))
            }
            MsSection(title: words.t("create.yourAccounts")) {
                ForEach(accounts, id: \.address) { account in
                    let on = owners.contains { $0.caseInsensitiveCompare(account.address) == .orderedSame }
                    Toggle(isOn: Binding(
                        get: { on },
                        set: { _ in toggle(account.address) }
                    )) {
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            Text(prose: account.name).typeRole(Typography.fieldLabel.scaled(textScale))
                            AddressTextView(MsWords.short(account.address), role: Typography.monoSmall.scaled(textScale))
                                .foregroundStyle(theme.fgSubtle)
                        }
                    }
                    .tint(theme.fgBase)
                    .accessibilityIdentifier("multisig.create.account")
                }
            }
            MsSection(title: words.t("create.members")) {
                ForEach(owners.filter { o in !accounts.contains { $0.address.caseInsensitiveCompare(o) == .orderedSame } }, id: \.self) { owner in
                    memberRow(owner)
                }
                MsField(label: words.t("create.byAddress"), placeholder: words.t("create.addressPlaceholder"), text: $typed, mono: true, error: typedError, identifier: "multisig.create.address")
                if let ens = ensLine {
                    MsNote(text: ens.text, tone: ens.tone, identifier: "multisig.create.ens")
                }
                MsField(label: words.t("create.memberName"), placeholder: words.t("create.memberNamePlaceholder"), text: $typedName, identifier: "multisig.create.memberName")
                HStack(spacing: Tokens.Space.s16) {
                    Button(words.t("create.addMember"), action: addTyped)
                        .disabled(typed.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("multisig.create.addMember")
                    if !contacts.isEmpty {
                        Button(words.t("create.fromContacts")) { pickingContact = true }
                            .accessibilityIdentifier("multisig.create.fromContacts")
                    }
                }
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                MsNote(text: words.t("create.order"))
            }
            if local.isEmpty {
                MsNote(text: words.t("create.needLocal"), tone: .warning, identifier: "multisig.create.needLocal")
            }
        }
        .confirmationDialog(words.t("create.fromContacts"), isPresented: $pickingContact) {
            ForEach(contacts.filter { c in !owners.contains { $0.caseInsensitiveCompare(c.address) == .orderedSame } }, id: \.address) { contact in
                Button(contact.name.isEmpty ? MsWords.short(contact.address) : contact.name) {
                    _ = addMember(contact.address, name: contact.name)
                }
            }
        }
    }

    private func memberRow(_ owner: String) -> some View {
        let kind = hub.accountsView.memberKinds[owner.lowercased()]
        return HStack(alignment: .top, spacing: Tokens.Space.s12) {
            IdenticonAvatar(seed: owner, size: 28, tappable: false)
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(prose: label(owner)).typeRole(Typography.fieldLabel.scaled(textScale)).foregroundStyle(theme.fgBase)
                AddressTextView(MsRules.eip55(owner), role: Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                MultisigKindChoiceView(hub: hub, words: words, address: owner, choice: kind)
            }
            Spacer(minLength: 0)
            Button { toggle(owner) } label: {
                LucideIcon(.close, size: LucideIconSize.action).foregroundStyle(theme.fgSubtle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(words.t("members.remove"))
        }
        .padding(.vertical, Tokens.Space.s8)
    }

    private func toggle(_ address: String) {
        if owners.contains(where: { $0.caseInsensitiveCompare(address) == .orderedSame }) {
            owners.removeAll { $0.caseInsensitiveCompare(address) == .orderedSame }
        } else {
            owners.append(address)
        }
    }

    /// Add `address` (any case) as a member, checksummed, with what is known of it.
    @discardableResult
    private func addMember(_ address: String, name memberName: String?) -> Bool {
        let value = address.trimmingCharacters(in: .whitespaces)
        switch addressEntry(address: value) {
        case "checksum": typedError = words.t("send.invalidChecksum"); return false
        case "valid": break
        default: typedError = words.t("send.invalidAddress"); return false
        }
        guard (try? multisigCall(method: "validOwner", argsJson: CoreJSON.string(["address": value]))) == "true" else {
            typedError = words.t("create.notMember")
            return false
        }
        guard !owners.contains(where: { $0.caseInsensitiveCompare(value) == .orderedSame }) else {
            typedError = words.t("create.duplicate")
            return false
        }
        let member = (try? checksumAddress(addressHex: value)) ?? value
        if let clean = memberName?.trimmingCharacters(in: .whitespaces), !clean.isEmpty {
            names[member.lowercased()] = String(clean.prefix(MsRules.memberNameMax))
        }
        owners.append(member)
        typed = ""
        typedName = ""
        typedError = nil
        ensAsked = nil
        return true
    }

    private func addTyped() {
        let value = typed.trimmingCharacters(in: .whitespaces)
        // A name is read on Ethereum and shown as its address first.
        if value.contains("."), !value.hasPrefix("0x") {
            ensAsked = value
            typedError = nil
            hub.send(["type": "resolve_ens", "name": value])
            return
        }
        addMember(value, name: typedName)
    }

    private var ensLine: (text: String, tone: MsNote.Tone)? {
        guard let asked = ensAsked, let ens = hub.accountsView.add.ens, ens["name"]?.string == asked else { return nil }
        switch ens.kind {
        case "found": return (words.t("create.ensResolved", ["name": asked]) + " " + (ens["address"]?.string ?? ""), .neutral)
        case "none": return (words.t("create.ensNotFound", ["name": asked]), .warning)
        case "unavailable": return (words.t("create.ensUnavailable", ["name": asked]), .warning)
        default: return (words.t("create.ensUnsupported", ["name": asked]), .warning)
        }
    }

    private func ensArrived(_ ens: MsJSON?) {
        guard let asked = ensAsked, ens?["name"]?.string == asked, ens?.kind == "found",
              let address = ens?["address"]?.string else { return }
        typed = address
        if typedName.isEmpty { typedName = asked }
    }

    /// r7: each member's kind, read once for the networks offered.
    private func readKinds() {
        let chainIds = networks
        let members = owners.filter { o in !accounts.contains { $0.address.caseInsensitiveCompare(o) == .orderedSame } }
            .filter { hub.accountsView.memberKinds[$0.lowercased()] == nil }
            .map { ["address": $0, "chain_ids": chainIds] as [String: Any] }
        guard !members.isEmpty else { return }
        hub.send(["type": "read_kinds", "members": members])
    }

    /// The machine's sensible start (all of two, else all but one) unless the
    /// person picked for these members — core-ms-2's `create.threshold`.
    private func toThreshold() {
        guard nameOk else { nameAsked = true; return }
        askAddress()
        step = .threshold
    }

    /// The threshold as the machine holds it (`create.threshold.value`).
    private var machineThreshold: MsJSON? { sheet.threshold.flatMap { $0.isNull ? nil : $0 } }

    // MARK: - Threshold and network

    private var thresholdStep: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            MsSection(title: words.t("create.threshold")) {
                let view = machineThreshold
                HStack {
                    Text(prose: words.ruleText(m: view?["value"]?.int ?? threshold, n: view?["members"]?.int ?? owners.count))
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .accessibilityIdentifier("multisig.create.thresholdValue")
                    Spacer()
                    // The machine's stepper (`step_threshold`), its bounds the core's.
                    Button { hub.send(["type": "step_threshold", "up": false]) } label: {
                        Text(prose: "−").font(Typography.rowTitle.scaled(textScale).font)
                            .frame(minWidth: 44, minHeight: 44).contentShape(Rectangle())
                    }
                    .disabled(view?["can_lower"]?.bool != true)
                    .accessibilityLabel(words.t("create.fewer"))
                    .accessibilityIdentifier("multisig.create.fewer")
                    Button { hub.send(["type": "step_threshold", "up": true]) } label: {
                        Text(prose: "+").font(Typography.rowTitle.scaled(textScale).font)
                            .frame(minWidth: 44, minHeight: 44).contentShape(Rectangle())
                    }
                    .disabled(view?["can_raise"]?.bool != true)
                    .accessibilityLabel(words.t("create.more"))
                    .accessibilityIdentifier("multisig.create.more")
                }
                .tint(theme.fgBase)
                if let view {
                    MsNote(text: thresholdNote(view["note"]), tone: view["risky"]?.bool == true ? .warning : .neutral,
                           identifier: "multisig.create.thresholdNote")
                }
            }
            MsSection(title: words.t("create.network")) {
                if networks.isEmpty {
                    MsNote(text: words.t("create.noSafeNetworks"), tone: .warning)
                } else {
                    Picker(words.t("create.network"), selection: Binding(
                        get: { chainId ?? -1 },
                        set: { chainId = $0; chose = true }
                    )) {
                        if chainId == nil { Text(prose: words.t("create.networkNeeded")).tag(-1) }
                        ForEach(networks, id: \.self) { id in Text(prose: words.network(id)).tag(id) }
                    }
                    .pickerStyle(.menu)
                    .tint(theme.fgBase)
                    .accessibilityIdentifier("multisig.create.network")
                }
                MsNote(text: words.t("create.networkHint"))
                if let other = otherSpaceLine {
                    MsNote(text: other, identifier: "multisig.create.otherSpace")
                }
            }
        }
    }

    /// The note the core chose (`MultisigThresholdNote`), in the corpus's words.
    private func thresholdNote(_ note: MsJSON?) -> String {
        switch note?.kind {
        case "solo": return words.t("create.solo")
        case "all": return words.t("create.thresholdAll")
        case "one": return words.t("create.thresholdOne")
        default:
            let spare = note?["count"]?.int ?? 0
            return words.t("create.thresholdSpare", count: spare, ["count": String(spare)])
        }
    }

    /// The networks offered: the space's, minus those found without the Safe
    /// contracts, the likely ones first (the core's order).
    private var networks: [Int] {
        let without = Set(spaceNetworks.filter { sheet.contracts(on: $0) == "missing" })
        let offered = spaceNetworks.filter { !without.contains($0) }
        let ordered = MsRules.likelyNetworks(["networks": offered, "hints": hints])
        return ordered.isEmpty ? offered : ordered
    }

    /// core-ms-2: the hints are the accounts machine's (`recent_networks`,
    /// read from this device's history; `used_networks`).
    private var hints: [String: Any] {
        ["used": hub.accountsView.usedNetworks, "recent": hub.accountsView.recentNetworks]
    }

    private var firstDefault: Int? {
        (MsRules.call("networks.firstDefault", ["networks": networks, "hints": hints]) as? NSNumber)?.intValue
    }

    private var otherSpaceLine: String? {
        guard !sheet.otherSpace.isEmpty else { return nil }
        let space = words.loc.t(NetworkSpace.read(VelaStore()) == .testnet ? "networkSpace.mainnet" : "networkSpace.testnet")
        return words.t("create.otherSpace", ["space": space, "networks": words.loc.list(sheet.otherSpace.map(words.network))])
    }

    // MARK: - Review

    private var draftAddressKey: String { "\(owners)|\(threshold)|\(cleanName)" }

    private func askAddress() {
        guard !owners.isEmpty, nameOk else { return }
        hub.send(["type": "draft", "owners": owners, "threshold": threshold, "name": cleanName])
    }

    private var reviewStep: some View {
        let creators = local
        let chosenCreator = creator.flatMap { c in creators.first { $0.caseInsensitiveCompare(c) == .orderedSame } } ?? creators.first
        return VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            MsRow(label: words.t("create.name"), value: cleanName)
            MsRow(label: words.t("create.threshold"), value: words.ruleText(m: threshold, n: owners.count))
            MsSection(title: words.t("create.members")) {
                ForEach(owners, id: \.self) { owner in
                    MsRow(label: label(owner), value: MsRules.eip55(owner), mono: true)
                }
            }
            if let address = sheet.draftAddress {
                MsRow(label: words.t("create.address"), value: address, mono: true, copyable: true)
                    .accessibilityIdentifier("multisig.create.draftAddress")
                MsNote(text: words.t("create.sameEverywhere"))
            } else if sheet.draftError != nil {
                // Round 16 leftover 4: `draft_error` is the core's English —
                // said in the catalog's words, never drawn.
                MsNote(text: words.createDraftFailed, tone: .danger)
            } else {
                ProgressView()
            }
            if let chainId {
                MsRow(label: words.t("create.network"), value: words.network(chainId))
            }
            if creators.count > 1 {
                Picker(words.t("create.creator"), selection: Binding(get: { chosenCreator }, set: { creator = $0 })) {
                    ForEach(creators, id: \.self) { c in Text(prose: label(c)).tag(Optional(c)) }
                }
                .pickerStyle(.menu)
                .tint(theme.fgBase)
            } else if let chosenCreator {
                MsRow(label: words.t("create.creator"), value: label(chosenCreator))
            }
            if sheet.phase == .failed, let error = sheet.error {
                MsNote(text: words.failureText(error), tone: .danger, identifier: "multisig.create.failed")
            }
        }
    }

    /// r16-9: each step's action row is the sheet's FOOT, outside the scroll
    /// (the web's r15-3 contract): it rides above the keyboard, so Next is
    /// never under it, and a step that outgrows the sheet scrolls its body,
    /// never its action.
    private var stepFooter: AnyView? {
        switch step {
        case .members:
            return AnyView(
                VelaButton(title: words.t("create.next"), kind: .ink, enabled: !owners.isEmpty && !local.isEmpty) {
                    MsKeyboard.dismiss()
                    toThreshold()
                }
                .accessibilityIdentifier("multisig.create.next")
            )
        case .threshold:
            return AnyView(
                HStack(spacing: Tokens.Space.s12) {
                    VelaButton(title: words.t("create.back"), kind: .secondary) { step = .members }
                    VelaButton(title: words.t("create.next"), kind: .ink, enabled: chainId != nil) {
                        askAddress()
                        step = .review
                    }
                    .accessibilityIdentifier("multisig.create.toReview")
                }
            )
        case .review:
            let chosenCreator = creator.flatMap { c in local.first { $0.caseInsensitiveCompare(c) == .orderedSame } } ?? local.first
            return AnyView(
                HStack(spacing: Tokens.Space.s12) {
                    VelaButton(title: words.t("create.back"), kind: .secondary, enabled: sheet.phase != .working) { step = .threshold }
                    VelaButton(
                        title: words.t("create.create"), kind: .primary,
                        enabled: sheet.draftAddress != nil && chosenCreator != nil && chainId != nil,
                        loading: sheet.phase == .working, haptic: .confirm
                    ) {
                        create(creator: chosenCreator)
                    }
                    .accessibilityIdentifier("multisig.create.create")
                }
            )
        case .created:
            return nil
        }
    }

    private func create(creator: String?) {
        guard let creator, let chainId else { return }
        let members = owners.reduce(into: [String: Any]()) { out, owner in
            var info: [String: Any] = [:]
            if let said = names[owner.lowercased()], !said.isEmpty { info["name"] = said }
            out[owner] = info
        }
        Task { @MainActor in
            await hub.refreshLocal()
            hub.send(["type": "create", "choice": [
                "owners": owners, "threshold": threshold, "name": cleanName,
                "chain_id": chainId, "creator": creator, "members": members,
            ]])
            let first = await hub.accounts.until(timeout: 300) { view -> MsCreateView? in
                guard let create = MultisigMachine.decode(MsAccountsView.self, from: view)?.create else { return nil }
                return create.address != nil || create.phase != .working ? create : nil
            }
            if let address = first?.address {
                created = address
                step = .created
                VelaHaptic.success.play()
            }
        }
    }

    // MARK: - Created

    private var createdStep: some View {
        let row = hub.accountsView.row(created)
        let solo = owners.count == 1
        let shared = Self.sharedLine(shared: row?.shared == true, solo: solo, refused: sheet.refused,
                                     working: sheet.phase == .working, words: words)
        return VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            Text(prose: words.t("create.created"))
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .accessibilityIdentifier("multisig.create.created")
            MsNote(text: shared, tone: row?.shared == true ? .positive : (sheet.refused != nil ? .warning : .neutral), identifier: "multisig.create.shared")
            if let created {
                MsRow(label: words.t("create.address"), value: created, mono: true, copyable: true)
            }
            MsNote(text: words.t(solo ? "create.createdBodySolo" : "create.createdBody"))
            if !solo, let created {
                VelaButton(title: words.t("create.copyInvite"), kind: .ink) { onInvite(created) }
                    .accessibilityIdentifier("multisig.create.invite")
            }
            VelaButton(title: words.t("create.done"), kind: .secondary) {
                if let created { onCreated(created) }
            }
            .accessibilityIdentifier("multisig.create.done")
        }
    }

    /// Where the created multisig's record stands, in words. Round 16
    /// leftover 4: `refused` is never drawn as the service's text. r17-19: a
    /// record the service ANSWERED and did not take is the core's `refused`
    /// (§30) — said as what is true and what to do (`settings.shareAgainNote`),
    /// never "the multisig service can't be reached", which only a service
    /// that did not answer is.
    ///
    /// r18-8 (§28, §32): refused by this test build before it left
    /// (`production_write_refused`) — the test build's sentence, never the
    /// service's refusal or its silence.
    static func sharedLine(shared: Bool, solo: Bool, refused: String?, working: Bool, words: MsWords) -> String {
        if shared { return words.t(solo ? "create.sharedSolo" : "create.shared") }
        if refused == MsWords.productionWriteRefusedKind { return words.loc.t(MsWords.productionWriteRefusedKey) }
        if refused != nil { return words.createRefused }
        return working ? words.t("create.savedSharing") : words.t("create.savedLater")
    }

    private func close() {
        if step == .created, let created { onCreated(created) } else { onClose() }
    }
}

/// r7: a member's kind — the accounts machine's (`member_kinds`): what the
/// chains say of it (a Vela account found, a smart contract account), else
/// the person's word, else asked; "Other wallet" hinted when no Vela
/// account is found, kept only if picked.
struct MultisigKindChoiceView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let words: MsWords
    let address: String
    let choice: MsKindChoice?

    var body: some View {
        if let choice {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                if choice.locked {
                    MsChip(text: kindLabel(choice.kind ?? "vela"))
                } else {
                    Picker(words.t("members.kind"), selection: Binding(
                        get: { choice.kind ?? "" },
                        set: { hub.send(["type": "pick_kind", "address": address, "kind": $0.isEmpty ? NSNull() : $0]) }
                    )) {
                        Text(prose: "—").tag("")
                        Text(prose: words.t("members.kindVela")).tag("vela")
                        Text(prose: words.t("members.kindWallet")).tag("wallet")
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("multisig.kind")
                }
                if let note = noteText(choice.note) {
                    Text(prose: note)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private func kindLabel(_ kind: String) -> String {
        switch kind {
        case "wallet": words.t("members.kindWallet")
        case "contract": words.t("members.kindContract")
        default: words.t("members.kindVela")
        }
    }

    private func noteText(_ note: String?) -> String? {
        switch note {
        case "known": words.t("members.kindKnown")
        case "contract": words.t("members.kindContractNote")
        case "likely": words.t("members.kindLikely")
        case "ask": words.t("members.kindAsk")
        default: nil
        }
    }
}
