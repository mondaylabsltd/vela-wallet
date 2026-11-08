package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import java.io.File
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.parentSafeCheck

/**
 * VELA-2026-002: the parent-Safe guard's vector, replayed through this shell's door to
 * the rule (uniffi `parent_safe_check`) — every case's verdict is the core's,
 * deep-equal to `expect`. The rule itself is the core's
 * (`tests/app_parent_safe_guard.rs`); this proves the Android door reaches it.
 */
class ParentSafeGuardVectorTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val vector = Wire.json.parseToJsonElement(
        File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json").readText(),
    ) as JsonObject

    @Test
    fun `every vector case is the core's verdict through uniffi`() {
        val cases = vector["cases"] as JsonArray
        assertTrue("the vector names its cases", cases.size >= 70)
        for (case in cases) {
            case as JsonObject
            val name = case["name"]!!.jsonPrimitive.content
            val input = Wire.json.encodeToString(JsonObject.serializer(), case["input"] as JsonObject)
            val got = Wire.json.parseToJsonElement(parentSafeCheck(input))
            assertEquals(name, case["expect"], got)
        }
    }

    @Test
    fun `the takeover on a Safe this account owns is refused as its owner`() {
        val case = (vector["cases"] as JsonArray).map { it as JsonObject }
            .first { it["name"]!!.jsonPrimitive.content.startsWith("a: execTransaction(swapOwner)") }
        val got = Wire.json.parseToJsonElement(
            parentSafeCheck(Wire.json.encodeToString(JsonObject.serializer(), case["input"] as JsonObject)),
        ) as JsonObject
        assertEquals("refuse", got["verdict"]!!.jsonPrimitive.content)
        assertEquals("owner", got["reason"]!!.jsonPrimitive.content)
        assertEquals(vector["parent"], (got["target"] as JsonObject)["safe"])
    }
}
