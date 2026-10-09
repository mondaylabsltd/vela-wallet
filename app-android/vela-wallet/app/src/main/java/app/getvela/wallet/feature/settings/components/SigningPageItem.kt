package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import uniffi.vela_core_uniffi.SignerIntegrityLine
import uniffi.vela_core_uniffi.SignerIntegrityState

/**
 * Spec 102: what is TRUSTED about a signing page, said in one line — the
 * core's integrity verdict ("Version 0ba8ee8c · matches Vela's published build
 * list · checked 14:02", or why it will not open). Never "certified".
 */
@Immutable
data class IntegrityLineModel(
    val text: String,
    val tone: IntegrityTone,
    /** The page may be opened on this line. */
    val opens: Boolean,
    /** The eight hex characters of the version, set in the mono face so they read as the hash they are. */
    val version: String = "",
    /** The line asks the person to trust a custom page's own build. */
    val asksToTrust: Boolean = false,
)

enum class IntegrityTone { Ok, Checking, Caution, Refused }

/** The core's line in the person's words, with the tone its state carries. */
fun integrityModel(line: SignerIntegrityLine, strings: VelaStrings): IntegrityLineModel = IntegrityLineModel(
    text = SignerPageChecks.words(line, strings),
    tone = when (line.state) {
        SignerIntegrityState.MATCHES, SignerIntegrityState.TRUSTED_HERE -> IntegrityTone.Ok
        SignerIntegrityState.CHECKING -> IntegrityTone.Checking
        SignerIntegrityState.UNCHECKED, SignerIntegrityState.ASK_TO_TRUST -> IntegrityTone.Caution
        else -> IntegrityTone.Refused
    },
    opens = line.opens,
    version = line.version,
    asksToTrust = line.state == SignerIntegrityState.ASK_TO_TRUST,
)

/**
 * The line itself: a glyph for its tone — the shield that backs the word
 * "trusted", a clock while it is checked, a warning when it will not open
 * (a line that is red only by colour is one some people cannot see is red) —
 * and the words, the version in the mono face.
 */
@Composable
fun IntegrityLine(model: IntegrityLineModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val (glyph, tint) = when (model.tone) {
        IntegrityTone.Ok -> VelaIcons.ShieldCheck to colors.successBase
        IntegrityTone.Checking -> VelaIcons.Clock to colors.fgSubtle
        IntegrityTone.Caution -> VelaIcons.TriangleAlert to colors.warningBase
        IntegrityTone.Refused -> VelaIcons.TriangleAlert to colors.errorBase
    }
    val text = buildAnnotatedString {
        val at = if (model.version.length == 8) model.text.indexOf(model.version) else -1
        if (at < 0) {
            append(model.text)
        } else {
            append(model.text.substring(0, at))
            withStyle(SpanStyle(fontFamily = VelaMonoFontFamily)) { append(model.version) }
            append(model.text.substring(at + model.version.length))
        }
    }
    Row(
        modifier = modifier.semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        Icon(
            imageVector = glyph,
            contentDescription = null,
            tint = tint,
            // Centred on the FIRST line of a line that may wrap.
            modifier = Modifier.padding(top = VelaSpacing.xs).size(VelaIconSize.sm),
        )
        Text(
            text = text,
            color = if (model.tone == IntegrityTone.Refused) colors.errorBase else colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaLeading.normal * VelaTextSize.sm,
        )
    }
}

/** One signing page as a person meets it: its name, its address, whose keys it reaches, and its line. */
@Immutable
data class SigningPageItemModel(
    val url: String,
    /** "Vela's official signing page", the person's label, or "Self-hosted · <domain>". */
    val title: String,
    /** The address, without its scheme: `sign.getvela.app`. */
    val address: String,
    /** "Keys on getvela.app" — which accounts this page can sign for (R1). */
    val domain: String,
    val integrity: IntegrityLineModel,
    val official: Boolean,
    /** The version the line asks to trust, when it does. */
    val trustVersion: String? = null,
    /** The person's own label for the page; empty when they gave none (the rename field's start). */
    val label: String = "",
)

/**
 * A signing page row — Settings → Signing pages, "Where you review and sign"
 * and the "Use a trusted signing page" pickers all draw this one: the shield,
 * the page's name, where it lives and whose keys it reaches (the address in
 * the mono face), and its integrity line.
 *
 * [selected] marks the venue in force with the accent and a check; [reason]
 * (R1) dims a choice that cannot reach the account's keys and says why in
 * place of the line; [actions] sit on the name's line (rename, remove);
 * [trailing] ends the row (a clear).
 */
@Composable
fun SigningPageItem(
    model: SigningPageItemModel,
    modifier: Modifier = Modifier,
    selected: Boolean? = null,
    reason: String? = null,
    onClick: (() -> Unit)? = null,
    actions: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    val enabled = reason == null
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(
                if (onClick != null) {
                    Modifier.clickable(enabled = enabled, role = if (selected != null) Role.RadioButton else Role.Button, onClick = onClick)
                } else {
                    Modifier
                },
            )
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            .semantics { if (selected != null) this.selected = selected },
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        RowGlyph(VelaIcons.ShieldCheck, enabled)
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = model.title,
                    color = if (selected == true) colors.accentBase else colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    modifier = Modifier.weight(1f).alpha(if (enabled) 1f else VelaOpacity.disabled),
                )
                actions?.invoke()
            }
            Text(
                text = buildAnnotatedString {
                    withStyle(SpanStyle(fontFamily = VelaMonoFontFamily)) { append(model.address) }
                    if (model.domain.isNotBlank()) append("  ·  ").also { append(model.domain) }
                },
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.alpha(if (enabled) 1f else VelaOpacity.disabled),
            )
            if (reason != null) {
                Text(
                    text = reason,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    lineHeight = VelaLeading.normal * VelaTextSize.sm,
                )
            } else {
                IntegrityLine(model.integrity, modifier = Modifier.padding(top = VelaSpacing.xs))
            }
        }
        if (selected == true) ChosenMark()
        trailing?.invoke()
    }
}

/** A row's leading glyph: the lucide icon, muted, dimmed with a disabled row. */
@Composable
internal fun RowGlyph(icon: ImageVector, enabled: Boolean = true) {
    Icon(
        imageVector = icon,
        contentDescription = null,
        tint = VelaTheme.colors.fgMuted,
        modifier = Modifier
            .padding(top = VelaSpacing.xs)
            .size(VelaIconSize.lg)
            .alpha(if (enabled) 1f else VelaOpacity.disabled),
    )
}

/** The venue in force: the accent check at the row's end. */
@Composable
internal fun ChosenMark() {
    Icon(
        imageVector = VelaIcons.Check,
        contentDescription = null,
        tint = VelaTheme.colors.accentBase,
        modifier = Modifier.padding(top = VelaSpacing.xs).size(VelaIconSize.md),
    )
}

/** A row's small action glyph (clear) in a full-size target. */
@Composable
fun SigningPageAction(icon: ImageVector, label: String, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .size(VelaSizing.hitTarget)
            .clip(CircleShape)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(imageVector = icon, contentDescription = null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.md))
    }
}

/** A row's text action on the name's line: "Rename", "Remove". */
@Composable
fun SigningPageTextAction(label: String, danger: Boolean = false, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Text(
        text = label,
        color = if (danger) colors.errorBase else colors.accentBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        modifier = Modifier
            .clip(CircleShape)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = VelaSpacing.sm, vertical = VelaSpacing.xs),
    )
}

/**
 * A self-hosted page's own build asks to be trusted on this device: its line
 * is the question ("Version 3f9a1c22 is new to Vela. Trust it on this
 * device?"), this the answer (`settings.signing.pageTrust`) — under the row's
 * words, past the glyph and its gap. One quiet accent word, never a button
 * that outshouts the line it answers.
 */
@Composable
fun TrustAnswer(label: String, modifier: Modifier = Modifier, indent: Boolean = true, onClick: () -> Unit) {
    Text(
        text = label,
        color = VelaTheme.colors.accentBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = modifier
            .padding(start = if (indent) VelaIconSize.lg + VelaSpacing.lg else VelaSpacing.none, bottom = VelaSpacing.md)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = VelaSpacing.sm),
    )
}
