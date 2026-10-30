package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * Deriving the address.
 *
 * Three task rows, driven by the stage the core reported — never by elapsed
 * time. A bar that advances on a timer tells the person something the wallet
 * does not know, and the moment they are most owed the truth is while their key
 * set is being frozen. This is why spec 014's elapsed-seconds ring is gone from
 * the create flow.
 *
 * The percentage meter that used to head the rows is gone too (founder call,
 * 2026-08-25, and the desktop had already reached the same conclusion): it
 * LOOKED measured and was not — the same three statuses the rows below name,
 * divided by three — and its label named one phase while another was running.
 * What is left is honest: what finished, what is running, what has not started.
 *
 * The running row SPINS. Every one of these three waits on something outside
 * the app (a passkey prompt, a derivation, a network write), and a still dot
 * says nothing about whether anything is still happening.
 */
@Composable
fun ColumnScope.ProgressScreen(position: ProgressPosition, keyCount: Int) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    Column(modifier = Modifier.weight(1f).fillMaxWidth()) {
        Text(
            text = strings.t(I18nKeys.Create.PROGRESS_TITLE),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl3,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            text = strings.t(
                I18nKeys.Create.PROGRESS_SUBTITLE,
                mapOf("count" to keyCount.toString()),
            ),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
            lineHeight = VelaLeading.normal * VelaTextSize.lg,
        )

        Spacer(modifier = Modifier.height(VelaSpacing.xl4))

        PROGRESS_TASKS.forEachIndexed { index, task ->
            val done = index < position.activeTask
            val active = index == position.activeTask
            Row(
                modifier = Modifier.fillMaxWidth().height(VelaSizing.hitTarget),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(modifier = Modifier.size(VelaIconSize.lg), contentAlignment = Alignment.Center) {
                    when {
                        done -> Icon(
                            imageVector = VelaIcons.Check,
                            contentDescription = null,
                            tint = colors.successBase,
                            modifier = Modifier.size(VelaIconSize.base),
                        )
                        active -> CircularProgressIndicator(
                            color = colors.fgBase,
                            strokeWidth = VelaBorder.emphasis,
                            modifier = Modifier.size(VelaIconSize.base),
                        )
                        else -> Box(
                            modifier = Modifier
                                .size(VelaIconSize.xs)
                                .clip(RoundedCornerShape(VelaRadius.full))
                                .background(colors.borderStrong),
                        )
                    }
                }
                Spacer(modifier = Modifier.size(VelaSpacing.lg))
                Text(
                    text = strings.t(task),
                    // The row before the live one is finished, the row after has
                    // not started. Neither is emphasised: only what is happening
                    // right now is.
                    color = if (active) colors.fgBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = if (active) VelaFontWeight.semibold else VelaFontWeight.regular,
                    fontSize = VelaTextSize.lg,
                )
            }
        }
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
}
