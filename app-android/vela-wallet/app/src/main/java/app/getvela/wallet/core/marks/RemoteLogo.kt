package app.getvela.wallet.core.marks

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import app.getvela.wallet.core.net.VelaHttp
import okhttp3.Cache
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * A logo fetched from the chain-data endpoint, drawn over the glyph the shell
 * would draw anyway (the web's `RemoteLogo.svelte`, spec 047): the first URL
 * that answers with an image wins, a miss is remembered so the glyph stays
 * put without a refetch storm, and nothing is drawn until the bytes are here
 * — the fallback is the whole mark, not a blank circle.
 */
/**
 * How long a logo that failed stays failed (spec 082 RE10, W20) — the core's
 * rule (`markMissTtlMs`): a missing, refused or undrawable image for the
 * session; a throttled, broken or unreached one for a minute, and until the
 * network comes back. A miss is remembered at all because a token with no
 * logo would otherwise be fetched on every scroll.
 */
object LogoMisses {
    /** The session: a miss that will not heal by asking again. */
    private const val SESSION = Long.MAX_VALUE

    private val until = java.util.concurrent.ConcurrentHashMap<String, Long>()

    /** Whether [url] is still a miss at [nowMs]; an expired one is forgotten. */
    fun missed(url: String, nowMs: Long = System.currentTimeMillis()): Boolean {
        val expiry = until[url] ?: return false
        if (nowMs < expiry) return true
        until.remove(url, expiry)
        return false
    }

    /**
     * [status]: the HTTP status the fetch answered with (a 2xx whose bytes do
     * not draw included); `null` when no answer came — [kind] names why.
     */
    fun record(url: String, status: Int?, kind: String = TRANSPORT, nowMs: Long = System.currentTimeMillis()) {
        val ttl = uniffi.vela_core_uniffi.markMissTtlMs(kind, status?.takeIf { it in 0..0xFFFF }?.toUShort())
        until[url] = ttl?.let { nowMs + it.toLong() } ?: SESSION
    }

    /** The network came back: every miss that may heal is asked for again. */
    fun clearTransient() {
        until.entries.removeIf { it.value != SESSION }
    }

    fun clear() = until.clear()

    const val TRANSPORT = "transport"
}

object LogoStore {
    private val memory = object : LruCache<String, Bitmap>(96) {}

    @Volatile
    private var client: OkHttpClient? = null

    // Derived from the one shared read client (SC-107: nothing else opens a
    // socket), with a disk cache so a relaunch draws yesterday's logos before
    // the network answers.
    private fun client(context: Context): OkHttpClient = client ?: synchronized(this) {
        client ?: VelaHttp.client.newBuilder()
            .cache(Cache(File(context.cacheDir, "logos"), 16L * 1024 * 1024))
            .callTimeout(8, TimeUnit.SECONDS)
            .build()
            .also { client = it }
    }

    fun cached(url: String): Bitmap? = memory.get(url)

    suspend fun load(context: Context, url: String): Bitmap? {
        memory.get(url)?.let { return it }
        if (LogoMisses.missed(url)) return null
        return withContext(Dispatchers.IO) {
            // The status rides along: what the miss is decides how long it lasts.
            val fetched = runCatching {
                client(context.applicationContext).newCall(Request.Builder().url(url).build()).execute().use { response ->
                    response.code to (if (response.isSuccessful) response.body?.bytes() else null)
                }
            }.getOrNull()
            val bitmap = fetched?.second?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
            if (bitmap == null) LogoMisses.record(url, fetched?.first) else memory.put(url, bitmap)
            bitmap
        }
    }

    /** Tests and the erase-device path: forget everything held in memory. */
    fun clear() { memory.evictAll(); LogoMisses.clear() }
}

@Composable
fun RemoteLogo(
    urls: List<String>,
    size: Dp,
    modifier: Modifier = Modifier,
    fallback: @Composable () -> Unit,
) {
    if (urls.isEmpty()) { fallback(); return }
    val context = LocalContext.current
    // Keyed on the URLs, state included (issue #267). `produceState(initial, urls)`
    // keeps its state across a key change and only restarts the producer — so a
    // row re-used for another token (a network filter switched under a list)
    // still held the previous token's bitmap, and the producer's "already have
    // one" check kept it: BNB drawn with the ETH logo until something else
    // recomposed the row. A new set of URLs starts from ITS cache or the glyph.
    val state = remember(urls) { mutableStateOf(urls.firstNotNullOfOrNull(LogoStore::cached)) }
    LaunchedEffect(urls) {
        if (state.value != null) return@LaunchedEffect
        for (url in urls) {
            val loaded = LogoStore.load(context, url)
            if (loaded != null) { state.value = loaded; return@LaunchedEffect }
        }
    }
    val image = state.value
    if (image == null) {
        fallback()
    } else {
        Image(
            bitmap = image.asImageBitmap(),
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = modifier.size(size).clip(CircleShape),
        )
    }
}
