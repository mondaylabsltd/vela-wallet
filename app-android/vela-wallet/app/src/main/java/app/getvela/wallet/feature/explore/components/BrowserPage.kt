package app.getvela.wallet.feature.explore.components

import android.os.SystemClock
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.browser.core.BrowserEngine
import kotlinx.coroutines.delay

/**
 * The page (spec 044): the tab's engine, hosted where the drawings put the
 * demo page. Leaving 探索 leaves this composable, so nothing of the page is
 * ever painted over the wallet; the engine itself lives on in the controller.
 *
 * Keyed by the engine: a tab switch is a different WebView, and an
 * `AndroidView` whose factory already ran would otherwise keep showing the old
 * one. While on screen the engine borrows the activity (spec 070) — a page's
 * `alert()` needs a window — and gives it back when it leaves.
 *
 * [cover]: the page as it last left the screen (the switcher's snapshot).
 * A WebView put back on screen paints nothing — white — until its renderer
 * has a frame for it again: about 100 ms on the Xiaomi when a live tab was
 * resumed from the home (spec 099 navigation). The snapshot sits over it
 * until the WebView says its content is ready for the next draw
 * ([BrowserEngine.whenDrawn]), and never longer than [COVER_MAX_MS].
 */
@Composable
fun BrowserPage(engine: BrowserEngine, modifier: Modifier = Modifier, cover: ImageBitmap? = null) {
    val activity = LocalContext.current
    key(engine) {
        // Decided once per showing: a snapshot taken later (as this page leaves) covers nothing.
        val shown = remember { cover }
        var covered by remember { mutableStateOf(shown != null) }
        DisposableEffect(engine, activity) {
            engine.attach(activity)
            if (covered) {
                val since = SystemClock.uptimeMillis()
                val asked = engine.whenDrawn {
                    if (covered) VelaLog.event("browser.page", "cover lifted", "by" to "drawn", "ms" to (SystemClock.uptimeMillis() - since))
                    covered = false
                }
                if (!asked) covered = false
            }
            onDispose { engine.detach() }
        }
        if (covered) {
            LaunchedEffect(Unit) {
                delay(COVER_MAX_MS)
                if (covered) VelaLog.event("browser.page", "cover lifted", "by" to "timeout", "ms" to COVER_MAX_MS)
                covered = false
            }
        }
        Box(modifier.fillMaxSize()) {
            AndroidView(
                // Clipped to its own slot (087 F21). Compose hosts the WebView in a
                // holder that does NOT clip its child, and draws that holder straight
                // into the screen's canvas; a WebView with no frame yet — a slow
                // first load, before anything commits — paints its background with
                // `drawColor`, which fills the whole clip. The page's white went over
                // the address bar for the twenty seconds a stalled site took to fail.
                modifier = Modifier.fillMaxSize().clipToBounds(),
                factory = {
                    // A WebView is one view: whatever hosted it before lets it go.
                    (engine.webView.parent as? android.view.ViewGroup)?.removeView(engine.webView)
                    engine.webView
                },
                onReset = {},
                onRelease = { view -> (view.parent as? android.view.ViewGroup)?.removeView(view) },
            )
            if (covered && shown != null) {
                // The snapshot is the page scaled to a card's width from its top:
                // the same geometry, filled back out to the page's width.
                Image(
                    bitmap = shown,
                    contentDescription = null,
                    contentScale = ContentScale.FillWidth,
                    alignment = Alignment.TopCenter,
                    modifier = Modifier.fillMaxSize().clipToBounds(),
                )
            }
        }
    }
}

/** The longest a snapshot covers a page put back on screen, should the WebView never say it drew. */
private const val COVER_MAX_MS = 400L
