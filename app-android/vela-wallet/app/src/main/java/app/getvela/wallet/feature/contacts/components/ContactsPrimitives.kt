package app.getvela.wallet.feature.contacts.components

import android.provider.Settings
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.contacts.ContactsIcon

/**
 * Contacts-feature metrics, derived strictly from tokens (research D9: mock
 * measures with no direct token are named here, the same escape the wallet's
 * `WalletMetrics` uses — nothing enters the DTCG export, so
 * `DesignTokenDriftTest` stays untouched).
 */
internal object ContactsMetrics {
    /** 36dp rounded-square group tile (space.xl4 32 + space.sm 4). */
    val groupTile: Dp = VelaSpacing.xl4 + VelaSpacing.sm

    /** 64dp detail hero identicon (measured from the C2 mock: 64px at 390pt). */
    val heroAvatar: Dp = VelaSpacing.xl4 * 2

    /** 32dp group-member row identicon (space.xl4). */
    val memberAvatar: Dp = VelaSpacing.xl4

    /** 20dp A–Z index rail column (space.xl2). */
    val indexRailWidth: Dp = VelaSpacing.xl2

    /** 56dp letter-bubble HUD (size.emptyStateCircle). */
    val bubbleSize: Dp = VelaSizing.emptyStateCircle

    /** 72dp per revealed swipe action (space.xl5 48 + space.xl3 24). */
    val swipeActionWidth: Dp = VelaSpacing.xl5 + VelaSpacing.xl3

    /** 28dp group chip height (space.xl2 20 + space.md 8). */
    val chipHeight: Dp = VelaSpacing.xl2 + VelaSpacing.md
}

/**
 * Named motion constants for the SPEC-sheet behaviours (FR-011). Values that
 * already exist as core.motion tokens are aliased rather than re-stated; the
 * bubble-HUD pair has no token counterpart and is pinned here.
 */
internal object ContactsMotion {
    /** Index-rail bubble fade-in — 120ms ease-out (mobile SPEC sheet). */
    const val bubbleIn: Int = 120

    /** Index-rail bubble fade-out — 80ms ease-out (mobile SPEC sheet). */
    const val bubbleOut: Int = 80
}

/**
 * The system reduce-motion switch, read the same way MainActivity reads it
 * (FR-019 house pattern). Previews/inspection always report "not reduced" so
 * tooling renders the full treatment.
 */
@Composable
internal fun rememberReducedMotion(): Boolean {
    if (LocalInspectionMode.current) return false
    val context = LocalContext.current
    return remember(context) {
        Settings.Global.getFloat(
            context.contentResolver,
            Settings.Global.ANIMATOR_DURATION_SCALE,
            1f,
        ) == 0f
    }
}

/** Model enum → shared lucide glyph (models stay free of UI types). */
internal fun glyphFor(icon: ContactsIcon): ImageVector = when (icon) {
    ContactsIcon.AddContact -> VelaIcons.UserRoundPlus
    ContactsIcon.Import -> VelaIcons.Download
    ContactsIcon.Export -> VelaIcons.Upload
    ContactsIcon.Edit -> VelaIcons.Pencil
    ContactsIcon.Delete -> VelaIcons.Trash2
    ContactsIcon.Send -> VelaIcons.ArrowUpRight
    ContactsIcon.MoveGroup -> VelaIcons.UsersRound
}

/** Full-width hairline between list rows (border.base). */
@Composable
internal fun Hairline(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(VelaBorder.hairline)
            .background(VelaTheme.colors.borderBase),
    )
}

/**
 * Bare icon affordance in a page header (person-add, pencil, ⋯, back chevron).
 * Sized to the 44dp hit target even though the glyph is 20dp.
 */
@Composable
internal fun ContactsIconButton(
    icon: ImageVector,
    contentDescription: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: androidx.compose.ui.graphics.Color? = null,
) {
    Box(
        modifier = modifier
            .size(VelaSizing.hitTarget)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = contentDescription,
            tint = tint ?: VelaTheme.colors.fgBase,
            modifier = Modifier.size(VelaIconSize.lg),
        )
    }
}
