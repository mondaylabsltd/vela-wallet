package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaStatusBadge
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.PromptKind

/**
 * The one modal in the v2 flow: an interruption the person has to answer.
 *
 * The whole journey is a full screen; only FAILURES are modal, because a failure
 * genuinely does stop everything until it is acknowledged. A form someone is
 * halfway through is not an interruption, which is why spec 014's sheet — which
 * held the entire create flow — is gone.
 *
 * `confirmable` is the core's word for "this answer changes the flow". The
 * recovery offer is the only prompt where declining is a decision rather than a
 * dismissal, and it is the only one that gets two real buttons. Every other
 * prompt has one, because dismissing it and answering it are the same act.
 *
 * **A dismissal is always `accepted = false`.** Swiping the sheet away must
 * reach the core as a refusal, or a machine waiting on a `prompt_answered`
 * hangs with nothing on screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FlowSheet(kind: PromptKind, confirmable: Boolean, onAnswer: (Boolean) -> Unit) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val copy = promptCopy(kind, strings)

    VelaModalSheet(
        onDismissRequest = { onAnswer(false) },
        containerColor = colors.bgRaised,
        dragHandle = {
            Box(
                modifier = Modifier
                    .padding(vertical = VelaSpacing.lg)
                    .size(
                        width = VelaSizing.sheetHandleWidth,
                        height = VelaSizing.sheetHandleHeight,
                    )
                    .clip(RoundedCornerShape(VelaRadius.full))
                    .background(colors.borderStrong),
            )
        },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .imePadding()
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
        ) {
            VelaStatusBadge(variant = badgeFor(kind.type))
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Text(
                text = copy.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = copy.message,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.lg,
                lineHeight = VelaLeading.normal * VelaTextSize.lg,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl4))

            if (confirmable && copy.confirmable) {
                VelaPrimaryButton(
                    text = copy.confirmLabel.orEmpty(),
                    onClick = { onAnswer(true) },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                VelaSecondaryButton(
                    text = copy.cancelLabel.orEmpty(),
                    onClick = { onAnswer(false) },
                    modifier = Modifier.fillMaxWidth(),
                )
            } else {
                VelaPrimaryButton(
                    text = strings.t(I18nKeys.Flow.CLOSE),
                    onClick = { onAnswer(false) },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

/**
 * The badge each prompt wears.
 *
 * Spec 014 had eighteen `OutcomeKind` values and this feature does not reduce
 * them so much as RELOCATE them: eight of the eighteen are no longer sheets
 * because v2 gave them somewhere better — `Created` is the Done screen,
 * `SignedIn` is the wallet, `SyncFailed` is the Retry screen with the key list
 * intact, `VerifyStuck` is the Name screen with a changed submit label, the
 * three cancellations are the Name screen's quiet status line, and
 * `AccountNotFound` arrives as a `sign_in_failed` prompt carrying the registry's
 * own words. `deviations.md` carries the full table.
 *
 * What is left is what a sheet is for: nine prompt kinds, each an interruption.
 */
private fun badgeFor(type: String): app.getvela.wallet.core.designsystem.components.BadgeVariant =
    when (type) {
        "recover_offer" -> app.getvela.wallet.core.designsystem.components.BadgeVariant.Info
        "not_discoverable" -> app.getvela.wallet.core.designsystem.components.BadgeVariant.Warning
        else -> app.getvela.wallet.core.designsystem.components.BadgeVariant.Error
    }
