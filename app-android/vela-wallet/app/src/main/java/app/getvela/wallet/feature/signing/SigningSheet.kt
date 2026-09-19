package app.getvela.wallet.feature.signing

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.signing.components.AllowanceEditor
import app.getvela.wallet.feature.signing.components.SignWithRow
import app.getvela.wallet.feature.signing.components.SigningAmount
import app.getvela.wallet.feature.signing.components.SigningBalances
import app.getvela.wallet.feature.signing.components.SigningCard
import app.getvela.wallet.feature.signing.components.SigningCode
import app.getvela.wallet.feature.signing.components.SigningHeader
import app.getvela.wallet.feature.signing.components.SigningIntent
import app.getvela.wallet.feature.signing.components.SigningNftHero
import app.getvela.wallet.feature.signing.components.SigningParty
import app.getvela.wallet.feature.signing.components.SigningPositive
import app.getvela.wallet.feature.signing.components.SigningRows
import app.getvela.wallet.feature.signing.components.SigningSentence
import app.getvela.wallet.feature.signing.components.SigningSwapPair
import app.getvela.wallet.feature.signing.components.SigningWarning
import app.getvela.wallet.feature.signing.components.SignerRow
import app.getvela.wallet.feature.signing.components.SlideToConfirm
import app.getvela.wallet.feature.signing.components.TechDetails

/**
 * The signing sheet (spec 022): the universal block renderer plus a fixed
 * footer — technical details → fee → signer → slide — over the page that asked
 * for the signature, so the site you are dealing with never leaves the screen.
 *
 * Dismissal is rejection. There is no "Reject" button anywhere, because a
 * wallet with one teaches people to reach for it without reading.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SigningSheet(
    model: SigningScreenModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    onConfirm: () -> Unit = onDismiss,
    /** Spec 044: the guard's chips and custom amount reach the machine. */
    onChip: (String) -> Unit = {},
    onCustomAmount: (String) -> Unit = {},
    onSignWith: (String?) -> Unit = {},
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = VelaTheme.colors.bgRaised,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        modifier = modifier,
    ) {
        SigningSheetContent(model = model, onConfirm = onConfirm, onChip = onChip, onCustomAmount = onCustomAmount, onSignWith = onSignWith)
    }
}

/** The sheet's body, hostable anywhere (the preview gallery mounts it bare). */
@Composable
fun SigningSheetContent(
    model: SigningScreenModel,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
    onChip: (String) -> Unit = {},
    onCustomAmount: (String) -> Unit = {},
    /** `null` toggles the list; an id picks a method and closes it. */
    onSignWith: (String?) -> Unit = {},
) {
    val colors = VelaTheme.colors
    var techOverride by remember(model.state) { mutableStateOf<Boolean?>(null) }
    val techOpen = techOverride ?: model.techOpen

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX)
            .padding(bottom = VelaSpacing.xl3),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        SigningHeader(
            name = model.dappName,
            host = model.dappHost,
            letter = model.dappLetter,
            tint = model.dappTint,
            networkName = model.networkName,
            networkDot = model.networkDot,
            own = model.dappOwn,
            iconUrls = model.dappIconUrls,
            networkLogoUrl = model.networkLogoUrl,
        )

        // The universal renderer: blocks in mock order, out. Nothing here knows
        // what a swap or a permit IS — which is what lets all 33 scenarios, and
        // the ones nobody has drawn yet, come out of one code path.
        model.blocks.forEach { block ->
            when (block) {
                is SigningBlock.Intent -> SigningIntent(block.text, block.tone)
                is SigningBlock.Amount ->
                    SigningAmount(block.line, card = block.card, note = block.note)

                is SigningBlock.Swap -> SigningSwapPair(block.pay, block.receive)
                is SigningBlock.Nft -> SigningNftHero(block.id, block.collection)
                is SigningBlock.Sentence -> SigningSentence(block.text, block.tone)
                is SigningBlock.Allowance -> AllowanceEditor(
                    block.label, block.value, block.valueTone, block.chips,
                    block.note, block.resultingTotal,
                    custom = block.custom, onChip = onChip, onCustomAmount = onCustomAmount,
                )

                is SigningBlock.Party ->
                    SigningParty(block.label, block.name, block.address, block.badge)

                is SigningBlock.Rows -> SigningRows(block.rows)
                is SigningBlock.Warning -> SigningWarning(block.tone, block.text)
                is SigningBlock.Positive -> SigningPositive(block.text)
                is SigningBlock.Code -> SigningCode(block.lines, block.note)
                is SigningBlock.Card -> SigningCard(block.title, block.rows, block.tone)
                is SigningBlock.Balances ->
                    SigningBalances(block.title, block.rows, block.note, block.noteTone)
            }
        }

        Box(
            Modifier
                .fillMaxWidth()
                .height(VelaBorder.hairline)
                .background(colors.borderBase),
        )

        TechDetails(model.tech, techOpen, onToggle = { techOverride = !techOpen })
        SigningFee(model.fee)
        SignerRow(model.signerLabel, model.signerName, model.signerSeed)
        model.signWith?.let { SignWithRow(it, onSignWith) }
        SlideToConfirm(
            hint = model.confirmHint,
            action = model.confirmAction,
            enabled = model.confirmEnabled,
            onConfirm = onConfirm,
        )
    }
}
