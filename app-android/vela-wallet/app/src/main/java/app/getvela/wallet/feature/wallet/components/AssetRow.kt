package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.core.marks.RemoteLogo
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.AssetRowModel

/** Masked fiat line glyphs (presentation of the Masked variant, mock H5). */
private const val FIAT_MASK = "••••"

/**
 * Token icon (spec vocabulary #10): circular lettermark glyph on bg.sunken with
 * a bottom-trailing chain-dot badge.
 */
@Composable
fun TokenIcon(
    ticker: String,
    badgeColor: Color,
    modifier: Modifier = Modifier,
    /** Spec 047: the web's logo rules — candidates in order, the badge's logo, the badge hidden when it repeats the token. */
    logoUrls: List<String> = emptyList(),
    badgeLogoUrl: String? = null,
    badgeHidden: Boolean = false,
    /**
     * Spec 021: `inline` is the mark inside a line of text — the fee row's fee
     * token, a fact row's network, a notice banner's chain. A size PROP and not
     * a scaled wrapper: the glyph has to shrink with the circle, and scaling
     * only the box clips a three-letter ticker out of it.
     */
    inline: Boolean = false,
) {
    val colors = VelaTheme.colors
    val circle = if (inline) VelaIconSize.xl else WalletMetrics.avatarSize
    Box(modifier = modifier, contentAlignment = Alignment.BottomEnd) {
        // Spec 047: the logo from the chain-data endpoint when it answers; the
        // drawn ticker glyph is the whole fallback (the web's TokenIcon).
        RemoteLogo(urls = logoUrls, size = circle) {
            Box(
                modifier = Modifier
                    .size(circle)
                    .background(colors.bgSunken, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = ticker.take(3).uppercase(),
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    // Two thirds of the row glyph, which keeps a three-letter
                    // ticker inside the smaller circle.
                    fontSize = if (inline) VelaTextSize.xs * 0.66f else VelaTextSize.xs,
                    maxLines = 1,
                )
            }
        }
        // The inline mark carries no chain dot: at that diameter the dot is a
        // few pixels of colour on an already-crowded glyph, and the row it sits
        // in has said which chain this is. The badge is also hidden when it
        // would repeat the token (ETH on Ethereum, XDAI on Gnosis) — the
        // founder's ruling of 2026-09-05, the web's `balanceTokenBadgeChainId`.
        if (!inline && !badgeHidden) ChainBadge(color = badgeColor, logoUrl = badgeLogoUrl)
    }
}

/** The same icon from a flow's mark model. */
@Composable
fun TokenIcon(mark: TokenMarkModel, modifier: Modifier = Modifier, inline: Boolean = false) = TokenIcon(
    ticker = mark.ticker,
    badgeColor = mark.badgeColor,
    modifier = modifier,
    logoUrls = mark.logoUrls,
    badgeLogoUrl = mark.badgeLogoUrl,
    badgeHidden = mark.badgeHidden,
    inline = inline,
)

/**
 * Asset row (spec vocabulary #9): TokenIcon, ticker + chain name, trailing
 * balance + fiat line. Variants: no-price (orange 无价格, mock H4), masked
 * (both lines dotted, mock H5), long-value truncation (H7).
 */
@Composable
fun AssetRow(
    model: AssetRowModel,
    modifier: Modifier = Modifier,
    /**
     * Spec 021 SD1b: the row is off the network the multi-send is locked to.
     * Still readable and still there — it is a token the person owns — but not
     * selectable, and saying so by weight rather than by hiding it.
     */
    dimmed: Boolean = false,
    /** Spec 021 SD1b: chosen for a multi-token send. */
    selected: Boolean = false,
    /** Spec 021 SD2d: a trailing control (Max) after the numbers. */
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(
                if (selected) {
                    // The whole row lifts rather than growing a checkbox: the
                    // list IS the selection, and a column of empty boxes down
                    // the leading edge would push the token marks off the
                    // margin every other screen aligns to.
                    Modifier
                        .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                        .padding(horizontal = VelaSpacing.md)
                } else {
                    Modifier
                }
            )
            .then(
                if (onClick != null && !dimmed) Modifier.clickable(onClick = onClick) else Modifier
            )
            .alpha(if (dimmed) VelaOpacity.disabled else 1f)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TokenIcon(ticker = model.ticker, badgeColor = model.badgeColor, logoUrls = model.logoUrls, badgeLogoUrl = model.badgeLogoUrl, badgeHidden = model.badgeHidden)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = model.ticker,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = model.chain,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(
            modifier = Modifier.weight(1f),
            horizontalAlignment = Alignment.End,
        ) {
            Text(
                text = model.balance,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                textAlign = TextAlign.End,
            )
            when (val fiat = model.fiat) {
                is AssetFiatModel.Value -> Text(
                    text = fiat.text,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    textAlign = TextAlign.End,
                )
                is AssetFiatModel.NoPrice -> Text(
                    text = fiat.text,
                    color = colors.warningBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    textAlign = TextAlign.End,
                )
                AssetFiatModel.Masked -> Text(
                    text = FIAT_MASK,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    textAlign = TextAlign.End,
                )
                AssetFiatModel.None -> Unit
            }
        }
        if (trailing != null) {
            Spacer(modifier = Modifier.width(VelaSpacing.lg))
            trailing()
        }
    }
}
