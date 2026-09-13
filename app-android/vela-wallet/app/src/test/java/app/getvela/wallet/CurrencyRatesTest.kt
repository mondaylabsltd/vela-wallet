package app.getvela.wallet

import app.getvela.wallet.feature.settings.core.CurrencyRates
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import java.math.BigInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What a dollar is worth in somebody's own currency.
 *
 * Two rules carry the whole file: a `<CCY>/USD` feed answers the OPPOSITE
 * direction from the rate this returns, and a rate that cannot be had is
 * `null` rather than 1.
 */
class CurrencyRatesTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private class OneEndpoint : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun word(value: BigInteger) = value.toString(16).padStart(64, '0')

    /**
     * A `latestRoundData` answer: five words, the price second.
     *
     * `answer` is `<CCY>/USD` — what one unit of that currency costs in
     * dollars — in the feed's own decimals.
     */
    private fun roundData(answer: BigInteger) = word(BigInteger.valueOf(1)) +
        word(answer) + word(BigInteger.ZERO) + word(BigInteger.ZERO) + word(BigInteger.ZERO)

    /** A batch answer: `latestRoundData` then `decimals()`, per feed, in order. */
    private fun feedBatch(feeds: List<Pair<BigInteger, Int>?>): RpcPostResult {
        val entries = feeds.flatMap { feed ->
            if (feed == null) listOf(null, null)
            else listOf(roundData(feed.first), word(BigInteger.valueOf(feed.second.toLong())))
        }
        val elements = entries.map { data ->
            val payload = data ?: ""
            word(if (data == null) BigInteger.ZERO else BigInteger.ONE) +
                word(BigInteger.valueOf(0x40)) +
                word(BigInteger.valueOf((payload.length / 2).toLong())) +
                payload
        }
        val offsets = StringBuilder()
        var offset = entries.size * 32L
        elements.forEach {
            offsets.append(word(BigInteger.valueOf(offset)))
            offset += it.length / 2
        }
        val hex = "0x" + word(BigInteger.valueOf(32)) +
            word(BigInteger.valueOf(entries.size.toLong())) +
            offsets + elements.joinToString("")
        return FakeRpcTransport.body(hex)
    }

    /** The feeds, in the order [CurrencyRates] batches them. */
    private val order = listOf(
        "EUR", "GBP", "JPY", "CNY", "AUD", "CAD", "CHF",
        "KRW", "BRL", "MXN", "PHP", "IDR", "ARS",
    )

    private fun rates(
        prices: Map<String, Pair<BigInteger, Int>>,
        endpoint: String = "",
    ): CurrencyRates {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val transport = FakeRpcTransport { _, _ ->
            feedBatch(order.map { code -> prices[code] })
        }
        val pool = RpcPool(FakeStore(), OneEndpoint(), scope, transport = transport)
        runBlocking { pool.start() }
        return CurrencyRates(pool = pool, store = FakeStore(), endpoint = { endpoint })
    }

    // -- the inversion -------------------------------------------------------

    /**
     * **The line that would be silently, hugely wrong.**
     *
     * JPY/USD is about 0.0068 — one yen is most of a cent. The rate a display
     * currency needs is the other way round: about 147 yen to the dollar.
     * Returning the feed's own number would show a Japanese person a balance
     * roughly twenty-two thousand times too small, with no sign anything is
     * amiss.
     */
    @Test
    fun `a fiat feed is inverted into a USD to currency rate`() = runBlocking {
        // 0.0068 USD per JPY, at the feed's 8 decimals.
        val subject = rates(mapOf("JPY" to (BigInteger.valueOf(680_000) to 8)))

        val rate = subject.resolve("JPY")!!

        assertEquals(1.0 / 0.0068, rate, 0.01)
        assertTrue("a dollar buys more than a hundred yen, not a fraction of one", rate > 100)
    }

    /**
     * The feeds do not share a decimals: most report 8, PHP reports 18.
     *
     * Assuming 8 for PHP is a rate wrong by a factor of 10^10 — and in the
     * direction that makes somebody's money look like nothing.
     */
    @Test
    fun `each feed is scaled by the decimals it reports`() = runBlocking {
        val subject = rates(
            mapOf(
                "EUR" to (BigInteger.valueOf(108_000_000) to 8), //          1.08 USD per EUR
                "PHP" to (BigInteger("17000000000000000") to 18), //         0.017 USD per PHP
            ),
        )

        assertEquals(1.0 / 1.08, subject.resolve("EUR")!!, 0.001)
        assertEquals(1.0 / 0.017, subject.resolve("PHP")!!, 0.5)
    }

    // -- null is not one ------------------------------------------------------

    @Test
    fun `a currency nothing can price answers null, never one`() = runBlocking {
        // No Chainlink feed, no FX endpoint configured. The core reads null as
        // "keep showing dollars"; a 1 here would claim one dollar is one dong.
        val subject = rates(emptyMap())

        assertNull(subject.resolve("VND"))
    }

    @Test
    fun `a feed that failed does not fall back to one`() = runBlocking {
        val subject = rates(emptyMap())

        assertNull(subject.resolve("JPY"))
    }

    @Test
    fun `USD is exactly one without asking anybody`() = runBlocking {
        val subject = rates(emptyMap())

        assertEquals(1.0, subject.resolve("USD")!!, 0.0)
    }

    @Test
    fun `a zero or negative feed answer prices nothing`() = runBlocking {
        // A feed reporting zero would invert to infinity.
        val subject = rates(mapOf("EUR" to (BigInteger.ZERO to 8)))

        assertNull(subject.resolve("EUR"))
    }

    // -- the endpoint's shapes ------------------------------------------------

    @Test
    fun `both provider shapes are accepted`() {
        val subject = rates(emptyMap())

        val array = subject.normalize("""[{"base":"USD","quote":"EUR","rate":0.92}]""")
        val record = subject.normalize("""{"rates":{"eur":0.92,"vnd":25000}}""")

        assertEquals(0.92, array!!["EUR"]!!, 1e-9)
        assertEquals(1.0, array["USD"]!!, 1e-9)
        assertEquals(25000.0, record!!["VND"]!!, 1e-9)
    }

    @Test
    fun `a response that prices nothing is not a rate table`() {
        val subject = rates(emptyMap())

        // Only the `USD: 1` this code puts there itself — the provider said
        // nothing, and answering with that alone would look like a working
        // table that happens to know one currency.
        assertNull(subject.normalize("""{"rates":{}}"""))
        assertNull(subject.normalize("[]"))
        assertNull(subject.normalize("not json at all"))
    }

    @Test
    fun `a provider's rubbish values are dropped, not stored`() {
        val subject = rates(emptyMap())

        val table = subject.normalize("""{"rates":{"EUR":0.92,"XXX":0,"YYY":-3,"ZZZ":"abc"}}""")!!

        assertEquals(setOf("USD", "EUR"), table.keys)
    }
}
