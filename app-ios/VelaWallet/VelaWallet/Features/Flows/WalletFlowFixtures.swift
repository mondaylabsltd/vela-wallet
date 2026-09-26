//
//  WalletFlowFixtures.swift
//  VelaWallet
//
//  Named for the file, not the type: spec 014's onboarding flow already owns
//  a `WalletFlowFixtures.swift`, and two Swift files with one basename collide in
//  the build's stringsdata even across folders.
//
//  Canonical wallet-flow fixtures (spec 021 — the iOS port of the web's
//  `src/lib/flows/fixtures.ts`, byte-for-byte the same canon).
//
//  Pure data plus assembly. Nothing here fetches, signs, formats a number
//  or decides a business rule.
//
//  Where a mock invented content the product already has a canon for, the
//  canon wins: the contact picker uses spec 018's roster and every address
//  is spec 015's or spec 018's, so identicon artwork matches across
//  features and across clients. Chain colours are fixture DATA, hosted in
//  DesignSystem/ChainPalette like spec 015's.
//

import SwiftUI

enum WalletFlowFixtures {
    // MARK: - Canon

    struct NetworkFixture {
        let name: String
        let code: String
        let color: Color
        let chainId: String
    }

    /// The eight supported networks, in the order R1 lists them.
    static let networks: [NetworkFixture] = [
        NetworkFixture(name: "Ethereum", code: "ETH", color: ChainPalette.ethereum, chainId: "1"),
        NetworkFixture(name: "BNB Chain", code: "BNB", color: ChainPalette.bnb, chainId: "56"),
        NetworkFixture(name: "Polygon", code: "POL", color: ChainPalette.polygon, chainId: "137"),
        NetworkFixture(name: "Arbitrum", code: "ARB", color: ChainPalette.arbitrum, chainId: "42161"),
        NetworkFixture(name: "Optimism", code: "OP", color: ChainPalette.optimism, chainId: "10"),
        NetworkFixture(name: "Base", code: "BASE", color: ChainPalette.base, chainId: "8453"),
        NetworkFixture(name: "Avalanche", code: "AVAX", color: ChainPalette.avalanche, chainId: "43114"),
        NetworkFixture(name: "Gnosis", code: "GNO", color: ChainPalette.gnosis, chainId: "100"),
    ]

    /// USDT on Ethereum — the real contract, as the mocks print it.
    static let usdtContract = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    static let usdtContractShort = "0xdAC1…1ec7"

    // Spec 018's roster, reused rather than re-invented.
    private static let aliceDisplay = "0x9F3c…21aE"
    private static let aliceFull = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE"
    private static let aHaoFull = "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02"
    private static let holdOnDisplay = "0xCafe…F00d"
    private static let holdOnFull = "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d"

    private static let txHashReceived = "0x8f3a…c21d"
    private static let txHashSent = "0x3c2d…8e1f"

    /// Split a 0x address into the two lines the mocks wrap it into.
    /// 42 characters, so 21 and 21 — an even break rather than one that leaves
    /// a stub on the second line.
    static func addressLines(_ address: String) -> [String] {
        let half = (address.count + 1) / 2
        let index = address.index(address.startIndex, offsetBy: half)
        return [String(address[..<index]), String(address[index...])]
    }

    private struct AssetFixture {
        let ticker: String
        let chain: String
        let color: Color
        let balance: String
        let fiat: String
    }

    /// The assets T1 lists, verbatim from the mock.
    private static let assets: [AssetFixture] = [
        AssetFixture(ticker: "BNB", chain: "BNB Chain", color: ChainPalette.bnb, balance: "0.8533", fiat: "$496.46"),
        AssetFixture(ticker: "ETH", chain: "Arbitrum", color: ChainPalette.arbitrum, balance: "0.2253", fiat: "$422.62"),
        AssetFixture(ticker: "ETH", chain: "Ethereum", color: ChainPalette.ethereum, balance: "0.0689", fiat: "$129.25"),
        AssetFixture(ticker: "XDAI", chain: "Gnosis", color: ChainPalette.gnosis, balance: "74.3965", fiat: "$74.38"),
        AssetFixture(ticker: "USDT", chain: "Ethereum", color: ChainPalette.ethereum, balance: "53.4836", fiat: "$53.48"),
        AssetFixture(ticker: "USDC", chain: "Polygon", color: ChainPalette.polygon, balance: "12.04", fiat: "$12.04"),
    ]

    /// SD1's order differs from T1's: the picker leads with what you'd send.
    private static let sendAssets: [AssetFixture] = [
        assets[4],
        assets[2],
        AssetFixture(ticker: "USDC", chain: "Ethereum", color: ChainPalette.ethereum, balance: "18.20", fiat: "$18.20"),
        assets[0],
        assets[3],
    ]

    private static func row(_ a: AssetFixture) -> AssetRowModel {
        AssetRowModel(
            ticker: a.ticker, chain: a.chain, badgeColor: a.color,
            balance: a.balance, fiat: .value(a.fiat), masked: false
        )
    }

    // MARK: - Assembly

    private static func pill(_ loc: Loc) -> FlowPillModel {
        FlowPillModel(dots: ChainPalette.pillDots, label: loc.t("componentsUi.networkFilter.pillAll"))
    }

    private static func receiveList(_ loc: Loc) -> ReceiveListModel {
        ReceiveListModel(
            header: FlowHeaderModel(
                title: loc.t("receive.title"),
                backLabel: loc.t("receive.a11yBack")
            ),
            subtitle: loc.t("receive.networksLine", vars: ["count": String(WalletFixtures.networkCount)]),
            searchPlaceholder: loc.t("receive.searchNetworkPlaceholder"),
            emptyText: loc.t("receive.searchNetworkEmpty"),
            rows: networks.map { n in
                NetworkRowModel(
                    name: n.name,
                    code: n.code,
                    badgeColor: n.color,
                    addressDisplay: WalletFixtures.identity.addressDisplay,
                    copyLabel: loc.t("componentsUi.identiconViewer.copyAddress"),
                    qrLabel: loc.t("componentsUi.scanner.title")
                )
            }
        )
    }

    private static func receiveQr(_ loc: Loc, asset: Bool) -> ReceiveQrModel {
        let network = networks[0]
        return ReceiveQrModel(
            title: asset
                ? loc.t("receive.qrTitleAsset", vars: ["network": network.name, "symbol": "USDT"])
                : loc.t("receive.qrTitleNetwork", vars: ["network": network.name]),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            contract: asset
                ? ContractLineModel(
                    label: loc.t("receive.tokenContract"),
                    value: usdtContractShort,
                    copyLabel: loc.t("componentsUi.identiconViewer.copyAddress")
                )
                : nil,
            account: AddressCardModel(
                name: WalletFixtures.identity.name,
                identiconSeed: WalletFixtures.identity.addressFull,
                lines: addressLines(WalletFixtures.identity.addressFull),
                copyLabel: loc.t("componentsUi.identiconViewer.copyAddress")
            ),
            centre: asset
                ? TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.gnosis)
                : TokenMarkModel(ticker: network.code, badgeColor: network.color),
            warning: loc.t("receive.warningReminder"),
            saveImage: loc.t("receive.request.saveImage"),
            viewOnExplorer: loc.t("history.viewOnExplorer")
        )
    }

    private static func shareCard(_ loc: Loc) -> ShareCardModel {
        let network = networks[0]
        return ShareCardModel(
            headline: loc.t("receive.shareCardHeadline"),
            name: WalletFixtures.identity.name,
            lines: addressLines(WalletFixtures.identity.addressFull),
            networkNote: loc.t("receive.shareCardNetworkNote", vars: ["network": network.name]),
            networkMark: TokenMarkModel(ticker: network.code, badgeColor: network.color),
            identiconSeed: WalletFixtures.identity.addressFull,
            wordmark: "Vela Wallet"
        )
    }

    static func scan(_ loc: Loc) -> ScanModel {
        ScanModel(
            title: loc.t("componentsUi.scanner.title"),
            hint: loc.t("componentsUi.scanner.hint"),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            tools: [
                ScanToolModel(id: .gallery, label: loc.t("componentsUi.scanner.gallery")),
                ScanToolModel(id: .torch, label: loc.t("componentsUi.scanner.torch")),
                ScanToolModel(id: .flip, label: loc.t("componentsUi.scanner.flipCamera")),
            ]
        )
    }

    private static func historyGroups(_ loc: Loc) -> [ActivityGroupModel] {
        let sent = loc.t("history.labelSent")
        let received = loc.t("history.labelReceived")
        func to(_ name: String, _ clock: String) -> String {
            "\(loc.t("history.toName", vars: ["name": name])) · \(clock)"
        }
        func from(_ name: String, _ clock: String) -> String {
            "\(loc.t("history.fromName", vars: ["name": name])) · \(clock)"
        }

        return [
            ActivityGroupModel(label: loc.t("componentsUi.dayGroup.today"), rows: [
                ActivityRowModel(
                    kind: .sent, title: sent, subtitle: to("hold on", "14:02"),
                    amount: "−2", unit: "POL", positive: false, masked: false,
                    badgeColor: ChainPalette.polygon
                ),
                ActivityRowModel(
                    kind: .received, title: received, subtitle: from(aliceDisplay, "11:20"),
                    amount: "+120", unit: "USDT", positive: true, masked: false,
                    badgeColor: ChainPalette.ethereum
                ),
            ]),
            ActivityGroupModel(label: loc.t("componentsUi.dayGroup.yesterday"), rows: [
                ActivityRowModel(
                    kind: .received, title: received, subtitle: from("Alice", "20:15"),
                    amount: "+50", unit: "USDC", positive: true, masked: false,
                    badgeColor: ChainPalette.base
                ),
                ActivityRowModel(
                    kind: .sent, title: sent, subtitle: to("Bob", "09:12"),
                    amount: "−0.4", unit: "XDAI", positive: false, masked: false,
                    badgeColor: ChainPalette.gnosis
                ),
            ]),
            // A literal date once the run of named days ends — the mock's 8月12日.
            ActivityGroupModel(label: "8/12", rows: [
                ActivityRowModel(
                    kind: .received, title: received, subtitle: from("0x21aE…9F3c", "08:44"),
                    amount: "+0.9", unit: "BNB", positive: true, masked: false,
                    badgeColor: ChainPalette.bnb
                ),
            ]),
        ]
    }

    private static func history(_ loc: Loc) -> HistoryModel {
        HistoryModel(
            header: FlowHeaderModel(
                title: loc.t("history.navTitle"),
                backLabel: loc.t("receive.a11yBack"),
                pill: pill(loc)
            ),
            mode: .rows,
            emptyText: loc.t("history.emptyFilter"),
            groups: historyGroups(loc)
        )
    }

    private static func txDetail(_ loc: Loc, received: Bool) -> TxDetailModel {
        let network = received ? networks[0] : networks[2]
        var facts: [FactRowModel] = [
            FactRowModel(
                label: received ? loc.t("componentsTx.detail.from") : loc.t("componentsTx.detail.to"),
                value: received ? aliceDisplay : "hold on",
                lead: .identicon(received ? aliceFull : holdOnFull),
                mono: received,
                copy: loc.t("componentsUi.identiconViewer.copyAddress")
            ),
            FactRowModel(
                label: loc.t("componentsTx.detail.labelChain"),
                value: network.name,
                lead: .token(TokenMarkModel(ticker: network.code, badgeColor: network.color))
            ),
        ]
        // Only an ERC-20 transfer has a contract. A3's native coin does not,
        // and an empty row there invites "which contract?".
        if received {
            facts.append(FactRowModel(
                // 代币合约, not 合约: the token sheet is already about a token,
                // so it says "contract"; a transaction has to say WHICH one.
                label: loc.t("receive.tokenContract"),
                value: usdtContractShort,
                mono: true,
                copy: loc.t("componentsUi.identiconViewer.copyAddress")
            ))
        }
        facts.append(FactRowModel(
            label: loc.t("componentsTx.detail.labelDate"),
            value: "\(loc.t("componentsUi.dayGroup.today")) \(received ? "11:20" : "14:02")"
        ))
        facts.append(FactRowModel(
            label: loc.t("componentsTx.detail.labelHash"),
            value: received ? txHashReceived : txHashSent,
            mono: true,
            copy: loc.t("componentsUi.identiconViewer.copyAddress")
        ))

        return TxDetailModel(
            title: received
                ? loc.t("history.txLabelReceived", vars: ["symbol": "USDT"])
                : loc.t("history.txLabelSent", vars: ["symbol": "POL"]),
            status: StatusChipModel(text: loc.t("componentsTx.receipt.statusConfirmed"), tone: .success),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            amount: received ? "+120 USDT" : "−2 POL",
            fiat: received ? "≈ $120.00" : "≈ $0.98",
            positive: received,
            facts: facts,
            viewOnExplorer: loc.t("history.viewOnExplorer")
        )
    }

    private static func assetsModel(_ loc: Loc, empty: Bool) -> AssetsModel {
        AssetsModel(
            header: FlowHeaderModel(
                title: loc.t("assets.sectionTitle"),
                backLabel: loc.t("receive.a11yBack"),
                action: loc.t("assets.addToken"),
                pill: pill(loc)
            ),
            searchPlaceholder: loc.t("assets.searchPlaceholder"),
            rows: empty ? [] : assets.map(row),
            addByAddress: loc.t("assets.addByAddress"),
            empty: empty ? assetsEmpty(loc) : nil
        )
    }

    /// T4's guided-empty body. Its own function because the live list needs
    /// the same words on a T1 that turned out empty.
    static func assetsEmpty(_ loc: Loc) -> AssetsEmptyModel {
        AssetsEmptyModel(
            title: loc.t("assets.emptyTitle"),
            caption: loc.t("assets.emptySubtext"),
            cta: loc.t("addToken.navTitle"),
            hintTitle: loc.t("assets.notShowingTitle"),
            hintBody: loc.t("assets.notShowingBody")
        )
    }

    private static func tokenDetail(_ loc: Loc) -> TokenDetailModel {
        TokenDetailModel(
            mark: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
            symbol: "USDT",
            chain: "Ethereum",
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            balance: "53.4836 USDT",
            fiat: "$53.48",
            receive: loc.t("tokenDetail.receive"),
            send: loc.t("tokenDetail.send"),
            facts: [
                FactRowModel(
                    label: loc.t("tokenDetail.labelPrice"),
                    value: loc.t("tokenDetail.priceValue", vars: ["symbol": "USDT", "value": "$1.00"])
                ),
                FactRowModel(
                    label: loc.t("tokenDetail.labelContract"),
                    value: usdtContractShort,
                    mono: true,
                    copy: loc.t("componentsUi.identiconViewer.copyAddress")
                ),
                FactRowModel(label: loc.t("tokenDetail.labelDecimals"), value: "6"),
                FactRowModel(label: loc.t("addToken.labelNetwork"), value: "Ethereum"),
            ],
            transactionsTitle: loc.t("tokenDetail.labelTransactions"),
            rows: [
                ActivityRowModel(
                    kind: .received,
                    title: loc.t("history.labelReceived"),
                    subtitle: "\(loc.t("history.fromName", vars: ["name": aliceDisplay])) · \(loc.t("componentsUi.dayGroup.today"))",
                    amount: "+120", unit: "USDT", positive: true, masked: false,
                    badgeColor: ChainPalette.polygon
                ),
                ActivityRowModel(
                    kind: .sent,
                    title: loc.t("history.labelSent"),
                    subtitle: "\(loc.t("history.toName", vars: ["name": "Alice"])) · 8/10",
                    amount: "−30", unit: "USDT", positive: false, masked: false,
                    badgeColor: ChainPalette.polygon
                ),
            ],
            viewOnExplorer: loc.t("tokenDetail.viewOnExplorer")
        )
    }

    /// T3 / T3b and their T5 / T5b failure variants.
    enum AddVariant {
        case erc20, native, erc20Invalid, erc20NotFound, nativeIncompatible
    }

    private static func addToken(_ loc: Loc, _ variant: AddVariant) -> AddTokenModel {
        let isNative = variant == .native || variant == .nativeIncompatible
        let avax = networks[6]

        if isNative {
            return AddTokenModel(
                title: loc.t("addToken.navTitle"),
                closeLabel: loc.t("componentsUi.identiconViewer.close"),
                tab: .native,
                tabErc20: loc.t("addToken.tabErc20"),
                tabNative: loc.t("addToken.tabNative"),
                fieldLabel: loc.t("addToken.netSearchLabel"),
                fieldValue: "Avalanche",
                fieldPlaceholder: loc.t("addToken.netSearchPlaceholder"),
                result: .network(
                    mark: TokenMarkModel(ticker: avax.code, badgeColor: avax.color),
                    name: avax.name,
                    chip: variant == .nativeIncompatible
                        ? StatusChipModel(text: loc.t("addToken.notCompatible"), tone: .error)
                        : StatusChipModel(text: loc.t("addToken.compatible"), tone: .success),
                    facts: [
                        FactRowModel(label: loc.t("addToken.labelChainId"), value: avax.chainId),
                        FactRowModel(label: loc.t("addToken.labelNativeToken"), value: avax.code),
                    ],
                    link: variant == .nativeIncompatible
                        ? "\(loc.t("addToken.errorNotCompatible")) · \(loc.t("addToken.deployContracts"))"
                        : nil
                ),
                cta: loc.t("addToken.addNetworkBtn"),
                ctaDisabled: variant != .native
            )
        }

        let value: String = switch variant {
        case .erc20Invalid: String(usdtContract.dropLast(4))
        case .erc20NotFound: "0x1234…abcd"
        default: usdtContract
        }
        let result: AddTokenResult = switch variant {
        case .erc20Invalid: .none
        case .erc20NotFound:
            .notFound("\(loc.t("addToken.notFoundTitle")) — \(loc.t("addToken.notFoundMessage"))")
        default:
            .token(
                mark: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
                name: "Tether USD",
                detail: "USDT · \(loc.t("tokenDetail.labelDecimals")) 6 · Ethereum",
                chip: nil
            )
        }

        return AddTokenModel(
            title: loc.t("addToken.navTitle"),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            tab: .erc20,
            tabErc20: loc.t("addToken.tabErc20"),
            tabNative: loc.t("addToken.tabNative"),
            network: AddTokenNetworkModel(
                mark: TokenMarkModel(ticker: networks[0].code, badgeColor: networks[0].color),
                name: networks[0].name,
                pickLabel: loc.t("addToken.netPickerSearchPlaceholder")
            ),
            fieldLabel: loc.t("addToken.tokenAddressLabel"),
            fieldValue: value,
            fieldPlaceholder: usdtContract,
            fieldError: variant == .erc20Invalid ? loc.t("addToken.invalidAddress") : nil,
            result: result,
            cta: loc.t("addToken.addToWalletBtn"),
            ctaDisabled: variant != .erc20
        )
    }

    private static func sendPick(_ loc: Loc, multi: Bool) -> SendPickModel {
        SendPickModel(
            header: FlowHeaderModel(
                title: multi ? loc.t("send.multiSendTitle") : loc.t("send.selectTokenTitle"),
                backLabel: loc.t("receive.a11yBack"),
                pill: pill(loc)
            ),
            searchPlaceholder: loc.t("send.searchPlaceholder"),
            filters: [
                FilterChipModel(id: "all", label: loc.t("history.filterAll"), selected: true),
                FilterChipModel(id: "stable", label: loc.t("send.filterStable"), selected: false),
                FilterChipModel(id: "gas", label: loc.t("send.filterGas"), selected: false),
                FilterChipModel(id: "other", label: loc.t("send.filterOther"), selected: false),
            ],
            notice: multi
                ? SendNoticeModel(
                    mark: TokenMarkModel(ticker: networks[0].code, badgeColor: networks[0].color),
                    text: loc.t("send.multiSendChainNotice", vars: ["network": networks[0].name])
                )
                : nil,
            rows: sendAssets.map(row),
            selection: multi
                // The first three rows are on Ethereum; the last two are not,
                // which is exactly what the greying is there to explain.
                ? SendSelectionModel(
                    selected: [true, true, true, false, false],
                    dimmed: [false, false, false, true, true],
                    selectAll: loc.t("send.selectAllValuable")
                )
                : nil,
            cta: multi
                ? SendCtaModel(
                    label: loc.t("send.multiSendContinue", vars: ["n": "3", "chain": networks[0].name]),
                    accent: true
                )
                : SendCtaModel(label: loc.t("send.multiSendTitle"), accent: false)
        )
    }

    private static func sendForm(_ loc: Loc, mode: SendFormMode) -> SendFormModel {
        let feeValue: String = switch mode {
        case .single: "0.0021 ETH · ≈$0.55"
        case .split: "0.0034 ETH · ≈$0.89"
        case .sweep: "0.0041 ETH · ≈$1.07"
        }
        let fee = FeeRowModel(
            label: loc.t("componentsUi.gas.networkFee"),
            mark: TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum),
            value: feeValue,
            openLabel: loc.t("send.feeTokenLabel")
        )
        let header = FlowHeaderModel(
            title: mode == .sweep
                ? loc.t("send.multiSendTitle")
                : loc.t("send.sendTitle", vars: ["symbol": "USDT"]),
            backLabel: loc.t("receive.a11yBack")
        )

        if mode == .sweep {
            return SendFormModel(
                header: header,
                mode: mode,
                sweepSummary: loc.t("send.multiSendSummary", vars: ["n": "3", "chain": networks[0].name]),
                sweepRows: [
                    SweepRowModel(
                        mark: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
                        symbol: "USDT",
                        balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "53.4836"]),
                        amount: "53.4836", max: loc.t("send.maxBtn")
                    ),
                    SweepRowModel(
                        mark: TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum),
                        symbol: "ETH",
                        balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "0.0689"]),
                        amount: "0.05", max: loc.t("send.maxBtn")
                    ),
                    SweepRowModel(
                        mark: TokenMarkModel(ticker: "USDC", badgeColor: ChainPalette.ethereum),
                        symbol: "USDC",
                        balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "18.20"]),
                        amount: "18.20", max: loc.t("send.maxBtn")
                    ),
                ],
                recipient: RecipientFieldModel(
                    label: loc.t("send.recipientLabel"),
                    lines: [aliceDisplay],
                    identiconSeed: aliceFull,
                    pickLabel: loc.t("send.recipientPickAria"),
                    scanLabel: loc.t("send.scanAria"),
                    note: loc.t("send.multiSendSameRecipient")
                ),
                fee: fee,
                cta: loc.t("send.continueBtn")
            )
        }

        let token = SendTokenCardModel(
            mark: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
            symbol: "USDT",
            detail: "Ethereum · \(loc.t("send.balanceLabel", vars: ["amount": "53.4836"]))",
            max: mode == .single ? loc.t("send.maxBtn") : nil
        )

        if mode == .split {
            return SendFormModel(
                header: header,
                mode: mode,
                token: token,
                recipients: [
                    RecipientCardModel(
                        ordinal: loc.t("send.recipientN", vars: ["n": "1"]),
                        name: aliceDisplay, identiconSeed: aliceFull, amount: "50",
                        removeLabel: loc.t("send.removeRecipient")
                    ),
                    RecipientCardModel(
                        ordinal: loc.t("send.recipientN", vars: ["n": "2"]),
                        name: "Alice", identiconSeed: aHaoFull, amount: "30",
                        removeLabel: loc.t("send.removeRecipient")
                    ),
                    RecipientCardModel(
                        ordinal: loc.t("send.recipientN", vars: ["n": "3"]),
                        name: "hold on", identiconSeed: holdOnFull, amount: "40",
                        removeLabel: loc.t("send.removeRecipient")
                    ),
                ],
                recipientActions: [
                    RecipientActionModel(id: .add, label: loc.t("send.addRecipient")),
                    RecipientActionModel(id: .contacts, label: loc.t("send.fromContacts")),
                    RecipientActionModel(id: .importList, label: loc.t("send.batchImport")),
                ],
                summary: SummaryLineModel(
                    label: "\(loc.t("send.splitTotalLabel")) · \(loc.t("send.recipientCount_other", vars: ["count": "3"]))",
                    value: "120 USDT · ≈$120.00"
                ),
                fee: fee,
                cta: loc.t("send.continueBtn")
            )
        }

        return SendFormModel(
            header: header,
            mode: mode,
            token: token,
            amount: AmountFieldModel(
                value: "120", fiat: "≈ $120.00", denomLabel: loc.t("send.feeTokenLabel")
            ),
            recipient: RecipientFieldModel(
                label: loc.t("send.recipientLabel"),
                lines: addressLines(aliceFull),
                identiconSeed: aliceFull,
                pickLabel: loc.t("send.recipientPickAria"),
                // The scan button its own SWEEP sibling has drawn since 021.
                // Without it the single send's recipient row offered the
                // address book and nothing else, and the scanner spec 055
                // built was reachable only from inside that book.
                scanLabel: loc.t("send.scanAria")
            ),
            addRecipient: loc.t("send.addRecipient"),
            fee: fee,
            cta: loc.t("send.continueBtn")
        )
    }

    private static func contactPick(_ loc: Loc) -> ContactPickModel {
        ContactPickModel(
            title: loc.t("send.pickContactTitle"),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            searchPlaceholder: loc.t("send.pickContactSearch"),
            scanRow: loc.t("send.scanToFill"),
            groupsTitle: loc.t("contacts.sectionGroups"),
            groups: [
                ContactGroupModel(
                    name: "家人",
                    count: loc.t("contacts.groupMembers", vars: ["count": "3"]),
                    colors: [ChainPalette.polygon, ChainPalette.bnb]
                ),
                ContactGroupModel(
                    name: "工作",
                    count: loc.t("contacts.groupMembers", vars: ["count": "5"]),
                    colors: [ChainPalette.gnosis, ChainPalette.arbitrum]
                ),
            ],
            contactsTitle: loc.t("contacts.title"),
            contacts: [
                ContactEntryModel(name: "Alice", group: "家人", addressDisplay: aliceDisplay, identiconSeed: aliceFull),
                ContactEntryModel(name: "阿豪", addressDisplay: "0x77Bd…4F02", identiconSeed: aHaoFull),
                ContactEntryModel(name: "hold on", addressDisplay: holdOnDisplay, identiconSeed: holdOnFull),
            ]
        )
    }

    private static func feeTokenPick(_ loc: Loc) -> FeeTokenPickModel {
        FeeTokenPickModel(
            title: loc.t("send.feeTokenLabel"),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            hint: loc.t("send.feeTokenHint"),
            estimateLabel: loc.t("send.feeTokenEstimate"),
            rows: [
                FeeTokenRowModel(
                    mark: TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum),
                    symbol: "ETH",
                    balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "0.0689"]),
                    fee: "~0.0021 ETH", selected: true
                ),
                FeeTokenRowModel(
                    mark: TokenMarkModel(ticker: "USDC", badgeColor: ChainPalette.ethereum),
                    symbol: "USDC",
                    balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "18.20"]),
                    fee: "~0.55 USDC", selected: false
                ),
                FeeTokenRowModel(
                    mark: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
                    symbol: "USDT",
                    balanceLabel: loc.t("send.balanceLabel", vars: ["amount": "53.4836"]),
                    fee: "~0.55 USDT", selected: false
                ),
            ]
        )
    }

    private static func batchImport(_ loc: Loc) -> BatchImportModel {
        BatchImportModel(
            title: loc.t("send.batchTitle"),
            closeLabel: loc.t("componentsUi.identiconViewer.close"),
            unitFiat: loc.t("send.batchUnitFiat", vars: ["code": "CNY"]),
            unitToken: loc.t("send.batchUnitToken", vars: ["sym": "USDT"]),
            unit: .fiat,
            pasteValue: "0xabc… , 5000\n0xdef… , 8000",
            pastePlaceholder: loc.t("send.batchPastePlaceholder"),
            importFile: "\(loc.t("send.batchImportFile")) (xlsx / csv / txt)",
            template: loc.t("send.batchTemplate"),
            rateSection: loc.t("send.batchRateSection"),
            rateLabel: loc.t("send.batchRateLabel", vars: ["sym": "USDT"]),
            rateValue: "7.25 CNY",
            rateHint: loc.t("send.batchRateHint", vars: ["code": "CNY", "sym": "USDT"]),
            parsedLabel: loc.t("send.batchParsedCount", vars: ["n": "3"]),
            rows: [
                BatchRowModel(ok: true, address: aliceDisplay, conversion: "5,000 CNY → 689.66"),
                BatchRowModel(ok: true, address: "0x21aE…9F3c", conversion: "8,000 CNY → 1,103.45"),
                BatchRowModel(ok: false, address: "0x12zz…\(loc.t("send.batchBadAddress"))", conversion: "—"),
            ],
            rejectedText: loc.t("send.batchRejected_one", vars: ["count": "1"]),
            // Two of three rows parsed, so the button offers two — never three.
            cta: loc.t("send.batchApply_other", vars: ["count": "2"]),
            ctaDisabled: false
        )
    }

    private static func sendConfirm(_ loc: Loc, mode: SendFormMode) -> SendConfirmModel {
        let estFee: String = switch mode {
        case .single: "~0.0021 ETH · ≈$0.55"
        case .split: "~0.0034 ETH · ≈$0.89"
        case .sweep: "~0.0041 ETH · ≈$1.07"
        }
        let facts: [FactRowModel] = [
            FactRowModel(
                label: loc.t("send.fromLabel"),
                value: WalletFixtures.identity.name,
                lead: .identicon(WalletFixtures.identity.addressFull)
            ),
            FactRowModel(
                label: loc.t("send.toLabel"),
                value: mode == .split
                    ? loc.t("send.recipientCount_other", vars: ["count": "3"])
                    : aliceDisplay,
                lead: mode == .split ? nil : .identicon(aliceFull),
                mono: mode != .split
            ),
            FactRowModel(
                label: loc.t("componentsTx.detail.labelChain"),
                value: networks[0].name,
                lead: .token(TokenMarkModel(ticker: networks[0].code, badgeColor: networks[0].color))
            ),
            FactRowModel(label: loc.t("send.estFeeLabel"), value: estFee),
        ]
        let header = FlowHeaderModel(
            title: loc.t("send.confirmTitle"),
            backLabel: loc.t("receive.a11yBack")
        )

        switch mode {
        case .sweep:
            return SendConfirmModel(
                header: header,
                amount: loc.t("componentsTx.receipt.assetsCount", vars: ["n": "3"]),
                subline: loc.t(
                    "send.confirmTotalLine",
                    vars: ["fiat": "$200.90", "network": networks[0].name]
                ),
                facts: facts,
                breakdown: [
                    BreakdownRowModel(
                        lead: TokenMarkModel(ticker: "USDT", badgeColor: ChainPalette.ethereum),
                        label: "USDT", value: "53.4836 USDT · ≈$53.48"
                    ),
                    BreakdownRowModel(
                        lead: TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum),
                        label: "ETH", value: "0.05 ETH · ≈$93.79"
                    ),
                    BreakdownRowModel(
                        lead: TokenMarkModel(ticker: "USDC", badgeColor: ChainPalette.ethereum),
                        label: "USDC", value: "18.20 USDC · ≈$18.20"
                    ),
                ],
                cta: loc.t("send.confirmSendBtn")
            )
        case .split:
            return SendConfirmModel(
                header: header,
                amount: "120 USDT",
                subline: "≈ $120.00",
                facts: facts,
                breakdown: [
                    BreakdownRowModel(identiconSeed: aliceFull, label: aliceDisplay, value: "50 USDT"),
                    BreakdownRowModel(identiconSeed: aHaoFull, label: "Alice", value: "30 USDT"),
                    BreakdownRowModel(identiconSeed: holdOnFull, label: "hold on", value: "40 USDT"),
                ],
                cta: loc.t("send.confirmSendBtn")
            )
        case .single:
            return SendConfirmModel(
                header: header,
                mark: TokenMarkModel.of(chainId: 1, symbol: "USDT", color: ChainPalette.ethereum),
                amount: "120",
                amountUnit: "USDT",
                subline: "≈ $120.00",
                facts: facts,
                cta: loc.t("send.confirmSendBtn")
            )
        }
    }

    private static func sendReceipt(_ loc: Loc, stage: ReceiptStage) -> SendReceiptModel {
        let header = FlowHeaderModel(
            title: loc.t("send.sendTitle", vars: ["symbol": "USDT"]),
            backLabel: loc.t("receive.a11yBack")
        )
        switch stage {
        case .submitting:
            return SendReceiptModel(
                header: header, stage: stage,
                title: loc.t("send.txSubmitting"),
                captions: [
                    loc.t("send.txPreparingBiometric"),
                    loc.t("send.txBackgroundHint"),
                ],
                cta: loc.t("send.txCloseBackground"), ctaAccent: false
            )
        case .submitted, .failed:
            return SendReceiptModel(
                header: header, stage: .submitted,
                title: loc.t("send.txSubmittedTitle"),
                captions: [
                    loc.t("send.txWaitingConfirm"),
                    loc.t("send.txTypicalTime", vars: ["chainName": networks[0].name, "estSecs": "12"]),
                ],
                cta: loc.t("send.txCloseBackground"), ctaAccent: false
            )
        case .confirmed:
            return SendReceiptModel(
                header: header, stage: stage,
                title: loc.t("send.txConfirmedTitle", vars: ["amount": "120", "symbol": "USDT"]),
                captions: [
                    "\(loc.t("history.toName", vars: ["name": aliceDisplay])) · \(networks[0].name)",
                ],
                hash: ReceiptHashModel(
                    label: loc.t("componentsTx.receipt.txHash"),
                    value: txHashReceived,
                    copyLabel: loc.t("componentsUi.identiconViewer.copyAddress")
                ),
                viewOnExplorer: loc.t("history.viewOnExplorer"),
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        }
    }

    // MARK: - Builder

    /// Build one state (spec.md's state matrix).
    static func build(_ state: FlowStateId, loc: Loc) -> FlowScreenModel {
        let scale: CGFloat = state == .r2x ? 1.35 : 1

        func screen(_ base: FlowBase, _ sheet: WalletFlowSheet? = nil) -> FlowScreenModel {
            FlowScreenModel(state: state, base: base, sheet: sheet, textScale: scale)
        }

        switch state {
        case .r1:
            return screen(.receive(receiveList(loc)))
        case .r2, .r2x:
            return screen(.receive(receiveList(loc)), .receiveQr(receiveQr(loc, asset: false)))
        case .r3:
            return screen(.receive(receiveList(loc)), .receiveQr(receiveQr(loc, asset: true)))
        case .r4:
            return screen(.share(shareCard(loc)))
        case .s1:
            return screen(.scan(scan(loc)))
        case .a1:
            return screen(.history(history(loc)))
        case .a2:
            return screen(.history(history(loc)), .txDetail(txDetail(loc, received: true)))
        case .a3:
            return screen(.history(history(loc)), .txDetail(txDetail(loc, received: false)))
        case .t1:
            return screen(.assets(assetsModel(loc, empty: false)))
        case .t2:
            return screen(.assets(assetsModel(loc, empty: false)), .tokenDetail(tokenDetail(loc)))
        case .t3:
            return screen(.assets(assetsModel(loc, empty: false)), .addToken(addToken(loc, .erc20)))
        case .t3b:
            return screen(.assets(assetsModel(loc, empty: false)), .addToken(addToken(loc, .native)))
        case .t4:
            return screen(.assets(assetsModel(loc, empty: true)))
        case .t5:
            return screen(.assets(assetsModel(loc, empty: false)), .addToken(addToken(loc, .erc20Invalid)))
        case .t5b:
            return screen(.assets(assetsModel(loc, empty: false)), .addToken(addToken(loc, .nativeIncompatible)))
        case .sd1:
            return screen(.sendPick(sendPick(loc, multi: false)))
        case .sd1b:
            return screen(.sendPick(sendPick(loc, multi: true)))
        case .sd2:
            return screen(.sendForm(sendForm(loc, mode: .single)))
        case .sd2b:
            return screen(.sendForm(sendForm(loc, mode: .split)))
        case .sd2d:
            return screen(.sendForm(sendForm(loc, mode: .sweep)))
        case .sd2c:
            return screen(.sendForm(sendForm(loc, mode: .split)), .batchImport(batchImport(loc)))
        case .sd2e:
            return screen(.sendForm(sendForm(loc, mode: .single)), .contactPick(contactPick(loc)))
        case .sd2f:
            return screen(.sendForm(sendForm(loc, mode: .single)), .feeToken(feeTokenPick(loc)))
        case .sd3:
            return screen(.sendConfirm(sendConfirm(loc, mode: .single)))
        case .sd3b:
            return screen(.sendConfirm(sendConfirm(loc, mode: .split)))
        case .sd3c:
            return screen(.sendConfirm(sendConfirm(loc, mode: .sweep)))
        case .sd4a:
            return screen(.sendReceipt(sendReceipt(loc, stage: .submitting)))
        case .sd4b:
            return screen(.sendReceipt(sendReceipt(loc, stage: .submitted)))
        case .sd4c:
            return screen(.sendReceipt(sendReceipt(loc, stage: .confirmed)))
        }
    }
}
