//
//  SigningFixtures.swift
//  VelaWallet
//
//  Canonical signing fixtures (spec 022, data-model.md §3 — the single canon
//  all four platforms port; web reference: src/lib/signing/fixtures.ts).
//  Amounts, addresses and contract names are verbatim mock content; every
//  label resolves through the corpus.
//
//  The catalogue doubles as the degradation ladder's regression suite:
//  cs23–cs24 and cs30–cs32 are the rungs below "verified descriptor", and
//  they are here so any change to the renderer has to face what a wallet
//  shows when it does NOT know what a transaction does.
//

import SwiftUI

enum SigningFixtures {
    // MARK: - Canon

    static let network = (name: "Ethereum", dot: ChainPalette.ethereum)
    static let feeValue = "~0.0021 ETH ≈ $5.40"

    /// Token marks: brand content, exactly like the wallet's chain colours.
    enum Mark {
        static let usdc = TokenMark(letter: "U", tint: BrandPalette.usdc)
        static let eth = TokenMark(letter: "E", tint: BrandPalette.eth)
        static let weth = TokenMark(letter: "W", tint: BrandPalette.weth)
        static let spweth = TokenMark(letter: "S", tint: BrandPalette.spweth)
        static let usdt = TokenMark(letter: "T", tint: BrandPalette.usdt)
        static let contact = TokenMark(letter: "A", tint: BrandPalette.contact)
    }

    /// dApps as the signing header draws them.
    private enum D {
        static let uniswap = (name: "Uniswap", host: "app.uniswap.org", letter: "U",
                              tint: BrandPalette.uniswap)
        static let oneinch = (name: "1inch", host: "app.1inch.io", letter: "1",
                              tint: BrandPalette.oneinch)
        static let opensea = (name: "OpenSea", host: "opensea.io", letter: "O",
                              tint: BrandPalette.opensea)
        static let morpho = (name: "Morpho", host: "app.morpho.org", letter: "M",
                             tint: BrandPalette.morpho)
        static let safe = (name: "Safe", host: "app.safe.global", letter: "S",
                           tint: BrandPalette.safe)
        static let ens = (name: "ENS", host: "app.ens.domains", letter: "E",
                          tint: BrandPalette.ens)
        static let phish = (name: "opensae-mint", host: "opensae-mint.xyz", letter: "O",
                            tint: BrandPalette.unknown)
    }

    private enum Addr {
        static let alice = "0xaF5e…b3e1"
        static let aliceFull = "0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1"
        static let vitalik = "0xd8dA…6045"
        static let oneinchRouter = "0x1111…0582"
        static let universalRouter = "0x3fC9…7FAD"
        static let uniswapV3 = "0x68b3…4dC5"
        static let bayc = "0xBC4C…f13D"
        static let conduit = "0x1E00…3c71"
        static let morphoVault = "0x38989B…21eB"
        static let unknown = "0x4e1dC6…A9C1"
        static let rewards = "0x067d3D…2ed1"
        static let usdt = "0xdAC1…1ec7"
        static let safe = "0x4167…461a"
        static let deployed = "0x1A2b…9304"
        static let deepest = "0x004C22…6819"
        static let usdcFull = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        static var selfShort: String { WalletFixtures.identity.addressDisplay }
    }

    // MARK: - Small builders

    private static func t(_ loc: Loc, _ key: String, _ vars: [String: String] = [:]) -> String {
        loc.t("componentsUi.signing.\(key)", vars: vars)
    }

    private static func a(_ loc: Loc, _ key: String, _ vars: [String: String] = [:]) -> String {
        loc.t("componentsUi.signingApprove.\(key)", vars: vars)
    }

    private static func chip(_ id: String, _ label: String,
                             _ state: AllowanceChip.State) -> AllowanceChip {
        AllowanceChip(id: id, label: label, state: state)
    }

    private static func onchainFee(_ loc: Loc) -> FeeModel {
        .onchain(label: loc.t("componentsUi.gas.networkFee"), value: feeValue, selector: nil)
    }

    private static func tech(_ loc: Loc, summary: String? = nil,
                             fn: (label: String, signature: String)? = nil,
                             params: [SigningRow] = [], identities: [TechIdentity] = [],
                             simResult: SigningRow? = nil,
                             raw: (label: String, hex: String)? = nil) -> TechModel {
        TechModel(
            title: t(loc, "advancedToggle"), summary: summary, fn: fn, params: params,
            identities: identities, simResult: simResult, raw: raw,
            copyLabel: t(loc, "copyValue"), explorerLabel: t(loc, "viewOnExplorer")
        )
    }

    /// cs1/cs29's five-layer panel — the one the founder asked to see in full.
    private static func transferTech(_ loc: Loc) -> TechModel {
        tech(
            loc,
            fn: (label: t(loc, "techFunction"), signature: "transfer(address to, uint256 value)"),
            params: [
                SigningRow(label: "to", value: Addr.alice, mono: true),
                SigningRow(label: "value",
                           value: t(loc, "techRawUnits", ["value": "1000000000", "n": "6"]),
                           mono: true),
            ],
            identities: [
                TechIdentity(role: t(loc, "techIdentityToken"), name: "USD Coin",
                             address: Addr.usdcFull, mark: Mark.usdc),
                TechIdentity(role: t(loc, "techIdentityRecipient"), name: "Alice Chen",
                             address: Addr.aliceFull, mark: Mark.contact),
            ],
            simResult: SigningRow(label: t(loc, "simResultLabel"),
                                  value: "−1,000 USDC · \(t(loc, "balanceMatchesHero"))"),
            raw: (label: "\(t(loc, "techRawData")) · \(t(loc, "byteSize", ["n": "68"]))",
                  hex: "0xa9059cbb000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1"
                      + "00000000000000000000000000000000000000000000000000000003b9aca00")
        )
    }

    private static func signer(_ loc: Loc) -> (label: String, name: String, seed: String) {
        (label: t(loc, "signingAccount"), name: WalletFixtures.identity.name,
         seed: WalletFixtures.identity.addressFull)
    }

    // MARK: - The catalogue

    // swiftlint:disable:next cyclomatic_complexity function_body_length
    static func build(_ state: SigningStateId, loc: Loc) -> SigningModel {
        let unknownDapp = (name: t(loc, "unverifiedTag"), host: "dapp.example.com", letter: "D",
                           tint: BrandPalette.unknown)

        switch state {
        // -- cs1–cs4: the transfer family --------------------------------
        case .cs1, .cs29:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSend"), tone: .neutral),
                    .amount(line: AmountLine(sign: "", value: "1,000", symbol: "USDC",
                                             token: Mark.usdc, fiat: "≈ $1,000.00")),
                    .sentence(text: t(loc, "summarySend",
                                      ["amount": "1,000 USDC", "to": "Alice Chen"]), tone: .accent),
                    .party(label: t(loc, "recipientLabel"), name: "Alice Chen",
                           address: Addr.alice,
                           badge: PartyBadge(text: t(loc, "contactTag"), tone: .neutral)),
                ],
                tech: transferTech(loc), techOpen: state == .cs29, fee: onchainFee(loc),
                signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSend"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs2:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSend"), tone: .neutral),
                    .amount(line: AmountLine(sign: "", value: "10", symbol: "ETH",
                                             token: Mark.eth, fiat: "≈ $25,604.00")),
                    .sentence(text: t(loc, "summarySend",
                                      ["amount": "10 ETH", "to": Addr.vitalik]), tone: .accent),
                    .party(label: t(loc, "recipientLabel"), name: "vitalik.eth",
                           address: Addr.vitalik,
                           badge: PartyBadge(text: t(loc, "firstTimeTag"), tone: .caution)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSend"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs3:
            return SigningModel(
                id: state, dapp: D.safe, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSend"), tone: .neutral),
                    .amount(line: AmountLine(sign: "", value: "0.5", symbol: "ETH",
                                             token: Mark.eth, fiat: "≈ $1,280.20")),
                    .positive(t(loc, "balanceSelfTransfer")),
                    .party(label: t(loc, "recipientLabel"),
                           name: t(loc, "selfName", ["name": WalletFixtures.identity.name]),
                           address: Addr.selfShort,
                           badge: PartyBadge(text: t(loc, "walletTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSend"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs4:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSend"), tone: .neutral),
                    .amount(line: AmountLine(sign: "", value: "100", symbol: "USDC",
                                             token: Mark.usdc, fiat: "≈ $100.00")),
                    .sentence(text: t(loc, "summarySendFrom",
                                      ["amount": "100 USDC", "to": Addr.vitalik]), tone: .accent),
                    .rows([SigningRow(label: t(loc, "labelFrom"), value: Addr.alice, mono: true)]),
                    .party(label: t(loc, "recipientLabel"), name: "vitalik.eth",
                           address: Addr.vitalik),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSend"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs5–cs8: approvals — unlimited kept as asked, and said ------
        case .cs5:
            return SigningModel(
                id: state, dapp: D.oneinch, network: network,
                blocks: [
                    .intent(text: t(loc, "intentApprove"), tone: .danger),
                    .allowance(
                        label: a(loc, "spendingCap"), value: a(loc, "unlimitedValue"),
                        valueTone: .danger,
                        // The site's own ask, preselected (2026-09-26): Permit2
                        // bundles revert when the wallet re-encodes the
                        // approve. A cap is one chip away.
                        chips: [chip("requested", a(loc, "requested"), .selected),
                                chip("balance", a(loc, "balanceCap"), .idle),
                                chip("custom", a(loc, "custom"), .idle),
                                chip("revoke", a(loc, "revoke"), .idle)]
                    ),
                    .party(label: t(loc, "spenderLabel"), name: "1inch Router",
                           address: Addr.oneinchRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                    .warning(tone: .danger, text: t(loc, "unlimitedWarning")),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                // Unlimited, seen and said — signable as asked.
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "intentApprove"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs6:
            return SigningModel(
                id: state, dapp: D.oneinch, network: network,
                blocks: [
                    .intent(text: t(loc, "intentApprove"), tone: .neutral),
                    .allowance(
                        label: a(loc, "spendingCap"), value: "1,240 USDC", valueTone: .neutral,
                        chips: [chip("requested", a(loc, "requested"), .idle),
                                chip("balance", a(loc, "balanceCap"), .selected),
                                chip("custom", a(loc, "custom"), .idle),
                                chip("revoke", a(loc, "revoke"), .idle)]
                    ),
                    .sentence(text: a(loc, "capSummary",
                                      ["spender": "1inch Router", "amount": "1,240 USDC"]),
                              tone: .neutral),
                    .party(label: t(loc, "spenderLabel"), name: "1inch Router",
                           address: Addr.oneinchRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "intentApprove"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs7:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentApprove"), tone: .neutral),
                    .allowance(
                        label: a(loc, "spendingCap"), value: "+100 USDC", valueTone: .neutral,
                        chips: [chip("requested", a(loc, "requested"), .selected),
                                chip("balance", a(loc, "balanceCap"), .idle),
                                chip("custom", a(loc, "custom"), .idle),
                                chip("revoke", a(loc, "revoke"), .idle)],
                        // increaseAllowance is an INCREMENT: the number that
                        // matters is the one it lands on, so the sheet adds up.
                        resultingTotal: SigningRow(label: a(loc, "resultingTotal"),
                                                   value: "350 USDC")
                    ),
                    .party(label: t(loc, "spenderLabel"), name: "Uniswap Router",
                           address: Addr.universalRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "intentApprove"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs8:
            return SigningModel(
                id: state, dapp: D.oneinch, network: network,
                blocks: [
                    .intent(text: t(loc, "intentRevoke"), tone: .neutral),
                    .allowance(
                        label: a(loc, "spendingCap"), value: a(loc, "revokeValue"),
                        valueTone: .neutral,
                        chips: [chip("requested", a(loc, "requested"), .idle),
                                chip("balance", a(loc, "balanceCap"), .idle),
                                chip("custom", a(loc, "custom"), .idle),
                                chip("revoke", a(loc, "revoke"), .selected)]
                    ),
                    .sentence(text: a(loc, "revokeSummary", ["spender": "1inch Router"]),
                              tone: .neutral),
                    .party(label: t(loc, "spenderLabel"), name: "1inch Router",
                           address: Addr.oneinchRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "intentRevoke"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs9–cs10: NFTs -----------------------------------------------
        case .cs9:
            return SigningModel(
                id: state, dapp: D.opensea, network: network,
                blocks: [
                    .intent(text: t(loc, "intentTransferNft"), tone: .neutral),
                    .nft(id: "#6529", collection: "Bored Ape Yacht Club"),
                    .sentence(text: t(loc, "summaryTransferNft",
                                      ["id": "#6529", "to": "Alice Chen"]), tone: .accent),
                    .party(label: t(loc, "recipientLabel"), name: "Alice Chen",
                           address: Addr.alice,
                           badge: PartyBadge(text: t(loc, "contactTag"), tone: .neutral)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs10:
            return SigningModel(
                id: state, dapp: D.opensea, network: network,
                blocks: [
                    .intent(text: a(loc, "verbApproveAll"), tone: .danger),
                    .allowance(
                        label: a(loc, "spendingCap"), value: a(loc, "allNfts"), valueTone: .danger,
                        // setApprovalForAll has no finite form to offer — two
                        // chips are the only honest choices.
                        chips: [chip("revoke", a(loc, "revokeAccess"), .idle),
                                chip("grant", a(loc, "grantAllAnyway"), .selected)]
                    ),
                    .sentence(text: t(loc, "summaryApproveNft",
                                      ["operator": "OpenSea Conduit"]), tone: .accent),
                    .party(label: a(loc, "collectionLabel"), name: "Bored Ape Yacht Club",
                           address: Addr.bayc,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                    .party(label: a(loc, "operatorLabel"), name: "OpenSea Conduit",
                           address: Addr.conduit),
                    .warning(tone: .caution, text: a(loc, "setApprovalAllWarn")),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: a(loc, "verbApproveAll"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs11–cs13: swaps ----------------------------------------------
        case .cs11, .cs33:
            let feeSelector: (title: String, options: [FeeTokenOption])? = state == .cs33
                ? (title: t(loc, "feeTokenTitle"), options: [
                    FeeTokenOption(id: "eth", mark: Mark.eth, name: "ETH",
                                   balance: "\(loc.t("componentsUi.gas.rowBalance")) 0.0689",
                                   fee: "~0.0021 ETH", selected: true),
                    FeeTokenOption(id: "usdc", mark: Mark.usdc, name: "USDC",
                                   balance: "\(loc.t("componentsUi.gas.rowBalance")) 1,240.00",
                                   fee: "~5.55 USDC", selected: false),
                ])
                : nil
            return SigningModel(
                id: state, dapp: D.oneinch, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSwap"), tone: .neutral),
                    .swap(
                        pay: AmountLine(sign: "−", value: "1,000", symbol: "USDC",
                                        token: Mark.usdc, fiat: "≈ $1,000.00",
                                        caption: t(loc, "labelPay")),
                        receive: AmountLine(sign: "+", value: "0.3042", symbol: "WETH",
                                            token: Mark.weth, fiat: "≈ $778.90",
                                            caption: t(loc, "labelMinReceived"), tone: .success)
                    ),
                    .sentence(text: t(loc, "summarySwap",
                                      ["pay": "1,000 USDC", "receive": "0.3042 WETH"]),
                              tone: .accent),
                    .party(label: t(loc, "interactingLabel"),
                           name: "1inch Aggregation Router · 1inch Network",
                           address: Addr.oneinchRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false,
                fee: .onchain(label: loc.t("componentsUi.gas.networkFee"), value: feeValue,
                              selector: feeSelector),
                signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSwap"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs12:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSwap"), tone: .neutral),
                    .swap(
                        pay: AmountLine(sign: "−", value: "0.5", symbol: "ETH", token: Mark.eth,
                                        fiat: "≈ $1,280.20", caption: t(loc, "labelPay")),
                        receive: AmountLine(sign: "+", value: "1,278.11", symbol: "USDC",
                                            token: Mark.usdc, fiat: "≈ $1,278.11",
                                            caption: t(loc, "labelMinReceived"), tone: .success)
                    ),
                    .sentence(text: t(loc, "summarySwap",
                                      ["pay": "0.5 ETH", "receive": "1,278.11 USDC"]),
                              tone: .accent),
                    .party(label: t(loc, "interactingLabel"), name: "Uniswap V3 Router",
                           address: Addr.uniswapV3,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSwap"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs13:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSwap"), tone: .neutral),
                    .swap(
                        pay: AmountLine(sign: "−", value: "1,000", symbol: "USDC",
                                        token: Mark.usdc, caption: t(loc, "labelPay")),
                        receive: AmountLine(sign: "+", value: "0.3042", symbol: "WETH",
                                            token: Mark.weth,
                                            caption: t(loc, "labelMinReceived"), tone: .success)
                    ),
                    .rows([SigningRow(label: t(loc, "labelDeadline"),
                                      value: t(loc, "expiredValue",
                                               ["time": "2026-08-14 18:00"]),
                                      valueTone: .caution)]),
                    .warning(tone: .caution, text: t(loc, "expiredWarning")),
                    .warning(tone: .danger, text: t(loc, "simWillFail")),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSwap"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs14–cs15: ERC-4626 vaults ------------------------------------
        case .cs14:
            return SigningModel(
                id: state, dapp: D.morpho, network: network,
                blocks: [
                    .intent(text: t(loc, "intentDeposit"), tone: .neutral),
                    .swap(
                        pay: AmountLine(sign: "−", value: "2", symbol: "WETH", token: Mark.weth,
                                        fiat: "≈ $5,120.80", caption: t(loc, "depositAsset")),
                        receive: AmountLine(sign: "+", value: "1.9631", symbol: "spWETH",
                                            token: Mark.spweth,
                                            caption: t(loc, "sharesReceived"), tone: .success)
                    ),
                    .warning(tone: .caution, text: t(loc, "unverifiedWarning")),
                    .party(label: t(loc, "interactingLabel"), name: "Morpho Vault · Morpho Labs",
                           address: Addr.morphoVault,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmDeposit"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs15:
            return SigningModel(
                id: state, dapp: D.morpho, network: network,
                blocks: [
                    .intent(text: t(loc, "intentWithdraw"), tone: .neutral),
                    .amount(line: AmountLine(sign: "+", value: "2", symbol: "WETH",
                                             token: Mark.weth, fiat: "≈ $5,120.80",
                                             tone: .success)),
                    .sentence(text: t(loc, "summaryReceive", ["amount": "2 WETH"]), tone: .accent),
                    .party(label: t(loc, "interactingLabel"), name: "Morpho Vault · Morpho Labs",
                           address: Addr.morphoVault,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmWithdraw"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs16–cs17: off-chain permits ----------------------------------
        case .cs16:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "permitIntent"), tone: .danger),
                    .sentence(text: t(loc, "summaryPermitUnlimited",
                                      ["spender": "Universal Router", "token": "USDC"]),
                              tone: .danger),
                    .party(label: t(loc, "spenderLabel"), name: "Universal Router",
                           address: Addr.universalRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                    .rows([
                        SigningRow(label: a(loc, "spendingCap"),
                                   value: "\(a(loc, "unlimitedValue")) USDC", valueTone: .danger),
                        SigningRow(label: a(loc, "expiresLabel"), value: "2026-09-14 19:30"),
                    ]),
                    // The whole reason this is danger and not caution: there is
                    // no editor to offer, because a signature cannot be capped.
                    .warning(tone: .danger, text: a(loc, "permitCantCap")),
                ],
                tech: tech(loc), techOpen: false, fee: .offchain(note: t(loc, "noNetworkFee")),
                signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs17:
            return SigningModel(
                id: state, dapp: D.uniswap, network: network,
                blocks: [
                    .intent(text: t(loc, "permitIntent"), tone: .neutral),
                    .sentence(text: t(loc, "summaryPermit",
                                      ["spender": "Universal Router", "amount": "1,000 USDC"]),
                              tone: .accent),
                    .party(label: t(loc, "spenderLabel"), name: "Universal Router",
                           address: Addr.universalRouter,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                    .rows([
                        SigningRow(label: a(loc, "spendingCap"), value: "1,000 USDC"),
                        SigningRow(label: t(loc, "labelDeadline"), value: "2030-03-14 08:26"),
                    ]),
                ],
                tech: tech(loc), techOpen: false, fee: .offchain(note: t(loc, "noNetworkFee")),
                signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs18–cs22: messages, readable down to a raw hash --------------
        case .cs18:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "typedDataIntent"), tone: .neutral),
                    .warning(tone: .caution, text: t(loc, "blindTypedWarning")),
                    .rows([
                        SigningRow(label: t(loc, "typedDomain"), value: "CoolProtocol · v2"),
                        SigningRow(label: t(loc, "typeLabel"), value: "Order"),
                        SigningRow(label: t(loc, "signingFor"), value: "dapp.example.com",
                                   valueTone: .accent),
                    ]),
                    .code(lines: ["{ \"maker\": \"0x14fB1f…D1eA5c\",",
                                  "  \"taker\": \"0x0000…0000\",",
                                  "  \"makerAmount\": \"1000000000\", … }"]),
                ],
                tech: tech(loc, summary: t(loc, "byteSize", ["n": "412"])), techOpen: false,
                fee: .offchain(note: t(loc, "noNetworkFee")), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs19:
            return SigningModel(
                id: state, dapp: D.ens, network: network,
                blocks: [
                    .intent(text: t(loc, "signInIntent"), tone: .neutral),
                    .rows([
                        SigningRow(label: t(loc, "siweDomain"), value: "app.ens.domains"),
                        SigningRow(label: t(loc, "siweStatement"), value: "登录以管理你的 ENS 名称"),
                    ]),
                    .code(lines: ["app.ens.domains wants you to sign in",
                                  "with your Ethereum account:",
                                  Addr.selfShort]),
                    .positive(t(loc, "siweOk", ["domain": "app.ens.domains"])),
                ],
                tech: tech(loc), techOpen: false, fee: .offchain(note: t(loc, "noNetworkFee")),
                signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs20:
            return SigningModel(
                id: state, dapp: D.phish, network: network,
                blocks: [
                    .intent(text: t(loc, "signInIntent"), tone: .danger),
                    // The mismatch goes ABOVE the facts: by the time somebody
                    // has read a login screen they have already decided.
                    .warning(tone: .danger,
                             text: t(loc, "siweMismatch",
                                     ["domain": "opensea.io", "origin": "opensae-mint.xyz"])),
                    .rows([
                        SigningRow(label: t(loc, "siweDomain"), value: "opensea.io",
                                   valueTone: .danger),
                        SigningRow(label: t(loc, "siweOrigin"), value: "opensae-mint.xyz",
                                   mono: true),
                        SigningRow(label: t(loc, "siweStatement"), value: "登录以查看你的 NFT"),
                    ]),
                    .code(lines: ["opensea.io wants you to sign in",
                                  "with your Ethereum account:",
                                  Addr.selfShort]),
                ],
                tech: tech(loc), techOpen: false, fee: .hidden, signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs21:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "messageIntent"), tone: .neutral),
                    .warning(tone: .caution, text: t(loc, "hexMessageWarning")),
                    .code(lines: ["0xdeadbeefcafebabe0102030405",
                                  "060708091011121314151617181920",
                                  "2122232425262728293031…"],
                          note: "(\(t(loc, "byteSize", ["n": "80"])))"),
                    .rows([SigningRow(label: t(loc, "signingFor"), value: "dapp.example.com")]),
                ],
                tech: tech(loc), techOpen: false, fee: .hidden, signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "signLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs22:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "ethSignIntent"), tone: .danger),
                    .sentence(text: t(loc, "ethSignBody"), tone: .danger),
                    .code(lines: ["0x9c22ff5f21f0b81b113e63f7db6da9",
                                  "4fedef11b2119b4088b89664fb9a3c", "b658"]),
                    .warning(tone: .danger, text: t(loc, "ethSignWarning")),
                ],
                tech: tech(loc), techOpen: false, fee: .hidden, signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs23–cs24: simulation as the protagonist ----------------------
        case .cs23:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentContractCall"), tone: .neutral),
                    .warning(tone: .caution, text: t(loc, "blindDecodeWarning",
                                                     ["bytes": "196"])),
                    .rows([SigningRow(label: t(loc, "labelAmount"),
                                      value: "0.1 ETH ≈ $256.04")]),
                    .party(label: t(loc, "interactingLabel"), name: t(loc, "unverifiedTag"),
                           address: Addr.unknown,
                           badge: PartyBadge(text: t(loc, "unverifiedTag"), tone: .caution)),
                    .balances(title: t(loc, "balanceChangesTitle"),
                              rows: [BalanceDeltaRow(symbol: "ETH", delta: "−0.1", tone: .neutral)],
                              note: t(loc, "blindButSimulated"), noteTone: .neutral),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs24:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentContractCall"), tone: .danger),
                    .sentence(text: t(loc, "drainSummary"), tone: .danger),
                    .balances(title: t(loc, "balanceChangesTitle"),
                              rows: [BalanceDeltaRow(symbol: "USDC", delta: "−8,450",
                                                     tone: .danger),
                                     BalanceDeltaRow(symbol: "ETH", delta: "−0.8", tone: .danger)],
                              note: t(loc, "drainWarning"), noteTone: .danger),
                    .party(label: t(loc, "interactingLabel"), name: t(loc, "unverifiedTag"),
                           address: Addr.unknown,
                           badge: PartyBadge(text: t(loc, "unverifiedTag"), tone: .caution)),
                    .warning(tone: .danger, text: t(loc, "blindDecodeWarning", ["bytes": "4"])),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs25–cs28: deploy, batch, Safe, burn --------------------------
        case .cs25:
            return SigningModel(
                id: state, dapp: D.safe, network: network,
                blocks: [
                    .intent(text: t(loc, "deployIntent"), tone: .neutral),
                    .sentence(text: t(loc, "summaryDeploy"), tone: .accent),
                    .rows([
                        SigningRow(label: t(loc, "deployBytecode"),
                                   value: t(loc, "byteSize", ["n": "246"])),
                        SigningRow(label: t(loc, "deployPredictedAddress"), value: Addr.deployed,
                                   mono: true),
                    ]),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs26:
            return SigningModel(
                id: state, dapp: D.oneinch, network: network,
                blocks: [
                    .intent(text: t(loc, "batchIntent"), tone: .neutral),
                    .sentence(text: t(loc, "batchSubtitle", ["count": "2"]), tone: .accent),
                    .card(title: t(loc, "batchStep",
                                   ["index": "1", "action": t(loc, "intentApprove")]),
                          rows: [SigningRow(label: a(loc, "spendingCap"), value: "100 USDC"),
                                 SigningRow(label: t(loc, "spenderLabel"), value: "1inch Router")],
                          tone: .neutral),
                    .card(title: t(loc, "batchStep",
                                   ["index": "2", "action": t(loc, "intentSwap")]),
                          rows: [SigningRow(label: t(loc, "labelPay"), value: "−100 USDC"),
                                 SigningRow(label: t(loc, "labelMinReceived"),
                                            value: "+0.0304 WETH")],
                          tone: .neutral),
                    .balances(title: t(loc, "balanceChangesTitle"),
                              rows: [BalanceDeltaRow(symbol: "USDC", delta: "−100",
                                                     tone: .neutral),
                                     BalanceDeltaRow(symbol: "WETH", delta: "+0.0304",
                                                     tone: .success)],
                              note: t(loc, "balanceMatchesHero"), noteTone: .neutral),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs27:
            return SigningModel(
                id: state, dapp: D.safe, network: network,
                blocks: [
                    .intent(text: t(loc, "safeIntent"), tone: .neutral),
                    .sentence(text: t(loc, "safeSummary"), tone: .accent),
                    // Safe's calldata nests, so the sheet decodes the inner
                    // call too: a wrapper that showed only the outer call would
                    // show nothing at all.
                    .card(title: t(loc, "safeInnerCall", ["action": t(loc, "intentSend")]),
                          rows: [SigningRow(label: t(loc, "labelAmount"), value: "250 USDC"),
                                 SigningRow(label: t(loc, "recipientLabel"), value: "Alice Chen")],
                          tone: .neutral),
                    .party(label: t(loc, "interactingLabel"), name: "Safe 1.4.1 · Safe Ecosystem",
                           address: Addr.safe,
                           badge: PartyBadge(text: t(loc, "verifiedTag"), tone: .success)),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs28:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentSend"), tone: .danger),
                    .amount(line: AmountLine(sign: "", value: "500", symbol: "USDT",
                                             token: Mark.usdt, tone: .danger),
                            card: true, note: t(loc, "sendingToTokenContract")),
                    .party(label: t(loc, "recipientLabel"), name: "Tether USD",
                           address: Addr.usdt,
                           badge: PartyBadge(text: t(loc, "contractTag"), tone: .danger)),
                    .warning(tone: .danger, text: t(loc, "tokenToContractWarning")),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmSend"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        // -- cs30–cs32: the ladder's lower rungs ---------------------------
        case .cs30:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentContractCall"), tone: .neutral),
                    .sentence(text: t(loc, "bestEffortSummary", ["fn": "execute(…)"]),
                              tone: .accent),
                    .warning(tone: .caution, text: t(loc, "bestEffortWarning")),
                    .rows([
                        SigningRow(label: t(loc, "techFunction"),
                                   value: "execute(bytes,bytes[],uint256)", mono: true),
                        SigningRow(label: t(loc, "techParam",
                                            ["index": "1", "name": "bytes"]),
                                   value: "0x0b00… (2)"),
                        SigningRow(label: t(loc, "techParam",
                                            ["index": "2", "name": "bytes[]"]), value: "2"),
                        SigningRow(label: t(loc, "techParam",
                                            ["index": "3", "name": "deadline"]),
                                   value: "2026-08-15 20:00"),
                    ]),
                    .party(label: t(loc, "interactingLabel"), name: t(loc, "unverifiedTag"),
                           address: Addr.unknown,
                           badge: PartyBadge(text: t(loc, "unverifiedTag"), tone: .caution)),
                    .balances(title: t(loc, "balanceChangesTitle"),
                              rows: [BalanceDeltaRow(symbol: "ETH", delta: "−0.1", tone: .neutral),
                                     BalanceDeltaRow(symbol: "USDC", delta: "+255.8",
                                                     tone: .success)],
                              note: t(loc, "bestEffortSimulated"), noteTone: .neutral),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs31:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentContractCall"), tone: .neutral),
                    .sentence(text: t(loc, "verifiedAbiSummary"), tone: .neutral),
                    .rows([
                        SigningRow(label: "claimRewards · ids", value: "[128, 129, 130]"),
                        SigningRow(label: "beneficiary",
                                   value: t(loc, "selfName", ["name": Addr.selfShort]),
                                   mono: true),
                        SigningRow(label: "restake", value: "true"),
                    ]),
                    .party(label: t(loc, "interactingLabel"), name: "RewardsVault",
                           address: Addr.rewards,
                           badge: PartyBadge(text: t(loc, "contractTag"), tone: .neutral)),
                    .warning(tone: .caution, text: t(loc, "verifiedAbiWarning")),
                    .balances(title: t(loc, "balanceChangesTitle"),
                              rows: [BalanceDeltaRow(symbol: "stETH", delta: "+4.21",
                                                     tone: .success)],
                              note: t(loc, "balanceMatchesHero"), noteTone: .neutral),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )

        case .cs32:
            return SigningModel(
                id: state, dapp: unknownDapp, network: network,
                blocks: [
                    .intent(text: t(loc, "intentContractCall"), tone: .neutral),
                    // The deepest rung: neither decode nor simulation. Both
                    // failures are stated plainly, and the amount is still
                    // shown — facts that ARE knowable are never withheld
                    // because the rest is not.
                    .warning(tone: .caution,
                             text: t(loc, "selectorNotListed", ["bytes": "4"])),
                    .warning(tone: .danger, text: t(loc, "simUnavailableWarning")),
                    .rows([SigningRow(label: t(loc, "labelAmount"),
                                      value: "0.25 ETH ≈ $640.10")]),
                    .party(label: t(loc, "interactingLabel"), name: t(loc, "unverifiedTag"),
                           address: Addr.deepest,
                           badge: PartyBadge(text: t(loc, "unverifiedTag"), tone: .caution)),
                    .code(lines: ["0x8fabe4c2000000000000000000000000",
                                  "d400866e00b055b20752a826cd5c89b8", "11de130b…"],
                          note: "(\(t(loc, "byteSize", ["n": "132"])))"),
                ],
                tech: tech(loc), techOpen: false, fee: onchainFee(loc), signer: signer(loc),
                confirm: (hint: t(loc, "slideToConfirm"), action: t(loc, "confirmLabel"),
                          enabled: true),
                panelTitle: t(loc, "signatureRequest")
            )
        }
    }
}

extension SigningModel {
    /// The signed-in wallet's identity over the fixture's signer row.
    func withIdentity(name: String, address: String) -> SigningModel {
        var copy = self
        copy.signer = (label: signer.label, name: name, seed: address)
        return copy
    }
}
