package app.getvela.wallet.feature.explore.components

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import app.getvela.wallet.feature.browser.core.BrowserEngine

/**
 * The page (spec 044): the tab's engine, hosted where the drawings put the
 * demo page. Leaving 探索 leaves this composable, so nothing of the page is
 * ever painted over the wallet; the engine itself lives on in the controller.
 *
 * Keyed by the engine: a tab switch is a different WebView, and an
 * `AndroidView` whose factory already ran would otherwise keep showing the old
 * one. While on screen the engine borrows the activity (spec 070) — a page's
 * `alert()` needs a window — and gives it back when it leaves.
 */
@Composable
fun BrowserPage(engine: BrowserEngine, modifier: Modifier = Modifier) {
    val activity = LocalContext.current
    key(engine) {
        DisposableEffect(engine, activity) {
            engine.attach(activity)
            onDispose { engine.detach() }
        }
        AndroidView(
            modifier = modifier.fillMaxSize(),
            factory = {
                // A WebView is one view: whatever hosted it before lets it go.
                (engine.webView.parent as? android.view.ViewGroup)?.removeView(engine.webView)
                engine.webView
            },
            onReset = {},
            onRelease = { view -> (view.parent as? android.view.ViewGroup)?.removeView(view) },
        )
    }
}
