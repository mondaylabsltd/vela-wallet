package app.getvela.wallet.feature.explore.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaLogo
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.ExploreEmptyCopy
import app.getvela.wallet.feature.explore.SiteModel
import app.getvela.wallet.feature.explore.TileModel

/**
 * Explore geometry the token set does not name (spec 022), MEASURED off
 * the mocks in design/explore at the 392×844 design frame.
 */
internal object ExploreMetrics {
    /** Favourites tile avatar (mock E2: x33–88, so 56). */
    val tileAvatar: Dp = VelaSizing.emptyStateCircle
    /** Site-row avatar (mock E2, 最近的 dApp rows). */
    val rowAvatar: Dp = VelaSpacing.xl4 + VelaSpacing.md
    /** Start-page search box (mock E2: y116–163). */
    val searchField: Dp = VelaSpacing.xl5
    /** Browsing address pill and the toolbar under the page (mock E4). */
    val addressPill: Dp = VelaSpacing.xl4 + VelaSpacing.md
    val browserBar: Dp = VelaSizing.emptyStateCircle
    /** The boxed tab count, in the header and in the toolbar (mock E2/E4). */
    val tabCount: Dp = VelaSpacing.xl3 + VelaSpacing.xs
    /** dApp avatar in the signing header, and the chip beside it (mock CS1).  */
    val signingAvatar: Dp = VelaSizing.controlSm
    val networkChip: Dp = VelaSpacing.xl3 + VelaSpacing.xs
    /** Slide-to-confirm: 342×56 track, 48 knob (mock CS1, row y=770). */
    val slideTrack: Dp = VelaSizing.emptyStateCircle
    val slideKnob: Dp = VelaSpacing.xl5
    /** Fraction of the track the knob must cross to commit (SPEC 动效). */
    const val SLIDE_COMMIT = 0.88f
    /** The token mark beside a hero amount, and the identicon in a chip. */
    val tokenMark: Dp = VelaSpacing.xl2 + VelaSpacing.xs
    val chipAvatar: Dp = VelaIconSize.base
}

/**
 * A site or token's mark: its first letter on a wash of its own brand colour.
 * Deliberately NOT a fetched favicon — a wallet that downloads an icon from the
 * site it is about to warn you about has handed that site a tracking pixel and
 * a way to impersonate a brand.
 */
@Composable
fun LetterAvatar(
    letter: String,
    tint: Color,
    modifier: Modifier = Modifier,
    size: Dp = ExploreMetrics.rowAvatar,
    muted: Boolean = false,
) {
    val colors = VelaTheme.colors
    Box(
        modifier = modifier
            .size(size)
            .background(if (muted) colors.bgSunken else tint.copy(alpha = 0.16f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = letter,
            color = if (muted) colors.fgMuted else tint,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl,
            maxLines = 1,
        )
    }
}

/** Favourites tile: the 56 avatar over a label one type step below a row's. */
@Composable
fun SiteTile(tile: TileModel, onOpen: (String) -> Unit, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .clickable {
                onOpen(if (tile is TileModel.Site) tile.site.id else "add")
            }
            .padding(vertical = VelaSpacing.sm),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        when (tile) {
            is TileModel.Site -> {
                LetterAvatar(tile.site.letter, tile.site.tint, size = ExploreMetrics.tileAvatar)
                Text(
                    text = tile.site.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center,
                )
            }

            is TileModel.Add -> {
                Box(
                    modifier = Modifier
                        .size(ExploreMetrics.tileAvatar)
                        .background(colors.bgSunken, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(VelaIcons.Plus, null, tint = colors.fgSubtle)
                }
                Text(
                    text = tile.label,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                )
            }
        }
    }
}

/** A site inside a group: mark, name, blurb, and the recent group's timestamp. */
@Composable
fun SiteRow(site: SiteModel, onOpen: (String) -> Unit, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable { onOpen(site.id) }
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        LetterAvatar(site.letter, site.tint)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = site.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
                maxLines = 1,
            )
            Text(
                text = site.subtitle ?: site.host,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        site.meta?.takeIf { it.isNotEmpty() }?.let {
            Text(
                text = it,
                color = colors.accentBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
    }
}

/**
 * The one input on the start page: a search box and an address bar at once,
 * because "type a name" and "type a URL" are the same act to everyone except a
 * browser engineer.
 */
@Composable
fun ExploreSearchField(
    placeholder: String,
    scanLabel: String,
    onSubmit: (String) -> Unit,
    modifier: Modifier = Modifier,
    /** Issue #273: the scan icon opens the scanner (a web address's code); without one it submits. */
    onScan: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    var text by remember { mutableStateOf("") }
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Row(
            modifier = Modifier
                .weight(1f)
                .height(ExploreMetrics.searchField)
                .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                .padding(horizontal = VelaSpacing.xl),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Icon(VelaIcons.Search, null, tint = colors.fgSubtle)
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (text.isEmpty()) {
                    Text(
                        text = placeholder,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.lg,
                        maxLines = 1,
                    )
                }
                BasicTextField(
                    value = text,
                    onValueChange = { text = it },
                    singleLine = true,
                    // Spec 044: the keyboard's Go submits the address, as the scan icon's tap does.
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Go),
                    keyboardActions = androidx.compose.foundation.text.KeyboardActions(onGo = { onSubmit(text) }),

                    cursorBrush = SolidColor(colors.accentBase),
                    textStyle = TextStyle(
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.lg,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
        Box(
            modifier = Modifier
                .size(VelaSizing.hitTarget)
                .clickable { onScan?.invoke() ?: onSubmit(text) },
            contentAlignment = Alignment.Center,
        ) {
            Icon(VelaIcons.ScanLine, scanLabel, tint = colors.fgMuted)
        }
    }
}

/** The start page with nothing on it yet (mock E1). */
@Composable
fun ExploreEmpty(copy: ExploreEmptyCopy, onBrowse: () -> Unit, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.xl5),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaLogo(darkTheme = VelaTheme.isDark, modifier = Modifier.size(VelaSizing.emptyStateCircle))
        Text(
            text = copy.title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
        )
        Text(
            text = copy.caption,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(VelaSpacing.sm))
        Box(
            modifier = Modifier
                .height(VelaSizing.controlLg)
                .border(VelaBorder.hairline, colors.borderStrong, CircleShape)
                .clickable(onClick = onBrowse)
                .padding(horizontal = VelaSpacing.xl4),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = copy.cta,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
        }
    }
}
