package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.WalletHeaderModel

/**
 * Wallet header (spec vocabulary #2): IdenticonAvatar + truncating name +
 * disclosure chevron + middle-truncated address.
 *
 * The trailing NetworkFilterPill is gone (founder call, 2026-08-26): it cost
 * the name and the address the width they needed, and a wallet whose name reads
 * "kimik3 ·…" is a header that has stopped doing its job. The header now owns
 * the row.
 *
 * [onIdenticon] makes the artwork its own control. It answers a different
 * question from the name — "is this the account I think it is?" — and the
 * founder's call is that it answers it wherever the artwork is drawn.
 */
@Composable
fun WalletHeaderRow(
    header: WalletHeaderModel,
    modifier: Modifier = Modifier,
    onIdenticon: (() -> Unit)? = null,
    identiconLabel: String? = null,
    /** Spec 047: the name line with its chevron opens the account switcher (the web's header switcher, 028 phase 9). */
    onSwitcher: (() -> Unit)? = null,
    /** Spec 105: the space label's tap — the switch prompt. */
    onSpace: (() -> Unit)? = null,
    /** ob-2 (P2.4): the sync line's "try now" — asks the session to publish again (`sync_pending`). */
    onSyncRetry: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onIdenticon != null) {
            IdenticonAvatar(
                tappable = false,
                seed = header.identiconSeed,
                contentDescription = identiconLabel ?: header.name,
                modifier = Modifier
                    .clip(CircleShape)
                    .clickable(onClick = onIdenticon),
            )
        } else {
            IdenticonAvatar(seed = header.identiconSeed, contentDescription = header.name, tappable = false)
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = if (onSwitcher != null) Modifier.clickable(onClick = onSwitcher) else Modifier,
            ) {
                Text(
                    text = header.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.ChevronDown,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
            Text(
                text = header.addressDisplay,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            // ob-2: saved here, not yet findable elsewhere. Quiet and neutral:
            // the session keeps trying on its own; after a while of failing,
            // the person may ask it to try now (web `WalletHeader` `.sync`).
            header.sync?.let { sync -> SyncLine(sync, onSyncRetry) }
        }
        // The space in front (spec 105), beside the account it shows: a quiet
        // pill in the mainnet space, amber in the testnet space.
        header.space?.let { space ->
            Spacer(modifier = Modifier.width(VelaSpacing.md))
            NetworkSpaceLabel(text = space, testnet = header.testnet, onClick = onSpace)
        }
    }
}

/** The header's sync line (ob-2): muted while it keeps trying, full ink once stalled, a retry when offered. */
@Composable
private fun SyncLine(sync: app.getvela.wallet.feature.wallet.WalletSyncLineModel, onRetry: (() -> Unit)?) {
    val colors = VelaTheme.colors
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .testTag(WALLET_SYNC_LINE_TAG)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
    ) {
        Text(
            text = sync.text,
            color = if (sync.stalled) colors.fgBase else colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.xs,
        )
        if (sync.retry != null && onRetry != null) {
            Spacer(modifier = Modifier.width(VelaSpacing.sm))
            Text(
                text = sync.retry,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xs,
                textDecoration = TextDecoration.Underline,
                modifier = Modifier
                    .testTag(WALLET_SYNC_RETRY_TAG)
                    .clickable(role = Role.Button, onClick = onRetry)
                    // ≥44dp target without growing the quiet line.
                    .padding(vertical = VelaSpacing.sm),
            )
        }
    }
}

/** The header's sync line and its retry (ob-2). */
const val WALLET_SYNC_LINE_TAG = "wallet-sync-line"
const val WALLET_SYNC_RETRY_TAG = "wallet-sync-retry"

/** The header's space label — tapping it offers the other space. */
@Composable
private fun NetworkSpaceLabel(text: String, testnet: Boolean, onClick: (() -> Unit)?) {
    val colors = VelaTheme.colors
    val shape = androidx.compose.foundation.shape.RoundedCornerShape(app.getvela.wallet.core.designsystem.tokens.VelaRadius.full)
    Text(
        text = text,
        color = if (testnet) colors.warningBase else colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.xs,
        maxLines = 1,
        modifier = Modifier
            .testTag(NETWORK_SPACE_LABEL_TAG)
            .clip(shape)
            .background(if (testnet) colors.warningSoft else colors.bgRaised)
            .border(VelaBorder.hairline, if (testnet) colors.warningBorder else colors.borderBase, shape)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm),
    )
}

/** The header's space label (spec 105). */
const val NETWORK_SPACE_LABEL_TAG = "network-space-label"
