package app.getvela.wallet.feature.flows.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic

/** The track: the web's and desktop's 36×20 (`--icon-3xl` × `--icon-lg`). */
private val TRACK_WIDTH = VelaIconSize.xl3
private val TRACK_HEIGHT = VelaIconSize.lg

/**
 * An on/off switch with its label — the receive code's "include network"
 * (spec 090), the product's one switch, drawn as web, desktop and iOS draw it.
 *
 * The whole row is the target, one [VelaHaptic.Select] per flip (a choice that
 * takes effect). A press stretches the thumb toward where it is going — the
 * shape-deform every pressed control here makes. Monochrome on purpose: the
 * accent belongs to the one action that moves money, so "on" is the ink
 * track. [onChange] `null` (the gallery) draws it inert.
 */
@Composable
fun FlowSwitchRow(
    label: String,
    isOn: Boolean,
    onChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val thumbWidth by animateDpAsState(if (pressed) VelaIconSize.lg else VelaIconSize.base, label = "thumb")
    val track by animateColorAsState(if (isOn) colors.fgBase else colors.borderStrong, label = "track")
    Row(
        modifier = modifier
            .fillMaxWidth()
            .testTag("receive.includeNetwork")
            .toggleable(
                value = isOn,
                enabled = onChange != null,
                role = Role.Switch,
                interactionSource = interaction,
                indication = null,
            ) { next ->
                haptic(VelaHaptic.Select)
                onChange?.invoke(next)
            }
            .padding(vertical = VelaSpacing.sm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(
            text = label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.weight(1f),
        )
        Box(
            modifier = Modifier
                .width(TRACK_WIDTH)
                .height(TRACK_HEIGHT)
                .background(track, CircleShape)
                .padding(VelaSpacing.xs),
            contentAlignment = if (isOn) Alignment.CenterEnd else Alignment.CenterStart,
        ) {
            Box(
                modifier = Modifier
                    .width(thumbWidth)
                    .height(VelaIconSize.base)
                    .background(colors.bgBase, CircleShape),
            )
        }
    }
}
