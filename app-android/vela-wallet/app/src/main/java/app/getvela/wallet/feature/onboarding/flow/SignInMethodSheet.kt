package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.KeyMethod

/**
 * The ways to sign in — this device, a nearby device by scan, a hardware
 * security key — the SAME three places creating a wallet offers per key, and
 * (spec 102) "Use a trusted signing page" for a wallet whose keys live on the
 * person's own domain. A wallet that lives on a security key is reachable even
 * when a platform passkey is also present, which the plain system route would
 * use silently.
 *
 * The scan (`Hybrid`) is "sign in with your phone" over caBLE (spec 019): this
 * device shows a QR, the phone that holds the passkey scans it, and the ceremony
 * runs over the BLE/tunnel channel that phone opens.
 *
 * The own-page entry is not a fourth place: it picks WHERE the keys belong (a
 * saved page, with its domain and integrity line), and the three places then
 * sign in there — on the page itself for a custom domain (R3), in the app for
 * `getvela.app`. The rows are the same shape as the create picker's,
 * deliberately.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SignInMethodSheet(
    onPick: (KeyMethod, String?) -> Unit,
    onDismiss: () -> Unit,
    /** Spec 102: the saved pages — `null` hides the entry (a surface with no page to offer). */
    ownPage: OwnPageModel? = null,
    /** The picker came up: read and check every page. */
    onOpenPages: () -> Unit = {},
    onAddPage: (String) -> Unit = {},
    /** "Trust this version" on a self-hosted page whose check asks. */
    onTrustPage: (String) -> Unit = {},
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    var picking by remember { mutableStateOf(false) }
    var chosen by remember { mutableStateOf<String?>(null) }

    VelaModalSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            Text(
                text = if (picking) ownPage?.title.orEmpty() else strings.t(I18nKeys.Login.HEADER),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
            if (picking && ownPage != null) {
                OwnPageList(
                    model = ownPage,
                    onPick = { url ->
                        chosen = url
                        picking = false
                    },
                    onAdd = onAddPage,
                    onTrust = onTrustPage,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                VelaSecondaryButton(strings.t("common.cancel"), onClick = { picking = false }, modifier = Modifier.fillMaxWidth())
                return@Column
            }
            KeyMethod.entries.forEach { method ->
                val (title, body) = methodCopy(method, KeyChooser.SignIn, strings)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onPick(method, chosen) }
                        .padding(vertical = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = title,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.semibold,
                            fontSize = VelaTextSize.lg,
                        )
                        Text(
                            text = body,
                            color = colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                            lineHeight = VelaLeading.normal * VelaTextSize.sm,
                        )
                    }
                    Icon(
                        imageVector = VelaIcons.ChevronRight,
                        contentDescription = null,
                        tint = colors.fgSubtle,
                        modifier = Modifier.size(VelaIconSize.lg),
                    )
                }
            }
            if (ownPage != null) {
                HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)
                OwnPageEntry(
                    chosen = chosen?.let { url -> ownPage.pages.firstOrNull { it.url == url } },
                    onOpen = {
                        onOpenPages()
                        picking = true
                    },
                    onClear = { chosen = null },
                    clearLabel = strings.t(I18nKeys.Create.REMOVE_KEY_BTN),
                    onTrust = onTrustPage,
                )
            }
        }
    }
}
