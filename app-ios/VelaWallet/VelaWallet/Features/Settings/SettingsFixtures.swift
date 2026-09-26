//
//  SettingsFixtures.swift
//  VelaWallet
//
//  Canonical settings fixtures (spec 023 — the single canon all four platforms
//  port; web reference: `src/lib/settings/fixtures.ts`).
//
//  Numbers, URLs, latencies and brand colours are DATA and identical across
//  platforms, so a reviewer comparing the four clients is comparing the same
//  wallet. Labels resolve through the corpus; components never format.
//
//  Where a mock shows a composed string ("200 条 · 1.0 MB") the parts are
//  composed HERE — composition order is a translation concern and a component
//  must never learn one.
//

import SwiftUI

enum SettingsFixtures {

    // MARK: - Canon

    static let accountName = "大表哥"
    static let addressFull = "0x14fB1f4E2b9C7a5D8e3F6a1B4c7D9e2F5a8B1D1eA5c"
    static let addressDisplay = "0x14fB...D1eA5c"

    private static let totalBalance = "$3,262.40"
    private static let appVersion = "1.0.0"
    private static let appCommit = "6ab8f"
    private static let networkCount = 12

    private struct AccountCanon {
        let name: String
        let addressFull: String
        let addressDisplay: String
        let amount: String
    }

    private static let accounts: [AccountCanon] = [
        AccountCanon(name: accountName, addressFull: addressFull,
                     addressDisplay: addressDisplay, amount: "$3,140.22"),
        AccountCanon(name: "旅行基金",
                     addressFull: "0x9a01c4E7b2F5a8D3e6C9b1A4d7F0e3B6c9D277C2b",
                     addressDisplay: "0x9a01...77C2b", amount: "$122.18"),
        AccountCanon(name: "试验田",
                     addressFull: "0x3Ce4f7A0b3D6e9C2a5F8b1E4d7C0a3F6b9E2A90f1",
                     addressDisplay: "0x3Ce4...A90f1", amount: "$0.00"),
    ]

    /// The eight networks ST9 lists, in order. The colour is the chain's own
    /// brand colour — data, not a theme token: it belongs to Ethereum and BNB,
    /// and must not flip with the appearance.
    private struct NetworkCanon {
        let id: String
        let name: String
        let letter: String
        let color: Color
        let chainId: Int
        let latencyMs: Int
        var custom: Bool = false
    }

    private static let networksCanon: [NetworkCanon] = [
        NetworkCanon(id: "ethereum", name: "Ethereum", letter: "E",
                     color: Color(red: 0.384, green: 0.494, blue: 0.918), chainId: 1, latencyMs: 45),
        NetworkCanon(id: "bnb", name: "BNB Chain", letter: "B",
                     color: Color(red: 0.941, green: 0.725, blue: 0.043), chainId: 56, latencyMs: 128),
        NetworkCanon(id: "polygon", name: "Polygon", letter: "P",
                     color: Color(red: 0.510, green: 0.278, blue: 0.898), chainId: 137, latencyMs: 45),
        NetworkCanon(id: "arbitrum", name: "Arbitrum", letter: "A",
                     color: Color(red: 0.157, green: 0.627, blue: 0.941), chainId: 42161, latencyMs: 45),
        NetworkCanon(id: "base", name: "Base", letter: "B",
                     color: Color(red: 0.0, green: 0.322, blue: 1.0), chainId: 8453, latencyMs: 45),
        NetworkCanon(id: "gnosis", name: "Gnosis", letter: "G",
                     color: Color(red: 0.180, green: 0.620, blue: 0.494), chainId: 100, latencyMs: 45),
        NetworkCanon(id: "tempo", name: "Tempo", letter: "T",
                     color: Color(red: 0.549, green: 0.549, blue: 0.549), chainId: 4217, latencyMs: 45),
        NetworkCanon(id: "xlayer", name: "X Layer", letter: "X",
                     color: Color(red: 0.549, green: 0.549, blue: 0.549), chainId: 196,
                     latencyMs: 0, custom: true),
    ]

    private static func canon(_ id: String) -> NetworkCanon {
        // A typo here is a programmer error in this file's own constants, and
        // `SettingsFixturesTests` catches it before anybody runs the app.
        networksCanon.first { $0.id == id }!
    }

    private static func mark(_ id: String) -> ChainMarkModel {
        let network = canon(id)
        return ChainMarkModel(letter: network.letter, color: network.color)
    }

    /// Language endonyms — NOT corpus strings. A language picker names each
    /// language IN that language, so the row reads the same whichever locale
    /// the app is in; that is the whole point of showing 日本語 to somebody who
    /// cannot read the current UI.
    static let localeEndonyms: [(id: String, label: String)] = [
        ("en", "English"),
        ("zh", "简体中文"),
        ("zh-TW", "繁體中文（台灣）"),
        ("zh-HK", "繁體中文（香港）"),
        ("ja", "日本語"),
        ("ko", "한국어"),
        ("vi", "Tiếng Việt"),
        ("id", "Bahasa Indonesia"),
        ("tr", "Türkçe"),
        ("es-MX", "Español (México)"),
        ("pt-BR", "Português (Brasil)"),
        ("fr", "Français"),
        ("de", "Deutsch"),
        ("ru", "Русский"),
        ("it", "Italiano"),
    ]

    /// Currency names come from the FX provider, not the corpus: the list is
    /// provider-driven, so their names are data here rather than 120 strings.
    /// Lives in `CurrencyCatalog` since spec 050, because the live picker
    /// offers the same eight; the values are unchanged.
    private static let currencies = CurrencyCatalog.entries

    private static let numberSamples =
        ["1,234,567.89", "1,234,567.89", "1.234.567,89", "1 234 567,89", "12,34,567.89"]
    private static let dateSamples =
        ["2026/06/13", "2026/06/13", "06/13/2026", "13/06/2026", "13.06.2026", "2026-06-13"]
    private static let timeSamples = ["13:45", "13:45", "1:45 PM"]

    // MARK: - Helpers

    /// `45ms`, or `在线 · 45ms` with a prefix. Past a second the unit becomes
    /// seconds AND the tone steps down to warning — the only way "1.2s" reads
    /// as slow rather than as a very small number.
    private static func latency(_ ms: Int, prefix: String? = nil) -> StatusPillModel {
        let tone: SettingsTone = ms >= 1000 ? .warn : .ok
        let value = ms >= 1000 ? String(format: "%.1fs", Double(ms) / 1000) : "\(ms)ms"
        return StatusPillModel(tone: tone, label: prefix.map { "\($0) · \(value)" } ?? value)
    }

    private static func chainMeta(_ loc: Loc, _ chainId: Int) -> String {
        loc.t(I18nKeys.SettingsUi.chainId, vars: ["chainId": String(chainId)])
    }

    // MARK: - Pages

    private static func sections(_ loc: Loc, advancedOpen: Bool) -> [SettingsSectionModel] {
        let k = I18nKeys.SettingsUi.self
        return [
            // 通讯录 and 反馈 are NOT on this page.
            //
            // The founder's ruling (2026-09-12, applied on Android in 047 and
            // recorded again in 054's plan): the address book has its own tab
            // and a settings row pointing at it is a second front door to one
            // room; the feedback sheet stays drawn and reachable from the
            // places that raise it, not from a list of preferences.
            SettingsSectionModel(
                rows: [
                    SettingsRowModel(id: "language", title: loc.t(k.languageTitle), icon: .globe,
                                     value: "简体中文 · \(loc.t(k.commonSystem))"),
                ],
                label: loc.t(k.sectionAppearance),
                appearanceControls: true
            ),
            SettingsSectionModel(
                rows: [
                    SettingsRowModel(id: "currency", title: loc.t(k.currencyTitle),
                                     icon: .coins, value: "USD · $1,234.56"),
                    SettingsRowModel(id: "number-format", title: loc.t(k.numberTitle),
                                     icon: .hash, value: numberSamples[0]),
                    SettingsRowModel(id: "date-format", title: loc.t(k.dateTitle),
                                     icon: .calendar, value: dateSamples[0]),
                    SettingsRowModel(id: "time-format", title: loc.t(k.timeTitle),
                                     icon: .clock, value: timeSamples[0]),
                ],
                label: loc.t(k.sectionLocalization)
            ),
            SettingsSectionModel(
                rows: [
                    SettingsRowModel(id: "networks", title: loc.t(k.networksTitle), icon: .network,
                                     subtitle: loc.t(k.networksSubtitle),
                                     value: loc.t(k.networkCount, vars: ["count": String(networkCount)])),
                    SettingsRowModel(id: "rpc-providers", title: loc.t(k.rpcProvidersTitle),
                                     icon: .server, subtitle: loc.t(k.rpcProvidersSubtitle)),
                    SettingsRowModel(id: "add-network", title: loc.t(k.addNetworkTitle),
                                     icon: .plus, subtitle: loc.t(k.addNetworkSubtitle)),
                    SettingsRowModel(id: "endpoints", title: loc.t(k.endpointsTitle),
                                     icon: .zap, subtitle: loc.t(k.endpointsSubtitle)),
                    // Spec 069: the default transaction speed — between the
                    // endpoints and the storage, as the web places it.
                    SettingsRowModel(id: feeSpeedRow, title: loc.t("settings.advanced.feeSpeedTitle"),
                                     icon: .clock, subtitle: loc.t("settings.advanced.feeSpeedSubtitle"),
                                     value: loc.t("send.gasTier.fast")),
                    // Spec 071: which Trusted Signer page this device opens —
                    // beside the speed, as every client places it.
                    SettingsRowModel(id: signerPageRow, title: loc.t("settings.signing.pageTitle"),
                                     icon: .link2, value: loc.t("settings.signing.pageOfficial")),
                    SettingsRowModel(id: "storage", title: loc.t(k.storageTitle),
                                     icon: .hardDrive, subtitle: loc.t(k.storageSubtitle)),
                ],
                label: loc.t(k.sectionAdvanced),
                collapsible: true
            ),
            SettingsSectionModel(rows: [
                SettingsRowModel(id: "about", title: loc.t(k.aboutTitle), icon: .info,
                                 value: loc.t(k.aboutSubtitle, vars: ["version": appVersion])),
            ]),
        ]
    }

    private static func networkRows(_ loc: Loc) -> [SettingsNetworkRowModel] {
        networksCanon.map { network in
            SettingsNetworkRowModel(
                id: network.id,
                mark: ChainMarkModel(letter: network.letter, color: network.color),
                name: network.name,
                meta: chainMeta(loc, network.chainId),
                badge: network.custom ? nil : latency(network.latencyMs),
                tag: network.custom ? loc.t(I18nKeys.SettingsUi.networkCustom) : nil,
                removable: network.custom
            )
        }
    }

    private static func networkDetail(_ loc: Loc, mismatch: Bool) -> NetworkDetailModel {
        let k = I18nKeys.SettingsUi.self
        let eth = canon("ethereum")
        return NetworkDetailModel(
            title: eth.name,
            subtitle: "\(chainMeta(loc, eth.chainId)) · ETH",
            mark: mark("ethereum"),
            name: eth.name,
            note: loc.t(k.networkBuiltinNote),
            badge: latency(eth.latencyMs, prefix: loc.t(k.networkOnline)),
            rpc: UrlFieldModel(id: "rpc", label: loc.t(k.fieldRpcUrl),
                               value: "https://eth.llamarpc.com",
                               hint: loc.t(k.networkSaveHint),
                               badge: latency(eth.latencyMs),
                               tone: mismatch ? .error : nil),
            explorer: UrlFieldModel(id: "explorer", label: loc.t(k.fieldExplorer),
                                    value: "https://etherscan.io"),
            callout: mismatch
                ? CalloutModel(tone: .danger,
                               text: loc.t(k.rpcChainMismatch,
                                           vars: ["reported": "56", "expected": "1"]))
                : nil
        )
    }

    /// ST10 search, ST10b compatible, ST10c incompatible — one builder.
    private static func addNetwork(_ loc: Loc, mode: String) -> AddNetworkModel {
        let k = I18nKeys.SettingsUi.self
        let grey = Color(red: 0.549, green: 0.549, blue: 0.549)
        let green = Color(red: 0.180, green: 0.620, blue: 0.494)

        if mode == "search" {
            return AddNetworkModel(
                title: loc.t(k.addNetworkTitle),
                subtitle: loc.t(k.addDescription),
                searchPlaceholder: loc.t(k.addSearch),
                results: [
                    SettingsNetworkRowModel(id: "zora",
                                            mark: ChainMarkModel(letter: "Z", color: grey),
                                            name: "Zora", meta: chainMeta(loc, 7_777_777)),
                    SettingsNetworkRowModel(id: "zircuit",
                                            mark: ChainMarkModel(letter: "Z", color: green),
                                            name: "Zircuit", meta: chainMeta(loc, 48_900)),
                    SettingsNetworkRowModel(id: "zora-sepolia",
                                            mark: ChainMarkModel(letter: "Z", color: grey),
                                            name: "Zora Sepolia", meta: chainMeta(loc, 999_999_999),
                                            tag: loc.t(k.addTestnet)),
                ]
            )
        }

        let ok = mode == "compatible"
        // Four rows in both verdicts: "incompatible" is only legible as an
        // answer if it shows WHICH requirement failed, so the list never
        // shortens. EntryPoint is deployed everywhere and passes in both.
        let checks = [
            CheckItemModel(label: "EntryPoint v0.7", ok: true),
            CheckItemModel(label: loc.t(k.addCheckSafe), ok: ok),
            CheckItemModel(label: loc.t(k.addCheckSigner), ok: ok),
            // Seven, not eight: spec 081 FR-009 dropped the fallback handler
            // the wallet never uses and gave the two passkey-signer contracts
            // their own sentence instead of this count.
            CheckItemModel(label: loc.t(k.addCheckRemaining, vars: ["count": "7"]), ok: ok),
        ]

        if ok {
            return AddNetworkModel(
                title: loc.t(k.addNetworkTitle),
                subtitle: "Zora · \(chainMeta(loc, 7_777_777))",
                searchPlaceholder: loc.t(k.addSearch),
                candidate: SettingsNetworkRowModel(
                    id: "zora", mark: ChainMarkModel(letter: "Z", color: grey), name: "Zora",
                    meta: loc.t(k.addBestRpc, vars: ["latencyMs": "182"]),
                    badge: StatusPillModel(tone: .ok, label: loc.t(k.addCompatible))
                ),
                checksTitle: loc.t(k.addCompatibilityCheck),
                checks: checks,
                customRpc: UrlFieldModel(id: "custom-rpc", label: loc.t(k.addCustomRpcTitle),
                                         value: "", placeholder: loc.t(k.addCustomRpcPlaceholder)),
                primary: loc.t(k.addButton)
            )
        }

        return AddNetworkModel(
            title: loc.t(k.addNetworkTitle),
            subtitle: "Zircuit · \(chainMeta(loc, 48_900))",
            searchPlaceholder: loc.t(k.addSearch),
            candidate: SettingsNetworkRowModel(
                id: "zircuit", mark: ChainMarkModel(letter: "Z", color: green), name: "Zircuit",
                meta: loc.t(k.addCompatibilityCheck),
                badge: StatusPillModel(tone: .error, label: loc.t(k.addIncompatible))
            ),
            checksTitle: loc.t(k.addCompatibilityCheck),
            checks: checks,
            callout: CalloutModel(tone: .warning, text: loc.t(k.addIncompatibleHint)),
            // An outline CTA plus a re-check link, not a greyed-out accent one:
            // an action you cannot take should not be dressed as the action you
            // came for.
            secondary: loc.t(k.addChainTool),
            recheck: loc.t(k.addRecheckWithRpc)
        )
    }

    private static func rpcProviders(_ loc: Loc) -> RpcProvidersModel {
        let k = I18nKeys.SettingsUi.self
        let notSet = loc.t(k.providerNotSet)
        func support(_ count: Int) -> String {
            loc.t(k.providerSupports,
                  vars: ["count": String(count), "total": String(networkCount)])
        }
        return RpcProvidersModel(
            title: loc.t(k.rpcProvidersTitle),
            subtitle: loc.t(k.rpcProvidersSubtitle),
            description: loc.t(k.providersDescription),
            providers: [
                ProviderCardModel(
                    id: "alchemy", name: "Alchemy",
                    badge: StatusPillModel(tone: .ok, label: loc.t(k.providerConnected)),
                    field: UrlFieldModel(id: "alchemy", label: "", value: "alch_k3y...9fQ2",
                                         action: loc.t(k.providerCheckKey)),
                    support: support(12)
                ),
                ProviderCardModel(
                    id: "drpc", name: "dRPC",
                    badge: StatusPillModel(tone: .neutral, label: notSet),
                    field: UrlFieldModel(id: "drpc", label: "", value: "", placeholder: notSet,
                                         action: loc.t(k.providerGetKey)),
                    link: "\(loc.t(k.providerGetKey)) →"
                ),
                ProviderCardModel(
                    id: "ankr", name: "Ankr",
                    badge: StatusPillModel(tone: .neutral, label: notSet),
                    field: UrlFieldModel(id: "ankr", label: "", value: "", placeholder: notSet,
                                         action: loc.t(k.providerGetKey)),
                    support: support(8)
                ),
            ]
        )
    }

    private static func endpoints(_ loc: Loc) -> EndpointsModel {
        let k = I18nKeys.SettingsUi.self
        return EndpointsModel(
            title: loc.t(k.endpointsTitle),
            description: loc.t(k.endpointsDescription),
            fields: [
                UrlFieldModel(id: "chain-data", label: loc.t(k.endpointChainData),
                              value: "https://ethereum-data.getvela.app",
                              hint: loc.t(k.endpointChainDataHint), badge: latency(62)),
                UrlFieldModel(id: "passkey", label: loc.t(k.endpointPasskey),
                              value: "https://p256-index-rs.getvela.app",
                              hint: loc.t(k.endpointPasskeyHint), badge: latency(88)),
                UrlFieldModel(id: "relay", label: loc.t(k.endpointRelay),
                              value: "https://vela-relay-cf.getvela.app",
                              hint: loc.t(k.endpointRelayHint), badge: latency(104)),
                UrlFieldModel(id: "fiat", label: loc.t(k.endpointFiat),
                              value: "https://vela-currency.getvela.app/v2/…",
                              hint: loc.t(k.endpointFiatHint), badge: latency(1200, prefix: loc.t(k.networkSlow))),
            ],
            reset: loc.t(k.endpointsReset)
        )
    }

    private static func storage(_ loc: Loc) -> StorageModel {
        let k = I18nKeys.SettingsUi.self
        let clear = loc.t(k.storageClear)
        func records(_ n: Int) -> String { loc.t(k.countRecords, vars: ["count": String(n)]) }
        return StorageModel(
            title: loc.t(k.storageTitle),
            subtitle: loc.t(k.storageSubtitle),
            amount: "2.4",
            unit: "MB",
            summary: loc.t(k.storageSummary, vars: ["count": "216"]),
            segments: [
                StorageSegmentModel(id: "user", label: loc.t(k.legendUserData), fraction: 0.5,
                                    color: Color(red: 0.353, green: 0.486, blue: 0.965)),
                StorageSegmentModel(id: "cache", label: loc.t(k.legendCaches), fraction: 0.3,
                                    color: Color(red: 0.239, green: 0.659, blue: 0.447)),
                StorageSegmentModel(id: "sessions", label: loc.t(k.legendSessions), fraction: 0.2,
                                    color: Color(red: 0.522, green: 0.510, blue: 0.478)),
            ],
            groups: [
                StorageGroupModel(label: loc.t(k.storageUserData), items: [
                    StorageItemModel(id: "transactions", label: loc.t(k.itemTransactions),
                                     meta: "\(records(200)) · 1.0 MB", action: clear, destructive: true),
                    StorageItemModel(id: "contacts", label: loc.t(k.itemContacts),
                                     meta: "\(loc.t(k.countContacts, vars: ["count": "18"])) · 42 KB",
                                     action: clear, destructive: true),
                    StorageItemModel(id: "custom", label: loc.t(k.itemCustom),
                                     meta: "\(loc.t(k.countItems, vars: ["count": "5"])) · 12 KB",
                                     action: clear, destructive: true),
                    StorageItemModel(id: "browsing", label: loc.t(k.itemBrowsing),
                                     meta: "\(records(31)) · 58 KB", action: clear, destructive: true),
                ]),
                StorageGroupModel(label: loc.t(k.storageCaches), items: [
                    StorageItemModel(id: "balances", label: loc.t(k.itemBalances),
                                     meta: "0.6 MB", action: clear),
                    StorageItemModel(id: "rates", label: loc.t(k.itemRates),
                                     meta: "96 KB", action: clear),
                    StorageItemModel(id: "scan", label: loc.t(k.itemScan),
                                     meta: "31 KB", action: clear),
                ], action: loc.t(k.storageClearAll)),
                StorageGroupModel(label: loc.t(k.storageConnections), items: [
                    StorageItemModel(id: "dapps", label: loc.t(k.itemDapps),
                                     meta: loc.t(k.countSites, vars: ["count": "4"]),
                                     action: loc.t(k.storageDisconnectAll), destructive: true),
                ]),
            ]
        )
    }

    private static func about(_ loc: Loc) -> AboutModel {
        let k = I18nKeys.SettingsUi.self
        return AboutModel(
            title: loc.t(k.aboutTitle),
            tagline: loc.t(k.aboutTagline),
            version: loc.t(k.aboutVersion, vars: ["version": appVersion, "commit": appCommit]),
            sectionTechnical: loc.t(k.aboutSectionTechnical),
            rows: [
                KeyValueRowModel(label: loc.t(k.aboutWalletLabel),
                                 value: loc.t(k.aboutWalletValue), mono: true),
                KeyValueRowModel(label: loc.t(k.aboutAuthLabel),
                                 value: loc.t(k.aboutAuthValue), mono: true),
                KeyValueRowModel(label: loc.t(k.aboutAccountLabel), value: loc.t(k.aboutAccountValue)),
                KeyValueRowModel(label: loc.t(k.aboutSignerLabel), value: loc.t(k.aboutSignerValue)),
                KeyValueRowModel(label: loc.t(k.aboutNetworksLabel),
                                 value: loc.t(k.aboutNetworksValue,
                                              vars: ["count": String(networkCount)])),
            ],
            links: [
                KeyValueRowModel(label: loc.t(k.aboutLinkWebsite), value: "getvela.app",
                                 mono: true, external: true,
                                 link: "https://getvela.app"),
                KeyValueRowModel(label: loc.t(k.aboutLinkGithub),
                                 value: "github.com/mondaylabsltd/vela-wallet",
                                 mono: true, external: true,
                                 link: "https://github.com/mondaylabsltd/vela-wallet"),
                KeyValueRowModel(label: loc.t(k.aboutLinkSafe), value: "safe.global",
                                 mono: true, external: true,
                                 link: "https://safe.global"),
            ],
            footer: loc.t(k.aboutFooter)
        )
    }

    // MARK: - Overlays

    private static func accountsSheet(_ loc: Loc) -> AccountsSheetModel {
        let k = I18nKeys.SettingsUi.self
        return AccountsSheetModel(
            title: loc.t(k.accountsTitle),
            summary: loc.t(k.accountsCount, vars: ["count": String(accounts.count)])
                + loc.t(k.accountsTotal, vars: ["amount": totalBalance]),
            rows: accounts.enumerated().map { index, account in
                AccountsSheetRowModel(name: account.name,
                                      addressDisplay: account.addressDisplay,
                                      addressFull: account.addressFull,
                                      amount: account.amount,
                                      selected: index == 0)
            },
            primary: loc.t(k.accountCreate),
            secondary: loc.t(k.accountSignIn)
        )
    }

    private static func signOutSheet(_ loc: Loc, warned: Bool) -> ConfirmSheetModel {
        let k = I18nKeys.SettingsUi.self
        return ConfirmSheetModel(
            title: loc.t(k.signOutTitle),
            body: loc.t(k.signOutDesc),
            confirm: loc.t(warned ? k.signOutAnyway : k.signOutButton),
            cancel: loc.t(k.signOutCancel),
            danger: true,
            note: loc.t(k.signOutKeeps),
            callout: warned ? CalloutModel(tone: .warning, text: loc.t(k.signOutWarning)) : nil
        )
    }

    private static func languageSheet(_ loc: Loc, current: String) -> SelectSheetModel {
        let k = I18nKeys.SettingsUi.self
        let label = localeEndonyms.first { $0.id == current }?.label ?? current
        var rows: [SelectRowModel] = [
            SelectRowModel(id: "system", label: loc.t(k.languageFollowSystem),
                           note: "\(loc.t(k.commonSystem)) · \(label)", selected: true),
        ]
        rows += localeEndonyms.map { SelectRowModel(id: $0.id, label: $0.label) }
        return SelectSheetModel(title: loc.t(k.languagePickerTitle), rows: rows,
                                subtitle: loc.t(k.languagePickerSubtitle),
                                footerNote: loc.t(k.languageContributeNote),
                                footerLink: loc.t(k.languageContributeCta))
    }

    /// The Settings row id of the default speed (spec 069).
    static let feeSpeedRow = "fee-speed"

    /// The default speed's sheet: the three speeds, fastest first, each with
    /// the line on what it buys — never `rapid`, which nothing offers.
    static func feeSpeedSheet(_ loc: Loc, selected: String) -> SelectSheetModel {
        SelectSheetModel(
            title: loc.t("settings.feeSpeed.title"),
            rows: [
                ("fast", "send.gasTierHintFast"),
                ("standard", "send.gasTierHintStandard"),
                ("slow", "send.gasTierHintSlow"),
            ].map { tier, hint in
                SelectRowModel(
                    id: tier, label: loc.t("send.gasTier.\(tier)"),
                    selected: tier == selected, detail: loc.t(hint)
                )
            },
            subtitle: loc.t("settings.feeSpeed.subtitle")
        )
    }

    /// The Settings row id of the Trusted Signer page (spec 071, 075).
    static let signerPageRow = "signer-page"

    private static func currencySheet(_ loc: Loc) -> SelectSheetModel {
        let k = I18nKeys.SettingsUi.self
        return SelectSheetModel(
            title: loc.t(k.currencySheetTitle),
            rows: currencies.enumerated().map { index, currency in
                SelectRowModel(id: currency.code, label: currency.code, glyph: currency.glyph,
                               caption: currency.name, selected: index == 0)
            },
            searchPlaceholder: loc.t(k.currencySearch)
        )
    }

    /// The three format pickers. Row 0 is always 自动 — it shows the sample the
    /// system would give, with the "自动 · 系统" note; the rest are explicit
    /// choices. One builder with three sample lists, not three near-identical
    /// ones.
    private static func formatSheet(
        _ loc: Loc, title: String, subtitle: String?, samples: [String],
        notes: [Int: String] = [:]
    ) -> SelectSheetModel {
        let k = I18nKeys.SettingsUi.self
        let auto = "\(loc.t(k.commonAutomatic)) · \(loc.t(k.commonSystem))"
        return SelectSheetModel(
            title: title,
            rows: samples.enumerated().map { index, sample in
                SelectRowModel(id: String(index), label: sample,
                               note: index == 0 ? auto : notes[index],
                               selected: index == 0, mono: true)
            },
            subtitle: subtitle
        )
    }

    private static func feedback(_ loc: Loc) -> FeedbackModel {
        let k = I18nKeys.SettingsUi.self
        let none = loc.t(k.bugPreviewNone)
        return FeedbackModel(
            title: loc.t(k.bugTitle),
            subtitle: loc.t(k.bugSubtitle),
            placeholder: loc.t(k.bugPlaceholder),
            addSteps: loc.t(k.bugAddSteps),
            previewToggle: loc.t(k.bugPreviewToggle),
            // Label AND value on every line: the point of this block is that
            // the person can read what is about to leave their device, and a
            // bare list of values is not readable.
            previewLines: [
                "\(loc.t(k.bugPreviewVersion)): v\(appVersion) (\(appCommit))",
                "\(loc.t(k.bugPreviewPlatform)): iOS 26.0",
                "\(loc.t(k.bugPreviewLanguage)): zh",
                "\(loc.t(k.bugPreviewRpc)): \(none)",
                "\(loc.t(k.bugPreviewFailures)): \(none)",
            ],
            consent: loc.t(k.bugConsent),
            send: loc.t(k.bugSend),
            githubLink: loc.t(k.bugGithub)
        )
    }

    // MARK: - Rescue

    private static func rpcBanner(_ loc: Loc) -> RpcBannerModel {
        let k = I18nKeys.SettingsUi.self
        return RpcBannerModel(
            text: loc.t(k.rpcUnavailableMultiple, vars: ["count": "2"]),
            chips: ["polygon", "gnosis"].map { id in
                RpcBannerChipModel(id: id, mark: mark(id), name: canon(id).name,
                                   action: loc.t(k.rpcFix))
            }
        )
    }

    /// SR2 (failing) and SR2b (restored) are one model with a flag.
    private static func rpcFix(_ loc: Loc, restored: Bool) -> RpcFixModel {
        let k = I18nKeys.SettingsUi.self
        let polygon = canon("polygon")
        return RpcFixModel(
            title: loc.t(k.rpcFixTitle),
            mark: mark("polygon"),
            name: polygon.name,
            meta: "\(chainMeta(loc, polygon.chainId)) · POL",
            badge: restored
                ? latency(96, prefix: loc.t(k.networkOnline))
                : StatusPillModel(tone: .error, label: loc.t(k.networkOffline)),
            callout: restored
                ? CalloutModel(tone: .success, text: loc.t(k.rpcFixRestored))
                : CalloutModel(tone: .warning, text: loc.t(k.rpcFixWarning)),
            field: UrlFieldModel(id: "rpc", label: loc.t(k.rpcFixLabel),
                                 value: "https://polygon-rpc.com",
                                 badge: restored ? latency(96) : nil,
                                 tone: restored ? .ok : .error),
            primary: loc.t(restored ? k.commonDone : k.rpcFixSave),
            // Nothing left to go and get once it works.
            providersLabel: restored ? nil : loc.t(k.rpcProvidersHint),
            providers: restored ? [] : ["Alchemy", "QuickNode", "dRPC", "Chainlist"],
            report: restored ? nil : loc.t(k.rpcReport)
        )
    }

    private static func balanceDetail(_ loc: Loc) -> BalanceDetailModel {
        let k = I18nKeys.SettingsUi.self
        return BalanceDetailModel(
            title: loc.t(k.balanceDetailTitle),
            summary: loc.t(k.balanceDetailTotal, vars: ["amount": totalBalance]),
            sectionPending: loc.t(k.balanceDetailNetworks),
            pendingNote: loc.t(k.balanceDetailNote),
            // Rate-limiting gets a grey line and no button because it resolves
            // itself; a dead RPC gets a red line and 立即重试 because it does not.
            pending: [
                BalanceDetailRowModel(id: "polygon", mark: mark("polygon"),
                                      name: canon("polygon").name,
                                      status: loc.t(k.balanceDetailRetrying), tone: .neutral),
                BalanceDetailRowModel(id: "gnosis", mark: mark("gnosis"),
                                      name: canon("gnosis").name,
                                      status: loc.t(k.balanceDetailFailed), tone: .error,
                                      action: loc.t(k.balanceDetailRetry)),
            ],
            sectionDone: loc.t(k.balanceDetailUpdated),
            done: [
                BalanceDetailRowModel(id: "ethereum", mark: mark("ethereum"),
                                      name: "Ethereum", amount: "$2,412.11"),
                BalanceDetailRowModel(id: "bnb", mark: mark("bnb"),
                                      name: "BNB Chain", amount: "$850.29"),
            ]
        )
    }

    private static func relayer(_ loc: Loc) -> RelayerModel {
        let k = I18nKeys.SettingsUi.self
        return RelayerModel(
            title: loc.t(k.relayerTitle),
            lead: loc.t(k.relayerLead),
            mark: mark("gnosis"),
            name: canon("gnosis").name,
            amountHint: loc.t(k.relayerAmountHint, vars: ["amount": "0.02", "symbol": "xDAI"]),
            qrCaption: loc.t(k.relayerAddressLabel),
            addressDisplay: "0x7Bd0...4E9c",
            copyLabel: loc.t(k.relayerCopy),
            callout: CalloutModel(tone: .warning, text: loc.t(k.relayerDisclaimer)),
            primary: loc.t(k.relayerRetry)
        )
    }

    private static func indexDown(_ loc: Loc) -> IndexDownModel {
        let k = I18nKeys.SettingsUi.self
        return IndexDownModel(
            title: loc.t(k.indexDownTitle),
            subtitle: loc.t(k.indexDownSubtitle),
            callout: CalloutModel(tone: .warning, text: loc.t(k.indexDownWarning)),
            field: UrlFieldModel(id: "endpoint", label: loc.t(k.indexDownEndpointLabel),
                                 value: "https://p256-index-rs.getvela.app",
                                 badge: StatusPillModel(tone: .error,
                                                        label: loc.t(k.networkOffline))),
            primary: loc.t(k.commonTryAgain),
            secondary: loc.t(k.indexDownEdit),
            footer: loc.t(k.indexDownFooter)
        )
    }

    // MARK: - State table

    private struct Shape {
        let page: SettingsPage
        let overlay: SettingsOverlay
        var rescue: Bool = false
        var backdrop: String?
    }

    private static func shape(_ state: SettingsStateId) -> Shape {
        switch state {
        case .st1, .st1b: Shape(page: .home, overlay: .none)
        case .st2: Shape(page: .home, overlay: .accounts)
        case .st3, .st3b: Shape(page: .home, overlay: .signOut)
        case .st4: Shape(page: .home, overlay: .language)
        case .st5: Shape(page: .home, overlay: .currency)
        case .st6: Shape(page: .home, overlay: .numberFormat)
        case .st7: Shape(page: .home, overlay: .dateFormat)
        case .st8: Shape(page: .home, overlay: .timeFormat)
        case .st9: Shape(page: .networks, overlay: .none)
        case .st9b: Shape(page: .networkDetail, overlay: .none)
        case .st10, .st10b, .st10c: Shape(page: .addNetwork, overlay: .none)
        case .st11: Shape(page: .rpcProviders, overlay: .none)
        case .st12: Shape(page: .endpoints, overlay: .none)
        case .st13: Shape(page: .storage, overlay: .none)
        case .st13b: Shape(page: .storage, overlay: .clearCaches, backdrop: "storage")
        case .st14: Shape(page: .about, overlay: .none)
        case .st15: Shape(page: .home, overlay: .feedback)
        case .st16: Shape(page: .home, overlay: .eraseDevice)
        case .sr1: Shape(page: .home, overlay: .none, rescue: true)
        case .sr2, .sr2b: Shape(page: .home, overlay: .rpcFix, rescue: true, backdrop: "wallet")
        case .sr3: Shape(page: .home, overlay: .balanceDetail, rescue: true, backdrop: "wallet")
        case .sr4: Shape(page: .home, overlay: .relayer, rescue: true, backdrop: "send")
        case .sr5: Shape(page: .home, overlay: .none, rescue: true)
        }
    }

    static func build(_ state: SettingsStateId, loc: Loc) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let form = shape(state)
        let addMode = switch state {
        case .st10b: "compatible"
        case .st10c: "incompatible"
        default: "search"
        }
        let backdropTitle = switch form.backdrop {
        case "wallet": loc.t(k.navWallet)
        case "send": loc.t(k.actionSend)
        case "storage": loc.t(k.storageTitle)
        default: loc.t(k.title)
        }

        return SettingsScreenModel(
            state: state,
            title: loc.t(k.title),
            page: form.page,
            overlay: form.overlay,
            rescue: form.rescue,
            tabs: TabsModel(wallet: loc.t(k.navWallet), contacts: loc.t(k.navContacts),
                            explore: loc.t(k.navExplore), settings: loc.t(k.navSettings)),
            account: SettingsAccountRowModel(name: accountName, addressDisplay: addressDisplay,
                                             addressFull: addressFull,
                                             action: loc.t(k.accountSwitch)),
            sections: sections(loc, advancedOpen: state == .st1b),
            theme: SegmentedModel(label: loc.t(k.themeTitle), segments: [
                SegmentModel(id: "light", label: loc.t(k.themeLight), icon: .sun),
                SegmentModel(id: "dark", label: loc.t(k.themeDark), icon: .moon),
                SegmentModel(id: "auto", label: loc.t(k.themeAuto), icon: .monitor),
            ], selected: "dark"),
            textScale: TextScaleModel(label: loc.t(k.textScale), steps: 7, index: 3),
            signOutLabel: loc.t(k.signOutButton),
            eraseTitle: loc.t(k.eraseTitle),
            eraseSubtitle: loc.t(k.eraseSubtitle),
            networksTitle: loc.t(k.networksTitle),
            networksSubtitle: loc.t(k.networksSubtitle),
            networks: networkRows(loc),
            addNetworkLabel: loc.t(k.addNetworkTitle),
            networkDetail: networkDetail(loc, mismatch: state == .st9b),
            addNetwork: addNetwork(loc, mode: addMode),
            rpcProviders: rpcProviders(loc),
            endpoints: endpoints(loc),
            storage: storage(loc),
            about: about(loc),
            accountsSheet: accountsSheet(loc),
            signOutSheet: signOutSheet(loc, warned: state == .st3b),
            languageSheet: languageSheet(loc, current: "zh"),
            currencySheet: currencySheet(loc),
            feeSpeedSheet: feeSpeedSheet(loc, selected: "fast"),
            numberSheet: formatSheet(loc, title: loc.t(k.numberTitle),
                                     subtitle: loc.t(k.numberSubtitle), samples: numberSamples,
                                     notes: [4: loc.t(k.noteIndian)]),
            dateSheet: formatSheet(loc, title: loc.t(k.dateTitle),
                                   subtitle: loc.t(k.dateSubtitle), samples: dateSamples),
            timeSheet: formatSheet(loc, title: loc.t(k.timeTitle),
                                   subtitle: loc.t(k.timeSubtitle), samples: timeSamples,
                                   notes: [1: loc.t(k.noteH24), 2: loc.t(k.noteH12)]),
            clearCachesSheet: ConfirmSheetModel(title: loc.t(k.storageClearTitle),
                                                body: loc.t(k.storageClearBody),
                                                confirm: loc.t(k.storageClearConfirm),
                                                cancel: loc.t(k.commonCancel), danger: false),
            eraseSheet: ConfirmSheetModel(title: loc.t(k.eraseTitle), body: loc.t(k.eraseDesc),
                                          confirm: loc.t(k.eraseConfirm),
                                          cancel: loc.t(k.eraseCancel), danger: true,
                                          note: loc.t(k.eraseKeeps),
                                          callout: CalloutModel(tone: .danger,
                                                                text: loc.t(k.eraseLoses))),
            feedback: feedback(loc),
            rpcBanner: state == .sr1 ? rpcBanner(loc) : nil,
            rpcFix: rpcFix(loc, restored: state == .sr2b),
            balanceDetail: balanceDetail(loc),
            relayer: relayer(loc),
            indexDown: indexDown(loc),
            backdropTitle: backdropTitle,
            closeLabel: loc.t(k.close)
        )
    }
}
