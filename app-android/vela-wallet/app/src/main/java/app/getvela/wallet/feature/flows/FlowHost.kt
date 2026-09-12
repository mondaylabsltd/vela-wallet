package app.getvela.wallet.feature.flows

import app.getvela.wallet.feature.scan.ScanCallbacks
import app.getvela.wallet.feature.scan.LiveScanSurface
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.flows.components.FlowScaffold
import app.getvela.wallet.feature.flows.components.ScanSurface

/**
 * The phone host: one flow state, rendered.
 *
 * Takes a [FlowScreenModel] and draws its base screen plus, where the state has
 * one, the sheet over it. Every screen in the four journeys goes through here,
 * so the gallery and the real app render the same thing by construction rather
 * than by discipline — there is no second code path for either to drift down.
 *
 * The 1.35× text scale rides through `LocalDensity`, exactly as spec 015's H7x
 * does, so one mechanism serves both features.
 */
@Composable
fun FlowHost(
    model: FlowScreenModel,
    modifier: Modifier = Modifier,
    onBack: () -> Unit = {},
    onNavigate: (FlowStep) -> Unit = {},
    /** Spec 048: a list row opens ITS item — the step with the row's id (the history and asset lists dropped it). */
    onOpen: (FlowStep, String) -> Unit = { _, _ -> },
    /** Spec 043: when the send is live, its taps go to the machine, not to the fixture's steps. */
    send: SendCallbacks? = null,
    addToken: AddTokenCallbacks? = null,
    /** Spec 047 US2: the receive sheet's 保存图片, when the host can render and share the card. */
    onSaveImage: (() -> Unit)? = null,
) {
    if (model.textScale != 1f) {
        val density = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(density.density, density.fontScale * model.textScale),
        ) {
            FlowHostContent(model, modifier, onBack, onNavigate, onOpen, send, addToken, onSaveImage)
        }
    } else {
        FlowHostContent(model, modifier, onBack, onNavigate, onOpen, send, addToken, onSaveImage)
    }
}

/** Where a screen can go next. Names match the web host's navigation intents. */
enum class FlowStep {
    ReceiveQr,
    TxDetail,
    TokenDetail,
    AddToken,
    SendForm,
    SendConfirm,
    SendReceipt,
    ContactPick,
    FeeToken,
    BatchImport,
    SendMulti,
    AddRecipient,
    Scan,
    Receive,
    Chains,
    Done,
}

@Composable
private fun FlowHostContent(
    model: FlowScreenModel,
    modifier: Modifier,
    onBack: () -> Unit,
    onNavigate: (FlowStep) -> Unit,
    onOpen: (FlowStep, String) -> Unit = { _, _ -> },
    send: SendCallbacks? = null,
    addToken: AddTokenCallbacks? = null,
    onSaveImage: (() -> Unit)? = null,
) {
    Box(modifier = modifier.fillMaxSize().background(VelaTheme.colors.bgBase)) {
        when (val base = model.base) {
            is FlowBase.Scan -> send?.scan?.let { live -> LiveScanSurface(model = base.model, callbacks = live) }
                ?: ScanSurface(model = base.model, onClose = onBack)
            is FlowBase.Share -> Box(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
                contentAlignment = Alignment.Center,
            ) {
                // Not a screen: the saved image, shown on its own so the
                // gallery and the save path render the very same artwork.
                ShareCardArtwork(model = base.model)
            }
            is FlowBase.Receive -> FlowScaffold(header = base.model.header, onBack = onBack) {
                ReceiveListBody(
                    model = base.model,
                    onQr = { onNavigate(FlowStep.ReceiveQr) },
                )
            }
            is FlowBase.History -> FlowScaffold(
                header = base.model.header,
                onBack = onBack,
                onPill = { onNavigate(FlowStep.Chains) },
            ) {
                HistoryBody(
                    model = base.model,
                    onSelect = { group, row ->
                        val id = base.model.groups.getOrNull(group)?.rows?.getOrNull(row)?.id
                        if (!id.isNullOrEmpty()) onOpen(FlowStep.TxDetail, id)
                    },
                )
            }
            is FlowBase.Assets -> FlowScaffold(
                header = base.model.header,
                onBack = onBack,
                onAction = { onNavigate(FlowStep.AddToken) },
                onPill = { onNavigate(FlowStep.Chains) },
            ) {
                AssetsBody(
                    model = base.model,
                    onSelect = { index ->
                        val id = base.model.rows.getOrNull(index)?.id
                        if (!id.isNullOrEmpty()) onOpen(FlowStep.TokenDetail, id)
                    },
                    onAdd = { onNavigate(FlowStep.AddToken) },
                    onReceive = { onNavigate(FlowStep.Receive) },
                )
            }
            is FlowBase.SendPick -> FlowScaffold(
                header = base.model.header,
                onBack = onBack,
                onPill = { onNavigate(FlowStep.Chains) },
            ) {
                SendPickBody(
                    model = base.model,
                    onSelect = { index -> send?.onSelectToken?.invoke(index) ?: onNavigate(FlowStep.SendForm) },
                    onSelectAll = { send?.onSelectAll?.invoke() },
                    onCta = {
                        val cta = send?.onPickCta
                        when {
                            cta != null -> cta()
                            send == null -> onNavigate(FlowStep.SendMulti)
                        }
                    },
                )
            }
            is FlowBase.SendForm -> FlowScaffold(header = base.model.header, onBack = onBack) {
                SendFormBody(
                    model = base.model,
                    onPickRecipient = { onNavigate(FlowStep.ContactPick) },
                    onScan = { send?.onScanOpen?.invoke() ?: onNavigate(FlowStep.Scan) },
                    onFee = { onNavigate(FlowStep.FeeToken) },
                    onRecipientAction = { action ->
                        send?.onRecipientAction?.invoke(action) ?: onNavigate(
                            when (action) {
                                RecipientAction.Import -> FlowStep.BatchImport
                                RecipientAction.Contacts -> FlowStep.ContactPick
                                RecipientAction.Add -> FlowStep.AddRecipient
                            }
                        )
                    },
                    onAddRecipient = { send?.onAddRecipient?.invoke() ?: onNavigate(FlowStep.AddRecipient) },
                    onRemoveRecipient = { index -> send?.onRemoveRecipient?.invoke(index) },
                    onRecipientAmount = send?.let { it.onRecipientAmount },
                    onRecipientAddress = send?.let { it.onRecipientAddress },
                    onContinue = { send?.onContinue?.invoke() ?: onNavigate(FlowStep.SendConfirm) },
                    onMax = { if (send != null) send.onMax() },
                    onDenom = { if (send != null) send.onDenom() },
                    onAmountChange = send?.onAmountChange,
                    onRecipientChange = send?.onRecipientChange,
                )
            }
            is FlowBase.SendConfirm -> FlowScaffold(header = base.model.header, onBack = onBack) {
                SendConfirmBody(
                    model = base.model,
                    onConfirm = { send?.onConfirm?.invoke() ?: onNavigate(FlowStep.SendReceipt) },
                    onNoticeAction = { send?.onNoticeAction?.invoke() },
                    onNoticeSecondary = { send?.onNoticeSecondary?.invoke() },
                )
            }
            is FlowBase.SendReceipt -> FlowScaffold(header = base.model.header, onBack = onBack) {
                SendReceiptBody(
                    model = base.model,
                    onCta = { send?.onReceiptCta?.invoke() ?: onNavigate(FlowStep.Done) },
                    onExplorer = { send?.onExplorer?.invoke() },
                )
            }
        }

        model.sheet?.let { sheet ->
            FlowSheetHost(sheet = sheet, onNavigate = onNavigate, send = send, addToken = addToken, onSaveImage = onSaveImage)
        }
    }
}

/**
 * The sheets, over whichever screen raised them.
 *
 * A new state means a new sheet: the dismissal key is the model itself, so
 * closing one does not suppress the next.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FlowSheetHost(sheet: FlowSheet, onNavigate: (FlowStep) -> Unit, send: SendCallbacks? = null, addToken: AddTokenCallbacks? = null, onSaveImage: (() -> Unit)? = null) {
    var dismissed by remember(sheet) { mutableStateOf(false) }
    if (dismissed) return

    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val dismiss = {
        dismissed = true
        send?.onSheetDismissed?.invoke()
    }
    ModalBottomSheet(
        onDismissRequest = { dismiss() },
        sheetState = state,
        containerColor = VelaTheme.colors.bgBase,
        dragHandle = { FlowSheetHandle() },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
        ) {
            SheetTitleRow(title = sheetTitle(sheet), close = sheetClose(sheet)) {
                dismiss()
            }
            when (sheet) {
                is FlowSheet.ReceiveQr -> ReceiveQrBody(model = sheet.model, onSave = { onSaveImage?.invoke() })
                is FlowSheet.TxDetail -> TxDetailBody(model = sheet.model)
                is FlowSheet.TokenDetail -> TokenDetailBody(
                    model = sheet.model,
                    onReceive = { onNavigate(FlowStep.Receive) },
                    onSend = { onNavigate(FlowStep.SendForm) },
                )
                is FlowSheet.AddToken -> AddTokenBody(
                    model = sheet.model,
                    onValueChange = addToken?.let { cb -> { text: String -> cb.onInput(text) } },
                    onSubmit = { addToken?.onSubmit?.invoke() },
                )
                is FlowSheet.ContactPick -> ContactPickBody(
                    model = sheet.model,
                    onScan = { if (send == null) onNavigate(FlowStep.Scan) },
                    onSelect = { index -> send?.onContactSelect?.invoke(index) },
                )
                is FlowSheet.FeeToken -> FeeTokenBody(
                    model = sheet.model,
                    onSelect = { index -> send?.onFeeSelect?.invoke(index) },
                )
                is FlowSheet.BatchImport -> BatchImportBody(
                    model = sheet.model,
                    onUnit = { id -> send?.onBatchUnit?.invoke(id) },
                    onFile = { send?.onBatchFile?.invoke() },
                    onTemplate = { send?.onBatchTemplate?.invoke() },
                    onApply = { send?.onBatchApply?.invoke() },
                    onPaste = send?.onBatchPaste,
                    onRate = send?.onBatchRate,
                    onRateReset = { send?.onBatchRateReset?.invoke() },
                )
            }
        }
    }
}

private fun sheetTitle(sheet: FlowSheet): String? = when (sheet) {
    // The QR, the transaction and the token draw their own heading inside the
    // body, so the sheet chrome would say it twice.
    is FlowSheet.ReceiveQr, is FlowSheet.TxDetail, is FlowSheet.TokenDetail -> null
    is FlowSheet.AddToken -> sheet.model.title
    is FlowSheet.ContactPick -> sheet.model.title
    is FlowSheet.FeeToken -> sheet.model.title
    is FlowSheet.BatchImport -> sheet.model.title
}

private fun sheetClose(sheet: FlowSheet): String = when (sheet) {
    is FlowSheet.ReceiveQr -> sheet.model.closeLabel
    is FlowSheet.TxDetail -> sheet.model.closeLabel
    is FlowSheet.TokenDetail -> sheet.model.closeLabel
    is FlowSheet.AddToken -> sheet.model.closeLabel
    is FlowSheet.ContactPick -> sheet.model.closeLabel
    is FlowSheet.FeeToken -> sheet.model.closeLabel
    is FlowSheet.BatchImport -> sheet.model.closeLabel
}

/** Token-tinted drag handle, matching spec 014's sheets. */
@Composable
private fun FlowSheetHandle() {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .size(width = VelaSizing.sheetHandleWidth, height = VelaSizing.sheetHandleHeight)
                .background(VelaTheme.colors.borderStrong, CircleShape),
        )
    }
}

/**
 * The sheet's own title row.
 *
 * The × is always there even though the grabber already dismisses by drag: a
 * sheet reached mid-transfer needs a way out that does not depend on knowing a
 * gesture.
 */
@Composable
private fun SheetTitleRow(title: String?, close: String, onClose: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (title != null) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
                lineHeight = VelaLeading.hero * VelaTextSize.xl2,
                modifier = Modifier.weight(1f),
            )
        } else {
            Spacer(modifier = Modifier.weight(1f))
        }
        Box(
            modifier = Modifier
                .size(VelaSizing.controlSm)
                .clickable(onClick = onClose),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = VelaIcons.Close,
                contentDescription = close,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.lg),
            )
        }
    }
}

/**
 * Spec 043: what a LIVE send does with a tap. Absent, every screen keeps the
 * fixture's behaviour (pushing the next drawn step), which is what the gallery
 * wants. Present, taps reach the send machine and the machine decides what is
 * on screen next.
 */
class SendCallbacks(
    val onSelectToken: (Int) -> Unit,
    val onAmountChange: (String) -> Unit,
    val onRecipientChange: (String) -> Unit,
    val onMax: () -> Unit,
    val onDenom: () -> Unit,
    val onContinue: () -> Unit,
    val onConfirm: () -> Unit,
    val onFeeSelect: (Int) -> Unit,
    val onContactSelect: (Int) -> Unit,
    val onSheetDismissed: () -> Unit,
    val onReceiptCta: () -> Unit,
    val onExplorer: () -> Unit,
    /** The confirm page's notice offered an action: retry after a treasury top-up or after a failed submit. */
    val onNoticeAction: () -> Unit = {},
    /** Spec 045 US4: the notice's second exit (the treasury pause's "not now"). */
    val onNoticeSecondary: () -> Unit = {},
    // Spec 045 US1 — the split's rows. Absent, the form keeps the fixture's hops.
    val onAddRecipient: (() -> Unit)? = null,
    val onRecipientAction: ((RecipientAction) -> Unit)? = null,
    val onRemoveRecipient: (Int) -> Unit = {},
    val onRecipientAmount: (Int, String) -> Unit = { _, _ -> },
    val onRecipientAddress: (Int, String) -> Unit = { _, _ -> },
    // Spec 046 US3 — the scanner: the form's scan icon, and the live surface's needs.
    val onScanOpen: (() -> Unit)? = null,
    val scan: ScanCallbacks? = null,
    // Spec 045 US2 — the sweep pick: select-all and the pick's own button.
    val onSelectAll: () -> Unit = {},
    val onPickCta: (() -> Unit)? = null,
    // Spec 045 US3 — the batch sheet.
    val onBatchUnit: (String) -> Unit = {},
    val onBatchPaste: ((String) -> Unit)? = null,
    val onBatchFile: () -> Unit = {},
    val onBatchTemplate: () -> Unit = {},
    val onBatchRate: ((String) -> Unit)? = null,
    val onBatchRateReset: () -> Unit = {},
    val onBatchApply: () -> Unit = {},
)

/** Spec 043 T046: the add-token sheet is the `manage_tokens` machine's when these are present. */
class AddTokenCallbacks(
    val onInput: (String) -> Unit,
    val onSubmit: () -> Unit,
)
