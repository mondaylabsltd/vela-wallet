package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.multisig.readingLines
import app.getvela.wallet.feature.multisig.reviewConcern
import java.io.File
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * r8-1 / r8-2 / r8-5: the core's reading vectors (`tests/vectors/multisig/
 * reading.json`), replayed through the link door, drawn by this shell —
 * every call at every level has its line, every concern its words, no
 * amount is worded with 18 decimals it does not have, and an unlimited
 * grant reads as a danger.
 */
class R8ReadingDrawnTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val words = MultisigWords(strings = strings, accounts = MsAccountsView(), nativeSymbol = { "USDC" })

    private val cases: List<JsonElement> by lazy {
        val file = File(repoRoot, "rust/crates/vela-core/tests/vectors/multisig/reading.json")
        Wire.json.parseToJsonElement(file.readText())["cases"].arr
    }

    private fun every(calls: List<JsonElement>): List<JsonElement> = calls.flatMap { c -> listOf(c) + if (c["intent"].kind == "batch") every(c["intent"]["calls"].arr) else emptyList() }

    @Test
    fun the_link_door_reads_each_vector_as_the_core_expects() {
        assertTrue(cases.isNotEmpty())
        for (case in cases) {
            val got = MsRules.call("review.reading", (case["args"] as JsonObject).toMap())
            assertNotNull(case["name"].str, got)
            assertEquals(case["name"].str, case["expect"]["reading"], got!!["reading"])
        }
    }

    @Test
    fun every_call_at_every_level_is_drawn_and_every_concern_has_words() {
        for (case in cases) {
            val name = case["name"].str.orEmpty()
            val expect = case["expect"]
            val review = JsonObject(
                mapOf(
                    "reading" to expect["reading"]!!,
                    "concerns" to (expect["concerns"] ?: JsonArray(emptyList())),
                    "sign" to JsonObject(mapOf("allowed" to JsonPrimitive(expect["refused"].bool != true), "anyway" to JsonPrimitive(expect["anyway"].bool == true))),
                ),
            )
            val calls = every(expect["reading"]["calls"].arr)
            val lines = readingLines(review, 5042002, words)
            val batches = calls.count { it["intent"].kind == "batch" }
            assertEquals("$name: one line per call, plus each batch's runner note", calls.size + batches, lines.size)
            // (The state guard's two are its own block, not a concern line.)
            for (concern in expect["concerns"].arr.filter { it.kind != "stateChange" && it.kind != "stateUnread" }) {
                assertNotNull("$name: ${concern.kind} has words", reviewConcern(words, concern))
            }
            // Never 18 assumed: an amount whose decimals are unknown is its raw base units.
            for (call in calls) {
                val amount = call["intent"]["amount"]
                if (amount != null && amount["decimals"] == null || amount?.get("decimals") is kotlinx.serialization.json.JsonNull) {
                    if (amount!!["unlimited"].bool == true) continue
                    val raw = amount["raw"].str.orEmpty()
                    assertTrue("$name: the raw base units are said", lines.any { l -> l.details.any { it.second.contains(raw) } })
                }
            }
            if (expect["reading"]["unlimited"].bool == true) {
                assertTrue("$name: an unlimited grant reads as a danger", lines.any { it.tone == MsTone.Danger })
            }
            if (expect["reading"]["nested"].bool == true) {
                assertTrue("$name: legs drawn under their batch", lines.any { it.depth >= 1 && it.label.contains(".") })
                assertTrue("$name: approve anyway, or nothing signed at all", expect["anyway"].bool == true || expect["refused"].bool == true)
            }
        }
    }

    private fun JsonObject.toMap(): Map<String, Any?> = mapValues { (_, v) -> toAny(v) }

    private fun toAny(v: JsonElement): Any? = when (v) {
        is JsonObject -> v.toMap()
        is JsonArray -> v.map(::toAny)
        is JsonPrimitive -> if (v.isString) v.content else v.content.toLongOrNull() ?: v.content.toDoubleOrNull() ?: v.content.toBooleanStrictOrNull()
        else -> null
    }
}
