package app.getvela.wallet.feature.onboarding

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaLogo
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBrand
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * Welcome: brand row, a two-line headline with one supporting sentence, and
 * the two ways in. Long-press on the mark opens the theme settings sheet (RN
 * precedent, FR-006).
 *
 * The v2 design (docs/design/onboarding-new, founder direction 2026-08-25), which
 * the web and the desktop already draw. The six-card carousel is gone: the
 * design is one column that says what the wallet IS before it says what to do
 * about it, and a deck of feature cards nobody swipes past the first of was the
 * opposite of that. The copy block still scrolls when it cannot fit (large font
 * scale, short screens) while the CTA stack stays pinned (US1 AS4).
 *
 * Spec 019 changed what the two CTAs DO. Creating a wallet leaves for a
 * full-screen journey; 我已有钱包 dispatches straight into the login machine —
 * the system passkey sheet is the next thing the person sees, so an
 * intermediate screen of our own would be a screen with nothing on it.
 * [signingIn] is the core's `busy`. The button neither hides nor dims: it turns
 * a spinner in place of its label and stays at full emphasis. A control that
 * vanished while a system dialog was up would read as the app having crashed
 * behind it, and a dimmed one reads as unavailable — which is the one thing
 * "working" must never look like. The passkey sheet is not instant, so this
 * button IS the progress indicator for the wait in front of it.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun WelcomeScreen(
    darkTheme: Boolean,
    onIntent: (OnboardingIntent) -> Unit,
    onLongPressLogo: () -> Unit,
    modifier: Modifier = Modifier,
    signingIn: Boolean = false,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    val entrance = remember {
        MutableTransitionState(false).apply { targetState = true }
    }

    // Two blocks, not one centred stack: brand and copy ride the top edge, the
    // CTAs ride the bottom, and the space between them is whatever the phone
    // has left over.
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .safeDrawingPadding()
            .padding(horizontal = VelaSizing.screenPaddingX),
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState()),
        ) {
            AnimatedVisibility(
                visibleState = entrance,
                enter = fadeIn(tween(VelaMotion.entranceFade)),
            ) {
                Column(modifier = Modifier.fillMaxWidth()) {
                    Spacer(modifier = Modifier.height(VelaSpacing.xl4))
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.combinedClickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = {},
                            onLongClick = onLongPressLogo,
                        ),
                    ) {
                        VelaLogo(
                            darkTheme = darkTheme,
                            contentDescription = VelaBrand.WORDMARK,
                            modifier = Modifier.size(VelaSizing.brandMark),
                        )
                        Spacer(modifier = Modifier.size(VelaSpacing.lg))
                        // A LABEL beside the mark: uppercase, heavy, widely
                        // tracked — not the xl4 display title it was when it
                        // headed a screen of its own.
                        Text(
                            text = VelaBrand.WORDMARK.uppercase(),
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl2,
                            letterSpacing = 0.11.em,
                        )
                    }
                    Spacer(modifier = Modifier.height(VelaSpacing.xl3))
                }
            }

            AnimatedVisibility(
                visibleState = entrance,
                enter = fadeIn(tween(VelaMotion.entranceFadeUp)) +
                    slideInVertically(tween(VelaMotion.entranceFadeUp)) { it / 8 },
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                ) {
                    // The copy carries its own line break: every locale breaks
                    // where its own sentence wants to, not where 390dp runs out.
                    // Its SIZE rides along for the same reason — a line that is
                    // 10.9em wide in Russian and 6.9em in Chinese cannot be set
                    // at one size and still fit 342dp. An unrecognised value
                    // keeps the design's size: a corpus ahead of this build must
                    // not shrink the headline on a string nobody knows.
                    val heroSize = when (strings.t(I18nKeys.Welcome.HERO_TITLE_FIT)) {
                        "long" -> VelaTextSize.heroLong
                        else -> VelaTextSize.hero
                    }
                    Text(
                        text = strings.t(I18nKeys.Welcome.HERO_TITLE),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = heroSize,
                        lineHeight = VelaLeading.hero * heroSize,
                        letterSpacing = (-0.02).em,
                    )
                    Text(
                        text = strings.t(I18nKeys.Welcome.HERO_SUBTITLE),
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.regular,
                        fontSize = VelaTextSize.lg,
                        lineHeight = VelaLeading.normal * VelaTextSize.lg,
                    )
                }
            }

            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
        }

        AnimatedVisibility(
            visibleState = entrance,
            enter = fadeIn(tween(VelaMotion.entranceFadeUp)),
        ) {
            Column {
                VelaPrimaryButton(
                    text = strings.t(I18nKeys.Welcome.CREATE_WALLET),
                    onClick = { onIntent(OnboardingIntent.CreateWallet) },
                    enabled = !signingIn,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                VelaSecondaryButton(
                    text = strings.t(I18nKeys.Welcome.ALREADY_HAVE_WALLET),
                    onClick = { onIntent(OnboardingIntent.RecoverWallet) },
                    loading = signingIn,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
            }
        }
    }
}
