package app.getvela.wallet.feature.signing.clearsigner

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.platform.Clipboard
import uniffi.vela_core_uniffi.qrMatrix

/**
 * Spec 075: the three sheets a Clear Signer request can raise before anything
 * is signed — where the signer is, the pairing code for another device, and
 * the six digits both screens must agree on.
 *
 * One entry point, driven by [ClearSignerChannel.state], so every surface that
 * can start a Clear Signer request (create, sign in, a send, a dApp request,
 * the key backup) shows the same thing without repeating the wiring.
 */
@Composable
fun ClearSignerSheets(
    state: ClearSignerChannel.State,
    onWhere: (thisDevice: Boolean) -> Unit,
    onConfirmCode: () -> Unit,
    onCancel: () -> Unit,
) {
    when (state) {
        is ClearSignerChannel.State.Where -> ClearSignerWhereSheet(onWhere, onCancel)
        is ClearSignerChannel.State.Pairing -> ClearSignerPairSheet(state.link, onCancel)
        is ClearSignerChannel.State.Code -> ClearSignerCodeSheet(state.code, onConfirmCode, onCancel)
        else -> Unit
    }
}

/**
 * "Where is your Clear Signer?"
 *
 * The page can be on this phone (a tab over the app, on its own loopback) or
 * on another device entirely — a laptop's browser holding the passkey. Both
 * are the same route as far as the wallet is concerned; only the wire differs,
 * so this is one question with two answers rather than two routes in the
 * method picker.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ClearSignerWhereSheet(onPick: (thisDevice: Boolean) -> Unit, onCancel: () -> Unit) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    ModalBottomSheet(
        onDismissRequest = onCancel,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            Text(
                text = strings.t("componentsUi.signing.clearSignerWhere"),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
            listOf(
                true to "componentsUi.signing.clearSignerThisDevice",
                false to "componentsUi.signing.clearSignerOtherDevice",
            ).forEach { (here, key) ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onPick(here) }
                        .padding(vertical = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = strings.t(key),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                        modifier = Modifier.weight(1f),
                    )
                    Icon(
                        imageVector = if (here) VelaIcons.Eye else VelaIcons.ScanLine,
                        contentDescription = null,
                        tint = colors.fgSubtle,
                        modifier = Modifier.size(VelaIconSize.lg),
                    )
                }
            }
        }
    }
}

/**
 * The pairing sheet: the link as a QR code, the link itself to copy, and the
 * line that says we are waiting for the other device.
 *
 * The fragment carries the relay, the room and `rk` — the fingerprint of this
 * wallet's session key — and a fragment never reaches a server, so the relay
 * learns nothing from the link even if somebody pastes it into a chat.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ClearSignerPairSheet(link: String, onCancel: () -> Unit) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val context = LocalContext.current
    ModalBottomSheet(
        onDismissRequest = onCancel,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Text(
                text = strings.t("componentsUi.signing.clearSignerPair"),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Text(
                text = strings.t("componentsUi.signing.clearSignerPairHint"),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            QrBlock(link)
            Text(
                text = link,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.xs,
                maxLines = 2,
            )
            Text(
                text = strings.t("componentsUi.signing.clearSignerPairWaiting"),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
            VelaPrimaryButton(
                strings.t("componentsUi.signing.clearSignerCopyLink"),
                onClick = {
                    Clipboard.copy(context, "Vela Clear Signer", link)
                },
                modifier = Modifier.fillMaxWidth(),
            )
            VelaSecondaryButton(
                strings.t("common.cancel"),
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * The six digits.
 *
 * Both ends derive them from the two public keys and the two nonces, so they
 * match only if nobody is in the middle. **The wallet sends nothing until the
 * person confirms** — that is what stops a stolen link from putting somebody
 * else's page in this conversation, which for a create would mean being handed
 * somebody else's key.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ClearSignerCodeSheet(code: String, onConfirm: () -> Unit, onCancel: () -> Unit) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    ModalBottomSheet(
        onDismissRequest = onCancel,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Text(
                text = code,
                color = colors.fgBase,
                fontFamily = VelaMonoFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                text = strings.t(
                    "componentsUi.signing.clearSignerCode",
                    mapOf("code" to code),
                ),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            VelaPrimaryButton(
                strings.t("componentsUi.signing.clearSignerCodeConfirm"),
                onClick = onConfirm,
                modifier = Modifier.fillMaxWidth(),
            )
            VelaSecondaryButton(
                strings.t("common.cancel"),
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * The link as a code somebody can point a camera at. The matrix is the core's
 * (`qrMatrix`, the encoder every Vela platform draws with), so the shell owns
 * only pixels — and white in both appearances, because a camera reads it.
 */
@Composable
private fun QrBlock(payload: String) {
    val matrix = remember(payload) { runCatching { qrMatrix(payload) }.getOrNull() } ?: return
    val width = matrix.width.toInt()
    Spacer(modifier = Modifier.height(VelaSpacing.sm))
    Canvas(modifier = Modifier.fillMaxWidth().aspectRatio(1f)) {
        val quiet = 2
        val units = width + quiet * 2
        val cell = size.minDimension / units
        drawRect(color = Color.White, size = Size(size.width, size.height))
        for (row in 0 until width) {
            for (col in 0 until width) {
                if (matrix.modules[row * width + col]) {
                    drawRect(
                        color = Color.Black,
                        topLeft = Offset((col + quiet) * cell, (row + quiet) * cell),
                        size = Size(cell, cell),
                    )
                }
            }
        }
    }
}
