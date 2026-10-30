package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import app.getvela.wallet.core.designsystem.components.ACK_LINK_TAG
import app.getvela.wallet.core.designsystem.components.VelaAckRow
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaTextField
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings

/**
 * Name the wallet, and accept the three gates.
 *
 * Three checkboxes, matching the core's `ACK_COUNT`, and every one of them a
 * FACT ABOUT WHERE SOMETHING ENDS UP: the public key and the name go into the
 * on-chain contract, the private key stays in the device's password manager or
 * on a security key, and the legal assent. Together they are the whole custody
 * story, and **none arrives pre-ticked** — a box that is already ticked records
 * nothing about what the person read.
 *
 * The recovery assurance that used to sit between them is gone. It described a
 * BENEFIT, and mixing one of those into a list of consequences teaches people to
 * skim the list.
 *
 * The field has no label and no helper line. The heading directly above it
 * already says "name your wallet", so a label restated it in smaller type — and
 * what the helper said (the name is stored on-chain) is now `ack0`, where a
 * person has to look at it rather than past it.
 *
 * The gates sit at the BOTTOM, against the button they gate: a checklist a
 * thumb reaches before the sentence does is one nobody reads. But they are
 * INSIDE the scrolling region, pushed down by the arrangement rather than
 * pinned by a weight — with the keyboard up the leftover height is smaller
 * than the form, and a pinned block took its space out of the field above,
 * which arrived on screen squashed to a sliver (founder-found 2026-08-25).
 * Scrolled instead, everything keeps its natural height and the focused field
 * is scrolled into view by the text field itself.
 */
@Composable
fun ColumnScope.NameScreen(
    name: String,
    nameEditable: Boolean,
    nameTooLong: Boolean,
    acks: List<Boolean>,
    canSubmit: Boolean,
    busy: Boolean,
    submitLabel: String,
    statusText: String?,
    showStartOver: Boolean,
    onName: (String) -> Unit,
    onToggleAck: (Int) -> Unit,
    onSubmit: () -> Unit,
    onStartOver: () -> Unit,
    onOpenPrivacy: () -> Unit,
    onOpenTerms: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    BoxWithConstraints(modifier = Modifier.weight(1f).fillMaxWidth()) {
        // `heightIn(min = maxHeight)` + `SpaceBetween` is what bottom-anchors
        // the gates INSIDE a scroll: a `Spacer(weight)` cannot, because the
        // height inside a scrolling column is unbounded and weighted children
        // collapse to nothing there.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .heightIn(min = maxHeight),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = strings.t(I18nKeys.Create.NAME_TITLE),
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl3,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))

                VelaTextField(
                    value = name,
                    onValueChange = { if (nameEditable) onName(it) },
                    label = "",
                    placeholder = strings.t(I18nKeys.Create.ACCOUNT_NAME_PLACEHOLDER),
                    errorText = if (nameTooLong) strings.t(I18nKeys.Create.NAME_TOO_LONG) else null,
                )
            }

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = VelaSpacing.xl3, bottom = VelaSpacing.xl),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                VelaAckRow(
                    checked = acks.getOrElse(0) { false },
                    onCheckedChange = { onToggleAck(0) },
                    text = AnnotatedString(strings.t(I18nKeys.Create.ACK0)),
                )
                VelaAckRow(
                    checked = acks.getOrElse(1) { false },
                    onCheckedChange = { onToggleAck(1) },
                    text = AnnotatedString(strings.t(I18nKeys.Create.ACK1)),
                )
                VelaAckRow(
                    checked = acks.getOrElse(2) { false },
                    onCheckedChange = { onToggleAck(2) },
                    text = legalLine(
                        lead = strings.t(I18nKeys.Create.ACK2),
                        privacy = strings.t(I18nKeys.Create.ACK2_PRIVACY_POLICY),
                        conjunction = strings.t(I18nKeys.Create.ACK2_AND),
                        terms = strings.t(I18nKeys.Create.ACK2_TERMS),
                        period = strings.t(I18nKeys.Create.ACK2_PERIOD),
                        // Neutral ink, underlined: the web's ack link (NameScreen
                        // `.ack a`, vis-21) — the accent belongs to value-moving
                        // actions only.
                        linkStyle = SpanStyle(color = colors.fgBase, textDecoration = TextDecoration.Underline),
                    ),
                    onLink = { which ->
                        when (which) {
                            LINK_PRIVACY -> onOpenPrivacy()
                            LINK_TERMS -> onOpenTerms()
                        }
                    },
                )

                if (statusText != null) {
                    Text(
                        text = statusText,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.regular,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
        }
    }

    VelaPrimaryButton(
        text = submitLabel,
        onClick = onSubmit,
        enabled = canSubmit,
        loading = busy,
        modifier = Modifier.fillMaxWidth(),
    )

    if (showStartOver) {
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = strings.t(I18nKeys.Create.START_OVER_BTN),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.base,
            modifier = Modifier
                .align(Alignment.CenterHorizontally)
                .clickable(onClick = onStartOver)
                .padding(VelaSpacing.md),
        )
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
}

private const val LINK_PRIVACY = "privacy"
private const val LINK_TERMS = "terms"

/**
 * The legal line, assembled from its five corpus fragments.
 *
 * Five keys rather than one interpolated sentence because the two link phrases
 * have to be styleable, and the conjunction and the full stop are grammar that
 * differs per locale — Chinese uses a different period character, and some
 * locales put the conjunction elsewhere entirely.
 *
 * The two documents open from the WORDS, the way they do on web and on the
 * desktop — this row used to name them in plain text and repeat them as a
 * separate link row underneath, which was the odd one out of the four shells.
 * They are STRING annotations rather than `LinkAnnotation`s: `VelaAckRow` does
 * the hit test against the text layout itself, because Compose laid the second
 * link's tap box over the first line (device-found 2026-08-25).
 */
private fun legalLine(
    lead: String,
    privacy: String,
    conjunction: String,
    terms: String,
    period: String,
    linkStyle: SpanStyle,
): AnnotatedString = buildAnnotatedString {
    append(lead)
    link(LINK_PRIVACY, privacy, linkStyle)
    append(conjunction)
    link(LINK_TERMS, terms, linkStyle)
    append(period)
}

private fun androidx.compose.ui.text.AnnotatedString.Builder.link(
    id: String,
    label: String,
    style: SpanStyle,
) {
    pushStringAnnotation(ACK_LINK_TAG, id)
    withStyle(style) { append(label) }
    pop()
}
