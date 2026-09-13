package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.tokens.VelaBrand

/**
 * In-app sailboat mark, built from the three paths of
 * docs/design/onboarding/logo-{dark,light}.svg (viewBox 258×260). Sails are identical
 * in both modes; the hull is themed (VelaBrand). Never given an app-icon
 * background (brand rule).
 */
@Composable
fun VelaLogo(
    darkTheme: Boolean,
    modifier: Modifier = Modifier,
    contentDescription: String? = null,
) {
    val hull = if (darkTheme) VelaBrand.hullOnDark else VelaBrand.hullOnLight
    val vector = remember(hull) { velaMark(hull) }
    Image(
        imageVector = vector,
        contentDescription = contentDescription,
        modifier = modifier,
    )
}

private fun velaMark(hull: Color): ImageVector =
    ImageVector.Builder(
        name = "VelaMark",
        defaultWidth = 56.dp,
        defaultHeight = 56.dp,
        viewportWidth = 258f,
        viewportHeight = 260f,
    ).apply {
        // Main sail
        path(fill = SolidColor(VelaBrand.sailMain)) {
            moveTo(122f, 0f)
            curveTo(70f, 53f, 38f, 118f, 18f, 187f)
            lineTo(122f, 187f)
            close()
        }
        // Secondary sail
        path(fill = SolidColor(VelaBrand.sailSoft)) {
            moveTo(142f, 42f)
            curveTo(193f, 75f, 225f, 128f, 240f, 187f)
            lineTo(142f, 187f)
            close()
        }
        // Hull (themed)
        path(fill = SolidColor(hull)) {
            moveTo(0f, 207f)
            lineTo(258f, 207f)
            curveTo(243f, 240f, 211f, 260f, 165f, 260f)
            lineTo(92f, 260f)
            curveTo(49f, 260f, 16f, 240f, 0f, 207f)
            close()
        }
    }.build()
