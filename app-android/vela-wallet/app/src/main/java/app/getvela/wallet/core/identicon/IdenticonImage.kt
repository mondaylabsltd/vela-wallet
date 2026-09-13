package app.getvela.wallet.core.identicon

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
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
 * Spec 049: the person's 头像样式 decides what is drawn. `identicon` is the
 * address's own artwork — the anti-forgery mark, and the default for that
 * reason; `initials` is a letter of [name] on an accent disc, the web's
 * `avatarSvgForClient` branch. The style is read from [LocalAvatarStyle], so
 * a change in Settings redraws every avatar on screen; the preference was
 * stored and ticked for two specs while nothing read it.
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
    /** The name shown beside the artwork; the initials style takes its first letter (`V` without one). */
    name: String? = null,
) {
    val open = LocalIdenticonViewer.current
    val modifier = if (tappable && open != null && seed.isNotBlank()) {
        modifier.clip(CircleShape).clickable { open(seed, name) }
    } else {
        modifier
    }
    if (LocalAvatarStyle.current == AvatarStyle.INITIALS) {
        InitialsDisc(letter = initialsLetter(name), size = size, modifier = modifier, contentDescription = contentDescription)
        return
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

/** The two avatar styles, as the preference spells them (`vela.avatarStyle`). */
object AvatarStyle {
    const val INITIALS = "initials"
    const val IDENTICON = "identicon"
}

/** The chosen avatar style, provided once at the root from the preferences; the default is the artwork. */
val LocalAvatarStyle = compositionLocalOf { AvatarStyle.IDENTICON }

/**
 * A letter on an accent disc — the web's `initialsSvg`: `--color-accent-soft`
 * under a bold `--color-accent-base` letter at 34/100 of the diameter. The
 * letter is sized from the disc, not from the text scale, so a 40dp disc and
 * a 96dp hero keep the same proportion the SVG has.
 */
@Composable
fun InitialsDisc(
    letter: String,
    size: Dp,
    modifier: Modifier = Modifier,
    contentDescription: String? = null,
) {
    val colors = VelaTheme.colors
    val fontSize = with(LocalDensity.current) { (size * INITIALS_RATIO).toSp() }
    val described = if (contentDescription != null) {
        modifier.semantics { this.contentDescription = contentDescription }
    } else {
        modifier
    }
    Box(
        modifier = described
            .size(size)
            .background(colors.accentSoft, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = letter,
            color = colors.accentBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = fontSize,
            maxLines = 1,
            softWrap = false,
        )
    }
}

private const val INITIALS_RATIO = 0.34f

/** An address-looking "name" (`0x88cC…6894`, a split row without a contact) is not a name. */
private val ADDRESS_LIKE = Regex("^0[xX][0-9a-fA-F]{2,}")

/**
 * The initials style's letter: the first character of the name, upper-cased —
 * `V` when there is no name to take one from (the web's fallback). One code
 * point, not one UTF-16 unit, so a name that starts with an emoji or a
 * supplementary-plane character keeps it whole.
 */
fun initialsLetter(name: String?): String {
    val trimmed = name?.trim().orEmpty()
    if (trimmed.isEmpty() || ADDRESS_LIKE.containsMatchIn(trimmed)) return "V"
    return String(Character.toChars(trimmed.codePointAt(0))).uppercase()
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

/**
 * The NavHost-hosted identicon viewer: `(seed, name) -> open the sheet`. The
 * name rides along so the viewer draws the same disc the person tapped in the
 * initials style. `null` where nothing hosts one (previews, the gallery).
 */
val LocalIdenticonViewer = compositionLocalOf<((String, String?) -> Unit)?> { null }
