package app.getvela.wallet

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.RelativeTime
import app.getvela.wallet.core.i18n.I18nRuntime
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.util.SimpleTimeZone

/**
 * "Updated 2m" is the core's relative time (issue 462); this shell keeps no
 * copy of the rule. Both of its routes are replayed here against the core's
 * vectors, `rust/crates/vela-core/tests/vectors/relative-time.json` — the
 * same file the core, the wasm and the Kotlin and Swift harnesses read: the
 * engine wrapper the screens hold ([I18nRuntime.relativeTime]), and
 * [RelativeTime.ago], which adds the device's half (whole seconds, the
 * zone's offset at the moment, the date preset).
 */
class RelativeTimeTest {

    private class Case(val name: String, val input: JsonObject, val expect: String)

    private val root: String = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")

    private fun cases(): List<Case> {
        val doc = Json.parseToJsonElement(File(root, "rust/crates/vela-core/tests/vectors/relative-time.json").readText()).jsonObject
        assertEquals("relative-time", doc["suite"]!!.jsonPrimitive.content)
        return doc["cases"]!!.jsonArray.map {
            val case = it.jsonObject
            assertEquals("format_relative_time", case["fn"]!!.jsonPrimitive.content)
            Case(
                case["name"]!!.jsonPrimitive.content,
                case["input"]!!.jsonObject,
                case["expect"]!!.jsonObject["value"]!!.jsonPrimitive.content,
            )
        }
    }

    private val engines = mutableMapOf<String, I18nRuntime>()

    private fun strings(lng: String): I18nRuntime = engines.getOrPut(lng) {
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize(lng) }
    }

    private fun JsonObject.str(key: String) = this[key]!!.jsonPrimitive.content

    @Test
    fun `every vector reads the same through the engine the screens hold`() {
        val cases = cases()
        assertTrue("the corpus shrank: ${cases.size} cases", cases.size >= 110)
        for (case in cases) {
            val input = case.input
            val got = strings(input.str("lng")).relativeTime(
                tsSeconds = input["ts_seconds"]!!.jsonPrimitive.long,
                nowMs = input["now_ms"]!!.jsonPrimitive.long,
                utcOffsetMinutes = input["utc_offset_minutes"]!!.jsonPrimitive.int,
                dateFormat = input.str("date_format"),
            )
            assertEquals(case.name, case.expect, got)
        }
        assertEquals("all fifteen languages", 15, engines.size)
    }

    /**
     * The device's half: a stamp in milliseconds (with a fraction, as the
     * core's `last_refreshed_at_ms` is a double) floors to whole seconds, the
     * zone gives its offset at the moment, and the person's preset is the
     * stored word. Every vector with a stored word reads the same this way.
     */
    @Test
    fun `the hero's route hands the core the same moment, zone and preset`() {
        var replayed = 0
        for (case in cases()) {
            val input = case.input
            val key = DateFormatKey.entries.firstOrNull { it.wire == input.str("date_format") }
            // `auto` is resolved from the locale before the call, and an
            // unknown word never comes from the shell: both are the engine's
            // test above.
            if (key == null || key == DateFormatKey.Auto) continue
            val seconds = input["ts_seconds"]!!.jsonPrimitive.long
            val got = RelativeTime.ago(
                atMs = seconds * 1000.0 + 999.5,
                nowMs = input["now_ms"]!!.jsonPrimitive.long,
                strings = strings(input.str("lng")),
                formats = Formats(date = key),
                zone = SimpleTimeZone(input["utc_offset_minutes"]!!.jsonPrimitive.int * 60_000, "vector"),
            )
            assertEquals(case.name, case.expect, got)
            replayed += 1
        }
        assertTrue("replayed $replayed", replayed >= 100)
    }

    /** A summer stamp read in winter takes the offset it had then, not the clock's. */
    @Test
    fun `the zone's offset is the moment's own`() {
        val berlin = java.util.TimeZone.getTimeZone("Europe/Berlin")
        // 2026-07-01 22:30 UTC is 00:30 on 2 July in Berlin (summer, UTC+2);
        // read on 1 December (winter, UTC+1) at the clock's offset it would
        // still be 1 July.
        val at = 1_782_945_000_000L
        val now = 1_796_126_400_000L
        assertEquals("2026-07-02", RelativeTime.ago(at.toDouble(), now, strings("en"), Formats(date = DateFormatKey.Iso), berlin))
    }
}
