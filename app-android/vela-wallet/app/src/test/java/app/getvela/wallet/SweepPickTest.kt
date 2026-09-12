package app.getvela.wallet

import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.SweepPick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SweepPickTest {
    private val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "0.5", decimals = 18, token_address = null, price_usd = 1.0)
    private val usdc = SendToken(network = "chain-100", chain_id = 100, symbol = "USDC", balance = "3", decimals = 6, token_address = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83", price_usd = 1.0)
    private val eth = SendToken(network = "chain-1", chain_id = 1, symbol = "ETH", balance = "0.01", decimals = 18, token_address = null, price_usd = 3000.0)
    private val ids = listOf(xdai, usdc, eth).map { SendLive.tokenId(it) }
    private val view = SendView(tokens = listOf(xdai, usdc, eth), multi_valuable_ids = ids)

    @Test
    fun `the first tick pins the chain before the toggle`() {
        assertEquals(listOf(SendEvent.SetMultiNetwork(100), SendEvent.ToggleMultiToken(ids[0])), SweepPick.tap(view, ids[0]))
    }

    @Test
    fun `a row on another chain is dimmed and a tap on it is nothing`() {
        val pinned = view.copy(multi_chain_id = 100, multi_selected_ids = listOf(ids[0]))
        assertEquals(listOf(false, false, true), SweepPick.dimmed(pinned))
        assertTrue(SweepPick.tap(pinned, ids[2]).isEmpty())
        assertEquals(listOf(SendEvent.ToggleMultiToken(ids[1])), SweepPick.tap(pinned, ids[1]))
    }

    @Test
    fun `clearing the last tick unpins so the next tick pins again`() {
        val pinned = view.copy(multi_chain_id = 100, multi_selected_ids = listOf(ids[0]))
        assertEquals(listOf(SendEvent.ToggleMultiToken(ids[0]), SendEvent.SetMultiNetwork(null)), SweepPick.tap(pinned, ids[0]))
        val two = pinned.copy(multi_selected_ids = listOf(ids[0], ids[1]))
        assertEquals(listOf(SendEvent.ToggleMultiToken(ids[0])), SweepPick.tap(two, ids[0]))
    }

    @Test
    fun `select all pins the first visible valuable row's chain when none is pinned`() {
        assertEquals(listOf(SendEvent.SetMultiNetwork(100), SendEvent.ToggleAllMultiTokens(ids)), SweepPick.selectAll(view, ids))
        assertEquals(listOf(SendEvent.SetMultiNetwork(1), SendEvent.ToggleAllMultiTokens(listOf(ids[2]))), SweepPick.selectAll(view, listOf(ids[2])))
        assertEquals(listOf(SendEvent.ToggleAllMultiTokens(ids)), SweepPick.selectAll(view.copy(multi_chain_id = 100), ids))
        assertTrue(SweepPick.selectAll(view.copy(multi_valuable_ids = emptyList()), ids).isEmpty())
    }
}
