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
        if let first = view.networks.first {
            copy.networkDetail = detail(first, loc: loc, fallback: model.networkDetail)
        }
        return copy
    }

    /// One row of 设置 → 网络.
    static func row(_ network: NetNetworkRowWire, loc: Loc) -> SettingsNetworkRowModel {
        SettingsNetworkRowModel(
            id: network.id,
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
            badge: badge(network.rpcHealth) ?? fallback.badge,
            rpc: field(fallback.rpc, value: network.rpcUrl, health: network.rpcHealth),
            explorer: field(fallback.explorer, value: network.explorerUrl,
                            health: network.explorerHealth),
            callout: mismatchCallout(network, loc: loc) ?? fallback.callout
        )
    }

    // MARK: - Pieces

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
            badge: badge(health) ?? fallback.badge,
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
