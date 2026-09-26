//
//  SettingsLive.swift
//  VelaWallet
//
//  The person's own networks, in the display models the drawn settings screen
//  already consumes.
//
//  A **sibling** of `SettingsFixtures`, not a replacement — and, unlike the
//  contacts screen, a partial one. `SettingsScreenModel` carries every page and
//  every sheet in one value, and only some of those surfaces have a machine
//  behind them yet. So this file follows the pattern spec 019 established on
//  this same type with `withIdentity`: build the fixture model, then swap the
//  fields the core now owns.
//
//  What that buys is honesty about the boundary. A field this file does not
//  touch is visibly still a fixture, and the list of what it touches is the
//  list of what is live.
//

import SwiftUI
import VelaCore

enum SettingsLive {

    /// Swap in the networks the core actually knows about — the list, each
    /// network's page, the wizard, the advanced row's count, and the two pages
    /// behind the providers and endpoints rows (spec 072).
    ///
    /// Everything else on the model — the other rows, the pickers, storage,
    /// about, every sheet — is left for its own builder below.
    static func withNetworks(
        _ view: NetViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        copy.networks = view.networks.map { row($0, loc: loc) }
        // One detail per chain, keyed by row id. A single `networkDetail` was
        // enough while the list was a fixture and became a lie the moment it
        // went live: every row opened the first chain's page, so tapping Gnosis
        // showed Ethereum's RPC under Gnosis's name.
        copy.networkDetails = Dictionary(
            uniqueKeysWithValues: view.networks.map {
                ($0.id, detail($0, loc: loc, fallback: model.networkDetail))
            }
        )
        if let first = view.networks.first {
            copy.networkDetail = detail(first, loc: loc, fallback: model.networkDetail)
        }
        copy.addNetwork = wizard(view.wizard, loc: loc, fallback: model.addNetwork)
        // The advanced row counts THIS device's list, custom chains included —
        // it read the drawing's twelve on every phone.
        let count = loc.t(I18nKeys.SettingsUi.networkCount, vars: ["count": String(view.networks.count)])
        copy.sections = model.sections.map { section in
            var updated = section
            updated.rows = section.rows.map { row in
                guard row.id == "networks" else { return row }
                var changed = row
                changed.value = count
                return changed
            }
            return updated
        }
        // The two pages behind the advanced rows are the core's view too
        // (spec 072), and each has a projection of its own now (spec 081
        // FR-001): `withEndpoints` and `withProviders`, applied beside this one
        // rather than from inside it.
        return copy
    }

    // MARK: - The service endpoints (spec 081 FR-001)

    /// The four services this wallet talks to, as the core holds them.
    ///
    /// Until this existed the page was the drawing, on a real phone, with live
    /// controls under it: it offered `https://p256-index-rs.getvela.app` — a
    /// host that has not answered since the index moved to `-v2` — as the
    /// person's own passkey index, and put an invented `88ms` beside it. Both
    /// halves were untrue and the second is the worse one, because a latency
    /// badge is a claim that something was measured.
    ///
    /// What comes from the core: the value (its draft, which is the stored URL
    /// until somebody types), the default as the placeholder, and the health.
    /// What stays the drawing's: the title, the description, each field's label
    /// and hint, and the reset button's words.
    static func withEndpoints(
        _ view: NetViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        copy.endpoints = EndpointsModel(
            title: model.endpoints.title,
            description: model.endpoints.description,
            fields: view.endpoints.map { endpoint in
                UrlFieldModel(
                    // **The core's own name for the field, not the drawing's
                    // slug.** This id is what the screen hands back when the
                    // field is typed into, and the store turns it straight
                    // back into `NetEndpointField` — so the round trip holds
                    // by construction rather than by a mapping table.
                    id: endpoint.field.rawValue,
                    label: loc.t(endpointLabel(endpoint.field)),
                    value: endpoint.value,
                    // The default is the PLACEHOLDER: an endpoint nobody has
                    // set shows, greyed, the URL it would use — never an empty
                    // box, and never that URL as if it had been chosen.
                    placeholder: endpoint.defaultValue,
                    hint: loc.t(endpointHint(endpoint.field)),
                    badge: servicePill(endpoint.health, loc: loc)
                )
            },
            reset: model.endpoints.reset
        )
        return copy
    }

    /// The RPC providers page, from the same machine.
    ///
    /// Sibling of the above and shipped with it for one reason: the events
    /// behind this page now reach the core, and the drawing prefills Alchemy's
    /// box with `alch_k3y...9fQ2`. A person who typed one character after that
    /// would have sent the mock key and half of their own to be saved.
    ///
    /// Whether a provider is configured is `has_key`, which is not
    /// `key.isEmpty`: a cleared key is REMOVED from storage, and the core is
    /// the one that knows the difference.
    static func withProviders(
        _ view: NetViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let notSet = loc.t(k.providerNotSet)
        var copy = model
        copy.rpcProviders = RpcProvidersModel(
            title: model.rpcProviders.title,
            subtitle: model.rpcProviders.subtitle,
            description: model.rpcProviders.description,
            providers: view.providers.enumerated().compactMap { index, provider in
                // The drawn card supplies nothing but its shape; every word on
                // it below is either the core's or the corpus's.
                guard let card = model.rpcProviders.providers[safe: index]
                    ?? model.rpcProviders.providers.first
                else { return nil }
                return ProviderCardModel(
                    id: provider.provider.rawValue,
                    name: providerName(provider.provider),
                    badge: provider.hasKey
                        ? StatusPillModel(tone: .ok, label: loc.t(k.providerConnected))
                        : StatusPillModel(tone: .neutral, label: notSet),
                    field: UrlFieldModel(
                        id: provider.provider.rawValue,
                        label: card.field.label,
                        value: provider.key,
                        placeholder: provider.hasKey ? nil : notSet,
                        action: loc.t(provider.hasKey ? k.providerCheckKey : k.providerGetKey)
                    ),
                    // The count the test actually reached, and only once it has
                    // finished. The drawing said "supports 12" on a phone that
                    // had never asked.
                    support: provider.test.flatMap { test in
                        test.done
                            ? loc.t(k.providerSupports, vars: [
                                "count": String(test.okCount), "total": String(test.total),
                            ])
                            : nil
                    },
                    link: provider.hasKey ? nil : "\(loc.t(k.providerGetKey)) →",
                    linkUrl: provider.hasKey ? nil : providerKeyUrl(provider.provider),
                    test: card.test
                )
            }
        )
        return copy
    }

    /// The drawn label for one endpoint field.
    ///
    /// Named per field rather than taken by position: the core's order and the
    /// drawing's happen to agree today, and a page that silently relabels
    /// somebody's relay as their passkey index if they ever stop agreeing is
    /// not worth the four lines it saves.
    private static func endpointLabel(_ field: NetEndpointFieldWire) -> String {
        let k = I18nKeys.SettingsUi.self
        return switch field {
        case .ethereumData: k.endpointChainData
        case .passkeyIndex: k.endpointPasskey
        case .bundlerService: k.endpointRelay
        case .fiatRates: k.endpointFiat
        }
    }

    private static func endpointHint(_ field: NetEndpointFieldWire) -> String {
        let k = I18nKeys.SettingsUi.self
        return switch field {
        case .ethereumData: k.endpointChainDataHint
        case .passkeyIndex: k.endpointPasskeyHint
        case .bundlerService: k.endpointRelayHint
        case .fiatRates: k.endpointFiatHint
        }
    }

    /// Where a provider hands out keys. The same three URLs Android opens
    /// (`SettingsLive.kt:609`), kept as a `switch` so a new provider cannot be
    /// added without somebody deciding where its link goes.
    static func providerKeyUrl(_ provider: NetProviderIdWire) -> String {
        switch provider {
        case .alchemy: "https://dashboard.alchemy.com/"
        case .drpc: "https://drpc.org/"
        case .ankr: "https://www.ankr.com/rpc/"
        }
    }

    private static func providerName(_ provider: NetProviderIdWire) -> String {
        switch provider {
        case .alchemy: "Alchemy"
        case .drpc: "dRPC"
        case .ankr: "Ankr"
        }
    }

    /// How one of the four services answered.
    ///
    /// Quiet while it is being checked — `checking` is a real state and a pill
    /// reading "checking" beside a box somebody is typing into is noise — then
    /// the measured latency, or the reason it did not answer. The three
    /// failures stay apart: refused for not being HTTPS, unreachable, and
    /// answered-but-not-by-this-service are three different things to fix.
    static func servicePill(_ health: NetServiceHealthWire, loc: Loc) -> StatusPillModel? {
        let k = I18nKeys.SettingsUi.self
        switch health {
        case .checking:
            return nil
        // The same latency wording the network pills use, so one endpoint is
        // never "88ms" here and "slow" a page away.
        case .ok(let latencyMs, _):
            return badge(.ok(latencyMs: latencyMs))
        case .notHttps:
            return StatusPillModel(tone: .error, label: loc.t(k.healthHttpsRequired))
        case .unreachable:
            return StatusPillModel(tone: .error, label: loc.t(k.networkOffline))
        case .invalidResponse:
            return StatusPillModel(tone: .error, label: loc.t(k.healthInvalid))
        }
    }

    /// The confirmation before "Reset to defaults" (FR-010), in the corpus's
    /// own words — the same question every shell asks.
    static func resetEndpointsConfirm(loc: Loc) -> ConfirmSheetModel {
        let k = I18nKeys.SettingsUi.self
        return ConfirmSheetModel(
            title: loc.t(k.endpointsResetTitle),
            body: loc.t(k.endpointsResetBody),
            confirm: loc.t(k.endpointsResetConfirm),
            cancel: loc.t(k.endpointsResetCancel),
            danger: true
        )
    }

    /// The confirmation before a custom network goes: its name and chain on
    /// the sheet, so the question names what it removes.
    static func removeNetworkConfirm(_ row: SettingsNetworkRowModel, loc: Loc) -> ConfirmSheetModel {
        let k = I18nKeys.SettingsUi.self
        return ConfirmSheetModel(
            title: loc.t(k.networkRemoveTitle),
            body: loc.t(k.networkRemoveBody),
            confirm: loc.t(k.networkRemoveConfirm),
            cancel: loc.t(k.networkRemoveCancel),
            danger: true,
            note: "\(row.name) · \(row.meta)"
        )
    }

    // MARK: - display_currency

    /// Swap in the currency the person actually chose.
    ///
    /// Two surfaces: the 货币 row's value on the home page, and which row of the
    /// picker reads as selected.
    /// The preference surfaces the settings page draws and had never read:
    /// language, the three formats, the theme and the size.
    ///
    /// Every row's VALUE is what is actually in force, and every sheet's
    /// selection is the same fact — a page that showed one thing in the row and
    /// another in the sheet would be two answers to one question.
    static func withPreferences(
        _ preferences: Preferences,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model

        // The rows' right-aligned values.
        copy.sections = model.sections.map { section in
            var updated = section
            updated.rows = section.rows.map { row in
                var changed = row
                switch row.id {
                case "language":
                    changed.value = languageValue(preferences.language, loc: loc)
                case "number-format":
                    changed.value = Formats.example(preferences.numberFormat)
                case "date-format":
                    changed.value = Formats.example(preferences.dateFormat)
                case "time-format":
                    changed.value = Formats.example(preferences.timeFormat)
                default:
                    return row
                }
                return changed
            }
            return updated
        }

        // The three format sheets are REBUILT from the presets themselves.
        //
        // The drawn sheets key their rows by position ("0", "1", …) over
        // hardcoded sample strings, which is fine for a picture and useless
        // for a choice: a tap has to name a preset, and the sample beside it
        // has to be what that preset would actually print. So each row's id is
        // the key, and its LABEL is a live example.
        let autoNote = "\(loc.t(I18nKeys.SettingsUi.commonAutomatic)) · "
            + loc.t(I18nKeys.SettingsUi.commonSystem)
        copy.numberSheet = formatSheet(
            model.numberSheet, chosen: preferences.numberFormat.rawValue, autoNote: autoNote,
            options: NumberFormatKey.allCases.map { ($0.rawValue, Formats.example($0)) }
        )
        copy.dateSheet = formatSheet(
            model.dateSheet, chosen: preferences.dateFormat.rawValue, autoNote: autoNote,
            options: DateFormatKey.allCases.map { ($0.rawValue, Formats.example($0)) }
        )
        copy.timeSheet = formatSheet(
            model.timeSheet, chosen: preferences.timeFormat.rawValue, autoNote: autoNote,
            options: TimeFormatKey.allCases.map { ($0.rawValue, Formats.example($0)) }
        )
        // The drawn sheet calls "follow the device" `system`; the STORED value
        // is `auto`, because that is what web and Android write. One mapping,
        // in one place.
        //
        // "Follow system" notes the language it resolves to NOW — the drawing
        // said 简体中文 on every phone.
        let resolved = languageName(loc.resolvedLanguage, loc: loc)
        copy.languageSheet = picked(
            model.languageSheet,
            id: preferences.language == "auto" ? "system" : preferences.language
        ) { row in
            guard row.id == "system" else { return row }
            var changed = row
            changed.note = "\(loc.t(I18nKeys.SettingsUi.commonSystem)) · \(resolved)"
            return changed
        }

        copy.theme = SegmentedModel(
            label: model.theme.label, segments: model.theme.segments,
            selected: themeSegment(preferences.theme)
        )
        copy.textScale = TextScaleModel(
            label: model.textScale.label,
            steps: TextScaleLevel.allCases.count,
            index: TextScaleLevel.allCases.firstIndex(of: preferences.textScale) ?? 2
        )
        return copy
    }

    /// One format sheet: a row per preset, labelled with what it would print.
    ///
    /// `auto` shows the sample it RESOLVES to, plus the "自动 · 系统" note —
    /// a row reading "automatic" and nothing else tells somebody nothing about
    /// what they would get.
    private static func formatSheet(
        _ sheet: SelectSheetModel, chosen: String, autoNote: String,
        options: [(id: String, example: String)]
    ) -> SelectSheetModel {
        SelectSheetModel(
            title: sheet.title,
            rows: options.map { option in
                SelectRowModel(
                    id: option.id,
                    label: option.example,
                    note: option.id == "auto" ? autoNote : nil,
                    selected: option.id == chosen,
                    mono: true
                )
            },
            subtitle: sheet.subtitle
        )
    }

    /// One sheet with exactly one row selected, and each row given whatever
    /// else it needs.
    private static func picked(
        _ sheet: SelectSheetModel, id: String, decorate: (SelectRowModel) -> SelectRowModel
    ) -> SelectSheetModel {
        SelectSheetModel(
            title: sheet.title,
            rows: sheet.rows.map { row in
                var changed = decorate(row)
                changed.selected = row.id == id
                return changed
            },
            subtitle: sheet.subtitle,
            searchPlaceholder: sheet.searchPlaceholder,
            footerNote: sheet.footerNote,
            footerLink: sheet.footerLink
        )
    }

    /// What a language tag is CALLED — in its own language, which is how the
    /// drawn sheet lists them, so the row's value and the sheet's label agree.
    static func languageName(_ tag: String, loc: Loc) -> String {
        if let endonym = SettingsFixtures.localeEndonyms.first(where: { $0.id == tag }) {
            return endonym.label
        }
        let locale = Locale(identifier: tag)
        return locale.localizedString(forIdentifier: tag)?.capitalized ?? tag
    }

    /// The language row's value: the language chosen, or — following the
    /// device — the one it resolves to, marked as the system's (the web's
    /// `languageValue`). It said "System" alone, which names no language.
    static func languageValue(_ stored: String, loc: Loc) -> String {
        guard stored == "auto" else { return languageName(stored, loc: loc) }
        return "\(languageName(loc.resolvedLanguage, loc: loc)) · \(loc.t(I18nKeys.SettingsUi.commonSystem))"
    }

    /// The drawn theme segments call "follow the system" `auto`; what is
    /// STORED is `system` (`vela.theme`, every shell's spelling). Both
    /// directions here, once — the tap used to go through
    /// `ThemeChoice(rawValue: "auto")`, which is nil, so once Light or Dark was
    /// picked the system's theme could never be chosen again (spec 072).
    static func themeSegment(_ choice: ThemeChoice) -> String {
        choice == .system ? "auto" : choice.rawValue
    }

    static func themeChoice(segment id: String) -> ThemeChoice? {
        id == "auto" ? .system : ThemeChoice(rawValue: id)
    }

    /// 测试 on every provider card, live only.
    ///
    /// The DRAWN panel has no such control: the gallery cannot ask a provider
    /// anything, so a button there would be a picture of an action. The live
    /// page can, and the core has had `provider_test_requested` since 050 with
    /// nothing to send it.
    static func withProviderTests(
        on model: SettingsScreenModel, loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        copy.rpcProviders = RpcProvidersModel(
            title: model.rpcProviders.title,
            subtitle: model.rpcProviders.subtitle,
            description: model.rpcProviders.description,
            providers: model.rpcProviders.providers.map { provider in
                var changed = provider
                changed.test = loc.t("settingsModals.rpcProviders.test")
                return changed
            }
        )
        return copy
    }

    /// The row id the screen routes to the host (spec 062).
    static let ethereumBackupRow = "ethereum-backup"

    /// The keys that control this wallet, with their Ethereum backup beneath
    /// them (spec 062) — ONE block, under the account it belongs to. A person
    /// offered "back up your keys" is owed the sight of them first.
    ///
    /// `keys == nil` is "still asking": a title and no guessed count. A registry
    /// that did not answer leaves the device's own memory on screen, labelled,
    /// without sync badges; one that answered with nothing is NOT called
    /// unreachable.
    static func withWalletKeys(
        _ keys: WalletKeys.Result?,
        backup: RegistryBackup.State?,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let rows = (keys?.rows ?? []).enumerated().map { index, row -> WalletKeyRowModel in
            var body = row.publicKeyHex.hasPrefix("0x") ? String(row.publicKeyHex.dropFirst(2)) : row.publicKeyHex
            if body.count == 130, body.hasPrefix("04") { body = String(body.dropFirst(2)) }
            let line = switch row.key.method {
            case .securityKey: k.keysProviderSecurityKey
            case .hybrid: k.keysProviderGeneric
            case .platform: k.keysProviderPlatform
            // Spec 075: the core now says `trusted_signer` for a key that lives
            // behind a Trusted Signer page, and where a key lives is the page.
            // The same sentence the "Sign with" chooser offers the route under,
            // so a person meets one thing whether creating, spending or looking.
            case .trustedSigner: I18nKeys.TrustedSigner.title
            }
            // The vault's name outranks the generic line — EXCEPT behind a page.
            // A Trusted Signer key's AAGUID is the authenticator on the page's far
            // side, which this wallet can reach no other way: naming that vault
            // points past the page exactly as "this device" did (found by the
            // Android device pass, 2026-09-22).
            let holder = row.key.method == .trustedSigner || row.key.providerName.isEmpty
                ? loc.t(line) : row.key.providerName
            return WalletKeyRowModel(
                id: index,
                name: row.key.name.isEmpty
                    ? loc.t(k.keysKeyN).replacingOccurrences(of: "{{n}}", with: String(index + 1))
                    : row.key.name,
                holder: holder,
                fingerprint: body.count >= 8 ? "\(body.prefix(4))…\(body.suffix(4))".lowercased() : "",
                pills: [
                    // The key this device signs with stands out, first
                    // (founder, 2026-09-26). The core marks at most one row.
                    row.signsHere ? KeyPillModel(text: loc.t(k.keysSignsHere), tone: .signsHere) : nil,
                    row.userVerified == true ? KeyPillModel(text: loc.t(k.keysUserVerified), tone: .verified) : nil,
                    row.synced.map { KeyPillModel(text: loc.t($0 ? k.keysSynced : k.keysNotSynced), tone: $0 ? .synced : .local) },
                ].compactMap { $0 },
                // The registry explorer's facts, in its order; what is absent is left out.
                details: [
                    KeyDetailModel(label: loc.t(k.keysPublicKey), value: row.publicKeyHex.isEmpty ? "" : "0x" + body130(row.publicKeyHex), mono: true, copy: true),
                    KeyDetailModel(label: loc.t(k.keysCredential), value: row.credentialId, mono: true, copy: true),
                    KeyDetailModel(label: "AAGUID", value: row.key.aaguid, mono: true, copy: false),
                    KeyDetailModel(
                        label: loc.t(k.keysTransport),
                        value: [row.key.authenticatorAttachment, row.key.transports].filter { !$0.isEmpty }.joined(separator: " · "),
                        mono: false, copy: false
                    ),
                    // WHICH page (spec 075). The caption says the route; only
                    // this says the place. Dropped by the filter below for every
                    // key that lives behind no page.
                    KeyDetailModel(
                        label: loc.t(I18nKeys.TrustedSigner.title), value: row.signerOrigin,
                        mono: false, copy: false
                    ),
                    KeyDetailModel(label: loc.t(k.keysAttestation), value: row.attestationHex, mono: true, copy: false),
                ].filter { !$0.value.isEmpty },
                key: row.key
            )
        }
        // A record with no key at all (the DEBUG read-only seed) has nothing to
        // list and nothing to back up: no block, rather than "Keys 0".
        if keys != nil, rows.isEmpty { return model }
        var next = model
        next.keys = WalletKeysModel(
            title: loc.t(k.keysTitle),
            subtitle: loc.t(k.keysSubtitle),
            count: keys == nil ? "" : String(rows.count),
            loading: keys == nil,
            note: keys?.source == .device ? loc.t(k.keysFromDevice) : nil,
            rows: rows,
            backup: ethereumBackupRow(backup, loc: loc),
            backupExplain: loc.t(k.backupExplain),
            copyLabel: loc.t(k.keysCopy),
            copiedLabel: loc.t(k.keysCopied)
        )
        return next
    }

    /// The key as stored, without a `0x` it may or may not have worn.
    private static func body130(_ hex: String) -> String {
        hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
    }

    /// The backup as a row: one line, three states, a chevron only when there
    /// is something to do; `nil` where there is no registry on Ethereum or the
    /// wallet was never registered at home.
    static func ethereumBackupRow(_ state: RegistryBackup.State?, loc: Loc) -> SettingsRowModel? {
        let k = I18nKeys.SettingsUi.self
        let subtitle: String
        switch state {
        case .unavailable, .notRegistered: return nil
        case nil: subtitle = loc.t(k.backupChecking)
        case .backedUp: subtitle = loc.t(k.backupBackedUp)
        case .notBackedUp: subtitle = loc.t(k.backupNotBackedUp)
        case .couldNotCheck: subtitle = loc.t(k.backupCouldNotCheck)
        }
        return SettingsRowModel(
            id: ethereumBackupRow,
            title: loc.t(k.backupTitle),
            icon: .upload,
            subtitle: subtitle,
            trailing: state == .notBackedUp ? .chevron : RowTrailing.none
        )
    }

    /// The account switcher, live — the SESSION's accounts and the balance
    /// core's cached totals, not the fixture three.
    ///
    /// Until this existed, `withIdentity` swapped the real name and address
    /// into row 0 and left the rest of the drawing standing, so a person with
    /// one wallet was shown three and two of them were somebody's mock
    /// (founder, 2026-09-16). Its comment said there was "no honest way to
    /// make them real without an account list the core does not expose yet" —
    /// the core has exposed one since 028 Phase 8, and the web and Android
    /// have been reading it ever since.
    ///
    /// Totals come from the switcher cache, so the sheet opens on last-known
    /// figures rather than spinners; an account the cache has never priced
    /// shows an EMPTY cell, never a mocked figure and never `0`.
    static func withAccounts(
        session: SessionView,
        balances: [BalanceCacheEntryWire],
        display: WalletLive.Display,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        guard session.hasWallet else { return model }
        let k = I18nKeys.SettingsUi.self

        func money(_ usd: Double) -> String {
            display.glyph + Formats.number(usd * display.rate,
                                           minimumFractionDigits: 2,
                                           maximumFractionDigits: 2)
        }
        /// The header's own truncation, so a row and the header above it
        /// never disagree about the same address.
        func shortenAddress(_ address: String) -> String {
            guard address.count > 14 else { return address }
            return "\(address.prefix(6))…\(address.suffix(4))"
        }
        func total(for address: String) -> Double? {
            balances.first { $0.address.caseInsensitiveCompare(address) == .orderedSame }?.usd
        }

        var copy = model
        copy.accountsSheet.rows = session.accounts.enumerated().map { index, row in
            let address = row.account.address
            let usd = total(for: address)
            return AccountsSheetRowModel(
                name: row.account.name,
                addressDisplay: shortenAddress(address),
                addressFull: address,
                amount: usd.map(money) ?? "",
                selected: index == session.activeIndex
            )
        }
        // Taking ONE wallet off this device (2026-09-23), resolved here because
        // the sheet resolves nothing of its own.
        copy.accountsSheet.remove = loc.t(I18nKeys.SettingsUi.accountRemove)
        copy.accountsSheet.removeBody = loc.t(I18nKeys.SettingsUi.accountRemoveBody)
        copy.accountsSheet.removeCancel = loc.t("settings.signOut.cancel")
        copy.accountsSheet.summary =
            loc.t(k.accountsCount, vars: ["count": String(session.accounts.count)])
            + loc.t(k.accountsTotal, vars: [
                "amount": money(session.accounts.reduce(0) { $0 + (total(for: $1.account.address) ?? 0) })
            ])
        return copy
    }

    /// The default transaction speed (spec 069): the row's value and which
    /// speed the sheet ticks, both from the `fee_tier_pref` core — so the
    /// Settings row and the send screen's folded control say the same thing.
    static func withFeeTier(
        _ view: FeeTierPrefViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        let sheet = SettingsFixtures.feeSpeedSheet(loc, selected: view.tier)
        copy.sections = model.sections.map { section in
            var updated = section
            updated.rows = section.rows.map { row in
                guard row.id == SettingsFixtures.feeSpeedRow else { return row }
                var changed = row
                changed.value = sheet.rows.first(where: \.selected)?.label ?? row.value
                return changed
            }
            return updated
        }
        copy.feeSpeedSheet = sheet
        return copy
    }

    /// Which Trusted Signer page this device opens (spec 071): the row's value
    /// and the page's sheet — every verdict in them the `sign_pref` core's, so
    /// the row cannot name one page while a signature opens another.
    static func withSignPref(
        _ view: SignPrefViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        copy.sections = model.sections.map { section in
            var updated = section
            updated.rows = section.rows.map { row in
                guard row.id == SettingsFixtures.signerPageRow else { return row }
                var changed = row
                changed.value = signerPageValue(view, loc: loc)
                return changed
            }
            return updated
        }
        let error: String? = switch view.signerUrlError {
        case "invalid": loc.t("settings.signing.pageInvalid")
        case "insecure": loc.t("settings.signing.pageInsecure")
        default: nil
        }
        copy.signerPage = SignerPageModel(
            title: loc.t("settings.signing.pageTitle"),
            subtitle: loc.t("settings.signing.pageSubtitle"),
            field: UrlFieldModel(
                id: "signer-url", label: "", value: view.signerUrl,
                placeholder: trustedSignerDefaultUrl(), tone: error == nil ? nil : .error
            ),
            error: error,
            foreign: view.signerUsesWalletPasskeys ? nil : loc.t("settings.signing.pageForeign"),
            save: loc.t("settings.signing.pageSave"),
            reset: view.signerUrlIsDefault ? nil : loc.t("settings.signing.pageReset")
        )
        return copy
    }

    /// "Official", or the host of the page a person chose.
    static func signerPageValue(_ view: SignPrefViewWire, loc: Loc) -> String {
        guard !view.signerUrlIsDefault else { return loc.t("settings.signing.pageOfficial") }
        return URL(string: view.signerUrl)?.host ?? view.signerUrl
    }

    static func withCurrency(
        _ view: CurrencyViewWire,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        copy.sections = model.sections.map { section in
            var updated = section
            updated.rows = section.rows.map { row in
                guard row.id == "currency" else { return row }
                var changed = row
                changed.value = currencyRowValue(view)
                return changed
            }
            return updated
        }
        copy.currencySheet = SelectSheetModel(
            title: model.currencySheet.title,
            rows: CurrencyCatalog.entries.map { entry in
                SelectRowModel(
                    id: entry.code, label: entry.code, glyph: entry.glyph,
                    caption: entry.name, selected: entry.code == view.code
                )
            },
            subtitle: model.currencySheet.subtitle,
            searchPlaceholder: model.currencySheet.searchPlaceholder,
            footerNote: model.currencySheet.footerNote,
            footerLink: model.currencySheet.footerLink
        )
        return copy
    }

    /// `USD · $1,234.56` — the code, then a sample amount in it.
    ///
    /// **A missing rate degrades; it never converts.** With `rate == nil` the
    /// sample stays the USD figure under a USD symbol rather than the same
    /// digits relabelled with a ¥, because relabelling is the lie: it tells
    /// somebody 1,234.56 dollars is 1,234.56 yuan. The core models that
    /// difference (`rate: null` is not `1`) and this is where the shell honours
    /// it.
    ///
    /// **A chosen currency is named even when it cannot be priced.** It used
    /// to fall back to "USD · $1,234.56" whenever the rate was missing, which
    /// told somebody who had picked CNY that they had not — the code alone is
    /// the honest row: their choice, and no figure (spec 072).
    static func currencyRowValue(_ view: CurrencyViewWire) -> String {
        let sample = 1_234.56
        guard view.committed else {
            // Nothing chosen: the USD placeholder is what is in force.
            let usd = CurrencyCatalog.entry("USD")
            return "USD · \(usd?.glyph ?? "$")\(format(sample))"
        }
        guard let rate = view.rate, rate > 0, let entry = CurrencyCatalog.entry(view.code) else {
            return view.code
        }
        return "\(view.code) · \(entry.glyph)\(format(sample * rate))"
    }

    /// In the person's own number format, as every other figure is.
    private static func format(_ amount: Double) -> String {
        Formats.number(amount, minimumFractionDigits: 2, maximumFractionDigits: 2)
    }

    // MARK: - The add-network wizard

    /// The wizard, driven by the core's phase.
    ///
    /// `fallback` supplies the labels and placeholders the drawing owns. Every
    /// verdict — the badge, the checks, whether 添加 is offered at all — is the
    /// core's; **`can_add` is never re-derived here**, because a second opinion
    /// is how a screen offers to add a chain the core will refuse.
    ///
    /// Three answers, never two (the web's `liveAddNetwork`): compatible,
    /// incompatible, and UNVERIFIED — the probes failed, so nothing was learned
    /// about the chain. That last one is worded "unable to verify" with a
    /// retry, never "incompatible": a proxy that refused once told the founder
    /// that Celo was incompatible, and Celo is not (spec 038 #E1).
    static func wizard(
        _ wizard: NetWizardViewWire,
        loc: Loc,
        fallback: AddNetworkModel
    ) -> AddNetworkModel {
        let k = I18nKeys.SettingsUi.self
        var model = AddNetworkModel(
            title: fallback.title,
            subtitle: fallback.subtitle,
            searchPlaceholder: fallback.searchPlaceholder
        )

        // No candidate yet — the search list.
        guard let info = wizard.chainInfo else {
            model.results = wizard.suggestions.map { entry in
                SettingsNetworkRowModel(
                    id: String(entry.chainId),
                    chainId: entry.chainId,
                    mark: mark(chainId: entry.chainId, name: entry.name),
                    name: entry.name,
                    meta: chainMeta(loc, entry.chainId)
                )
            }
            model.callout = errorCallout(wizard.error, loc: loc)
            return model
        }

        model.subtitle = "\(info.name) · \(chainMeta(loc, info.chainId))"
        model.customRpc = UrlFieldModel(
            id: "custom-rpc",
            label: loc.t(k.addCustomRpcTitle),
            value: wizard.customRpc,
            placeholder: loc.t(k.addCustomRpcPlaceholder)
        )
        func candidate(meta: String, badge: StatusPillModel?) -> SettingsNetworkRowModel {
            SettingsNetworkRowModel(
                id: String(info.chainId),
                chainId: info.chainId,
                mark: mark(chainId: info.chainId, name: info.name),
                name: info.name,
                meta: meta,
                badge: badge
            )
        }

        switch wizard.phase {
        case .resolving, .checking:
            // Still asking. A neutral pill, and no list of checks that have not
            // been made yet.
            model.candidate = candidate(
                meta: loc.t(k.addChecking),
                badge: StatusPillModel(tone: .neutral, label: loc.t(k.addCompatibilityCheck))
            )
            return model
        case .error:
            model.candidate = candidate(meta: chainMeta(loc, info.chainId), badge: nil)
            model.callout = errorCallout(wizard.error, loc: loc)
            // A check that could not run can run again; a refusal cannot.
            switch wizard.error {
            case .checkFailed, .noRpcEndpoint: model.recheck = loc.t(k.addRecheckWithRpc)
            default: break
            }
            return model
        default:
            break
        }

        guard let compat = wizard.compat else {
            model.candidate = candidate(meta: loc.t(k.addCompatibilityCheck), badge: nil)
            return model
        }
        if compat.rpcFailure != nil {
            // Unverified: nothing was learned, so there is no check list to
            // show — only the way to ask again.
            model.candidate = candidate(
                meta: loc.t(k.addCompatibilityCheck),
                badge: StatusPillModel(tone: .warn, label: loc.t(k.addUnableToVerify))
            )
            model.retry = loc.t(k.addRetry)
            model.recheck = loc.t(k.addRecheckWithRpc)
            return model
        }

        model.candidate = candidate(
            meta: candidateMeta(wizard, loc: loc),
            badge: compat.compatible
                ? StatusPillModel(tone: .ok, label: loc.t(k.addCompatible))
                : StatusPillModel(tone: .error, label: loc.t(k.addIncompatible))
        )
        model.checksTitle = loc.t(k.addCompatibilityCheck)
        model.checks = checks(compat, loc: loc)

        model.callout = errorCallout(wizard.error, loc: loc)
            ?? (wizard.compat?.compatible == false
                ? CalloutModel(tone: .warning, text: loc.t(k.addIncompatibleHint))
                // Spec 081 FR-009: compatible, and still not somewhere a wallet
                // with several passkeys can be created. Both halves are true;
                // the badge says the first, this says the second.
                : wizard.compat.map { $0.compatible && !$0.multiKeyReady } == true
                ? CalloutModel(tone: .warning, text: loc.t(k.addSingleKeyOnly))
                : nil)

        // The gate. An accent CTA appears only when the CORE says the chain can
        // be added; otherwise the re-check, because an action you cannot take
        // should not be dressed as the action you came for.
        if wizard.canAdd {
            model.primary = loc.t(k.addButton)
        } else if wizard.compat?.compatible == false {
            model.callout = CalloutModel(tone: .warning, text: loc.t(k.addIncompatibleHint))
            // The drawing's "Open Chain Setup Tool" is not offered: no client
            // has a page for it to open, and a button that goes nowhere is the
            // inert control this spec removes (SC-001).
            model.recheck = loc.t(k.addRecheckWithRpc)
        }
        return model
    }

    /// The four drawn rows over the core's eleven contracts.
    ///
    /// Not an invented summary: the drawing names three of `REQUIRED_CONTRACTS`
    /// individually — EntryPoint v0.7, Safe L2, WebAuthn Signer — and counts
    /// the remaining eight. 3 + 8 = 11, which is why the fixture's row reads
    /// 其余 8 项合约.
    ///
    /// **Known gap, recorded rather than papered over**: the P256 precompile
    /// participates in the core's verdict and has no drawn row and no corpus
    /// key. A chain rejected *only* for a missing precompile therefore shows
    /// four ticks under a 不兼容 badge — the exact illegibility this list was
    /// drawn to avoid. Folding it into 其余 8 项合约 would be worse: it would
    /// name eight contracts as the failure when the failure is the precompile.
    static func checks(_ compat: NetCompatibilityWire, loc: Loc) -> [CheckItemModel] {
        let k = I18nKeys.SettingsUi.self
        func deployed(_ name: String) -> Bool {
            compat.contracts.first { $0.name == name }?.deployed ?? false
        }
        let named = ["EntryPoint v0.7", "Safe L2", "WebAuthn Signer"]
        // The multi-key pair is spoken by its own callout, not folded into
        // 其余 N 项合约: a chain missing only those is whole for a one-key
        // wallet, and a red count row would say the opposite.
        let rest = compat.contracts.filter { !named.contains($0.name) && !$0.multiKeyOnly }
        return [
            CheckItemModel(label: "EntryPoint v0.7", ok: deployed("EntryPoint v0.7")),
            CheckItemModel(label: loc.t(k.addCheckSafe), ok: deployed("Safe L2")),
            CheckItemModel(label: loc.t(k.addCheckSigner), ok: deployed("WebAuthn Signer")),
            CheckItemModel(
                label: loc.t(k.addCheckRemaining, vars: ["count": String(rest.count)]),
                ok: !rest.isEmpty && rest.allSatisfy(\.deployed)
            ),
        ]
    }

    /// The line under the candidate's name: the winning RPC's latency once the
    /// race is decided, the "checking" line before that.
    private static func candidateMeta(_ wizard: NetWizardViewWire, loc: Loc) -> String {
        let k = I18nKeys.SettingsUi.self
        if let latency = wizard.compat?.bestRpcLatencyMs {
            return loc.t(k.addBestRpc, vars: ["latencyMs": String(Int(latency.rounded()))])
        }
        return loc.t(k.addCompatibilityCheck)
    }

    /// The core's refusal, in the words the corpus already has.
    private static func errorCallout(_ error: NetWizardErrorWire?, loc: Loc) -> CalloutModel? {
        let k = I18nKeys.SettingsUi.self
        guard let error else { return nil }
        let text = switch error {
        case .alreadyAdded: loc.t(k.addAlreadyAdded)
        case .notFound: loc.t(k.addChainNotFound)
        // The corpus has no "no RPC endpoint" sentence. `unableToVerify` is the
        // true thing rather than the exact thing: with no endpoint there is
        // nothing to verify against. Recorded as a wording gap.
        case .noRpcEndpoint: loc.t(k.addUnableToVerify)
        case .notCompatible: loc.t(k.addNotCompatible)
        // Not a verdict: the probes failed and nothing was learned.
        case .checkFailed: loc.t(k.addUnableToVerify)
        }
        return CalloutModel(tone: .warning, text: text)
    }

    /// One row of 设置 → 网络.
    static func row(_ network: NetNetworkRowWire, loc: Loc) -> SettingsNetworkRowModel {
        SettingsNetworkRowModel(
            id: network.id,
            chainId: network.chainId,
            mark: mark(chainId: network.chainId, name: network.displayName),
            name: network.displayName,
            meta: chainMeta(loc, network.chainId),
            badge: badge(network.rpcHealth),
            tag: network.isCustom ? loc.t(I18nKeys.SettingsUi.networkCustom) : nil,
            removable: network.isCustom
        )
    }

    /// One network's detail page.
    ///
    /// `fallback` supplies the labels and placeholders the drawing owns; the
    /// values, the health pill and the mismatch callout are the core's.
    static func detail(
        _ network: NetNetworkRowWire,
        loc: Loc,
        fallback: NetworkDetailModel
    ) -> NetworkDetailModel {
        NetworkDetailModel(
            title: network.displayName,
            subtitle: "\(chainMeta(loc, network.chainId)) · \(network.nativeSymbol)",
            mark: mark(chainId: network.chainId, name: network.displayName),
            name: network.displayName,
            // A custom network is not "built-in · cannot be removed".
            note: network.isCustom ? loc.t(I18nKeys.SettingsUi.networkCustom) : fallback.note,
            // **Never the fixture's pill.** `?? fallback.badge` used to be here,
            // and on a real phone it painted 在线 · 45ms over an endpoint
            // nothing had probed — the fixture's own constant, presented as a
            // measurement. Unmeasured is neutral and says nothing.
            badge: badge(network.rpcHealth) ?? Self.unmeasured,
            rpc: field(fallback.rpc, value: network.rpcUrl, health: network.rpcHealth,
                       tone: network.rpcChainMismatch == nil ? nil : .error),
            explorer: field(fallback.explorer, value: network.explorerUrl,
                            health: network.explorerHealth),
            // (Both fields drop the fixture's pill for the same reason.)
            callout: mismatchCallout(network, loc: loc) ?? fallback.callout
        )
    }

    // MARK: - Pieces

    /// What an endpoint nobody has probed wears: nothing that reads as an
    /// answer. Neutral is documented as "unset/idle, not failed", and the
    /// ellipsis is a glyph rather than copy, so it needs no corpus key.
    static let unmeasured = StatusPillModel(tone: .neutral, label: "\u{22EF}")

    /// The health pill. `nil` while nothing has been measured — an unmeasured
    /// endpoint is not a healthy one, and drawing a green pill before the probe
    /// answers is the screen making a claim the core has not.
    static func badge(_ health: NetProbeHealthWire?) -> StatusPillModel? {
        switch health {
        case .none:
            return nil
        case .checking:
            return StatusPillModel(tone: .neutral, label: "···")
        case .ok(let latencyMs):
            let ms = Int(latencyMs.rounded())
            let tone: SettingsTone = ms >= 1000 ? .warn : .ok
            let value = ms >= 1000 ? String(format: "%.1fs", latencyMs / 1000) : "\(ms)ms"
            return StatusPillModel(tone: tone, label: value)
        case .error:
            return StatusPillModel(tone: .error, label: "—")
        }
    }

    /// **The refusal, not a warning.** While the RPC reports a different chain
    /// than the network claims, the core writes nothing — so this callout is
    /// the screen saying why the edit did not stick.
    static func mismatchCallout(_ network: NetNetworkRowWire, loc: Loc) -> CalloutModel? {
        guard let mismatch = network.rpcChainMismatch else { return nil }
        return CalloutModel(
            tone: .danger,
            text: loc.t(
                I18nKeys.SettingsUi.rpcChainMismatch,
                // `reported` and `expected` — the variable names the fixture
                // already passes, so both builders fill the same sentence.
                vars: [
                    "reported": String(mismatch.reportedChainId),
                    "expected": String(mismatch.expectedChainId),
                ]
            )
        )
    }

    /// `tone` is the core's refusal (a red box for the RPC another chain
    /// answered), never the fixture's.
    private static func field(
        _ fallback: UrlFieldModel,
        value: String,
        health: NetProbeHealthWire?,
        tone: SettingsTone? = nil
    ) -> UrlFieldModel {
        UrlFieldModel(
            id: fallback.id,
            label: fallback.label,
            value: value,
            placeholder: fallback.placeholder,
            hint: fallback.hint,
            badge: badge(health),
            tone: tone,
            action: fallback.action
        )
    }

    // MARK: - The rescues the hero's status line opens (spec 058)

    /// SR3 — the balance by network: the chains still being read or
    /// unreachable, then the ones that settled, largest first.
    ///
    /// The web's `liveBalanceDetail`, ported. Two rules from it that matter:
    /// a rate-limited chain gets a grey line and NO button because it resolves
    /// itself, while an unreachable one gets a red line and 立即重试 because it
    /// does not; and a hidden balance stays hidden here — a person who hid the
    /// figure did not agree to have it broken out per chain.
    static func withBalanceDetail(
        _ balance: BalanceViewWire,
        display: WalletLive.Display,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let mask = "••••"
        // The same figure the hero prints, through the same two decisions: the
        // display currency's glyph and `Formats`' number shape. A second
        // formatter here would eventually disagree with the total above it.
        func money(_ usd: Double) -> String {
            display.glyph + Formats.number(usd * display.rate,
                                           minimumFractionDigits: 2,
                                           maximumFractionDigits: 2)
        }

        func chainName(_ id: Int) -> String {
            ChainCatalog.meta(id)?.displayName ?? chainMeta(loc, id)
        }
        func row(_ id: Int) -> ChainMarkModel {
            mark(chainId: id, name: chainName(id))
        }

        var pending: [BalanceDetailRowModel] = balance.rateLimitedChainIds.map { id in
            BalanceDetailRowModel(id: String(id), mark: row(id), name: chainName(id),
                                  status: loc.t(k.balanceDetailRetrying), tone: .neutral)
        }
        for id in balance.bannerChainIds where !pending.contains(where: { $0.id == String(id) }) {
            pending.append(BalanceDetailRowModel(
                id: String(id), mark: row(id), name: chainName(id),
                status: loc.t(k.balanceDetailFailed), tone: .error,
                action: loc.t(k.balanceDetailRetry)
            ))
        }

        var perChain: [Int: Double] = [:]
        for token in balance.tokens {
            guard let price = token.priceUsd, let amount = Double(token.balance) else { continue }
            let usd = amount * price
            guard usd.isFinite else { continue }
            perChain[token.chainId, default: 0] += usd
        }
        let done = perChain
            .filter { id, _ in !pending.contains(where: { $0.id == String(id) }) }
            .sorted { $0.value > $1.value }
            .map { id, usd in
                BalanceDetailRowModel(
                    id: String(id), mark: row(id), name: chainName(id),
                    amount: balance.hidden ? mask : money(usd)
                )
            }

        var live = model
        live.balanceDetail = BalanceDetailModel(
            title: model.balanceDetail.title,
            summary: loc.t(k.balanceDetailTotal, vars: [
                "amount": balance.hidden || balance.displayTotalUsd == nil
                    ? mask
                    : money(balance.displayTotalUsd ?? 0),
            ]),
            sectionPending: model.balanceDetail.sectionPending,
            pendingNote: model.balanceDetail.pendingNote,
            pending: pending,
            sectionDone: model.balanceDetail.sectionDone,
            done: done
        )
        return live
    }

    /// SR2 — the fix for ONE unreachable chain: which chain, what is stored for
    /// it now, and the field that replaces it.
    ///
    /// The chain is the first that failed. Per-chain rather than one global
    /// button because the fix IS per chain, which is the same argument the
    /// drawn banner makes.
    static func withRpcFix(
        chainId: Int,
        endpoint: String,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let name = ChainCatalog.meta(chainId)?.displayName ?? chainMeta(loc, chainId)
        var live = model
        live.rpcFix = RpcFixModel(
            title: model.rpcFix.title,
            mark: mark(chainId: chainId, name: name),
            name: name,
            meta: chainMeta(loc, chainId),
            badge: model.rpcFix.badge,
            callout: model.rpcFix.callout,
            field: UrlFieldModel(
                id: "rpc",
                label: loc.t(k.rpcFixLabel),
                value: endpoint,
                placeholder: model.rpcFix.field.placeholder,
                tone: model.rpcFix.field.tone
            ),
            primary: model.rpcFix.primary,
            providersLabel: model.rpcFix.providersLabel,
            providers: model.rpcFix.providers,
            report: model.rpcFix.report
        )
        return live
    }

    // MARK: - 存储 and 关于 (spec 058)

    /// Storage → Connections: one row per connected site (spec 070 FR-017),
    /// the web's `withLiveConnections`.
    ///
    /// The row is the SITE — its host, the account it sees, and Disconnect —
    /// and its id is the origin, which is what a tap revokes. With nothing
    /// connected the measured "dApp permissions" row stays, so the page still
    /// says there is nothing there rather than drawing an empty group.
    ///
    /// Runs AFTER `withStorage`: the sizes are that function's, the rows are
    /// this one's, and the other order would overwrite a site's address with
    /// "0 B".
    static func withConnections(
        _ sites: [DbrSiteViewWire],
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        guard !sites.isEmpty else { return model }
        let label = loc.t(I18nKeys.SettingsUi.storageConnections)
        var live = model
        live.storage = StorageModel(
            title: model.storage.title,
            subtitle: model.storage.subtitle,
            amount: model.storage.amount,
            unit: model.storage.unit,
            summary: model.storage.summary,
            segments: model.storage.segments,
            groups: model.storage.groups.map { group in
                guard group.label == label else { return group }
                return StorageGroupModel(
                    label: group.label,
                    items: sites.map { site in
                        StorageItemModel(
                            id: site.origin,
                            label: BrowserEngine.hostOf(origin: site.origin),
                            meta: AddressText.short(site.address),
                            // Singular: this row cuts off ONE site.
                            action: loc.t("explore.disconnect"),
                            destructive: true
                        )
                    },
                    action: group.action
                )
            }
        )
        return live
    }

    /// An erase that ran and left something behind (spec 072 FR-011): the
    /// sheet stays up with the reason in its own callout, and the person is
    /// still signed in — never sent to the first run over a partial wipe.
    static func withEraseFailure(
        _ survivors: [String]?, on model: SettingsScreenModel, loc: Loc
    ) -> SettingsScreenModel {
        guard let survivors, !survivors.isEmpty else { return model }
        var live = model
        // Named, as the desktop names them: "something stayed" is not
        // actionable, and the keys are what a person can report.
        live.eraseSheet.callout = CalloutModel(
            tone: .danger,
            text: "\(loc.t(I18nKeys.SettingsUi.eraseFailed)) (\(survivors.joined(separator: ", ")))"
        )
        return live
    }

    /// Whether a storage row is a connected site (its id an origin) rather
    /// than one of the measured rows.
    static func isConnectionRow(_ id: String) -> Bool { id.contains("://") }

    /// The storage page, measured.
    ///
    /// The fixture keeps every label, every action word and the order of the
    /// rows; what changes is every NUMBER — the total, the ring's three
    /// fractions, and each row's "n records · size". Android has reported real
    /// bytes since 047 and iOS drew "2.4 MB · 216 records" on every phone.
    ///
    /// A row with nothing in it says its size (`0 B`) rather than disappearing:
    /// the page is an inventory, and a missing row reads as data hidden
    /// somewhere else.
    static func withStorage(
        _ report: DeviceStorage.Report,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let total = DeviceStorage.size(report.totalBytes)
        let totalBytes = max(report.totalBytes, 1)

        func meta(_ id: String) -> String {
            guard let item = report.item(id) else { return DeviceStorage.sizeText(0) }
            let size = DeviceStorage.sizeText(item.bytes)
            guard let records = item.records else { return size }
            // Each row counts in the unit it holds — the corpus has three
            // count sentences and using "records" for contacts would be the
            // same shrug the fixture made.
            let count: String
            switch id {
            case "contacts": count = loc.t(k.countContacts, vars: ["count": String(records)])
            case "custom": count = loc.t(k.countItems, vars: ["count": String(records)])
            case "dapps": return loc.t(k.countSites, vars: ["count": String(records)])
            default: count = loc.t(k.countRecords, vars: ["count": String(records)])
            }
            return "\(count) · \(size)"
        }

        let storage = StorageModel(
            title: model.storage.title,
            subtitle: model.storage.subtitle,
            amount: total.amount,
            unit: total.unit,
            summary: loc.t(k.storageSummary, vars: ["count": String(report.totalRecords)]),
            segments: model.storage.segments.map { segment in
                let group = DeviceStorage.Group(rawValue: segment.id)
                return StorageSegmentModel(
                    id: segment.id,
                    label: segment.label,
                    fraction: group.map { Double(report.bytes(of: $0)) / Double(totalBytes) } ?? 0,
                    color: segment.color
                )
            },
            groups: model.storage.groups.map { group in
                StorageGroupModel(
                    label: group.label,
                    items: group.items.map { item in
                        StorageItemModel(id: item.id, label: item.label, meta: meta(item.id),
                                         action: item.action, destructive: item.destructive)
                    },
                    action: group.action
                )
            }
        )
        var live = model
        live.storage = storage
        return live
    }

    /// 关于, from the build that is running.
    ///
    /// `commit` is `unknown` unless the archive passed one (see `Info.plist`),
    /// and the caller substitutes the build number in that case — the page
    /// never prints a hash that is not this build's.
    static func withAbout(
        version: String,
        commit: String,
        networkCount: Int,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        var live = model
        let versionLine = loc.t(k.aboutVersion, vars: ["version": version, "commit": commit])
        live.about = AboutModel(
            title: model.about.title,
            tagline: model.about.tagline,
            version: versionLine,
            sectionTechnical: model.about.sectionTechnical,
            rows: model.about.rows.map { row in
                // The network count is the only technical row that is not a
                // constant about the wallet's design — it is what THIS device
                // has, custom chains included.
                row.label == loc.t(k.aboutNetworksLabel)
                    ? KeyValueRowModel(
                        label: row.label,
                        value: loc.t(k.aboutNetworksValue, vars: ["count": String(networkCount)]),
                        mono: row.mono,
                        external: row.external
                    )
                    : row
            },
            links: model.about.links,
            footer: model.about.footer
        )
        // The home row's subtitle names the same version.
        for index in live.sections.indices {
            for row in live.sections[index].rows.indices
            where live.sections[index].rows[row].id == "about" {
                live.sections[index].rows[row].subtitle =
                    loc.t(k.aboutSubtitle, vars: ["version": version])
            }
        }
        return live
    }

    private static func chainMeta(_ loc: Loc, _ chainId: Int) -> String {
        loc.t(I18nKeys.SettingsUi.chainId, vars: ["chainId": String(chainId)])
    }

    /// The circular chain mark.
    ///
    /// Colour is a **display** fact, not a core one: `vela-core` knows a chain's
    /// id, name, RPC and explorer, and deliberately not what colour it is. The
    /// eight brand colours are the ones the design system already ships; the
    /// core's other four builtins and every custom chain get the same neutral
    /// the drawing gives X Layer, so a chain nobody drew is never assigned
    /// somebody else's brand.
    static func mark(chainId: Int, name: String) -> ChainMarkModel {
        let letter = name.trimmingCharacters(in: .whitespaces).first
            .map { String($0).uppercased() } ?? "?"
        // The chain's own logo over the letter, as every other surface that
        // names a chain draws it (Android's `VelaChainMark`, the web's).
        return ChainMarkModel(letter: letter, color: chainColor(chainId),
                              logoUrl: Marks.chainLogoURL(chainId))
    }

    /// One chain, one colour, for every surface that draws a dot. Internal
    /// rather than private since 053: the browser's connection panel shows the
    /// chain a site is connected on, and two mappings would eventually
    /// disagree about which blue Base is.
    static func chainColor(_ chainId: Int) -> Color {
        switch chainId {
        case 1: ChainPalette.ethereum
        case 10: ChainPalette.optimism
        case 56: ChainPalette.bnb
        case 100: ChainPalette.gnosis
        case 137: ChainPalette.polygon
        case 8453: ChainPalette.base
        case 42161: ChainPalette.arbitrum
        case 43114: ChainPalette.avalanche
        // The neutral the drawing gives a chain it has no brand colour for —
        // `SettingsFixtures`' Tempo and X Layer rows wear the same value.
        default: ChainPalette.unbranded
        }
    }
}
