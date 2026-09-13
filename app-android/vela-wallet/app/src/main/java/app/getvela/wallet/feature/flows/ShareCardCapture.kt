package app.getvela.wallet.feature.flows

import android.graphics.Bitmap
import androidx.compose.ui.unit.Density
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.draw
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import java.io.ByteArrayOutputStream
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

/**
 * Spec 047 US2: the drawn share card, recorded into a graphics layer and
 * read back as a PNG — the very same composable the gallery draws (R4), so
 * what is shared is what was designed. Composed only while a capture is
 * wanted; invisible (alpha 0) and off the layout's flow.
 */
@Composable
fun ShareCardCapture(model: ShareCardModel, onCaptured: (ByteArray?) -> Unit) {
    val layer = rememberGraphicsLayer()
    val done = rememberUpdatedState(onCaptured)
    VelaTheme(darkTheme = false) {
        // Spec 048: the web's card is 480 wide, saved at 2× — the same here.
        CompositionLocalProvider(LocalDensity provides Density(density = 2f, fontScale = 1f)) {
        Box(
            modifier = Modifier
                .width(480.dp)
                .alpha(0f)
                .drawWithContent {
                    layer.record { this@drawWithContent.drawContent() }
                    drawLayer(layer)
                },
        ) {
            ShareCardArtwork(model = model)
        }
        }
    }
    LaunchedEffect(model) {
        // One frame for the layout to settle before the layer is read.
        delay(120)
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
