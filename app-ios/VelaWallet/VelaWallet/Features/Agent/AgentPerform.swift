//
//  AgentPerform.swift
//  VelaWallet
//
//  Doing an intent (spec 103): the calls this app's own controls make — the
//  same store, the same method, the same root function — so a person's tap
//  and an agent's request reach the rule that owns the change by one road.
//  The desktop's `perform.rs` and the web's `perform.ts`, action for action,
//  each over this app's own way of doing it: the token sheet searches every
//  network, as its screen does; deleting a network leaves the balances be, as
//  Settings' bin does here.
//
//  Transfers are not here: drafting one onto the review and following it is
//  the core's (`vela-agent` transfers, over `AgentShellBridge`'s send).
//  Seeing the change land is the runner's (`intent_rules::verdict`).
//
//  Every answer is `{"done":true}` or `{"refused":{"code":…,"detail":…}}`.
//

import Foundation
import VelaCore

@MainActor
struct AgentPerform {
    let graph: AppGraph
    let agent: AgentStore
    let page: AgentPage?

    nonisolated static let done = #"{"done":true}"#

    nonisolated static func refused(_ code: String, _ detail: String) -> String {
        let answer = ["refused": ["code": code, "detail": detail]]
        guard let data = try? JSONSerialization.data(withJSONObject: answer),
              let text = String(data: data, encoding: .utf8)
        else { return #"{"refused":{"code":"refused","detail":""}}"# }
        return text
    }

    /// Refused, in words.
    nonisolated static func refused(_ detail: String) -> String { refused("refused", detail) }

    private static let notOpen = refused("not_ready", "the wallet is not open")

    func perform(_ actionJson: String) async -> String {
        guard let action = try? CoreJSON.object(actionJson), let kind = action["type"] as? String else {
            return Self.refused("invalid_input", "not an action")
        }
        let text = { (key: String) in (action[key] as? String ?? "").trimmingCharacters(in: .whitespaces) }
        let chainId = (action["chain_id"] as? NSNumber)?.intValue ?? 0
        switch kind {
        case "show":
            guard let page else { return Self.notOpen }
            let screen = action["screen"] as? [String: Any] ?? [:]
            return page.show(screen).map { Self.refused($0) } ?? Self.done
        case "switch_account":
            guard let page else { return Self.notOpen }
            return await page.switchAccount(text("address"))
                ? Self.done : Self.refused("no wallet on this device has that address")
        case "remove_wallet":
            let address = text("address")
            guard let row = graph.session.view.accounts.first(where: {
                $0.account.address.caseInsensitiveCompare(address) == .orderedSame
            }) else { return Self.refused("no wallet on this device has that address") }
            graph.session.removeAccount(index: row.index)
            return Self.done
        case "sign_out":
            // Confirmed on the session's own sheet, as Settings' row asks it.
            graph.session.signOut()
            return Self.done
        case "set_preference":
            return setPreference(action["preference"] as? [String: Any] ?? [:])
        case "set_network_space":
            // The person allowed it (manage-level): the same switch as theirs.
            guard let space = NetworkSpaceKind(rawValue: text("space")) else {
                return Self.refused("invalid_input", "no such space")
            }
            graph.space.set(space)
            return Self.done
        case "save_contact":
            graph.contacts.save(address: text("address"), name: text("name"))
            return Self.done
        case "delete_contact":
            graph.contacts.delete(address: action["address"] as? String ?? "")
            return Self.done
        case "save_group":
            graph.contacts.saveGroup(id: action["id"] as? String, name: text("name"))
            return Self.done
        case "delete_group":
            graph.contacts.deleteGroup(id: action["id"] as? String ?? "")
            return Self.done
        case "set_group_members":
            graph.contacts.setGroupMembers(
                id: action["id"] as? String ?? "",
                members: action["members"] as? [String] ?? []
            )
            return Self.done
        case "set_contact_groups":
            graph.contacts.setContactGroups(
                address: action["address"] as? String ?? "",
                groupIds: action["group_ids"] as? [String] ?? []
            )
            return Self.done
        case "set_contact_favorite":
            // The star is a toggle: the check already said it is not so yet.
            graph.contacts.toggleFavorite(address: action["address"] as? String ?? "")
            return Self.done
        case "erase_device":
            guard let page else { return Self.notOpen }
            return await page.erase().map { Self.refused($0) } ?? Self.done
        case "clear_storage_item":
            guard let page else { return Self.notOpen }
            page.clearStorageItem(action["item"] as? String ?? "")
            return Self.done
        case "clear_caches":
            guard let page else { return Self.notOpen }
            page.clearCaches()
            return Self.done
        case "configure_agent":
            agent.apply(
                enabled: action["enabled"] as? Bool,
                url: action["bridge_url"] as? String,
                token: action["client_token"] as? String
            )
            return Self.done
        case "add_token":
            return await addToken(chainId: chainId, address: text("address"))
        case "delete_activity":
            // History's "Delete record": the feed tombstones it and drops the row.
            graph.activity.deleteRequested(id: action["id"] as? String ?? "")
            return Self.done
        case "set_balance_privacy":
            // The figure is a toggle: pressed only when it is not already so.
            let hidden = action["hidden"] as? Bool ?? false
            if (graph.wallet.balance?.hidden ?? false) != hidden { graph.wallet.togglePrivacy() }
            return Self.done
        case "refresh_balances":
            guard let page else { return Self.notOpen }
            page.refreshBalances()
            return Self.done
        case "add_network":
            return await addNetwork(chainId: chainId, rpcUrl: (action["rpc_url"] as? String).map {
                $0.trimmingCharacters(in: .whitespaces)
            })
        case "set_network_endpoint":
            return await setNetworkEndpoint(
                chainId: chainId, field: action["field"] as? String ?? "", url: text("url")
            )
        case "remove_network":
            let row = networks()["networks"].list.first {
                $0["chain_id"].int == chainId && $0["is_custom"].bool
            }
            graph.settings.deleteNetwork(id: row?["id"].string ?? "custom-\(chainId)")
            return Self.done
        case "set_provider_key":
            setProviderKey(provider: action["provider"] as? String ?? "", key: text("key"))
            return Self.done
        case "set_service_endpoint":
            setServiceEndpoint(field: action["field"] as? String ?? "", url: text("url"))
            return Self.done
        case "reset_service_endpoints":
            graph.settings.resetEndpoints()
            return Self.done
        default:
            return Self.refused("this wallet cannot do \(kind) here")
        }
    }

    // MARK: - Preferences

    /// As Settings' own controls set them: stored, then applied now.
    private func setPreference(_ preference: [String: Any]) -> String {
        let prefs = graph.preferences
        let value = preference["value"]
        let word = value as? String ?? ""
        switch preference["kind"] as? String {
        case "theme":
            guard let choice = ThemeChoice(rawValue: word) else { break }
            prefs.setTheme(choice)
            return Self.done
        case "language":
            prefs.setLanguage(word)
            graph.loc.apply(word)
            return Self.done
        case "text_scale":
            guard let level = TextScaleLevel(rawValue: word) else { break }
            prefs.setTextScale(level)
            UiScale.apply(prefs)
            return Self.done
        case "number_format":
            guard let key = NumberFormatKey(rawValue: word) else { break }
            prefs.setNumberFormat(key)
            Formats.apply(prefs)
            return Self.done
        case "date_format":
            guard let key = DateFormatKey(rawValue: word) else { break }
            prefs.setDateFormat(key)
            Formats.apply(prefs)
            return Self.done
        case "time_format":
            guard let key = TimeFormatKey(rawValue: word) else { break }
            prefs.setTimeFormat(key)
            Formats.apply(prefs)
            return Self.done
        case "currency":
            graph.settings.chooseCurrency(word)
            return Self.done
        case "fee_speed":
            // Rapid is a one-shot pick on the send form, never the default.
            if ["fast", "standard", "slow"].contains(word) { graph.settings.chooseFeeTier(word) }
            return Self.done
        case "analytics":
            return Self.refused(Self.noAnalytics)
        default:
            break
        }
        return Self.refused("invalid_input", "not a value this wallet offers")
    }

    /// The phone shares no usage statistics: there is no switch to turn.
    static let noAnalytics = "this wallet shares no usage statistics: there is nothing to switch"

    // MARK: - Networks and services

    /// The network ledger's view, in the core's own words.
    private func networks() -> CoreFacts { CoreFacts(json: graph.settings.networksJson) }

    /// How long the add wizard may take to look a chain up and check it.
    private static let checkMs = 45_000
    /// How long the RPC save gate may take to hear the endpoint's chain id.
    private static let gateMs = 20_000

    /// Settings → Add network: look the chain up, try `rpcUrl` first if
    /// given, check the wallet can serve it, and add it. A wizard already
    /// checked for this chain (the person's own, at its Add) is not checked
    /// again.
    private func addNetwork(chainId: Int, rpcUrl: String?) async -> String {
        let settings = graph.settings
        settings.openNetworks()
        let wizard = networks()["wizard"]
        let checked = wizard["phase"].string == "checked" && wizard["can_add"].bool
            && wizard["chain_info"]["chain_id"].int == chainId
            && (rpcUrl.map { wizard["custom_rpc"].string.trimmingCharacters(in: .whitespaces) == $0 } ?? true)
        if !checked {
            settings.resetWizard()
            settings.selectChain(chainId)
            if let rpcUrl {
                settings.editCustomRpc(rpcUrl)
                settings.selectChain(chainId, keepCustomRpc: true)
            }
        }
        // `""` once it can be added, else why not.
        let settled: String? = await until(Self.checkMs) {
            let wizard = networks()["wizard"]
            if wizard["phase"].string == "error" {
                return "add network: \(wizard["error"].name)"
            }
            guard wizard["phase"].string == "checked" else { return nil }
            if wizard["can_add"].bool { return "" }
            let compat = wizard["compat"]
            let why = [compat["blocker"], compat["rpc_failure"]].first { !$0.isNull }?.name
            return "add network: \(why ?? "not compatible")"
        }
        switch settled {
        case nil: return Self.refused("not_observed", "the network could not be checked in time")
        case let why? where !why.isEmpty: return Self.refused(why)
        default: settings.confirmAdd()
        }
        return Self.done
    }

    /// One network's card: open it, edit the field, leave it (which saves).
    /// An RPC goes through the core's save gate: one answering for another
    /// chain is not written, and the field is put back to what is in effect.
    private func setNetworkEndpoint(chainId: Int, field: String, url: String) async -> String {
        guard let wire = NetOverrideFieldWire(rawValue: field) else {
            return Self.refused("invalid_input", "not a network field")
        }
        let settings = graph.settings
        let shown = { () -> String? in
            networks()["networks"].list.first { $0["chain_id"].int == chainId }
                .map { $0[field == "rpc" ? "rpc_url" : "explorer_url"].string }
        }
        let before = shown()
        if before != url {
            settings.expandNetwork(chainId: chainId)
            settings.editOverride(chainId: chainId, field: wire, value: url)
        }
        settings.blurOverride(chainId: chainId)
        guard wire == .rpc else { return Self.done }
        let gate: Bool? = await until(Self.gateMs) {
            guard let row = networks()["networks"].list.first(where: { $0["chain_id"].int == chainId })
            else { return nil }
            if !row["rpc_chain_mismatch"].isNull { return false }
            return row["rpc_save_deferred"].bool ? nil : true
        }
        if gate == false {
            if let before { settings.editOverride(chainId: chainId, field: wire, value: before) }
            return Self.refused("that RPC answers for another chain; the network keeps the RPC it had")
        }
        return Self.done
    }

    /// RPC providers: open the page (it seeds the drafts), type the key, leave
    /// it. A key already in its field is only left — opening the page again
    /// would reseed every draft.
    private func setProviderKey(provider: String, key: String) {
        guard let wire = NetProviderIdWire(rawValue: provider) else { return }
        let settings = graph.settings
        let typed = networks()["providers"].list.contains {
            $0["provider"].string == provider && $0["key"].string == key
        }
        if !typed {
            settings.openProviders()
            settings.editProviderKey(provider: wire, value: key)
        }
        settings.blurProviderKey(id: provider)
    }

    /// Service endpoints: open the page, type the URL, leave it — or only
    /// leave it.
    private func setServiceEndpoint(field: String, url: String) {
        guard let wire = NetEndpointFieldWire(rawValue: field) else { return }
        let settings = graph.settings
        let typed = networks()["endpoints"].list.contains {
            $0["field"].string == field && $0["value"].string == url
        }
        if !typed {
            settings.openEndpoints()
            settings.editEndpoint(field: wire, value: url)
        }
        settings.blurEndpoint(field: wire)
    }

    // MARK: - Tokens

    /// How long the search and the save may each take.
    private static let tokenStepMs = 30_000

    /// The add-token sheet's own road: the address typed (the sheet searches
    /// every network for it), then the card for the network named saved.
    private func addToken(chainId: Int, address: String) async -> String {
        let tokens = graph.tokens
        tokens.open()
        tokens.input(address)
        let typed: Bool? = await until(Self.tokenStepMs) {
            guard let view = tokens.view, view.inputAddress == address else { return nil }
            return view.addressValid
        }
        guard typed == true else { return Self.refused("invalid_input", "that is not a contract address") }
        let searched: MtokViewWire? = await until(Self.tokenStepMs) {
            guard let view = tokens.view, !view.detecting,
                  !view.found.isEmpty || view.notFound || view.nativeAlias
            else { return nil }
            return view
        }
        guard let searched else {
            return Self.refused("not_observed", "the token could not be looked up in time")
        }
        if searched.nativeAlias {
            return Self.refused("that address is the network's own coin, already in the wallet")
        }
        guard let card = searched.found.first(where: { $0.chainId == chainId }) else {
            return Self.refused("not_found", "no token answers at that address on that network")
        }
        if card.added { return Self.done }
        tokens.save(chainId: chainId)
        let saved: Bool? = await until(Self.tokenStepMs) {
            guard let view = tokens.view else { return nil }
            let stored = view.found.contains { $0.chainId == chainId && $0.added }
            return !view.saving && (view.saveError || stored) ? view.saveError : nil
        }
        switch saved {
        case nil: return Self.refused("not_observed", "the token was not seen stored in time")
        case true: return Self.refused("the token could not be stored")
        case false: return Self.done
        }
    }

    // MARK: -

    /// Read `read` until it answers, or `ms` pass.
    private func until<T>(_ ms: Int, _ read: () -> T?) async -> T? {
        var waited = 0
        while true {
            if let value = read() { return value }
            if waited >= ms { return nil }
            try? await Task.sleep(nanoseconds: 50_000_000)
            waited += 50
        }
    }
}

/// A view as the core wrote it, read by the core's own field names — what
/// `intent_rules` and the desktop read too. Missing reads as empty.
nonisolated struct CoreFacts {
    let raw: Any?

    init(_ raw: Any?) { self.raw = raw }

    init(json: String?) {
        raw = json.flatMap { $0.data(using: .utf8) }.flatMap { try? JSONSerialization.jsonObject(with: $0) }
    }

    subscript(_ key: String) -> CoreFacts { CoreFacts((raw as? [String: Any])?[key]) }

    var list: [CoreFacts] { (raw as? [Any])?.map(CoreFacts.init) ?? [] }
    var string: String { raw as? String ?? "" }
    var bool: Bool { raw as? Bool ?? false }
    var int: Int? { (raw as? NSNumber)?.intValue }
    var isNull: Bool { raw == nil || raw is NSNull }

    /// A core value by its own name, as a refusal reads it: an enum's word,
    /// or a tagged one's `type`.
    var name: String {
        if let word = raw as? String { return word }
        if let type = self["type"].raw as? String { return type }
        return raw.map { "\($0)" } ?? ""
    }
}
