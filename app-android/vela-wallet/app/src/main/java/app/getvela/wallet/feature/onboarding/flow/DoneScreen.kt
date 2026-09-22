package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.background
import app.getvela.wallet.core.platform.Clipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaAddressStrip
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.passkey.PasskeyDirectory
import app.getvela.wallet.core.passkey.PasskeyProviderMark
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import kotlinx.coroutines.delay

/**
 * The wallet exists.
 *
 * This is the first moment an address is shown, and that ordering is a rule
 * rather than a layout choice: the core withholds `address` until the group has
 * landed and the account is saved, because an address shown earlier is an
 * address someone can fund before the wallet is reachable.
 *
 * The identicon is rendered from the address by the same core that derived it,
 * so what the person memorises here is what every other client draws.
 */
@Composable
fun ColumnScope.DoneScreen(
    address: String,
    walletName: String,
    keys: List<CreateKeyRow>,
    /**
     * The hand-off is in flight.
     *
     * `Completing` reaches the view as `Created` (the core collapses the pair),
     * so this flag is the ONLY thing that distinguishes "waiting for you" from
     * "already going". Without it the button stays live and unchanged through a
     * transition it has already started, which reads as a dead button and gets
     * tapped again — and every tap after the first is a silent no-op, because
     * the core's guard only answers in `Created`.
     */
    entering: Boolean,
    onEnter: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    Column(
        modifier = Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            // The tick is a badge, not a glyph beside the words: this is the one
            // screen in the flow that reports an outcome, and the design gives
            // it a disc.
            Box(
                modifier = Modifier
                    .size(VelaSizing.doneCheck)
                    .clip(RoundedCornerShape(VelaRadius.full))
                    .background(colors.successSoft),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.successBase,
                    modifier = Modifier.size(VelaIconSize.base),
                )
            }
            Spacer(modifier = Modifier.size(VelaSpacing.lg))
            Text(
                text = strings.t(I18nKeys.Create.SUCCESS_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            text = strings.t(
                I18nKeys.Create.SUCCESS_MESSAGE,
                mapOf("count" to keys.size.toString()),
            ),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
        )

        Spacer(modifier = Modifier.height(VelaSpacing.xl4))

        // Avatar beside the name, then the address under a rule. The caption
        // that used to sit here DESCRIBED the identicon ("an identity pattern
        // generated from the address") — a sentence narrating a picture that is
        // right next to it — and the "wallet address" label went for the same
        // reason: a 42-character 0x string in mono under a wallet's name is not
        // mistakable for anything else.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(VelaRadius.lg))
                .background(colors.bgRaised)
                .padding(VelaSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IdenticonImage(seed = address, size = VelaSizing.doneAvatar)
                Spacer(modifier = Modifier.size(VelaSpacing.lg))
                Text(
                    text = walletName,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl,
                )
            }

            DoneAddress(
                address = address,
                copyLabel = strings.t(I18nKeys.Flow.COPY_ADDRESS),
                copiedLabel = strings.t(I18nKeys.Flow.COPIED),
            )
        }

        Spacer(modifier = Modifier.height(VelaSpacing.xl3))

        keys.forEach { key ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val holder = key.providerName.ifEmpty {
                    PasskeyDirectory.holder(key.aaguid, VelaTheme.isDark)?.name.orEmpty()
                }
                val drewMark = PasskeyProviderMark(
                    key = key,
                    label = holder,
                    size = VelaIconSize.lg,
                )
                if (drewMark) {
                    Spacer(modifier = Modifier.width(VelaSpacing.lg))
                }
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = key.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    // Where the key lives, under the name it was given. Quieter
                    // than the name: one is the person's word for it, the other
                    // is the system's.
                    if (holder.isNotEmpty()) {
                        Text(
                            text = holder,
                            color = colors.fgSubtle,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                        )
                    }
                }
                Text(
                    text = strings.t(
                        if (key.synced) {
                            I18nKeys.Create.KEY_SYNCED_BADGE
                        } else {
                            I18nKeys.Create.KEY_DEVICE_ONLY_BADGE
                        },
                    ),
                    color = if (key.synced) colors.successBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.xs,
                )
            }
        }

        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
        Text(
            text = strings.t(I18nKeys.Create.VERIFY_HINT),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    }

    VelaPrimaryButton(
        text = strings.t(I18nKeys.Create.ENTER_WALLET_BTN),
        onClick = onEnter,
        modifier = Modifier.fillMaxWidth(),
        enabled = !entering,
        loading = entering,
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
}

/**
 * The done card's address: the WHOLE line is the copy target, and the
 * confirmation replaces it in place.
 *
 * Not [VelaAddressStrip]: the v2 card draws the address as bare text under a
 * rule, because it is the only 0x string on the screen and a sunken well around
 * it made the card look like a form.
 */
@Composable
private fun DoneAddress(address: String, copyLabel: String, copiedLabel: String) {
    val colors = VelaTheme.colors
    val context = LocalContext.current
    var copied by remember { mutableStateOf(false) }

    LaunchedEffect(copied) {
        if (copied) {
            delay(COPIED_FEEDBACK_MS)
            copied = false
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.sm))
            .clickable {
                if (Clipboard.copy(context, "address", address)) copied = true
            }
            .semantics { contentDescription = copyLabel },
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(VelaBorder.hairline)
                .background(colors.borderBase),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = if (copied) copiedLabel else address,
            color = if (copied) colors.successBase else colors.fgMuted,
            fontFamily = VelaMonoFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.base,
            lineHeight = VelaLeading.normal * VelaTextSize.base,
        )
    }
}

/** How long "copied" stands in for the address. */
private const val COPIED_FEEDBACK_MS = 2_000L
