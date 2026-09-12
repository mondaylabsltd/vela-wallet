package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.send.core.MtokEvent
import app.getvela.wallet.feature.send.core.MtokExecutor
import app.getvela.wallet.feature.send.core.MtokNetwork
import app.getvela.wallet.feature.send.core.MtokOperation
import app.getvela.wallet.feature.send.core.MtokShellResult
import app.getvela.wallet.feature.send.core.MtokView
import app.getvela.wallet.feature.wallet.core.Abi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ManageTokensCore

/**
 * Spec 043 T050: a token added by address through the real `manage_tokens`
 * machine — one multicall per network, the found card, the save into the
 * SAME `vela.customTokens` rows the balance walk reads, the cache
 * invalidation that prices it.
 */
class MtokMachineTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
    private val calls: MutableList<Int> = java.util.Collections.synchronizedList(ArrayList<Int>())
    private var invalidated = 0

    @After
    fun stop() = scope.cancel()

    /** ABI `string`: offset, length, bytes. */
    private fun abiString(value: String): String {
        val bytes = value.toByteArray(Charsets.UTF_8)
        val hex = bytes.joinToString("") { "%02x".format(it) }.padEnd(((bytes.size + 31) / 32) * 64, '0')
        return "20".padStart(64, '0') + bytes.size.toString(16).padStart(64, '0') + hex
    }

    private fun word(n: Long) = n.toString(16).padStart(64, '0')

    /** `aggregate3`'s return: `(bool success, bytes returnData)[]`. */
    private fun aggregate3(results: List<Pair<Boolean, String>>): String {
        val tuples = results.map { (ok, data) ->
            val padded = data.padEnd(((data.length + 63) / 64) * 64, '0')
            word(if (ok) 1 else 0) + word(0x40) + word((data.length / 2).toLong()) + padded
        }
        val offsets = StringBuilder()
        var running = results.size * 32L
        tuples.forEach { offsets.append(word(running)); running += it.length / 2 }
        return "0x" + word(0x20) + word(results.size.toLong()) + offsets + tuples.joinToString("")
    }

    private fun host(meta: (Int) -> String?): CoreHost<MtokView> {
        val executor = MtokExecutor(
            store = store,
            ethCall = { chainId, to, data ->
                calls += chainId
                assertEquals(Abi.MULTICALL3, to)
                assertTrue(data.startsWith("0x82ad56cb"))
                meta(chainId)
            },
            onInvalidated = { invalidated += 1 },
        )
        return CoreHost(
            bridge = ManageTokensCore().asBridge(),
            scope = scope,
            initial = MtokView(),
            serializer = MtokView.serializer(),
            perform = JsonShell.perform(MtokOperation.serializer(), MtokShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(MtokOperation.serializer(), MtokShellResult.serializer(), fallback = MtokShellResult.CacheInvalidated, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("core fault", error) },
        )
    }

    private val networks = listOf(MtokNetwork(100, "Gnosis"), MtokNetwork(137, "Polygon"))

    @Test
    fun `a token is found on the one network that answers, then saved and priced`() = runBlocking {
        val h = host { chainId ->
            if (chainId == 100) aggregate3(listOf(true to abiString("USD Coin on xDai"), true to abiString("USDC"), true to word(6))) else null
        }
        h.dispatch(MtokEvent.Start, MtokEvent.serializer())
        h.dispatch(MtokEvent.AddressInput(usdc), MtokEvent.serializer())
        val valid = withTimeout(10_000) { h.view.first { it.address_valid } }
        assertTrue(valid.found.isEmpty())
        h.dispatch(MtokEvent.DetectRequested(networks), MtokEvent.serializer())
        val found = withTimeout(10_000) { h.view.first { it.found.isNotEmpty() && !it.detecting } }
        assertEquals(listOf(100, 137).sorted(), calls.sorted())
        val card = found.found.single()
        assertEquals("USDC", card.symbol)
        assertEquals(6, card.decimals)
        assertEquals("Gnosis", card.network_name)
        assertFalse(card.added)

        h.dispatch(MtokEvent.SaveRequested(100), MtokEvent.serializer())
        val saved = withTimeout(10_000) { h.view.first { it.found.single().added && !it.saving } }
        assertTrue(saved.custom_tokens.any { it.contract_address.equals(usdc, ignoreCase = true) })
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.CUSTOM_TOKENS))
        assertEquals(1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals(100, row.getInt("chainId"))
        assertEquals("USDC", row.getString("symbol"))
        assertEquals(6, row.getInt("decimals"))
        assertTrue(row.getString("contractAddress").equals(usdc, ignoreCase = true))
        withTimeout(10_000) { while (invalidated == 0) kotlinx.coroutines.delay(20) }
        assertEquals("the balance walk was told once", 1, invalidated)
    }

    @Test
    fun `nothing answers, and the sheet says so`() = runBlocking {
        val h = host { null }
        h.dispatch(MtokEvent.Start, MtokEvent.serializer())
        h.dispatch(MtokEvent.AddressInput(usdc), MtokEvent.serializer())
        h.dispatch(MtokEvent.DetectRequested(networks), MtokEvent.serializer())
        val view = withTimeout(10_000) { h.view.first { it.not_found } }
        assertTrue(view.found.isEmpty())
        assertTrue(store.values[KeyValueStore.Keys.CUSTOM_TOKENS].isNullOrEmpty())
    }

    @Test
    fun `half an address is invalid and looks nothing up`() = runBlocking {
        val h = host { throw AssertionError("must not be asked") }
        h.dispatch(MtokEvent.Start, MtokEvent.serializer())
        h.dispatch(MtokEvent.AddressInput("0xDDAf"), MtokEvent.serializer())
        val view = withTimeout(10_000) { h.view.first { it.input_address == "0xDDAf" } }
        assertFalse(view.address_valid)
        assertTrue(calls.isEmpty())
    }

    @Test
    fun `a token admitted elsewhere is recognised at save time, and not written twice`() = runBlocking {
        store.values[KeyValueStore.Keys.CUSTOM_TOKENS] = JSONArray().put(
            org.json.JSONObject().put("id", "100_${usdc.lowercase()}").put("chainId", 100).put("contractAddress", usdc.lowercase()).put("symbol", "USDC").put("name", "USD Coin").put("decimals", 6),
        ).toString()
        val h = host { chainId -> if (chainId == 100) aggregate3(listOf(true to abiString("USD Coin"), true to abiString("USDC"), true to word(6))) else null }
        h.dispatch(MtokEvent.Start, MtokEvent.serializer())
        val loaded = withTimeout(10_000) { h.view.first { it.custom_tokens.isNotEmpty() } }
        assertEquals("USDC", loaded.custom_tokens.single().symbol)
        h.dispatch(MtokEvent.AddressInput(usdc), MtokEvent.serializer())
        h.dispatch(MtokEvent.DetectRequested(networks), MtokEvent.serializer())
        val found = withTimeout(10_000) { h.view.first { it.found.isNotEmpty() && !it.detecting } }
        // Ported verbatim from the web: the card does not read "added" until
        // a save is attempted — the save-time re-read is what recognises it.
        assertFalse(found.found.single().added)
        h.dispatch(MtokEvent.SaveRequested(100), MtokEvent.serializer())
        withTimeout(10_000) { h.view.first { it.found.single().added && !it.saving } }
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.CUSTOM_TOKENS))
        assertEquals("already stored: marked added, nothing written", 1, rows.length())
    }
}
