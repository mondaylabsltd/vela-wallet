package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
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
    asksToTrust = line.state == SignerIntegrityState.ASK_TO_TRUST,
)

/** The line itself: a quiet glyph for its tone, and the words. */
@Composable
fun IntegrityLine(model: IntegrityLineModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val (glyph, tint) = when (model.tone) {
        IntegrityTone.Ok -> VelaIcons.Check to colors.successBase
        IntegrityTone.Checking -> VelaIcons.Clock to colors.fgSubtle
        IntegrityTone.Caution -> VelaIcons.TriangleAlert to colors.warningBase
        IntegrityTone.Refused -> VelaIcons.TriangleAlert to colors.errorBase
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
            modifier = Modifier.padding(top = VelaSpacing.xs).size(VelaIconSize.sm),
        )
        Text(
            text = model.text,
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
    /** "Official", the person's label, or the host. */
    val title: String,
    /** The address, without its scheme: `sign.getvela.app`. */
    val address: String,
    /** "Keys on getvela.app" — which accounts this page can sign for (R1). */
    val domain: String,
    val integrity: IntegrityLineModel,
    val official: Boolean,
    /** The version the line asks to trust, when it does. */
    val trustVersion: String? = null,
)

/**
 * A signing page row — Settings → Signing pages, the venue sheet and the
 * "Use my own signing page" pickers all draw this one. [selected] draws the
 * radio of a choice; [reason] (R1) dims the row and says why it cannot be
 * chosen; [trailing] is the row's own action (a remove, a clear).
 */
@Composable
fun SigningPageItem(
    model: SigningPageItemModel,
    modifier: Modifier = Modifier,
    selected: Boolean? = null,
    reason: String? = null,
    onClick: (() -> Unit)? = null,
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
        if (selected != null) Radio(selected, enabled)
        Column(
            modifier = Modifier.weight(1f).alpha(if (enabled) 1f else VelaOpacity.disabled),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Text(
                text = model.title,
                color = if (selected == true) colors.accentBase else colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = listOf(model.address, model.domain).filter { it.isNotBlank() }.joinToString(" · "),
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
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
        trailing?.invoke()
    }
}

/** A choice's radio: a ring, filled with the accent when chosen. */
@Composable
internal fun Radio(selected: Boolean, enabled: Boolean = true) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .padding(top = VelaSpacing.xs)
            .size(VelaIconSize.md)
            .clip(CircleShape)
            .background(if (selected) colors.accentBase else colors.borderStrong)
            .alpha(if (enabled) 1f else VelaOpacity.disabled),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .size(if (selected) VelaIconSize.md * 0.4f else VelaIconSize.md * 0.78f)
                .clip(CircleShape)
                .background(if (selected) colors.fgInverse else colors.bgBase),
        )
    }
}

/** A row's small action glyph (remove, clear) in a full-size target. */
@Composable
fun SigningPageAction(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
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
