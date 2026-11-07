package app.getvela.wallet.navigation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.testTag
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavHostController
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.SessionRoute
import kotlinx.coroutines.delay

/** The ground's test tag (resource id to UiAutomator): the launch frame check finds it by name. */
const val LAUNCH_GROUND_TAG = "launch.ground"

/**
 * r18-3 (§33): the page's own ground over the NavHost while [LaunchFront]
 * says so — nothing of any screen, nothing to press, nothing read out. It
 * lifts once the session has read the store, the wallet route is the only
 * one shown, and a multisig in front at the last run is loaded (or
 * [LaunchFront.HOLD_MS] has passed since the session said Wallet).
 */
@Composable
internal fun LaunchGround(navController: NavHostController, route: SessionRoute) {
    val application = LocalContext.current.applicationContext as VelaWalletApplication
    val container = application.container
    val stored by container.launchFront.collectAsStateWithLifecycle()
    val accounts by container.multisig.accountsView.collectAsStateWithLifecycle()
    val visible by navController.visibleEntries.collectAsStateWithLifecycle()
    var expired by remember { mutableStateOf(false) }
    LaunchedEffect(route == SessionRoute.Wallet) {
        if (route == SessionRoute.Wallet && !expired) {
            delay(LaunchFront.HOLD_MS)
            expired = true
        }
    }
    // Once lifted it stays lifted: a later sign-out goes to Welcome as ever,
    // and a second wallet's hand-off is never covered.
    var lifted by rememberSaveable { mutableStateOf(false) }
    val walletSettled = visible.size == 1 && visible.single().destination.route == VelaDestinations.WALLET
    val screen = LaunchFront.screen(route, stored, hubLoaded = accounts.loaded, expired = expired, walletSettled = walletSettled)
    if (!lifted && screen != LaunchFront.Screen.Ground) {
        lifted = true
        VelaLog.event("launch", "ground lifted", "screen" to screen.name, "stored" to stored::class.simpleName, "expired" to expired)
    }
    if (lifted) return
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(VelaTheme.colors.bgBase)
            // Nothing under it takes a touch while it stands.
            .pointerInput(Unit) { awaitPointerEventScope { while (true) awaitPointerEvent().changes.forEach { it.consume() } } }
            .clearAndSetSemantics { testTag = LAUNCH_GROUND_TAG },
    )
}
