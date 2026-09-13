package app.getvela.wallet.feature.send.core

import app.getvela.wallet.feature.send.SendLive

/**
 * The sweep pick's shell-side rule (spec 045 D2, desktop 033): the FIRST tick
 * pins the chain (`SetMultiNetwork`) before the toggle; a row on another chain
 * is drawn dimmed and a tap on it is not an event (the core would refuse it —
 * a tappable row is an invitation it will not honour); clearing the last tick
 * unpins, so the next tick pins again. Everything else — which rows are
 * valuable, how much moves — is the core's.
 */
object SweepPick {
    /** The events one tap on `tokenId` becomes, in order; empty when the row is dimmed. */
    fun tap(view: SendView, tokenId: String): List<SendEvent> {
        val token = view.tokens.firstOrNull { SendLive.tokenId(it) == tokenId } ?: return emptyList()
        val pinned = view.multi_chain_id
        val selected = tokenId in view.multi_selected_ids
        return when {
            selected && view.multi_selected_ids.size == 1 ->
                listOf(SendEvent.ToggleMultiToken(tokenId), SendEvent.SetMultiNetwork(null))
            selected -> listOf(SendEvent.ToggleMultiToken(tokenId))
            pinned == null -> listOf(SendEvent.SetMultiNetwork(token.chain_id), SendEvent.ToggleMultiToken(tokenId))
            pinned != token.chain_id -> emptyList()
            else -> listOf(SendEvent.ToggleMultiToken(tokenId))
        }
    }

    /**
     * "Select all valuable" over the rows on screen: with no chain pinned yet,
     * the first visible valuable row's chain is pinned first, so the core's
     * scoped toggle has one chain to work in.
     */
    fun selectAll(view: SendView, visibleIds: List<String>): List<SendEvent> {
        if (view.multi_chain_id != null) return listOf(SendEvent.ToggleAllMultiTokens(visibleIds))
        val first = view.tokens.firstOrNull { SendLive.tokenId(it) in visibleIds && SendLive.tokenId(it) in view.multi_valuable_ids }
            ?: return emptyList()
        return listOf(SendEvent.SetMultiNetwork(first.chain_id), SendEvent.ToggleAllMultiTokens(visibleIds))
    }

    /** Which rows are drawn dimmed: another chain's, once one is pinned. */
    fun dimmed(view: SendView): List<Boolean> =
        view.tokens.map { token -> view.multi_chain_id != null && token.chain_id != view.multi_chain_id }
}
