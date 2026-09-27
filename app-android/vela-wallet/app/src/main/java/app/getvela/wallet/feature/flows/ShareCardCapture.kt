package app.getvela.wallet.feature.flows

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.marks.LogoStore
import java.io.ByteArrayOutputStream
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Spec 047 US2: the drawn share card, recorded into a graphics layer and
 * read back as a PNG — the very same composable the gallery draws (R4), so
 * what is shared is what was designed. Composed only while a capture is
 * wanted; invisible (alpha 0), measured unbounded so a narrow phone never
 * squeezes it, at the web's 2× (480 dp → 960 px wide, as tall as the
 * content makes it).
 *
 * The network's logo is fetched FIRST ([shareCardLogo]): a picture is taken
 * once, so a logo that arrives a frame late is a logo that is never in it.
 * Only when it is here — or has had its [LOGO_WAIT_MS] and not come — is the
 * card composed, and it is read back once a frame has actually drawn it.
 */
@Composable
fun ShareCardCapture(model: ShareCardModel, onCaptured: (ByteArray?) -> Unit) {
    val context = LocalContext.current
    val layer = rememberGraphicsLayer()
    val done = rememberUpdatedState(onCaptured)
    // `null` while the logo is on its way; then the logo, or none (the disc).
    var logo by remember(model) { mutableStateOf<Loaded?>(null) }
    val drawn = remember(model) { java.util.concurrent.atomic.AtomicBoolean(false) }
    LaunchedEffect(model) {
        logo = Loaded(shareCardLogo(context, model.chainLogoUrl))
    }
    val ready = logo ?: return
    VelaTheme(darkTheme = false) {
        // Spec 048: the web's card is 480 wide, saved at 2× — the same here.
        CompositionLocalProvider(LocalDensity provides Density(density = 2f, fontScale = 1f)) {
            Box(
                modifier = Modifier
                    .wrapContentSize(align = Alignment.TopStart, unbounded = true)
                    .alpha(0f)
                    .drawWithContent {
                        layer.record { this@drawWithContent.drawContent() }
                        drawLayer(layer)
                        drawn.set(true)
                    },
            ) {
                ShareCardArtwork(model = model, logo = ready.image)
            }
        }
    }
    LaunchedEffect(model, ready) {
        // Read back only after a frame has recorded the card (a fixed delay
        // raced a slow first frame), bounded in wall-clock time — a frame
        // count or a `delay` is virtual under a test clock; one more frame,
        // then the pixels.
        val start = System.nanoTime()
        while (!drawn.get() && System.nanoTime() - start < DRAW_WAIT_NS) withFrameNanos { }
        withFrameNanos { }
        val bytes = runCatching {
            val image = layer.toImageBitmap()
            withContext(Dispatchers.Default) {
                val out = ByteArrayOutputStream()
                image.asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, out)
                out.toByteArray()
            }
        }.getOrNull()
        done.value(bytes)
    }
}

private class Loaded(val image: ImageBitmap?)

/** The network's logo for the card, or `null` for the lettered disc. */
internal suspend fun shareCardLogo(context: Context, url: String?): ImageBitmap? {
    if (url.isNullOrBlank()) return null
    LogoStore.cached(url)?.let { return it.asImageBitmap() }
    // The load runs detached: the store's fetch blocks its thread, so a timeout
    // around it would still wait out the whole call. Here the card stops
    // waiting at the deadline, and a late logo still lands in the cache.
    val pending = logoScope.async { LogoStore.load(context.applicationContext, url) }
    return withTimeoutOrNull(LOGO_WAIT_MS) { pending.await() }?.asImageBitmap()
}

private val logoScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

/** How long the saved card waits for the network's logo before the disc stands in. */
internal const val LOGO_WAIT_MS = 4_000L

/** How long to wait for the card's first draw before reading back regardless. */
private const val DRAW_WAIT_NS = 2_000_000_000L
