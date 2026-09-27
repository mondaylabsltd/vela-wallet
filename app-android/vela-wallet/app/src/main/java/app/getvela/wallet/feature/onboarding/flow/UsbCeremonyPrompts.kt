package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import uniffi.vela_core_uniffi.CtapCredentialChoice

/**
 * The dialogs the app-owned CTAP path draws itself.
 *
 * Every OTHER passkey route on this platform hands the ceremony to a system
 * sheet, which draws its own PIN entry, touch prompt and account picker. The
 * app-owned USB path has no such sheet — it IS the client — so it has to say
 * these three things on its own behalf, the way the desktop does. The copy is
 * the shared corpus (spec 019 §5); nothing here is hard-coded.
 */

/**
 * The security key's PIN. `onSubmit(null)` / dismissal is a cancellation.
 *
 * **An in-app numeric keypad, deliberately — not a text field.** A security key
 * that also does OTP enumerates as a USB keyboard, and a phone with a keyboard
 * attached suppresses its on-screen IME. The person would be stranded: no soft
 * keyboard, and the key itself only emits an OTP on a touch, never a PIN. So the
 * PIN is entered on this app's own keypad, which owes nothing to the system
 * keyboard (iPhone-found 2026-08-27; the same hardware fact holds on Android).
 * FIDO2 PINs are numeric in the overwhelming majority of cases; an alphanumeric
 * PIN is the one case a numeric pad does not cover, noted for a follow-up.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UsbPinDialog(
    product: String,
    retries: Int,
    isRetry: Boolean,
    onSubmit: (String?) -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    var pin by remember { mutableStateOf("") }
    // Fully expanded from the start: the default half-expanded stop fits this
    // content only at fontScale 1.0 — a large system font (elderly settings)
    // pushed the keypad's last row and the confirm button below the fold, and
    // nothing suggested dragging (device-found on a OnePlus 5T at 1.3×,
    // 2026-08-28). The scroll is the second half of the same fix: whatever
    // still overflows a short screen stays reachable.
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    VelaModalSheet(
        onDismissRequest = { onSubmit(null) },
        sheetState = sheetState,
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Text(
                text = strings.t(I18nKeys.Create.PIN_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Text(
                text = strings.t(I18nKeys.Create.PIN_BODY, mapOf("product" to product)),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            // The masked PIN — a centred row of dots, or the label when empty.
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(VelaSpacing.xl4)
                    .padding(vertical = VelaSpacing.sm),
            ) {
                if (pin.isEmpty()) {
                    Text(
                        text = strings.t(I18nKeys.Create.PIN_LABEL),
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                } else {
                    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                        repeat(pin.length) {
                            Box(
                                modifier = Modifier
                                    .size(VelaSpacing.lg)
                                    .clip(RoundedCornerShape(50))
                                    .background(colors.fgBase),
                            )
                        }
                    }
                }
            }
            if (isRetry) {
                Text(
                    text = strings.t(I18nKeys.Create.PIN_REJECTED),
                    color = colors.errorBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (retries >= 0) {
                Text(
                    text = strings.t(
                        I18nKeys.Create.PIN_ATTEMPTS_LEFT,
                        mapOf("attempts" to retries.toString()),
                    ),
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            PinKeypad(
                onDigit = { digit -> if (pin.length < 63) pin += digit },
                onDelete = { pin = pin.dropLast(1) },
            )
            VelaPrimaryButton(
                text = strings.t(I18nKeys.Create.CONFIRM_KEY_BTN),
                onClick = { onSubmit(pin) },
                // FIDO2 requires at least 4 UTF-8 bytes.
                enabled = pin.length >= 4,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/** A 3×4 numeric keypad, owing nothing to the system keyboard. */
@Composable
private fun PinKeypad(
    onDigit: (Char) -> Unit,
    onDelete: () -> Unit,
) {
    val colors = VelaTheme.colors
    val rows = listOf(
        listOf("1", "2", "3"),
        listOf("4", "5", "6"),
        listOf("7", "8", "9"),
        listOf("", "0", "⌫"),
    )
    Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        for (row in rows) {
            Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                for (label in row) {
                    if (label.isEmpty()) {
                        Spacer(modifier = Modifier.weight(1f))
                    } else {
                        Box(
                            contentAlignment = Alignment.Center,
                            modifier = Modifier
                                .weight(1f)
                                .height(VelaSpacing.xl6)
                                .clip(RoundedCornerShape(VelaRadius.xl))
                                .background(colors.bgSunken)
                                .clickable {
                                    if (label == "⌫") onDelete() else onDigit(label[0])
                                },
                        ) {
                            Text(
                                text = label,
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.semibold,
                                fontSize = VelaTextSize.xl4,
                            )
                        }
                    }
                }
            }
        }
    }
}

/** Which of several wallets on one key. `onPick(null)` / dismissal cancels. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UsbWalletPicker(
    choices: List<CtapCredentialChoice>,
    onPick: (Int?) -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors

    VelaModalSheet(
        onDismissRequest = { onPick(null) },
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Text(
                text = strings.t(I18nKeys.Login.PICK_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            val product = choices.firstOrNull()?.product ?: ""
            Text(
                text = strings.t(I18nKeys.Login.PICK_BODY, mapOf("product" to product)),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            choices.forEachIndexed { index, choice ->
                if (index > 0) HorizontalDivider(color = colors.borderBase)
                Text(
                    text = choice.name.ifEmpty { strings.t(I18nKeys.Login.PICK_UNNAMED) },
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onPick(index) }
                        .padding(vertical = VelaSpacing.lg),
                )
            }
        }
    }
}

/**
 * The key is blinking — "touch it now". Not dismissable: the person's next act
 * is a physical touch, not a tap on screen. It clears when the ceremony's next
 * step arrives (the ViewModel sets the state to null).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UsbTouchIndicator(kind: String, product: String) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    // Over caBLE the "authenticator" is the person's phone, and the approval
    // happens THERE — "touch your security key" would send them hunting for
    // hardware they never owned. Same corpus keys the desktop and iOS use.
    val remote = product == "your phone"
    val body = when {
        remote -> strings.t(I18nKeys.Flow.TOUCH_REMOTE_BODY)
        kind == "fingerprint" -> strings.t(I18nKeys.Create.TOUCH_FINGERPRINT_BODY, mapOf("product" to product))
        kind == "select" -> strings.t(I18nKeys.Create.TOUCH_SELECT_BODY)
        else -> strings.t(I18nKeys.Create.TOUCH_BODY, mapOf("product" to product))
    }

    VelaModalSheet(
        // A blinking key is answered with a finger, not a swipe — but a sheet
        // that literally cannot be dismissed traps a person whose key went
        // away, so dismissal is allowed and simply lets the exchange time out.
        onDismissRequest = {},
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Text(
                text = if (remote) strings.t(I18nKeys.Flow.TOUCH_REMOTE_TITLE) else strings.t(I18nKeys.Create.TOUCH_TITLE),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Text(
                text = body,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
    }
}
