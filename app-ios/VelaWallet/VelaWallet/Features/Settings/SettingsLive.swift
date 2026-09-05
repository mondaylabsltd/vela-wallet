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

enum SettingsLive {

    /// Swap in the networks the core actually knows about.
    ///
    /// Everything else on the model — the sections, the theme and avatar
    /// pickers, storage, about, every sheet — is untouched fixture, and stays
    /// that way until its own machine is wired.
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
        return copy
    }

    // MARK: - display_currency

    /// Swap in the currency the person actually chose.
    ///
    /// Two surfaces: the 货币 row's value on the home page, and which row of the
    /// picker reads as selected.
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
    static func currencyRowValue(_ view: CurrencyViewWire) -> String {
        let sample = 1_234.56
        guard let rate = view.rate, rate > 0, view.committed,
              let entry = CurrencyCatalog.entry(view.code)
        else {
            // Degraded: say USD, show USD.
            let usd = CurrencyCatalog.entry("USD")
            return "USD · \(usd?.glyph ?? "$")\(format(sample))"
        }
        return "\(view.code) · \(entry.glyph)\(format(sample * rate))"
    }

    private static func format(_ amount: Double) -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 2
        formatter.maximumFractionDigits = 2
        return formatter.string(from: NSNumber(value: amount)) ?? String(format: "%.2f", amount)
    }

    // MARK: - The add-network wizard

    /// The wizard, driven by the core's phase.
    ///
    /// `fallback` supplies the labels and placeholders the drawing owns. Every
    /// verdict — the badge, the checks, whether 添加 is offered at all — is the
    /// core's; **`can_add` is never re-derived here**, because a second opinion
    /// is how a screen offers to add a chain the core will refuse.
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
        model.candidate = SettingsNetworkRowModel(
            id: String(info.chainId),
            chainId: info.chainId,
            mark: mark(chainId: info.chainId, name: info.name),
            name: info.name,
            meta: candidateMeta(wizard, loc: loc),
            badge: verdictBadge(wizard, loc: loc)
        )

        if let compat = wizard.compat {
            model.checksTitle = loc.t(k.addCompatibilityCheck)
            model.checks = checks(compat, loc: loc)
        }

        model.customRpc = UrlFieldModel(
            id: "custom-rpc",
            label: loc.t(k.addCustomRpcTitle),
            value: wizard.customRpc,
            placeholder: loc.t(k.addCustomRpcPlaceholder)
        )
        model.callout = errorCallout(wizard.error, loc: loc)
            ?? (wizard.compat?.compatible == false
                ? CalloutModel(tone: .warning, text: loc.t(k.addIncompatibleHint))
                : nil)

        // The gate. An accent CTA appears only when the CORE says the chain can
        // be added; otherwise the drawing's outline-plus-recheck pair, because
        // an action you cannot take should not be dressed as the action you
        // came for.
        if wizard.canAdd {
            model.primary = loc.t(k.addButton)
        } else if wizard.compat?.compatible == false {
            model.secondary = loc.t(k.addChainTool)
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
        let rest = compat.contracts.filter { !named.contains($0.name) }
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

    private static func verdictBadge(_ wizard: NetWizardViewWire, loc: Loc) -> StatusPillModel? {
        let k = I18nKeys.SettingsUi.self
        guard let compat = wizard.compat else { return nil }
        return compat.compatible
            ? StatusPillModel(tone: .ok, label: loc.t(k.addCompatible))
            : StatusPillModel(tone: .error, label: loc.t(k.addIncompatible))
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
            note: fallback.note,
            // **Never the fixture's pill.** `?? fallback.badge` used to be here,
            // and on a real phone it painted 在线 · 45ms over an endpoint
            // nothing had probed — the fixture's own constant, presented as a
            // measurement. Unmeasured is neutral and says nothing.
            badge: badge(network.rpcHealth) ?? Self.unmeasured,
            rpc: field(fallback.rpc, value: network.rpcUrl, health: network.rpcHealth),
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

    private static func field(
        _ fallback: UrlFieldModel,
        value: String,
        health: NetProbeHealthWire?
    ) -> UrlFieldModel {
        UrlFieldModel(
            id: fallback.id,
            label: fallback.label,
            value: value,
            placeholder: fallback.placeholder,
            hint: fallback.hint,
            badge: badge(health),
            tone: fallback.tone,
            action: fallback.action
        )
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
        return ChainMarkModel(letter: letter, color: chainColor(chainId))
    }

    private static func chainColor(_ chainId: Int) -> Color {
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
