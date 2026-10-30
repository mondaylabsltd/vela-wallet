package app.getvela.wallet.dev

import android.app.Activity
import android.content.Intent
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject

/**
 * Debug builds only (native parity F0.5): point the app at a local harness from
 * the launch line — the native twin of the web harness's seeded storage
 * (`e2e/onchain/harness.ts` `useHarness`):
 *
 *   adb shell am start -n app.getvela.wallet/.MainActivity \
 *     --es vela.serviceEndpoints '{"bundlerServiceURL":"http://127.0.0.1:8990","genesisServiceURL":"http://127.0.0.1:8987"}' \
 *     --es vela.rpcOverrides '{"100":"http://127.0.0.1:8945","8453":"http://127.0.0.1:8946"}' \
 *     --es vela.peerconnectRelay ws://127.0.0.1:8992
 *
 * Each is written to the store under the key every shell reads
 * (`vela.serviceEndpoints`, `vela.networkConfig`, `vela.peerconnectRelay`),
 * MERGED into what is there: an endpoint not named keeps its value, a network
 * override keeps its explorer, bundler and "Only use my node". When anything
 * changed the process restarts once (the machines read the store at boot, and
 * the pool keeps a chain's endpoints for the process); a launch that changes
 * nothing is a plain launch, so a script may repeat it.
 *
 * The release source set has no such class — `LaunchInputsBinding` there is a
 * no-op, and `scripts/check-release-apk.sh` refuses a package carrying this one.
 */
object ServiceLaunchInputs {
    const val SERVICE_ENDPOINTS = "vela.serviceEndpoints"
    const val RPC_OVERRIDES = "vela.rpcOverrides"
    const val PEERCONNECT_RELAY = "vela.peerconnectRelay"

    /** The stored key the relay URL goes under (the web harness's). */
    const val PEERCONNECT_KEY = "vela.peerconnectRelay"

    /** Snake-case spellings a script may use for the stored camelCase names. */
    private val ENDPOINT_ALIASES = mapOf(
        "ethereum_data_url" to "ethereumDataURL",
        "passkey_index_url" to "passkeyIndexURL",
        "bundler_service_url" to "bundlerServiceURL",
        "fiat_rates_url" to "fiatRatesURL",
        "genesis_service_url" to "genesisServiceURL",
    )
    private val ENDPOINT_NAMES = ENDPOINT_ALIASES.values.toSet()

    /**
     * `stored` (the `vela.serviceEndpoints` object, or null) with [input]'s
     * endpoints laid over it. An unknown name is refused, not stored.
     */
    fun mergeEndpoints(stored: String?, input: String): String {
        val into = stored?.let { runCatching { JSONObject(it) }.getOrNull() } ?: JSONObject()
        val given = JSONObject(input)
        for (key in given.keys()) {
            val name = ENDPOINT_ALIASES[key] ?: key.takeIf { it in ENDPOINT_NAMES }
                ?: throw IllegalArgumentException("not a service endpoint: $key")
            into.put(name, given.getString(key))
        }
        return into.toString()
    }

    /**
     * `stored` (the `vela.networkConfig` list, or null) with [input]'s RPCs set:
     * `{"100":"http://…"}` or `[{"chainId":100,"rpcURL":"http://…"}]`. An
     * existing record keeps every other field; a new one carries the rest empty
     * (the network's own defaults), as the web harness writes it.
     */
    fun mergeRpcOverrides(stored: String?, input: String): String {
        val list = stored?.let { runCatching { JSONArray(it) }.getOrNull() } ?: JSONArray()
        val wanted: List<Pair<Long, String>> = input.trim().let { text ->
            if (text.startsWith("[")) {
                val array = JSONArray(text)
                (0 until array.length()).map { i ->
                    val row = array.getJSONObject(i)
                    row.getLong("chainId") to (row.optString("rpcURL").ifEmpty { row.getString("rpc_url") })
                }
            } else {
                val obj = JSONObject(text)
                obj.keys().asSequence().map { key -> key.toLong() to obj.getString(key) }.toList()
            }
        }
        for ((chainId, url) in wanted) {
            require(chainId > 0) { "not a chain id: $chainId" }
            val at = (0 until list.length()).firstOrNull { list.optJSONObject(it)?.optLong("chainId") == chainId }
            if (at != null) {
                list.getJSONObject(at).put("rpcURL", url)
            } else {
                list.put(JSONObject().put("chainId", chainId).put("rpcURL", url).put("explorerURL", "").put("bundlerURL", ""))
            }
        }
        return list.toString()
    }

    /** What this launch asks for, as store writes: key → new value. Throws on a malformed input. */
    suspend fun writes(intent: Intent?, read: suspend (String) -> String?): Map<String, String> {
        if (intent == null) return emptyMap()
        val out = linkedMapOf<String, String>()
        intent.getStringExtra(SERVICE_ENDPOINTS)?.let {
            out[KeyValueStore.Keys.SERVICE_ENDPOINTS] = mergeEndpoints(read(KeyValueStore.Keys.SERVICE_ENDPOINTS), it)
        }
        intent.getStringExtra(RPC_OVERRIDES)?.let {
            out[KeyValueStore.Keys.NETWORK_CONFIG] = mergeRpcOverrides(read(KeyValueStore.Keys.NETWORK_CONFIG), it)
        }
        intent.getStringExtra(PEERCONNECT_RELAY)?.let { url ->
            require(url.startsWith("ws://") || url.startsWith("wss://")) { "not a relay URL: $url" }
            out[PEERCONNECT_KEY] = url
        }
        return out
    }

    /**
     * Apply this launch's inputs. `true` when something changed and the
     * process is restarting (the caller returns at once).
     */
    fun apply(activity: Activity): Boolean {
        val intent = activity.intent ?: return false
        if (listOf(SERVICE_ENDPOINTS, RPC_OVERRIDES, PEERCONNECT_RELAY).none { intent.hasExtra(it) }) return false
        val store = VelaStore(activity.applicationContext)
        val changed = runBlocking {
            val planned = runCatching { writes(intent) { key -> store.read(key) } }
                .onFailure { VelaLog.failure("debug.launchInputs", "refused", it) }
                .getOrNull() ?: return@runBlocking false
            var any = false
            for ((key, value) in planned) {
                if (store.read(key) == value) continue
                store.write(key, value)
                any = true
            }
            VelaLog.event("debug.launchInputs", if (any) "applied" else "unchanged", "keys" to planned.keys.joinToString(","))
            any
        }
        if (!changed) return false
        // The machines read these at boot and the pool keeps a chain's
        // endpoints for the process: start again, without the inputs.
        val again = Intent(intent).apply {
            removeExtra(SERVICE_ENDPOINTS)
            removeExtra(RPC_OVERRIDES)
            removeExtra(PEERCONNECT_RELAY)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        }
        activity.startActivity(again)
        activity.finishAffinity()
        Runtime.getRuntime().exit(0)
        return true
    }
}
