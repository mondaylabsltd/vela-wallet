package app.getvela.wallet.feature.wallet.components

import androidx.compose.runtime.Composable
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsScreenModel
import app.getvela.wallet.feature.settings.SettingsSheet
import app.getvela.wallet.feature.wallet.WalletRescue

/**
 * The home status line's rescue, as ONE sheet over the wallet (spec 092):
 * the settings sheet host with its content swapped — SR6's list of every
 * unreachable network, a row's SR2, or SR3 — never a sheet on a sheet.
 *
 * The two ways out are kept apart on purpose. The sheet's own ✕ steps back
 * ([WalletRescue.closed]: a fix opened from the list returns to it). A swipe
 * down, the scrim or Back reach here only after Material has hidden the sheet,
 * so they close all of it ([WalletRescue.swiped]) — routing them "back to the
 * list" left an invisible list behind, its re-reads still running.
 */
@Composable
fun WalletRescueSheet(
    rescue: WalletRescue,
    model: SettingsScreenModel,
    onMove: (WalletRescue) -> Unit,
    onBalanceRetry: (String) -> Unit = {},
    onRpcFixField: (String) -> Unit = {},
    onRpcFixPrimary: () -> Unit = {},
) {
    if (rescue.overlay == SettingsOverlay.None) return
    SettingsSheet(
        model = model,
        overlay = rescue.overlay,
        onDismiss = { onMove(rescue.swiped()) },
        onClose = { onMove(rescue.closed()) },
        onSignOut = {},
        onUnreachableFix = { chainId -> onMove(rescue.fix(chainId)) },
        onBalanceRetry = onBalanceRetry,
        onRpcFixField = onRpcFixField,
        onRpcFixPrimary = onRpcFixPrimary,
    )
}
