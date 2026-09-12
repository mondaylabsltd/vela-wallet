package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.net.VelaHttp
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject

/**
 * The `clear_signing` machine's five arms (spec 044 T030; the desktop's
 * `executor/clear_signing.rs`): descriptors from the data endpoint the
 * settings machine names, selector candidates from openchain then 4byte
 * (cached), a routed `eth_call` with the one distinction the core cannot
 * re-derive (answered / REVERTED / could not ask), the clock, a timer.
 */
class ClearExecutor(
    /** The chain-data base URL (`ethereumDataURL` in the service endpoints). */
    private val dataBase: () -> String,
    /** `eth_call` through the pool: `(result, reverted)` — `(null, true)` is a revert, `(null, false)` no answer. */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> Pair<String?, Boolean>,
) {
    private val selectorCache = HashMap<String, List<String>>()

    suspend fun perform(operation: ClearOperation): ClearShellResult = when (operation) {
        is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(operation.path, descriptor(operation.path))
        is ClearOperation.RpcEthCall -> {
            val (result, reverted) = ethCall(operation.chain_id, operation.to, operation.data)
            ClearShellResult.RpcAnswer(operation.probe, operation.chain_id, operation.to, result, rpc_error = result == null && reverted)
        }
        is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates(lookupSelector(operation.selector))
        is ClearOperation.Timer -> {
            delay(operation.ms.toLong())
            ClearShellResult.TimedOut(operation.token)
        }
        ClearOperation.Now -> ClearShellResult.Clock(System.currentTimeMillis().toDouble())
    }

    fun neutralAnswer(operation: ClearOperation): ClearShellResult = when (operation) {
        is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(operation.path, null)
        is ClearOperation.RpcEthCall -> ClearShellResult.RpcAnswer(operation.probe, operation.chain_id, operation.to, null, false)
        is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
        is ClearOperation.Timer -> ClearShellResult.TimedOut(operation.token)
        ClearOperation.Now -> ClearShellResult.Clock(System.currentTimeMillis().toDouble())
    }

    private suspend fun descriptor(path: String): String? = withContext(Dispatchers.IO) {
        val base = dataBase().ifBlank { return@withContext null }
        runCatching {
            VelaHttp.client.newBuilder().callTimeout(DESCRIPTOR_TIMEOUT_MS, TimeUnit.MILLISECONDS).build()
                .newCall(Request.Builder().url(base.trimEnd('/') + "/" + path.trimStart('/')).header("accept", "application/json").get().build())
                .execute()
                .use { response -> if (response.code == 200) response.body?.string() else null }
        }.getOrNull()
    }

    /** Eight hex digits, or nothing is worth asking about. */
    private fun normaliseSelector(raw: String): String? {
        val hex = raw.removePrefix("0x").lowercase()
        return hex.takeIf { it.length == 8 && it.all { c -> c in '0'..'9' || c in 'a'..'f' } }
    }

    private suspend fun lookupSelector(raw: String): List<String> {
        val selector = normaliseSelector(raw) ?: return emptyList()
        selectorCache[selector]?.let { return it }
        val found = withContext(Dispatchers.IO) {
            fromOpenchain("https://api.openchain.xyz", selector).ifEmpty { fromFourByte(selector) }
        }
        if (found.isNotEmpty()) selectorCache[selector] = found
        return found
    }

    private fun getJson(url: String): String? = runCatching {
        VelaHttp.client.newBuilder().callTimeout(SELECTOR_TIMEOUT_MS, TimeUnit.MILLISECONDS).build()
            .newCall(Request.Builder().url(url).header("accept", "application/json").get().build())
            .execute()
            .use { response -> if (response.isSuccessful) response.body?.string() else null }
    }.getOrNull()

    private fun fromOpenchain(host: String, selector: String): List<String> {
        val body = getJson("$host/signature-database/v1/lookup?function=0x$selector&filter=true") ?: return emptyList()
        val list = runCatching { JSONObject(body).optJSONObject("result")?.optJSONObject("function")?.optJSONArray("0x$selector") }.getOrNull() ?: return emptyList()
        return (0 until list.length()).mapNotNull { list.optJSONObject(it)?.optString("name")?.ifBlank { null } }
    }

    private fun fromFourByte(selector: String): List<String> {
        val body = getJson("https://www.4byte.directory/api/v1/signatures/?hex_signature=0x$selector") ?: return emptyList()
        val results = runCatching { JSONObject(body).optJSONArray("results") }.getOrNull() ?: return emptyList()
        return (0 until results.length()).mapNotNull { results.optJSONObject(it)?.optString("text_signature")?.ifBlank { null } }
            .sortedBy { it.length }
    }

    companion object {
        const val DESCRIPTOR_TIMEOUT_MS = 6_000L
        const val SELECTOR_TIMEOUT_MS = 5_000L
    }
}
