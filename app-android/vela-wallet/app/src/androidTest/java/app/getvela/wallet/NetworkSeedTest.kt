package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A seed for the device pass on SC-002, and a cross-client check in its own
 * right.
 *
 * Adding a custom network needs the chain index spec 041 brings, so there is no
 * in-app route to one in spec 040. What there IS is the stored shape every
 * other Vela client writes — so this puts those exact bytes on the device and
 * the settings page has to make sense of them. If the camelCase keys were
 * wrong, the row would simply not appear.
 *
 * ```bash
 * P=app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * adb shell am instrument -w -e class 'app.getvela.wallet.NetworkSeedTest#seedACustomNetwork' "$P"
 * # …remove it in the UI…
 * adb shell am instrument -w -e class 'app.getvela.wallet.NetworkSeedTest#itIsGone' "$P"
 * ```
 */
@RunWith(AndroidJUnit4::class)
class NetworkSeedTest {

    private val store get() = VelaStore(InstrumentationRegistry.getInstrumentation().targetContext)

    @Test
    fun seedACustomNetwork() = runBlocking {
        // Written as another client would write it: camelCase keys, plain
        // integers, no nulls for absent optionals.
        val stored = """
            [{"id":"custom-7777","displayName":"Seven Testnet","chainId":7777,
              "iconLabel":"S","iconColor":"#FFFFFF","iconBg":"#2E9E7E",
              "logoURL":"","isL2":false,"rpcURL":"https://rpc.seven.example",
              "explorerURL":"https://scan.seven.example","bundlerURL":"",
              "nativeSymbol":"SVN","addedAt":"2026-09-05T03:00:00Z"}]
        """.trimIndent().replace("\n", "").replace("  ", "")

        assertTrue(store.write(KeyValueStore.Keys.CUSTOM_NETWORKS, stored))
    }

    @Test
    fun itIsGone() = runBlocking {
        val raw = store.read(KeyValueStore.Keys.CUSTOM_NETWORKS)
        assertTrue(
            "the removed network must not be on disk any more (was: $raw)",
            raw == null || !raw.contains("custom-7777"),
        )
    }

    @Test
    fun clearTheSeed() = runBlocking {
        store.remove(KeyValueStore.Keys.CUSTOM_NETWORKS)
        assertNull(store.read(KeyValueStore.Keys.CUSTOM_NETWORKS))
    }
}
