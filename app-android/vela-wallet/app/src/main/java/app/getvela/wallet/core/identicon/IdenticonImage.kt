package app.getvela.wallet.core.identicon

import android.graphics.BitmapFactory
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.foundation.clickable
import android.util.LruCache
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import uniffi.vela_core_uniffi.identiconNormalizeSeed
import uniffi.vela_core_uniffi.identiconPlaceholderPng
import uniffi.vela_core_uniffi.identiconPng

/**
 * Nimiq identicon rendered by vela-core (spec 015, research.md D1): the shared
 * `identicon_png` rasterization decoded via [BitmapFactory], so the artwork is
 * byte-identical across platforms. Seeds pass through vela-core's
 * `normalize_seed` — never lowercase locally (spec FR-006). Empty/invalid seeds
 * fall back to the shared placeholder artwork.
 *
 * Preview/tooling note: the native library is not loadable in the preview
 * process, so every FFI touch sits behind runCatching and the composable falls
 * back to a plain token-colored circle (same spirit as WelcomePreviews'
 * PreviewStrings fake).
 */
@Composable
fun IdenticonImage(
    seed: String,
    size: Dp,
    modifier: Modifier = Modifier,
    contentDescription: String? = null,
    /**
     * Spec 048 (the founder: 说好了 identicon 要能点击放大): every artwork drawn
     * from an address opens the viewer, the way the web's `Identicon.svelte`
     * is a button. The viewer is hosted once, at the NavHost, and reached
     * through [LocalIdenticonViewer]; a site that manages its own tap, or
     * draws for a picture (the share card), passes `false`.
     */
    tappable: Boolean = true,
) {
    val open = LocalIdenticonViewer.current
    val modifier = if (tappable && open != null && seed.isNotBlank()) {
        modifier.clip(CircleShape).clickable { open(seed) }
    } else {
        modifier
    }
    val density = LocalDensity.current
    val sizePx = with(density) { size.roundToPx() }.coerceAtLeast(1)
    val bitmap = remember(seed, sizePx) { identiconBitmap(seed, sizePx) }
    if (bitmap != null) {
        Image(
            bitmap = bitmap,
            contentDescription = contentDescription,
            modifier = modifier
                .size(size)
                .clip(CircleShape),
        )
    } else {
        // Native engine unavailable (preview/tooling): plain circle, tokens only.
        val fill = VelaTheme.colors.bgSunken
        val outline = VelaTheme.colors.borderStrong
        Canvas(modifier = modifier.size(size)) {
            drawCircle(color = fill)
            drawCircle(color = outline, style = Stroke(width = VelaBorder.hairline.toPx()))
        }
    }
}

/** Decoded-bitmap LRU keyed on normalized seed + pixel size (identicons repeat per row). */
private val identiconCache = LruCache<String, ImageBitmap>(128)

private fun identiconBitmap(seed: String, sizePx: Int): ImageBitmap? = runCatching {
    val normalized = if (seed.isBlank()) "" else identiconNormalizeSeed(seed)
    val key = "$normalized@$sizePx"
    identiconCache.get(key)?.let { return@runCatching it }
    val bytes = if (normalized.isEmpty()) {
        identiconPlaceholderPng(sizePx.toUInt())
    } else {
        runCatching { identiconPng(normalized, sizePx.toUInt()) }
            .getOrElse { identiconPlaceholderPng(sizePx.toUInt()) }
    }
    val decoded = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
        ?: return@runCatching null
    decoded.asImageBitmap().also { identiconCache.put(key, it) }
}.getOrNull()

/** The NavHost-hosted identicon viewer: `seed -> open the sheet`. `null` where nothing hosts one (previews, the gallery). */
val LocalIdenticonViewer = compositionLocalOf<((String) -> Unit)?> { null }
