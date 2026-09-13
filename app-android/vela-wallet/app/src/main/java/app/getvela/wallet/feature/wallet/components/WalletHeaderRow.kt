package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.WalletHeaderModel

/**
 * Wallet header (spec vocabulary #2): IdenticonAvatar + truncating name +
 * disclosure chevron + middle-truncated address.
 *
 * The trailing NetworkFilterPill is gone (founder call, 2026-08-26): it cost
 * the name and the address the width they needed, and a wallet whose name reads
 * "kimik3 ·…" is a header that has stopped doing its job. The header now owns
 * the row.
 *
 * [onIdenticon] makes the artwork its own control. It answers a different
 * question from the name — "is this the account I think it is?" — and the
 * founder's call is that it answers it wherever the artwork is drawn.
 */
@Composable
fun WalletHeaderRow(
    header: WalletHeaderModel,
    modifier: Modifier = Modifier,
    onIdenticon: (() -> Unit)? = null,
    identiconLabel: String? = null,
    /** Spec 047: the name line with its chevron opens the account switcher (the web's header switcher, 028 phase 9). */
    onSwitcher: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onIdenticon != null) {
            IdenticonAvatar(
                tappable = false,
                seed = header.identiconSeed,
                name = header.name,
                contentDescription = identiconLabel ?: header.name,
                modifier = Modifier
                    .clip(CircleShape)
                    .clickable(onClick = onIdenticon),
            )
        } else {
            IdenticonAvatar(seed = header.identiconSeed, name = header.name, contentDescription = header.name, tappable = false)
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = if (onSwitcher != null) Modifier.clickable(onClick = onSwitcher) else Modifier,
            ) {
                Text(
                    text = header.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.ChevronDown,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
            Text(
                text = header.addressDisplay,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
