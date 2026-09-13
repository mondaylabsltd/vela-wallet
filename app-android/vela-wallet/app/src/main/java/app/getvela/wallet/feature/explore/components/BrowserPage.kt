package app.getvela.wallet.feature.explore.components

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import app.getvela.wallet.feature.browser.core.BrowserEngine

/**
 * The page (spec 044): the tab's engine, hosted where the drawings put the
 * demo page. Leaving 探索 leaves this composable, so nothing of the page is
 * ever painted over the wallet; the engine itself lives on in the controller.
 */
@Composable
fun BrowserPage(engine: BrowserEngine, modifier: Modifier = Modifier) {
    AndroidView(
        modifier = modifier.fillMaxSize(),
        factory = { engine.webView },
        onReset = {},
        onRelease = { view -> (view.parent as? android.view.ViewGroup)?.removeView(view) },
    )
}
