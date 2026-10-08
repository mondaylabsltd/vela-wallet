package app.getvela.wallet

import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.components.tokenGlyph
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Which logo a mark wears is the core's (`remote_mark.rs`); this shell keeps
 * no copy of the rule. Its own module (`Marks`, and the `TokenMarkModel` that
 * `WalletLive` builds from it) is replayed here against the core's vectors,
 * `rust/crates/vela-core/tests/vectors/marks.json` — the same file the core,
 * the wasm, the Kotlin and the Swift harnesses read — so a shell that drew a
 * different logo than the rule says fails here, not on somebody's phone.
 */
class MarksTest {
    @After fun reset() { Marks.base = "" }

    private class Case(val name: String, val fn: String, val input: JsonObject, val expect: JsonElement)

    private fun cases(): List<Case> {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val file = File(root, "rust/crates/vela-core/tests/vectors/marks.json")
        val doc = Json.parseToJsonElement(file.readText()).jsonObject
        assertEquals("marks", doc["suite"]!!.jsonPrimitive.content)
        return doc["cases"]!!.jsonArray.map {
            val case = it.jsonObject
            Case(
                case["name"]!!.jsonPrimitive.content,
                case["fn"]!!.jsonPrimitive.content,
                case["input"]!!.jsonObject,
                case["expect"]!!,
            )
        }
    }

    private fun JsonObject.str(key: String): String = this[key]!!.jsonPrimitive.content
    private fun JsonObject.strOrNull(key: String): String? = this[key]!!.let { if (it is JsonNull) null else it.jsonPrimitive.content }
    private fun JsonObject.strings(key: String): List<String> = (this[key] as JsonArray).map { it.jsonPrimitive.content }

    @Test
    fun `every vector reads the same through this shell's marks`() {
        val cases = cases()
        assertTrue("the corpus shrank: ${cases.size} cases", cases.size >= 36)
        val seen = mutableSetOf<String>()
        for (case in cases) {
            seen += case.fn
            val input = case.input
            // The endpoint as Settings would store it; the core reads blank as the built-in one.
            Marks.base = input.str("ethereum_data_url")
            val chainId = input["chain_id"]!!.jsonPrimitive.int
            when (case.fn) {
                "token_mark" -> {
                    val symbol = input.str("symbol")
                    val address = input.strOrNull("token_address")
                    val named = input.strings("named")
                    val expect = case.expect.jsonObject
                    val view = Marks.tokenMark(chainId, symbol, address, named)
                    assertMark(case.name, expect, view.glyph, view.logoUrls, view.badgeChainId?.toInt(), view.badgeLogoUrl)

                    // What the screens draw: the shell's model and its circle's letters.
                    val model = WalletLive.mark(chainId, symbol, address, named)
                    assertEquals("${case.name}: logos on screen", view.logoUrls, model.logoUrls)
                    assertEquals("${case.name}: badge drawn exactly when the core names its chain", expect["badge_chain_id"] is JsonNull, model.badgeHidden)
                    assertEquals("${case.name}: badge logo on screen", expect.strOrNull("badge_logo_url"), model.badgeLogoUrl)
                    assertEquals("${case.name}: the circle's letters are the core's glyph", expect.str("glyph"), tokenGlyph(model.ticker))
                }
                "chain_mark" -> {
                    val native = input.str("native_symbol")
                    val expect = case.expect.jsonObject
                    val view = Marks.chainMark(chainId, native)
                    assertMark(case.name, expect, view.glyph, view.logoUrls, view.badgeChainId?.toInt(), view.badgeLogoUrl)

                    val model = WalletLive.chainMark(chainId, native)
                    assertEquals("${case.name}: logos on screen", view.logoUrls, model.logoUrls)
                    assertTrue("${case.name}: a network never wears a badge", model.badgeHidden)
                    assertNull(model.badgeLogoUrl)
                    assertEquals("${case.name}: the circle's letters are the core's glyph", expect.str("glyph"), tokenGlyph(model.ticker))
                }
                "chain_logo_url" -> {
                    val value = case.expect.jsonObject["value"]!!
                    assertEquals(case.name, if (value is JsonNull) null else value.jsonPrimitive.content, Marks.chainLogoUrl(chainId))
                }
                else -> error("no arm for `${case.fn}` (${case.name}) — add it here")
            }
        }
        assertEquals("every function the core exports is replayed", setOf("token_mark", "chain_mark", "chain_logo_url"), seen)
    }

    private fun assertMark(name: String, expect: JsonObject, glyph: String, logos: List<String>, badgeChain: Int?, badgeLogo: String?) {
        assertEquals("$name: glyph", expect.str("glyph"), glyph)
        assertEquals("$name: logo_urls", expect.strings("logo_urls"), logos)
        val chain = expect["badge_chain_id"]!!
        assertEquals("$name: badge_chain_id", if (chain is JsonNull) null else chain.jsonPrimitive.int, badgeChain)
        assertEquals("$name: badge_logo_url", expect.strOrNull("badge_logo_url"), badgeLogo)
    }

    /**
     * The endpoint is read when the mark is asked for: a re-pointed
     * 服务节点 changes every logo from then on, with no relaunch, and blank
     * is the built-in endpoint rather than "letters only".
     */
    @Test
    fun `a changed endpoint changes the next mark, and blank is the built-in one`() {
        Marks.base = ""
        assertEquals(listOf("https://ethereum-data.getvela.app/chainlogos/eip155-1.png"), Marks.tokenMark(1, "ETH", null).logoUrls)
        Marks.base = "https://data.example/"
        assertEquals(listOf("https://data.example/chainlogos/eip155-1.png"), Marks.tokenMark(1, "ETH", null).logoUrls)
        assertEquals("https://data.example/chainlogos/eip155-8453.png", Marks.chainLogoUrl(8453))
    }

    /**
     * The kind rule, on this shell: a NETWORK wears its own logo with no
     * badge; ETH on Base as a coin wears Ethereum's logo with Base's badge.
     * A network row that asked for the coin's mark showed Ethereum beside
     * "Base".
     */
    @Test
    fun `a network wears its own logo, a coin its home chain's`() {
        Marks.base = "https://data.example/"
        val network = WalletLive.chainMark(8453, "ETH")
        assertEquals(listOf("https://data.example/chainlogos/eip155-8453.png"), network.logoUrls)
        assertTrue(network.badgeHidden)
        val coin = WalletLive.mark(8453, "ETH", null)
        assertEquals(listOf("https://data.example/chainlogos/eip155-1.png"), coin.logoUrls)
        assertFalse(coin.badgeHidden)
        assertEquals("https://data.example/chainlogos/eip155-8453.png", coin.badgeLogoUrl)
        assertEquals("the dot under the badge is the coin's chain's colour", WalletLive.badge(8453), coin.badgeColor)
    }

    /**
     * The badge that carries a chain's logo is one size on all four shells:
     * 16 across with a 1.5 ring, the logo 13 inside it — what the web's
     * `.badge.with-logo` (border-box) and the desktop's draw. It was a 12
     * logo in a 2 ring here.
     */
    @Test
    fun `the logo badge is 16 with a 1 and a half ring`() {
        val metrics = app.getvela.wallet.feature.wallet.components.WalletMetrics
        assertEquals(16f, metrics.badgeLogoRingSize.value, 0f)
        assertEquals(13f, metrics.badgeLogoSize.value, 0f)
    }

    /** Chain 0 names no network: nothing is asked of the endpoint for it. */
    @Test
    fun `chain 0 asks the endpoint for nothing`() {
        assertNull(Marks.chainLogoUrl(0))
        assertTrue(Marks.chainMark(0, "ETH").logoUrls.isEmpty())
        assertTrue(Marks.tokenMark(0, "USDC", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").logoUrls.isEmpty())
        assertNull("a negative id is not a chain either", Marks.chainLogoUrl(-1))
    }

    /**
     * Spec 082 RE10 (W20): how long a failed logo stays failed is the core's
     * (`markMissTtlMs`) — a 404, a refusal or bytes that do not draw for the
     * session; a throttle, a server's bad minute or no answer for a minute,
     * and until the network comes back.
     */
    @Test
    fun `a failed logo stays failed as long as the core says`() {
        app.getvela.wallet.core.marks.LogoMisses.clear()
        val misses = app.getvela.wallet.core.marks.LogoMisses
        val t0 = 1_000_000L
        misses.record("https://x/404.png", 404, nowMs = t0)
        misses.record("https://x/403.png", 403, nowMs = t0)
        misses.record("https://x/garbage.png", 200, nowMs = t0)
        misses.record("https://x/429.png", 429, nowMs = t0)
        misses.record("https://x/503.png", 503, nowMs = t0)
        misses.record("https://x/offline.png", null, nowMs = t0)
        val later = t0 + 60_000L
        for (session in listOf("404", "403", "garbage")) assertTrue(session, misses.missed("https://x/$session.png", later))
        for (transient in listOf("429", "503", "offline")) {
            assertTrue("$transient inside the minute", misses.missed("https://x/$transient.png", t0 + 59_999L))
            assertFalse("$transient asked again after it", misses.missed("https://x/$transient.png", later))
        }

        misses.record("https://x/503.png", 503, nowMs = t0)
        misses.clearTransient()
        assertFalse("the network came back: a bad minute is forgiven", misses.missed("https://x/503.png", t0))
        assertTrue("a 404 is not", misses.missed("https://x/404.png", t0))
        misses.clear()
    }
}
