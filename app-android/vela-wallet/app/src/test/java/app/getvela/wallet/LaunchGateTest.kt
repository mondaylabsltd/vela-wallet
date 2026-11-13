package app.getvela.wallet

import app.getvela.wallet.core.platform.LaunchGate
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The launch animation's replay rule on Android is the core's
 * (`vela_core::launch`): at most once a week, held to the core's own vectors
 * (`tests/vectors/launch/launch-animation.json`) through the store the app
 * reads it from — the web's `constants.test.ts` does the same for its copies.
 */
class LaunchGateTest {
    private val vectors = JSONObject(
        File(
            System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"),
            "rust/crates/vela-core/tests/vectors/launch/launch-animation.json",
        ).readText(),
    )

    @Test
    fun `every vector, through the store`() = runBlocking {
        assertEquals("the core's key is the web's", vectors.getString("storage_key"), uniffi.vela_core_uniffi.launchAnimationStorageKey())
        val cases = vectors.getJSONArray("cases")
        assertTrue(cases.length() >= 8)
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val store = FakeStore()
            if (!case.isNull("last_played_ms")) store.values[vectors.getString("storage_key")] = case.getLong("last_played_ms").toString()
            assertEquals(case.getString("name"), case.getBoolean("due"), LaunchGate.decide(store, case.getLong("now_ms"), case.getBoolean("disabled")))
        }
    }

    @Test
    fun `playing starts the week, and the next cold start within it is quiet`() = runBlocking {
        val store = FakeStore()
        val now = 1_700_000_000_000L
        assertTrue(LaunchGate.decide(store, now, disabled = false))
        LaunchGate.played(store, now)
        assertEquals(now.toString(), store.values[uniffi.vela_core_uniffi.launchAnimationStorageKey()])
        assertFalse(LaunchGate.decide(store, now + 60_000, disabled = false))
        assertTrue(LaunchGate.decide(store, now + 7L * 24 * 60 * 60 * 1000, disabled = false))
    }

    @Test
    fun `an unreadable stamp or store plays it, never silences it`() = runBlocking {
        val key = uniffi.vela_core_uniffi.launchAnimationStorageKey()
        assertTrue(LaunchGate.decide(FakeStore(mapOf(key to "not a number")), 1_000, disabled = false))
        assertTrue(LaunchGate.decide(FakeStore().apply { failReads = true }, 1_000, disabled = false))
        assertNull(LaunchGate.lastPlayedMs("NaN"))
        assertNull(LaunchGate.lastPlayedMs("Infinity"))
        assertEquals(42.0, LaunchGate.lastPlayedMs(" 42 ")!!, 0.0)
    }
}
