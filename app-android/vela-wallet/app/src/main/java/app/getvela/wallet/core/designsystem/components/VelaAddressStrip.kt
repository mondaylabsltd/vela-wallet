package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.background
import app.getvela.wallet.core.platform.Clipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import kotlinx.coroutines.delay

/**
 * Copyable wallet-address strip (spec 014, A11): sunken full-width row, single
 * line with tail truncation, trailing copy affordance. Activating it copies
 * the FULL untruncated address and shows a transient "copied" confirmation
 * (icon swap + a11y label swap). Clipboard is the only I/O this component —
 * and this feature — is allowed.
 */
@Composable
fun VelaAddressStrip(
    address: String,
    copyLabel: String,
    copiedLabel: String,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    val context = LocalContext.current
    var copied by remember(address) { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            // Visual confirmation hold only — not a retry/backoff timer (FR-011).
            delay(VelaMotion.copiedFeedbackHold.toLong())
            copied = false
        }
    }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .clickable(role = Role.Button, onClickLabel = copyLabel) {
                if (Clipboard.copy(context, "address", address)) copied = true
            }
            .heightIn(min = VelaSizing.controlLg)
            .padding(horizontal = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = address,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.regular,
            fontSize = VelaTextSize.base,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Icon(
            imageVector = if (copied) VelaIcons.Check else VelaIcons.Copy,
            contentDescription = if (copied) copiedLabel else copyLabel,
            tint = if (copied) colors.successBase else colors.fgMuted,
            modifier = Modifier.size(VelaIconSize.md),
        )
    }
}
