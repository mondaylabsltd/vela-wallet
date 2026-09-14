//
//  SweepPick.swift
//  VelaWallet
//
//  What one tap on the sweep picker means.
//
//  Ported from `app-android/.../feature/send/core/SweepPick.kt` (spec 045
//  research D2). Every judgement is the core's — which tokens are worth
//  sweeping, how much of each actually moves — and what is here is only the
//  order of the events one tap becomes.
//
//  ## The picking flag is the shell's, and only that flag
//
//  The core's `multiSelectMode` flips at **confirm**, not when the tick boxes
//  appear, so "are we picking" cannot be read from it. That one flag lives in
//  the screen; every other question goes to the core.
//
//  ## Unticking the last token releases the chain
//
//  Without it, somebody who ticks the wrong token first is pinned to that chain
//  until they leave the screen entirely — the wallet having quietly decided
//  something they did not mean to decide.
//

import Foundation

enum SweepPick {

    /// The events one tap becomes, **in order**. Empty means the row is dimmed
    /// and the tap is not an answer to anything.
    static func tap(view: SendViewWire, tokenId: String, chainId: Int) -> [[String: Any]] {
        let selected = view.multiSelectedIds.contains(tokenId)
        let pinned = view.multiChainId

        if selected, view.multiSelectedIds.count == 1 {
            return [
                ["type": "toggle_multi_token", "token_id": tokenId],
                ["type": "set_multi_network", "chain_id": NSNull()],
            ]
        }
        if selected {
            return [["type": "toggle_multi_token", "token_id": tokenId]]
        }
        if pinned == nil {
            return [
                ["type": "set_multi_network", "chain_id": chainId],
                ["type": "toggle_multi_token", "token_id": tokenId],
            ]
        }
        // A row on another chain is drawn dimmed and is not tappable; this is
        // the second lock on the same door.
        if pinned != chainId { return [] }
        return [["type": "toggle_multi_token", "token_id": tokenId]]
    }

    /// "Select all valuable", scoped to what is **on screen**.
    ///
    /// The shell supplies the visible ids and nothing else: which of them are
    /// valuable is the core's, and a filtered list must not sweep rows a person
    /// cannot see.
    static func selectAll(
        view: SendViewWire, visibleIds: [String], chainOf: (String) -> Int?
    ) -> [[String: Any]] {
        if view.multiChainId != nil {
            return [["type": "toggle_all_multi_tokens", "visible_ids": visibleIds]]
        }
        // Nothing pinned yet: the first VALUABLE visible row decides the chain.
        guard let first = visibleIds.first(where: { view.multiValuableIds.contains($0) }),
              let chainId = chainOf(first)
        else { return [] }
        return [
            ["type": "set_multi_network", "chain_id": chainId],
            ["type": "toggle_all_multi_tokens", "visible_ids": visibleIds],
        ]
    }

    /// Whether a row is on a chain the pick has left behind.
    static func dimmed(view: SendViewWire, chainId: Int) -> Bool {
        guard let pinned = view.multiChainId else { return false }
        return pinned != chainId
    }
}
