package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.settings.AccountsSheetBody
import app.getvela.wallet.feature.settings.AccountsSheetModel

/**
 * The home's account switcher (spec 047, the founder's 2026-09-12 note that
 * the phone had no wallet switch on the home): the same sheet the settings
 * page draws — every account on this device with its total, the active one
 * ticked, create and sign-in below — opened from the header's name line, the
 * way the web's header switcher opens (028 phase 9).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountSwitcherSheet(
    sheet: AccountsSheetModel,
    onDismiss: () -> Unit,
    onSelect: (Int) -> Unit,
    onPrimary: () -> Unit,
    onSecondary: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = VelaTheme.colors.bgBase,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl)
                .padding(bottom = VelaSpacing.xl)
                .navigationBarsPadding(),
        ) {
            AccountsSheetBody(sheet = sheet, onSelect = onSelect, onPrimary = onPrimary, onSecondary = onSecondary)
        }
    }
}
