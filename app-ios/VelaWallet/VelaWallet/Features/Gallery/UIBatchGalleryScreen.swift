//
//  UIBatchGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for PR 3 (the UI batch), reached with `VELA_PAGE=pr3` and
//  `VELA_STATE=<board>` — the surfaces no other gallery can put on screen
//  without a ceremony, a second device or a chain that refuses the wallet.
//  Every board is the PRODUCTION view over fixture data:
//
//  - `cable-create` / `cable-signin` (issue #480) — the "Phone or tablet ·
//    scan a code" sheet over a page, as tall as its content, Cancel at the
//    bottom. The same one sheet create, add-key, sign-in and the account
//    switcher's sign-in raise.
//  - `insert-key` — "Insert your security key" with its way out for a key
//    the app's own USB route cannot reach: "Use Apple's security-key sheet"
//    and which keys that is for.
//  - `backup-checking` / `backup-not-copied` / `backup-copied` /
//    `backup-could-not-check` / `backup-cannot-copy` — Settings' Keys block
//    with the "Copy this wallet's record to Ethereum" row in each state the
//    core words (`BackupState::row`): its line, its tone and what a tap does.
//  - `p256-settings-none` / `p256-settings-missing` — Settings → Add network
//    refusing a chain: no P-256 verifier (said plainly, no button) vs missing
//    contracts (Chain Setup, for that chain). `p256-dapp-none` /
//    `p256-dapp-missing` — the same two on a dApp's add-network sheet.
//
//  Fixture data only: nothing here is scanned, signed or sent.
//

#if DEBUG

import SwiftUI
import VelaCore

struct UIBatchGalleryScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var scheme
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "p256-settings-none", "p256-settings-missing":
            SettingsScreen(model: refusedSettings(noP256: state.hasSuffix("none")), loc: loc)
        case "p256-dapp-none", "p256-dapp-missing":
            ScrollView {
                AddNetworkPanelView(model: ExploreLive.addNetwork(
                    refusedDappAdd(noP256: state.hasSuffix("none")), loc: loc
                ))
            }
            .background(theme.bgRaised.ignoresSafeArea())
        case let board where board.hasPrefix("backup-"):
            backupBoard(String(board.dropFirst("backup-".count)))
        case "insert-key":
            sheetBoard(title: loc.t(I18nKeys.Create.keysTitle)) {
                UsbInsertKeySheet(loc: loc, onUseSystemSheet: {}, onCancel: {})
            }
        case "cable-signin":
            cableBoard(chooser: .signIn)
        default:
            cableBoard(chooser: .create)
        }
    }

    // MARK: - A refused network: no P-256 verifier, or missing contracts

    private static let refusedChain = 48_900

    /// A finished check the core refused, with its ruling written out
    /// (`net_blocker`): no P-256 wins and carries no setup address; missing
    /// contracts carries Chain Setup's, for this chain.
    private static func refusedCompat(noP256: Bool) -> NetCompatibilityWire {
        let names = [
            "Deterministic Deployment Proxy", "Safe Singleton Factory", "Multicall3",
            "EntryPoint v0.7", "Safe L2", "Safe Proxy Factory", "Safe 4337 Module",
            "Safe Module Setup", "WebAuthn Signer", "MultiSend",
        ]
        // Without the precompile every contract may well be there; with it,
        // these three are what this chain lacks.
        let missing: Set<String> = noP256 ? [] : ["Safe L2", "Safe 4337 Module", "Safe Module Setup"]
        return NetCompatibilityWire(
            chainId: refusedChain, compatible: false, multiKeyReady: false,
            contracts: names.map {
                NetContractStatusWire(name: $0, address: "0x", deployed: !missing.contains($0), multiKeyOnly: false)
            },
            p256Available: !noP256, bestRpcUrl: "https://rpc.example", bestRpcLatencyMs: 182, rpcFailure: nil,
            blocker: noP256 ? "no_p256" : "missing_contracts",
            hintKey: "settingsModals.addNetwork." + (noP256 ? "noP256Hint" : "incompatibleHint"),
            setupUrl: noP256 ? nil : "https://getvela.app/chain-setup?chain=\(refusedChain)"
        )
    }

    private func refusedSettings(noP256: Bool) -> SettingsScreenModel {
        let base = SettingsFixtures.build(.st10c, loc: loc)
        let wizard = NetWizardViewWire(
            phase: .checked, query: "", customRpc: "", suggestions: [],
            chainInfo: NetChainInfoWire(
                chainId: Self.refusedChain, name: "Zircuit", shortName: "zircuit", nativeName: "Ether",
                nativeSymbol: "ETH", nativeDecimals: 18, rpcUrl: "https://rpc.example", rpcUrls: [],
                explorerUrl: "https://explorer.example", logoUrl: "", isTestnet: false
            ),
            compat: Self.refusedCompat(noP256: noP256), error: nil, canAdd: false
        )
        var model = base
        model.addNetwork = SettingsLive.wizard(wizard, loc: loc, fallback: base.addNetwork)
        return model
    }

    private func refusedDappAdd(noP256: Bool) -> NetDappAddViewWire {
        NetDappAddViewWire(
            tab: "t1", id: "a1", origin: "https://app.example", host: "app.example",
            chainId: Self.refusedChain, name: "Zircuit", nativeSymbol: "ETH",
            rpcHost: "rpc.example", explorerHost: "explorer.example", fromSite: false,
            phase: .notCompatible, reportedChainId: nil,
            compat: Self.refusedCompat(noP256: noP256), canAdd: false
        )
    }

    // MARK: - "Copy this wallet's record to Ethereum"

    /// The row the core attaches to each finished check (`BackupState::row`),
    /// written out: a drawing has no chain behind it.
    private static func backupCheck(_ state: String) -> RegistryBackup.Check? {
        func row(_ subtitle: String, _ tone: RegistryBackup.Row.Tone, _ action: RegistryBackup.Row.Action) -> RegistryBackup.Row {
            // The core gives every state its paragraph but the one that can
            // never be copied (`BackupRow.explain_key`, PR 3 note 6).
            RegistryBackup.Row(
                titleKey: "settingsModals.backup.title",
                subtitleKey: "settingsModals.backup.\(subtitle)", tone: tone, action: action,
                explainKey: subtitle == "cannotCopy" ? nil : RegistryBackup.Row.explainKey
            )
        }
        switch state {
        case "not-copied":
            return RegistryBackup.Check(
                state: .notBackedUp,
                call: RegistryBackup.Call(chainId: 1, to: "0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9", data: "0xcd438f9b"),
                row: row("notBackedUp", .neutral, .copy)
            )
        case "copied":
            return RegistryBackup.Check(state: .backedUp, call: nil, row: row("backedUp", .positive, .none))
        case "could-not-check":
            return RegistryBackup.Check(state: .couldNotCheck, call: nil, row: .couldNotCheck)
        case "cannot-copy":
            return RegistryBackup.Check(state: .notCopyable, call: nil, row: row("cannotCopy", .neutral, .none))
        default:
            // Still asking.
            return nil
        }
    }

    private func backupBoard(_ state: String) -> some View {
        func key(_ name: String, provider: String, method: KeyMethod, synced: Bool) -> WalletKeys.Row {
            WalletKeys.Row(
                key: CreateKeyRow(
                    name: name, authenticatorAttachment: method == .securityKey ? "cross-platform" : "platform",
                    transports: method == .securityKey ? "usb" : "internal", confirmed: true,
                    synced: synced, syncedKnown: true, aaguid: "", providerName: provider,
                    method: method, kind: method
                ),
                synced: synced,
                publicKeyHex: "04" + String(repeating: "ab", count: 64)
            )
        }
        let keys = WalletKeys.Result(source: .registry, rows: [
            key("Everyday wallet", provider: "Apple Passwords", method: .platform, synced: true),
            key("", provider: "", method: .securityKey, synced: false),
        ])
        let model = SettingsLive.withWalletKeys(
            keys, backup: Self.backupCheck(state), on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        return SettingsScreen(model: model, loc: loc)
    }

    // MARK: - Issue #480: the scan-a-code sheet

    /// A caBLE payload's shape: `FIDO:/` and the digits the phone scans. Not
    /// a live one — no device answers it.
    private static let cablePayload = "FIDO:/" + String(repeating: "0914372806155923481170265394", count: 6)

    private func cableBoard(chooser: KeyChooser) -> some View {
        sheetBoard(title: loc.t(chooser == .create ? I18nKeys.Create.keysTitle : I18nKeys.Login.header)) {
            CableQrSheet(loc: loc, payload: Self.cablePayload, chooser: chooser)
        }
    }

    /// A ceremony sheet over the page it rises from: what shows above a
    /// content-sized sheet is the screen the person came from.
    private func sheetBoard(title: String, @ViewBuilder sheet: @escaping () -> some View) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(title)
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgBase)
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Layout.screenPaddingX)
        .background(theme.bgBase.ignoresSafeArea())
        .sheet(isPresented: .constant(true)) {
            sheet().themed(scheme)
        }
    }
}

#endif
