package app.getvela.wallet.feature.settings

import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.ui.graphics.graphicsLayer
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.components.VelaLabelBesideValue
import androidx.compose.foundation.layout.navigationBarsPadding
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.material3.SnackbarData
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarHost
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.runtime.withFrameNanos
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.unit.Dp
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.foundation.focusable
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.layout.wrapContentHeight
import kotlinx.coroutines.launch
import app.getvela.wallet.feature.settings.components.FeedbackLineIcon
import kotlinx.coroutines.delay
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.text.style.LineBreak
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.material3.LocalTextStyle
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Dispatchers
import app.getvela.wallet.feature.settings.components.ScreenshotTileView
import app.getvela.wallet.feature.settings.components.FeedbackScreenshotsSection
import app.getvela.wallet.feature.settings.components.ScreenshotFocusReturn
import app.getvela.wallet.feature.settings.components.ScreenshotViewer
import app.getvela.wallet.feature.settings.components.ViewerImage
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.diagnostics.ScreenshotTray
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules
import app.getvela.wallet.core.diagnostics.ScreenshotPrep
import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.designsystem.components.VelaStatusBadge
import app.getvela.wallet.core.designsystem.components.BadgeVariant
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.collectAsState
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.compose.rememberLauncherForActivityResult
import android.net.Uri
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.RoundedCornerShape
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.ui.semantics.Role
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import app.getvela.wallet.core.data.DebugMode
import app.getvela.wallet.feature.settings.components.VelaSwitchRow
import uniffi.vela_core_uniffi.VersionTaps
import uniffi.vela_core_uniffi.prefsVersionTapped
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.settings.components.SettingsDivider
import app.getvela.wallet.feature.settings.components.VelaWalletKeysBlock
import app.getvela.wallet.feature.settings.components.SettingsSectionLabel
import app.getvela.wallet.feature.settings.components.VelaAccountRow
import app.getvela.wallet.feature.settings.components.VelaCallout
import app.getvela.wallet.feature.settings.components.VelaChainMark
import app.getvela.wallet.feature.settings.components.VelaCheckList
import app.getvela.wallet.feature.settings.components.VelaDangerCard
import app.getvela.wallet.feature.settings.components.VelaKeyValueRow
import app.getvela.wallet.feature.settings.components.VelaNetworkRow
import app.getvela.wallet.feature.settings.components.VelaSegmentedControl
import app.getvela.wallet.feature.settings.components.VelaSelectRow
import app.getvela.wallet.feature.settings.components.VelaSettingsRow
import app.getvela.wallet.feature.settings.components.VelaStatusPill
import app.getvela.wallet.feature.settings.components.VelaStorageBar
import app.getvela.wallet.feature.settings.components.VelaStorageGroup
import app.getvela.wallet.feature.settings.components.VelaTextScaleSlider
import app.getvela.wallet.feature.settings.components.VelaUrlField
import app.getvela.wallet.feature.settings.components.settingsIcon
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.components.VelaTabBar

/**
 * The settings surface (spec 023, ST1–ST16 + SR1–SR5).
 *
 * One screen, not sixteen. The mocks are a page (`Home` plus seven pushed
 * sub-pages) crossed with an overlay (nine sheets), and everything inside both
 * is assembled from `components/`. Which page and which overlay a state shows
 * is DATA — the fixture layer says so — so the gallery pins a state by handing
 * over a model, and the real app moves between them by tapping.
 *
 * Navigation is local state seeded from the model. Business state is not wired:
 * the callbacks are how the nav host hooks the two behaviours that already
 * exist (signing out, and leaving for another tab).
 */

/** What the host can respond to. Everything else is presentation. */
data class SettingsActions(
    val onSelectTab: (VelaTab) -> Unit = {},
    val onSignOut: () -> Unit = {},
    val onOpenContacts: () -> Unit = {},
    /** The Ethereum backup row (spec 062); the caller decides whether there is anything to do. */
    val onEthereumBackup: () -> Unit = {},
    /**
     * A row picked in one of the select sheets — currency today, the language
     * and format sheets when their machines arrive.
     *
     * The overlay says WHICH sheet, so one callback serves five of them and a
     * new sheet does not widen this class. Returning is the host's business:
     * the sheet closes here, and what the pick means is the core's.
     */
    val onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
    /**
     * A field changed, keystroke by keystroke, identified by the id its model
     * carries.
     *
     * Per keystroke rather than on blur because the CORE holds the draft: it
     * echoes the value back in its view, so the box a person types into is
     * showing the core's state rather than a second copy that can disagree
     * with it. The web shell does the same.
     */
    val onFieldEdited: (fieldId: String, value: String) -> Unit = { _, _ -> },
    /** The field lost focus — the core's commit point, where it validates and saves. */
    val onFieldCommitted: (fieldId: String) -> Unit = {},
    /** The bin on a custom network row. */
    val onRemoveNetwork: (id: String) -> Unit = {},
    /**
     * A network row was opened.
     *
     * This is what asks the core to check that endpoint — the row was drawn as
     * tappable from the start and nothing was listening, so the probe arms
     * landed with no caller. (Spec 041 phase 2.)
     */
    val onOpenNetwork: (id: String) -> Unit = {},
    /** Typing in the add-network search. */
    val onSearchNetwork: (String) -> Unit = {},
    /** Choosing one of its results, by chain id. */
    val onPickNetwork: (String) -> Unit = {},
    /** Committing the chosen network, once the core's checks have passed. */
    val onConfirmAddNetwork: () -> Unit = {},
    /** 恢复默认 on the service-endpoints page. */
    val onResetEndpoints: () -> Unit = {},
    /** The endpoints page opened: probe every service (the web's `endpoints_opened`). */
    val onEndpointsOpened: () -> Unit = {},
    // Spec 047 US1: the rows that do what they say.
    val onSegment: (group: String, id: String) -> Unit = { _, _ -> },
    val onTextScale: (Int) -> Unit = {},
    val onStorageClear: (String) -> Unit = {},
    val onClearCaches: () -> Unit = {},
    val onErase: () -> Unit = {},
    /** Spec 048: what the person typed goes into the report. */
    val onFeedbackSend: (what: String, steps: String, screenshots: List<ByteArray>) -> Unit = { _, _, _ -> },
    /** Spec 078 round 3: the sheet closed — the next open starts a fresh report. */
    val onFeedbackClosed: () -> Unit = {},
    /** The sheet opened: an answer that arrives now is shown in it. */
    val onFeedbackOpened: () -> Unit = {},
    /** The page's notice (an answer after the sheet was closed) has been shown. */
    val onFeedbackNoticeShown: () -> Unit = {},
    /** Spec 048: the network detail's RPC / explorer overrides, and the add-network sheet's custom RPC + 重新检查. */
    val onOverrideEdited: (chainId: Long, field: String, value: String) -> Unit = { _, _, _ -> },
    val onOverrideCommitted: (chainId: Long) -> Unit = {},
    val onCustomRpc: (String) -> Unit = {},
    val onRecheckNetwork: () -> Unit = {},
    /** Spec 048: a provider's 检查密钥 / 获取密钥. */
    val onProviderTest: (String) -> Unit = {},
    val onFeedbackGithub: () -> Unit = {},
    val onOpenLink: (String) -> Unit = {},
    val onRelayerRetry: () -> Unit = {},
    /** SR3's 立即重试 on a chain that did not answer, by chain id. */
    val onBalanceRetry: (String) -> Unit = {},
    val onAccountSelect: (Int) -> Unit = {},
    val onAccountPrimary: () -> Unit = {},
    val onAccountSecondary: () -> Unit = {},
    /** The RPC fix sheet's URL being typed, and its Save & Retry / Done. */
    val onRpcFixField: (String) -> Unit = {},
    val onRpcFixPrimary: () -> Unit = {},
    /** Spec 071: the Trusted Signer page, as typed, and back to the official one. */
    val onSignerUrlSave: (String) -> Unit = {},
    val onSignerUrlReset: () -> Unit = {},
    /** Spec 072: a page came on screen — the providers and endpoints pages ask the core to load and test. */
    val onPageShown: (SettingsPage) -> Unit = {},
    /** Spec 072: a sheet came up (or went: `None`) — the account sheet asks for every account's total. */
    val onOverlayShown: (SettingsOverlay) -> Unit = {},
    /** Spec 091: seven taps on About's version revealed the debug-mode switch — store it. The notice and the haptic are the screen's. */
    val onDebugModeRevealed: () -> Unit = {},
    /** Spec 091: the revealed debug-mode switch, turned. */
    val onDebugMode: (on: Boolean) -> Unit = {},
)

/**
 * About's hidden entry (spec 091): the taps on the version so far, kept while
 * About is on screen. What a tap means is the core's rule
 * (`prefsVersionTapped`: seven, each within a second of the one before,
 * nothing once the switch is revealed — and nothing ever in a release build,
 * whose build fact is all this hands over).
 */
internal class VersionTapCounter(
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    private val developerBuild: Boolean = app.getvela.wallet.BuildConfig.DEBUG,
) {
    private var taps = VersionTaps(0u, 0.0)

    /** One tap, with the switch as it stands. `true` exactly when this tap revealed it. */
    fun tap(mode: DebugMode): Boolean {
        val answer = prefsVersionTapped(taps, now(), mode.wire, developerBuild)
        taps = answer.taps
        return answer.revealed
    }
}

@Composable
fun SettingsRoute(
    model: SettingsScreenModel,
    modifier: Modifier = Modifier,
    actions: SettingsActions = SettingsActions(),
) {
    // Seeds, not bindings: a gallery state pins where this opens, and a person
    // tapping owns it from then on.
    // Spec 048: keyed on the model's page too, so a page another route asked for (the add-token 原生代币 tab) wins over a remembered one.
    var page by rememberSaveable(model.state, model.page) { mutableStateOf(model.page) }
    LaunchedEffect(page) { actions.onPageShown(page) }
    // Spec 072: the network a trash tap asked about, until the sheet answers.
    var pendingRemoval by rememberSaveable { mutableStateOf<String?>(null) }
    var overlay by remember(model.state) { mutableStateOf(model.overlay) }
    LaunchedEffect(overlay) { actions.onOverlayShown(overlay) }
    // The storage row waiting on an answer, and the warning its group carries
    // (spec 058): 清除 asks before it removes.
    var pendingStorage by remember { mutableStateOf<Pair<StorageItemModel, String>?>(null) }
    var advancedOpen by rememberSaveable(model.state) {
        mutableStateOf(model.state == SettingsScreenState.ST1B)
    }

    // A report whose sheet was closed mid-send ends HERE, visibly: a notice
    // with the way onward (the founder: 反馈成功或失败都要有提示).
    val noticeHost = remember { SnackbarHostState() }
    val notice = model.feedback.notice
    LaunchedEffect(notice) {
        if (notice == null) return@LaunchedEffect
        val result = noticeHost.showSnackbar(message = notice.message, actionLabel = notice.action, duration = SnackbarDuration.Long)
        if (result == SnackbarResult.ActionPerformed) actions.onOpenLink(notice.url)
        actions.onFeedbackNoticeShown()
    }
    // Spec 091: About's hidden entry. Revealing says so once — the haptic and
    // the page's notice — and the switch is the preferences' from then on.
    val versionTaps = remember(page) { VersionTapCounter() }
    val tapHaptic = rememberVelaHaptic()
    val noticeScope = rememberCoroutineScope()
    val debugMode = model.about.debugMode
    Box(modifier = modifier.fillMaxSize()) {
    SettingsScreen(
        model = model,
        page = page,
        overlay = overlay,
        advancedOpen = advancedOpen,
        modifier = Modifier.fillMaxSize(),
        onRow = { id ->
            when (id) {
                "contacts" -> actions.onOpenContacts()
                SettingsLive.ETHEREUM_BACKUP_ROW -> actions.onEthereumBackup()
                "networks" -> page = SettingsPage.Networks
                "rpc-providers" -> page = SettingsPage.RpcProviders
                "add-network" -> page = SettingsPage.AddNetwork
                "endpoints" -> {
                    page = SettingsPage.Endpoints
                    // Without this the pills had nothing to say: the probe wave
                    // (`openEndpoints`) was defined and never called (device-found).
                    actions.onEndpointsOpened()
                }
                "storage" -> page = SettingsPage.Storage
                "about" -> page = SettingsPage.About
                "language" -> overlay = SettingsOverlay.Language
                "currency" -> overlay = SettingsOverlay.Currency
                SettingsFixtures.FEE_SPEED_ROW -> overlay = SettingsOverlay.FeeSpeed
                SettingsFixtures.SIGNER_PAGE_ROW -> overlay = SettingsOverlay.SignerPage
                "number-format" -> overlay = SettingsOverlay.NumberFormat
                "date-format" -> overlay = SettingsOverlay.DateFormat
                "time-format" -> overlay = SettingsOverlay.TimeFormat
                "feedback" -> overlay = SettingsOverlay.Feedback
                // Community: out to the brand's own page (its app takes over when installed).
                else -> CommunityLinks.urlFor(id)?.let(actions.onOpenLink)
            }
        },
        onBack = { page = SettingsPage.Home },
        onToggleAdvanced = { advancedOpen = !advancedOpen },
        onOpenOverlay = { overlay = it },
        onDismissOverlay = { overlay = SettingsOverlay.None },
        onSheetSelect = { sheet, id ->
            actions.onSheetSelect(sheet, id)
            // The sheet closes on the pick, before the core has answered. The
            // alternative — waiting for the view to come back — leaves a
            // person tapping a row that visibly does nothing while a storage
            // write completes.
            overlay = SettingsOverlay.None
        },
        onSelectTab = actions.onSelectTab,
        onSignOut = actions.onSignOut,
        onFieldEdited = actions.onFieldEdited,
        onFieldCommitted = actions.onFieldCommitted,
        onRemoveNetwork = { id -> pendingRemoval = id; overlay = SettingsOverlay.RemoveNetwork },
        onConfirmRemoveNetwork = {
            pendingRemoval?.let(actions.onRemoveNetwork)
            pendingRemoval = null
            overlay = SettingsOverlay.None
        },
        // Spec 048: a network row opens ITS detail page (the row only expanded the core's override before).
        onOpenNetwork = { id -> actions.onOpenNetwork(id); page = SettingsPage.NetworkDetail },
        onSearchNetwork = actions.onSearchNetwork,
        onPickNetwork = actions.onPickNetwork,
        onConfirmAddNetwork = actions.onConfirmAddNetwork,
        onResetEndpoints = { overlay = SettingsOverlay.ResetEndpoints },
        onConfirmResetEndpoints = {
            overlay = SettingsOverlay.None
            actions.onResetEndpoints()
        },
        onSegment = actions.onSegment,
        onTextScale = actions.onTextScale,
        onStorageClear = { id ->
            val group = model.storage.groups.firstOrNull { g -> g.items.any { it.id == id } }
            val item = group?.items?.firstOrNull { it.id == id }
            if (item != null) {
                pendingStorage = item to group.label
                overlay = SettingsOverlay.ClearStorageItem
            }
        },
        storageConfirm = pendingStorage?.let { (item, warning) ->
            ConfirmSheetModel(
                title = item.label,
                body = warning,
                confirm = item.action,
                cancel = model.clearCachesSheet.cancel,
                danger = item.destructive,
            )
        },
        onConfirmStorage = {
            pendingStorage?.let { (item, _) -> actions.onStorageClear(item.id) }
            pendingStorage = null
            overlay = SettingsOverlay.None
        },
        onClearCaches = { actions.onClearCaches(); overlay = SettingsOverlay.None },
        onErase = actions.onErase,
        onFeedbackSend = actions.onFeedbackSend,
        onOverrideEdited = actions.onOverrideEdited,
        onOverrideCommitted = actions.onOverrideCommitted,
        onCustomRpc = actions.onCustomRpc,
        onRecheckNetwork = actions.onRecheckNetwork,
        onProviderTest = actions.onProviderTest,
        onFeedbackGithub = actions.onFeedbackGithub,
        onFeedbackClosed = actions.onFeedbackClosed,
        onOpenLink = actions.onOpenLink,
        onRelayerRetry = actions.onRelayerRetry,
        onBalanceRetry = actions.onBalanceRetry,
        onAccountSelect = { index -> actions.onAccountSelect(index); overlay = SettingsOverlay.None },
        onAccountPrimary = actions.onAccountPrimary,
        onAccountSecondary = actions.onAccountSecondary,
        onRpcFixField = actions.onRpcFixField,
        onRpcFixPrimary = {
            // Save & Retry keeps the sheet up to show the probe's answer;
            // Done (the probe said ok) is the one that closes it.
            val close = model.rpcFix.restored
            actions.onRpcFixPrimary()
            if (close) overlay = SettingsOverlay.None
        },
        onSignerUrlSave = actions.onSignerUrlSave,
        onSignerUrlReset = actions.onSignerUrlReset,
        onFeedbackOpened = actions.onFeedbackOpened,
        onVersionTap = {
            if (versionTaps.tap(debugMode.mode)) {
                actions.onDebugModeRevealed()
                tapHaptic(VelaHaptic.Success)
                noticeScope.launch { noticeHost.showSnackbar(debugMode.revealedNotice) }
            }
        },
        onDebugMode = actions.onDebugMode,
    )
    // Above the tab bar and the system navigation bar — never under them
    // (device pass: at the largest size the notice sat half behind both).
    SnackbarHost(
        hostState = noticeHost,
        modifier = Modifier
            .align(Alignment.BottomCenter)
            .navigationBarsPadding()
            .padding(bottom = VelaSizing.tabBar + VelaSpacing.md)
            .padding(horizontal = VelaSizing.screenPaddingX),
    ) { data -> FeedbackNoticeBar(data) }
    }
}

/**
 * The page's notice: a dark pill with the answer and its one action — the
 * app's toast shape, with room for the way onward. A notice with nothing to
 * do next (spec 091: "debug mode is now available") is the message alone.
 */
@Composable
private fun FeedbackNoticeBar(data: SnackbarData) {
    val colors = VelaTheme.colors
    val pill = Modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(VelaRadius.lg))
        .background(colors.fgBase)
        .padding(start = VelaSpacing.lg, top = VelaSpacing.sm, bottom = VelaSpacing.sm, end = VelaSpacing.sm)
    val message: @Composable () -> Unit = {
        Text(
            text = data.visuals.message,
            color = colors.bgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(vertical = VelaSpacing.sm),
        )
    }
    // `VelaLabelBesideValue` needs both parts: with no action it has no value
    // to measure (the app died on the first action-less notice).
    val action = data.visuals.actionLabel
    if (action == null) {
        Box(modifier = pill) { message() }
        return
    }
    // The message whole and the action beside it when both fit; otherwise the
    // action takes its own line under the message (never a word broken to
    // make room — the largest size wrapped "发送" alone onto a second line).
    VelaLabelBesideValue(
        modifier = pill,
        gap = VelaSpacing.md,
        rowGap = 0.dp,
        label = message,
        value = {
            Text(
                text = action,
                color = colors.bgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
                modifier = Modifier
                    .clip(RoundedCornerShape(VelaRadius.md))
                    .clickable(role = Role.Button) { data.performAction() }
                    .heightIn(min = VelaSizing.controlSm)
                    .wrapContentHeight(Alignment.CenterVertically)
                    .padding(horizontal = VelaSpacing.md),
            )
        },
    )
}

@Composable
@Suppress("LongParameterList", "LongMethod")
fun SettingsScreen(
    model: SettingsScreenModel,
    page: SettingsPage,
    overlay: SettingsOverlay,
    advancedOpen: Boolean,
    modifier: Modifier = Modifier,
    onRow: (String) -> Unit = {},
    onBack: () -> Unit = {},
    onToggleAdvanced: () -> Unit = {},
    onOpenOverlay: (SettingsOverlay) -> Unit = {},
    onDismissOverlay: () -> Unit = {},
    onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
    onSelectTab: (VelaTab) -> Unit = {},
    onSignOut: () -> Unit = {},
    onFieldEdited: (String, String) -> Unit = { _, _ -> },
    onFieldCommitted: (String) -> Unit = {},
    onRemoveNetwork: (String) -> Unit = {},
    onConfirmRemoveNetwork: () -> Unit = {},
    onConfirmResetEndpoints: () -> Unit = {},
    onOpenNetwork: (String) -> Unit = {},
    onSearchNetwork: (String) -> Unit = {},
    onPickNetwork: (String) -> Unit = {},
    onConfirmAddNetwork: () -> Unit = {},
    onResetEndpoints: () -> Unit = {},
    onSegment: (String, String) -> Unit = { _, _ -> },
    onTextScale: (Int) -> Unit = {},
    onStorageClear: (String) -> Unit = {},
    /** Spec 058: the question a storage row's 清除 asks first. */
    storageConfirm: ConfirmSheetModel? = null,
    onConfirmStorage: () -> Unit = {},
    onClearCaches: () -> Unit = {},
    onErase: () -> Unit = {},
    onOverrideEdited: (Long, String, String) -> Unit = { _, _, _ -> },
    onOverrideCommitted: (Long) -> Unit = {},
    onCustomRpc: (String) -> Unit = {},
    onRecheckNetwork: () -> Unit = {},
    onProviderTest: (String) -> Unit = {},
    onFeedbackSend: (String, String, List<ByteArray>) -> Unit = { _, _, _ -> },
    onFeedbackGithub: () -> Unit = {},
    onFeedbackClosed: () -> Unit = {},
    onFeedbackOpened: () -> Unit = {},
    onOpenLink: (String) -> Unit = {},
    onRelayerRetry: () -> Unit = {},
    onBalanceRetry: (String) -> Unit = {},
    onAccountSelect: (Int) -> Unit = {},
    onAccountPrimary: () -> Unit = {},
    onAccountSecondary: () -> Unit = {},
    onRpcFixField: (String) -> Unit = {},
    onRpcFixPrimary: () -> Unit = {},
    onSignerUrlSave: (String) -> Unit = {},
    onSignerUrlReset: () -> Unit = {},
    /** Spec 091: a tap on About's version — the hidden entry. */
    onVersionTap: () -> Unit = {},
    /** Spec 091: About's debug-mode switch, turned. */
    onDebugMode: (Boolean) -> Unit = {},
) {
    val colors = VelaTheme.colors

    // SR5 replaces the whole screen: it blocks both creating and signing in, so
    // there is nothing behind it to go back to.
    if (model.state == SettingsScreenState.SR5) {
        IndexDownScreen(model.indexDown, modifier = modifier)
        return
    }

    // SR2–SR4 are sheets over ANOTHER screen (the wallet, the send flow), so
    // the body behind them is a dimmed title rather than the settings list.
    val rescue = model.selectedTab == "wallet"

    // Spec 081: tapping the page is how a person leaves a field they have
    // finished typing in, and Compose keeps focus until something takes it.
    // An endpoint commits on focus loss, so without this a typed URL sat
    // uncommitted while the health badge went on reporting the OLD host as
    // online — measured on the Xiaomi, showing "Online · 779ms" for
    // `https://index.invalid`. Back only hides the keyboard; it does not
    // clear focus either.
    val focusManager = LocalFocusManager.current
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .pointerInput(Unit) {
                detectTapGestures(onTap = { focusManager.clearFocus() })
            },
    ) {
        Column(modifier = Modifier.fillMaxSize().safeDrawingPadding()) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = VelaSizing.screenPaddingX),
            ) {
                when {
                    rescue -> {
                        Text(
                            text = model.backdropTitle,
                            color = colors.fgSubtle.copy(alpha = VelaOpacity.dim),
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl3,
                            modifier = Modifier.padding(top = VelaSpacing.xl4, bottom = VelaSpacing.xl),
                        )
                        if (model.rpcBanner != null) {
                            RpcBanner(model.rpcBanner)
                        }
                    }
                    page == SettingsPage.Home -> {
                        Text(
                            text = model.title,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl3,
                            modifier = Modifier.padding(top = VelaSpacing.xl4, bottom = VelaSpacing.xl),
                        )
                        SettingsHomeBody(
                            model = model,
                            advancedOpen = advancedOpen,
                            onRow = onRow,
                            onToggleAdvanced = onToggleAdvanced,
                            onOpenOverlay = onOpenOverlay,
                            onSignOut = onSignOut,
                            onSegment = onSegment,
                            onTextScale = onTextScale,
                        )
                    }
                    else -> {
                        val (title, subtitle) = pageHeader(model, page)
                        SettingsNavHeader(title, subtitle, model.closeLabel, onBack)
                        SettingsPageBody(
                            model = model,
                            page = page,
                            onOpenOverlay = onOpenOverlay,
                            onFieldEdited = onFieldEdited,
                            onFieldCommitted = onFieldCommitted,
                            onRemoveNetwork = onRemoveNetwork,
                            onOpenNetwork = onOpenNetwork,
                            onSearchNetwork = onSearchNetwork,
                            onPickNetwork = onPickNetwork,
                            onConfirmAddNetwork = onConfirmAddNetwork,
                            onResetEndpoints = onResetEndpoints,
                            onStorageClear = onStorageClear,
                            onOpenLink = onOpenLink,
                            onOverrideEdited = onOverrideEdited,
                            onOverrideCommitted = onOverrideCommitted,
                            onCustomRpc = onCustomRpc,
                            onRecheckNetwork = onRecheckNetwork,
                            onProviderTest = onProviderTest,
                            onAddNetwork = { onRow("add-network") },
                            onVersionTap = onVersionTap,
                            onDebugMode = onDebugMode,
                        )
                    }
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            }
            VelaTabBar(
                tabs = model.tabs,
                selected = if (rescue) VelaTab.Wallet else VelaTab.Settings,
                onSelect = onSelectTab,
            )
        }

        if (overlay != SettingsOverlay.None) {
            SettingsSheet(
                model = model,
                overlay = overlay,
                onDismiss = onDismissOverlay,
                onSignOut = onSignOut,
                onSheetSelect = onSheetSelect,
                storageConfirm = storageConfirm,
                onConfirmStorage = onConfirmStorage,
                onConfirmRemoveNetwork = onConfirmRemoveNetwork,
                onConfirmResetEndpoints = onConfirmResetEndpoints,
                onClearCaches = onClearCaches,
                onErase = onErase,
                onFeedbackSend = onFeedbackSend,
                onFeedbackGithub = onFeedbackGithub,
                onFeedbackClosed = onFeedbackClosed,
                onFeedbackOpened = onFeedbackOpened,
                onOpenLink = onOpenLink,
                onRelayerRetry = onRelayerRetry,
                onBalanceRetry = onBalanceRetry,
                onAccountSelect = onAccountSelect,
                onAccountPrimary = onAccountPrimary,
                onAccountSecondary = onAccountSecondary,
                onRpcFixField = onRpcFixField,
                onRpcFixPrimary = onRpcFixPrimary,
                onSignerUrlSave = onSignerUrlSave,
                onSignerUrlReset = onSignerUrlReset,
            )
        }
    }
}

private fun pageHeader(model: SettingsScreenModel, page: SettingsPage): Pair<String, String?> =
    when (page) {
        SettingsPage.Networks -> model.networksTitle to model.networksSubtitle
        SettingsPage.NetworkDetail -> model.networkDetail.title to model.networkDetail.subtitle
        SettingsPage.AddNetwork -> model.addNetwork.title to model.addNetwork.subtitle
        SettingsPage.RpcProviders -> model.rpcProviders.title to model.rpcProviders.subtitle
        SettingsPage.Endpoints -> model.endpoints.title to null
        SettingsPage.Storage -> model.storage.title to model.storage.subtitle
        SettingsPage.About -> model.about.title to null
        SettingsPage.Home -> model.title to null
    }

/** Back arrow + title + optional second line (ST9/ST9b/ST10/ST11/ST12/…). */
@Composable
private fun SettingsNavHeader(
    title: String,
    subtitle: String?,
    backLabel: String,
    onBack: () -> Unit,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.xl, bottom = VelaSpacing.lg),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Icon(
            imageVector = VelaIcons.ChevronLeft,
            contentDescription = backLabel,
            tint = colors.fgBase,
            modifier = Modifier
                .size(VelaSizing.hitTarget)
                .clickable(onClick = onBack)
                .padding(VelaSpacing.lg),
        )
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}

@Composable
private fun SettingsHomeBody(
    model: SettingsScreenModel,
    advancedOpen: Boolean,
    onRow: (String) -> Unit,
    onToggleAdvanced: () -> Unit,
    onOpenOverlay: (SettingsOverlay) -> Unit,
    onSignOut: () -> Unit = {},
    onSegment: (String, String) -> Unit = { _, _ -> },
    onTextScale: (Int) -> Unit = {},
) {
    VelaAccountRow(model.account) { onOpenOverlay(SettingsOverlay.Accounts) }

    // Under the account it belongs to (spec 062): which keys, then their backup.
    model.keys?.let { VelaWalletKeysBlock(it, onRow) }

    model.sections.forEach { section ->
        if (section.label != null) {
            SettingsSectionLabel(
                label = section.label,
                collapsible = section.collapsible,
                collapsed = section.collapsible && !advancedOpen,
                onToggle = onToggleAdvanced,
            )
        }
        val hidden = section.collapsible && !advancedOpen
        if (!hidden) {
            section.rows.forEachIndexed { index, row ->
                VelaSettingsRow(
                    row = row,
                    divider = index < section.rows.lastIndex,
                    onClick = onRow,
                )
            }
        }
        // The two appearance controls are not rows: they are the control
        // itself, shown inline under 语言 (ST1).
        if (section.appearanceControls) {
            VelaTextScaleSlider(
                steps = model.textScale.steps,
                index = model.textScale.index,
                label = model.textScale.label,
                onChange = onTextScale,
            )
            VelaSegmentedControl(
                label = model.theme.label,
                segments = model.theme.segments.map { seg ->
                    Triple(seg.id, seg.label, seg.icon?.let(::settingsIcon))
                },
                selectedId = model.theme.selected,
                onSelect = { onSegment("theme", it) },
            )
        }
    }

    // Issue #322: Sign Out is a settings ROW — the glyph, the title and the
    // chevron its neighbours carry — not centred grey text, which read as a
    // caption under About rather than something to tap.
    //
    // It ASKS THE CORE. The session machine answers with its own sheet — the
    // one carrying the pending-upload warning and a way back out — and that
    // sheet IS the confirmation. Raising ST3 in front of it made leaving a
    // wallet three taps and two sheets saying the same sentence (founder,
    // 2026-09-16); ST3/ST3b stay the fixture boards they always were,
    // reachable from a seeded overlay.
    Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    VelaSettingsRow(
        row = SettingsRowModel(id = "sign-out", title = model.signOutLabel, icon = SettingsIcon.LogOut),
        divider = false,
        onClick = { onSignOut() },
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    VelaDangerCard(model.eraseTitle, model.eraseSubtitle) {
        onOpenOverlay(SettingsOverlay.EraseDevice)
    }
}

/**
 * A [VelaUrlField] wired to the core.
 *
 * Focus is what commits. The core validates and saves on blur — not on every
 * keystroke — so a field that reported only its edits would let a person type a
 * perfectly good RPC URL that never reached storage, with the box still showing
 * it. Both halves or neither.
 */
@Composable
private fun EditableUrlField(
    field: UrlFieldModel,
    onEdited: (String, String) -> Unit,
    onCommitted: (String) -> Unit,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    var focused by remember(field.id) { mutableStateOf(false) }
    VelaUrlField(
        label = field.label,
        value = field.value,
        placeholder = field.placeholder,
        hint = field.hint,
        badge = field.badge,
        tone = field.tone,
        action = action,
        onAction = onAction,
        onValueChange = { value -> onEdited(field.id, value) },
        modifier = Modifier.onFocusChanged { state ->
            if (focused && !state.isFocused) onCommitted(field.id)
            focused = state.isFocused
        },
    )
}

@Composable
@Suppress("LongMethod")
private fun SettingsPageBody(
    model: SettingsScreenModel,
    page: SettingsPage,
    onOpenOverlay: (SettingsOverlay) -> Unit,
    onFieldEdited: (String, String) -> Unit = { _, _ -> },
    onFieldCommitted: (String) -> Unit = {},
    onRemoveNetwork: (String) -> Unit = {},
    onOpenNetwork: (String) -> Unit = {},
    onSearchNetwork: (String) -> Unit = {},
    onPickNetwork: (String) -> Unit = {},
    onConfirmAddNetwork: () -> Unit = {},
    onResetEndpoints: () -> Unit = {},
    onStorageClear: (String) -> Unit = {},
    onOpenLink: (String) -> Unit = {},
    onOverrideEdited: (Long, String, String) -> Unit = { _, _, _ -> },
    onOverrideCommitted: (Long) -> Unit = {},
    onCustomRpc: (String) -> Unit = {},
    onRecheckNetwork: () -> Unit = {},
    onProviderTest: (String) -> Unit = {},
    onAddNetwork: () -> Unit = {},
    onVersionTap: () -> Unit = {},
    onDebugMode: (Boolean) -> Unit = {},
) {
    val colors = VelaTheme.colors
    when (page) {
        SettingsPage.Networks -> {
            model.networks.forEach { row ->
                VelaNetworkRow(
                    row = row,
                    deleteLabel = model.removeNetworkLabel.ifEmpty { null },
                    onClick = onOpenNetwork,
                    onDelete = onRemoveNetwork,
                )
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onAddNetwork)
                    .padding(top = VelaSpacing.xl3),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = VelaIcons.Plus,
                    contentDescription = null,
                    tint = colors.infoBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
                Spacer(modifier = Modifier.size(VelaSpacing.md))
                // A link, not a CTA: adding a network is navigation, and accent
                // is reserved for actions that move value.
                Text(
                    text = model.addNetworkLabel,
                    color = colors.infoBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
            }
        }

        SettingsPage.NetworkDetail -> {
            val detail = model.networkDetail
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl3),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                VelaChainMark(detail.mark)
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = detail.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = VelaTextSize.xl,
                    )
                    Text(
                        text = detail.note,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
                VelaStatusPill(detail.badge)
            }
            // Spec 048: the overrides are typed here (the web's NetworkDetailPanel);
            // committed to the core when the field loses focus.
            EditableUrlField(
                field = detail.rpc,
                onEdited = { id, value -> onOverrideEdited(detail.chainId, id, value) },
                onCommitted = { onOverrideCommitted(detail.chainId) },
            )
            if (detail.callout != null) {
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                VelaCallout(detail.callout)
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            EditableUrlField(
                field = detail.explorer,
                onEdited = { id, value -> onOverrideEdited(detail.chainId, id, value) },
                onCommitted = { onOverrideCommitted(detail.chainId) },
            )
        }

        SettingsPage.AddNetwork -> {
            val add = model.addNetwork
            if (add.candidate == null) {
                // The box was drawn read-only with an empty value, so the
                // fixture's three results sat under a search nobody could
                // perform. `onValueChange` has been on this component all
                // along; nothing was passing one.
                VelaUrlField(
                    label = "",
                    value = add.query,
                    placeholder = add.searchPlaceholder,
                    onValueChange = onSearchNetwork,
                    keyboard = KeyboardType.Text,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                add.results.forEach { row ->
                    VelaNetworkRow(row = row, onClick = onPickNetwork)
                }
            } else {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                ) {
                    VelaChainMark(add.candidate.mark)
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = add.candidate.name,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl,
                        )
                        Text(
                            text = add.candidate.meta,
                            color = colors.fgSubtle,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                    if (add.candidate.badge != null) VelaStatusPill(add.candidate.badge)
                }
                if (add.checksTitle != null) {
                    VelaCheckList(add.checksTitle, add.checks)
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                if (add.customRpc != null) {
                    VelaUrlField(
                        label = add.customRpc.label,
                        value = add.customRpc.value,
                        placeholder = add.customRpc.placeholder,
                        onValueChange = onCustomRpc,
                    )
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                if (add.callout != null) {
                    VelaCallout(add.callout)
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                // An outline CTA plus a re-check link when it cannot be added:
                // an action you cannot take should not be dressed as the action
                // you came for.
                if (add.primary != null) {
                    // The button that writes a network somebody's money will be
                    // read from. It was drawn with an empty handler; the core
                    // only offers it when its own checks passed.
                    VelaPrimaryButton(
                        add.primary,
                        onClick = onConfirmAddNetwork,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (add.secondary != null) {
                    VelaSecondaryButton(add.secondary, onClick = {}, modifier = Modifier.fillMaxWidth())
                }
                if (add.recheck != null) {
                    Text(
                        text = add.recheck,
                        color = colors.infoBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.clickable(onClick = onRecheckNetwork).fillMaxWidth().padding(top = VelaSpacing.xl),
                    )
                }
            }
        }

        SettingsPage.RpcProviders -> {
            Text(
                text = model.rpcProviders.description,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.xl3),
            )
            model.rpcProviders.providers.forEach { provider ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = provider.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = VelaTextSize.xl,
                        modifier = Modifier.weight(1f),
                    )
                    VelaStatusPill(provider.badge)
                }
                EditableUrlField(
                    field = provider.field,
                    action = provider.action,
                    onEdited = onFieldEdited,
                    onCommitted = onFieldCommitted,
                    onAction = { onProviderTest(provider.id) },
                )
                if (provider.support != null) {
                    Text(
                        text = provider.support,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.padding(top = VelaSpacing.md),
                    )
                }
                if (provider.link != null && provider.linkUrl != null) {
                    Text(
                        text = provider.link,
                        color = colors.infoBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.clickable { onOpenLink(provider.linkUrl) }.padding(top = VelaSpacing.md),
                    )
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            }
        }

        SettingsPage.Endpoints -> {
            Text(
                text = model.endpoints.description,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.xl3),
            )
            model.endpoints.fields.forEach { field ->
                EditableUrlField(
                    field = field,
                    onEdited = onFieldEdited,
                    onCommitted = onFieldCommitted,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            }
            // Drawn as a label in spec 023 and never given a click — found on
            // a device by tapping it and watching the store not change. The
            // core has had `reset_endpoints_to_defaults` all along.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onResetEndpoints)
                    .padding(top = VelaSpacing.xl),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = VelaIcons.RefreshCw,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                Spacer(modifier = Modifier.size(VelaSpacing.md))
                Text(
                    text = model.endpoints.reset,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }

        SettingsPage.Storage -> {
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Text(
                    text = model.storage.amount,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl4,
                )
                Text(
                    text = model.storage.unit,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = model.storage.summary,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            VelaStorageBar(model.storage.segments)
            model.storage.groups.forEach { group ->
                VelaStorageGroup(
                    group = group,
                    onClear = onStorageClear,
                    onGroupAction = { onOpenOverlay(SettingsOverlay.ClearCaches) },
                )
            }
        }

        SettingsPage.About -> {
            Column(
                modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.xl3),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
            ) {
                Text(
                    text = model.about.tagline,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = model.about.version,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                    // Spec 091: the hidden entry — seven quick taps reveal debug
                    // mode. Nothing marks it as a control: no ripple.
                    modifier = Modifier.clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = onVersionTap,
                    ),
                )
            }
            Text(
                text = model.about.sectionTechnical,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
            model.about.rows.forEach { VelaKeyValueRow(it) }
            // Spec 091: once revealed, the switch stays here — so it can be
            // turned off again.
            val debugMode = model.about.debugMode
            if (debugMode.mode.revealed) {
                VelaSwitchRow(
                    title = debugMode.title,
                    body = debugMode.body,
                    checked = debugMode.mode.on,
                    onCheckedChange = onDebugMode,
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            model.about.links.forEach { VelaKeyValueRow(it, modifier = Modifier.clickable { onOpenLink(it.value) }) }
            Text(
                text = model.about.footer,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.xl3),
            )
        }

        SettingsPage.Home -> Unit
    }
}

/**
 * SR1's amber banner: the count of unreachable networks, then one chip per
 * network with its own 修复. Per-chain rather than one global button, because
 * the fix IS per chain — a shared button would have to ask which one first.
 */
@Composable
private fun RpcBanner(banner: RpcBannerModel) {
    val colors = VelaTheme.colors
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg)
            .background(colors.warningSoft)
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Icon(
                imageVector = VelaIcons.TriangleAlert,
                contentDescription = null,
                tint = colors.warningBase,
                modifier = Modifier.size(VelaIconSize.md),
            )
            Text(
                text = banner.text,
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            banner.chips.forEach { chip ->
                Row(
                    modifier = Modifier
                        .background(colors.bgBase)
                        .padding(VelaSpacing.md),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    VelaChainMark(chip.mark, size = VelaIconSize.xl)
                    Text(
                        text = chip.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    // The only accent on this banner: the thing that fixes it.
                    Text(
                        text = chip.action,
                        color = colors.accentBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
        }
    }
}

/**
 * SR5 — the passkey index is unreachable. The endpoint is editable right here,
 * because "the service is down" and "you pointed it at the wrong host" look
 * identical from the inside, and only one is something the person can fix.
 */
@Composable
private fun IndexDownScreen(model: IndexDownModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .safeDrawingPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX, vertical = VelaSpacing.xl5),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        Text(
            text = model.title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl3,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            text = model.subtitle,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        VelaCallout(model.callout)
        VelaUrlField(
            label = model.field.label,
            value = model.field.value,
            badge = model.field.badge,
        )
        VelaPrimaryButton(model.primary, onClick = {}, modifier = Modifier.fillMaxWidth())
        VelaSecondaryButton(model.secondary, onClick = {}, modifier = Modifier.fillMaxWidth())
        Text(
            text = model.footer,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * Every overlay the phone draws as a bottom sheet — ONE host whose content
 * swaps, never a sheet stacked on a sheet. Internal since spec 092: the home
 * hosts its status-line rescues (SR6's list, a row's SR2, SR3) over the
 * wallet with the same bodies, instead of sending the person to Settings.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
@Suppress("LongMethod")
internal fun SettingsSheet(
    model: SettingsScreenModel,
    overlay: SettingsOverlay,
    /** A swipe down, the scrim or Back — Material has hidden the sheet already. */
    onDismiss: () -> Unit,
    onSignOut: () -> Unit,
    /** The sheet's own ✕. The same as [onDismiss] unless a host steps back instead (spec 092). */
    onClose: () -> Unit = onDismiss,
    onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
    storageConfirm: ConfirmSheetModel? = null,
    onConfirmStorage: () -> Unit = {},
    onConfirmRemoveNetwork: () -> Unit = {},
    onConfirmResetEndpoints: () -> Unit = {},
    onClearCaches: () -> Unit = {},
    onErase: () -> Unit = {},
    onFeedbackSend: (String, String, List<ByteArray>) -> Unit = { _, _, _ -> },
    onFeedbackGithub: () -> Unit = {},
    onFeedbackClosed: () -> Unit = {},
    onFeedbackOpened: () -> Unit = {},
    onOpenLink: (String) -> Unit = {},
    onRelayerRetry: () -> Unit = {},
    onBalanceRetry: (String) -> Unit = {},
    onUnreachableFix: (Int) -> Unit = {},
    onAccountSelect: (Int) -> Unit = {},
    onAccountPrimary: () -> Unit = {},
    onAccountSecondary: () -> Unit = {},
    onRpcFixField: (String) -> Unit = {},
    onRpcFixPrimary: () -> Unit = {},
    onSignerUrlSave: (String) -> Unit = {},
    onSignerUrlReset: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    // The shared host (VelaModalSheet) carries the app's text size into the
    // sheet's own window — without it every settings sheet stayed at the
    // standard size (found checking the report sheet at the largest size) —
    // and keeps the sheet still under a fling (its size never follows its
    // offset; upward overscroll never reaches it).
    VelaModalSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.bgBase,
        followAppTextSize = true,
    ) {
      // The ✕ is the host's, handed to every body's SheetTitle, which puts it
      // in its own column beside the title: every sheet opens with one, so
      // none can forget it. It used to float over the scrolling content —
      // over the fifth screenshot's ✕ (a remove tap closed the sheet), over
      // a scrolled field, over the subtitle (design review, 078 round 3).
      // The drag handle alone is not an affordance a first-time reader knows.
      // The cap is what makes the scroll below mean anything: a wrap-height
      // column has no overflow to scroll, so verticalScroll alone silently
      // did nothing and the sheet still ended at Português.
      val maxSheetHeight = (LocalConfiguration.current.screenHeightDp * 0.88f).dp
      val sheetScroll = rememberScrollState()
      CompositionLocalProvider(
          LocalSheetScroll provides sheetScroll,
          LocalSheetClose provides SheetClose(model.closeLabel, onClose),
          LocalSheetBodyMax provides maxSheetHeight - VelaSpacing.xl3,
      ) {
        Box(modifier = Modifier.fillMaxWidth().heightIn(max = maxSheetHeight)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                // Without this the language sheet simply ends at Português:
                // fifteen locales are taller than the sheet, and the three
                // below the fold — plus the contribute footer — were
                // unreachable. Every sheet here can outgrow the screen once a
                // translation runs long, so the scroll belongs to the host.
                .verticalScroll(sheetScroll)
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
        ) {
            when (overlay) {
                SettingsOverlay.Accounts -> AccountsSheetBody(model.accountsSheet, onSelect = onAccountSelect, onPrimary = onAccountPrimary, onSecondary = onAccountSecondary)
                SettingsOverlay.SignOut -> ConfirmSheetBody(
                    model.signOutSheet,
                    onConfirm = onSignOut,
                    onCancel = onDismiss,
                )
                SettingsOverlay.Language -> SelectSheetBody(model.languageSheet, onFooterLink = { onOpenLink("https://github.com/mondaylabsltd/vela-wallet/issues") }) {
                    onSheetSelect(SettingsOverlay.Language, it)
                }
                SettingsOverlay.Currency -> SelectSheetBody(model.currencySheet) {
                    onSheetSelect(SettingsOverlay.Currency, it)
                }
                SettingsOverlay.FeeSpeed -> SelectSheetBody(model.feeSpeedSheet) {
                    onSheetSelect(SettingsOverlay.FeeSpeed, it)
                }
                SettingsOverlay.SignerPage -> SignerPageSheetBody(
                    model.signerPage,
                    onSave = onSignerUrlSave,
                    onReset = onSignerUrlReset,
                    onDone = onDismiss,
                )
                SettingsOverlay.NumberFormat -> SelectSheetBody(model.numberSheet) {
                    onSheetSelect(SettingsOverlay.NumberFormat, it)
                }
                SettingsOverlay.DateFormat -> SelectSheetBody(model.dateSheet) {
                    onSheetSelect(SettingsOverlay.DateFormat, it)
                }
                SettingsOverlay.TimeFormat -> SelectSheetBody(model.timeSheet) {
                    onSheetSelect(SettingsOverlay.TimeFormat, it)
                }
                SettingsOverlay.RemoveNetwork -> ConfirmSheetBody(
                    model.removeNetworkSheet,
                    onConfirm = onConfirmRemoveNetwork,
                    onCancel = onDismiss,
                )
                SettingsOverlay.ResetEndpoints -> ConfirmSheetBody(
                    model.resetEndpointsSheet,
                    onConfirm = onConfirmResetEndpoints,
                    onCancel = onDismiss,
                )
                SettingsOverlay.ClearStorageItem -> storageConfirm?.let { sheet ->
                    ConfirmSheetBody(sheet, onConfirm = onConfirmStorage, onCancel = onDismiss)
                }
                SettingsOverlay.ClearCaches -> ConfirmSheetBody(
                    model.clearCachesSheet,
                    onConfirm = onClearCaches,
                    onCancel = onDismiss,
                )
                SettingsOverlay.EraseDevice -> ConfirmSheetBody(
                    model.eraseSheet,
                    onConfirm = onErase,
                    onCancel = onDismiss,
                )
                SettingsOverlay.Feedback -> FeedbackSheetBody(model.feedback, onSend = onFeedbackSend, onGithub = onFeedbackGithub, onOpen = onOpenLink, onDone = onDismiss, onOpened = onFeedbackOpened, onClosed = onFeedbackClosed)
                SettingsOverlay.RpcFix -> RpcFixSheetBody(model.rpcFix, onRpcFixPrimary, onRpcFixField)
                SettingsOverlay.BalanceDetail -> BalanceDetailSheetBody(model.balanceDetail, onBalanceRetry)
                SettingsOverlay.Unreachable -> UnreachableSheetBody(model.unreachable, onUnreachableFix)
                SettingsOverlay.Relayer -> RelayerSheetBody(model.relayer, onRelayerRetry)
                SettingsOverlay.None -> Unit
            }
        }
        }
      }
    }
}

/** The settings sheet host's ✕: what it says to TalkBack and what it does. */
private class SheetClose(val label: String, val onClose: () -> Unit)

/** Provided by the settings sheet host; a body drawn elsewhere (the account switcher) gets none. */
private val LocalSheetClose = staticCompositionLocalOf<SheetClose?> { null }

/** The host's scroll, for a body that must move it without an animation (the report's fallback). */
private val LocalSheetScroll = staticCompositionLocalOf<ScrollState?> { null }

/** The tallest a body can be before the sheet scrolls — for a state that centres itself (the filed report). */
private val LocalSheetBodyMax = staticCompositionLocalOf<Dp?> { null }

/**
 * The ✕ itself: a plain glyph (no filled disc — design language principle 7)
 * in a 48dp target, in its own column so nothing is ever under it.
 */
@Composable
private fun SheetCloseButton(close: SheetClose) {
    Box(
        modifier = Modifier
            // The glyph's right edge on the screen margin, like the back chevron's.
            .offset(x = VelaSpacing.md)
            .size(VelaSizing.sheetClose)
            .clip(CircleShape)
            .clickable(role = Role.Button, onClick = close.onClose)
            .semantics { contentDescription = close.label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(imageVector = VelaIcons.Close, contentDescription = null, tint = VelaTheme.colors.fgMuted, modifier = Modifier.size(VelaIconSize.lg))
    }
}

/** A title-less state (the filed report): the ✕ alone, on its own line at the top end. */
@Composable
private fun SheetCloseRow() {
    val close = LocalSheetClose.current ?: return
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { SheetCloseButton(close) }
}

@Composable
private fun SheetTitle(title: String, subtitle: String? = null) {
    val colors = VelaTheme.colors
    val close = LocalSheetClose.current
    // Only the TITLE shares its line with the ✕ (it wraps beside it, never
    // under it); the subtitle runs the full width below.
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Text(
            text = title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
            modifier = Modifier
                .weight(1f)
                .heightIn(min = if (close != null) VelaSizing.sheetClose else 0.dp)
                .wrapContentHeight(Alignment.CenterVertically),
        )
        if (close != null) {
            Spacer(modifier = Modifier.width(VelaSpacing.sm))
            SheetCloseButton(close)
        }
    }
    if (subtitle != null) {
        Text(
            text = subtitle,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.xs),
        )
    }
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
}

@Composable
private fun SelectSheetBody(sheet: SelectSheetModel, onFooterLink: (() -> Unit)? = null, onSelect: (String) -> Unit = {}) {
    val colors = VelaTheme.colors
    SheetTitle(sheet.title, sheet.subtitle)
    // Spec 072: the search box filters — it was drawn and did nothing.
    var query by remember(sheet.title) { mutableStateOf("") }
    if (sheet.searchPlaceholder != null) {
        VelaUrlField(
            label = "",
            value = query,
            placeholder = sheet.searchPlaceholder,
            keyboard = androidx.compose.ui.text.input.KeyboardType.Text,
            onValueChange = { query = it },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
    }
    val needle = query.trim()
    sheet.rows
        .filter { row ->
            needle.isEmpty() ||
                row.label.contains(needle, ignoreCase = true) ||
                row.caption?.contains(needle, ignoreCase = true) == true ||
                row.id.contains(needle, ignoreCase = true)
        }
        .forEach { VelaSelectRow(it, onClick = onSelect) }
    if (sheet.footerNote != null) {
        Text(
            text = sheet.footerNote,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(top = VelaSpacing.xl),
        )
    }
    if (sheet.footerLink != null) {
        Text(
            text = sheet.footerLink,
            color = colors.infoBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.clickable(enabled = onFooterLink != null) { onFooterLink?.invoke() }.padding(top = VelaSpacing.md),
        )
    }
}

@Composable
private fun ConfirmSheetBody(
    sheet: ConfirmSheetModel,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    val colors = VelaTheme.colors
    SheetTitle(sheet.title)
    Text(
        text = sheet.body,
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.lg,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    if (sheet.note != null) {
        Text(
            text = sheet.note,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(bottom = VelaSpacing.xl),
        )
    }
    if (sheet.callout != null) {
        VelaCallout(sheet.callout)
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
    }
    // The tone picks the CTA's colour, so "清除缓存" is accent and "全部清除" is
    // red without either screen owning a button of its own.
    if (sheet.danger) {
        VelaDangerButton(sheet.confirm, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
    } else {
        VelaPrimaryButton(sheet.confirm, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
    }
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(sheet.cancel, onClick = onCancel, modifier = Modifier.fillMaxWidth())
}

@Composable
internal fun AccountsSheetBody(
    sheet: AccountsSheetModel,
    onSelect: (Int) -> Unit = {},
    onPrimary: () -> Unit = {},
    onSecondary: () -> Unit = {},
    // Spec 017's narrow half (2026-09-23): a wallet can leave this device
    // without taking the others. `null` draws no affordance at all, which is
    // what the settings gallery and the fixtures want.
    onRemove: ((Int) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    var removing by remember(sheet.rows.size) { mutableStateOf<Int?>(null) }
    SheetTitle(sheet.title)
    Text(
        text = sheet.summary,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.lg),
    )
    // Not VelaSelectRow: a select row is a label and a note, so reusing it
    // silently dropped the identicon and the address, and three accounts became
    // three names with no way to tell which key each one is.
    sheet.rows.forEachIndexed { index, row ->
        if (index > 0) SettingsDivider()
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onSelect(index) }
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            IdenticonImage(
                seed = row.addressFull,
                size = VelaSpacing.xl4,
                contentDescription = row.name,
            )
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Text(
                    text = row.name,
                    color = if (row.selected) colors.accentBase else colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = row.addressDisplay,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
            Text(
                text = row.amount,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            if (row.selected) {
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.accentBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
            if (onRemove != null && sheet.remove.isNotEmpty()) {
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.Close,
                    contentDescription = sheet.remove,
                    tint = colors.fgSubtle,
                    modifier = Modifier
                        .size(VelaIconSize.md)
                        .clickable { removing = index },
                )
            }
        }
    }
    // Asked before it happens, because the row it takes is the one under a
    // finger that was aiming to switch.
    removing?.let { index ->
        val row = sheet.rows.getOrNull(index)
        if (row == null) {
            removing = null
        } else {
            androidx.compose.material3.AlertDialog(
                onDismissRequest = { removing = null },
                title = { androidx.compose.material3.Text(row.name) },
                text = { androidx.compose.material3.Text(sheet.removeBody) },
                confirmButton = {
                    androidx.compose.material3.TextButton(onClick = {
                        removing = null
                        onRemove?.invoke(index)
                    }) {
                        androidx.compose.material3.Text(sheet.remove, color = colors.errorBase)
                    }
                },
                dismissButton = {
                    androidx.compose.material3.TextButton(onClick = { removing = null }) {
                        androidx.compose.material3.Text(sheet.removeCancel)
                    }
                },
            )
        }
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    VelaPrimaryButton(sheet.primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(sheet.secondary, onClick = onSecondary, modifier = Modifier.fillMaxWidth())
}

/**
 * ST15 — the in-app report (spec 078 round 3; the web's `FeedbackBody`, with
 * screenshots since the founder's ask of 2026-09-26).
 *
 * 发送 POSTs to getvela.app ([app.getvela.wallet.core.diagnostics.BugReport]);
 * the preview is the payload's `environment`, line for line, and the
 * screenshots go as the tiles show them — each re-encoded first, which is
 * what strips EXIF. Two endings, drawn here rather than in a second sheet:
 * filed says which issue it became and offers to open it; refused / offline /
 * timed out offers the prefilled GitHub form — the ONLY remaining road, never
 * replaced by an apology — and keeps 发送 below it, because "try again" is a
 * real answer to a 429 or a dropped connection.
 *
 * The button is dimmed only while nothing is typed (the endpoint refuses an
 * empty report); while it is sending it turns a spinner at full emphasis.
 * Screenshots never block it.
 */
@Composable
private fun FeedbackSheetBody(
    model: FeedbackModel,
    onSend: (what: String, steps: String, screenshots: List<ByteArray>) -> Unit = { _, _, _ -> },
    onGithub: () -> Unit = {},
    onOpen: (String) -> Unit = {},
    onDone: () -> Unit = {},
    onOpened: () -> Unit = {},
    onClosed: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    val closed by rememberUpdatedState(onClosed)
    DisposableEffect(Unit) {
        onOpened()
        onDispose { closed() }
    }
    // The tray first — before any early return — so a fallback keeps the tiles.
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val thumbnailPx = with(LocalDensity.current) { VelaSizing.screenshotTile.roundToPx() * 2 }
    val tray = remember {
        ScreenshotTray<Uri, ScreenshotPrep.Prepared>(scope) { uri ->
            withContext(Dispatchers.Default) {
                // A debug walk can hold the placeholder on screen ([BugReport.debugPrepareDelayMs]).
                if (BugReport.debugPrepareDelayMs > 0) delay(BugReport.debugPrepareDelayMs)
                ScreenshotPrep.prepare(context.contentResolver, uri, thumbnailPx)
            }
        }
    }
    val tiles by tray.tiles.collectAsState()
    val notice by tray.notice.collectAsState()
    val room = (BugReport.MAX_SCREENSHOTS - tiles.size).coerceAtLeast(0)
    // The system photo picker — no permission; without it (no GMS, old Play
    // services) the contract falls back to the document picker, which does
    // not honour a maximum: the tray takes the first that fit and says so.
    // The multiple contract needs at least two, so one slot left is a single pick.
    val pickMany = rememberLauncherForActivityResult(
        remember(room.coerceAtLeast(2)) { ActivityResultContracts.PickMultipleVisualMedia(room.coerceAtLeast(2)) },
    ) { uris -> tray.add(uris) }
    val pickOne = rememberLauncherForActivityResult(remember { ActivityResultContracts.PickVisualMedia() }) { uri ->
        uri?.let { tray.add(listOf(it)) }
    }
    // The no-GMS road, forced for a debug walk ([BugReport.forceDocumentPicker]).
    val pickDocuments = rememberLauncherForActivityResult(remember { ActivityResultContracts.OpenMultipleDocuments() }) { uris ->
        tray.add(uris)
    }
    val pick = {
        val request = PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)
        runCatching {
            when {
                room == 0 -> Unit
                BugReport.forceDocumentPicker -> pickDocuments.launch(arrayOf("image/*"))
                room >= 2 -> pickMany.launch(request)
                else -> pickOne.launch(request)
            }
        }
            .onFailure { VelaLog.event("feedback", "picker unavailable", "error" to it.javaClass.simpleName) }
        Unit
    }

    val status = model.status
    // The form's height, kept for the filed state: the sheet does not collapse
    // under the person's thumb, and the success block sits at its optical centre.
    var formHeightPx by remember { mutableIntStateOf(0) }
    if (status is FeedbackStatus.Filed) {
        LaunchedEffect(status) { haptic(VelaHaptic.Success) }
        FeedbackFiled(model, status, heightPx = formHeightPx, onOpen = onOpen, onDone = onDone)
        return
    }
    Column(modifier = Modifier.fillMaxWidth().onSizeChanged { formHeightPx = it.height }) {
    SheetTitle(model.title, model.subtitle)
    val sending = status is FeedbackStatus.Sending
    // Spec 048: the box is typed into; what is typed goes into the report.
    var what by rememberSaveable { mutableStateOf("") }
    var steps by rememberSaveable { mutableStateOf("") }
    var stepsOpen by rememberSaveable { mutableStateOf(false) }
    var previewOpen by rememberSaveable { mutableStateOf(true) }
    // 发送 pressed while a tile is still being prepared: waiting for it.
    var awaitingTiles by remember { mutableStateOf(false) }
    // v3 B9: while the report is sending nothing in the form may change.
    val inert = sending || awaitingTiles
    FeedbackField(value = what, placeholder = model.placeholder, minLines = 4, enabled = !inert, onValueChange = { what = it })
    if (stepsOpen) {
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FeedbackField(value = steps, placeholder = model.stepsPlaceholder, minLines = 3, enabled = !inert, onValueChange = { steps = it })
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
    } else {
        // The form marks steps required; this is the box behind the promise.
        Text(
            text = model.addSteps,
            color = colors.infoBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier
                .clickable(enabled = !inert, role = Role.Button) { stepsOpen = true }
                .padding(vertical = VelaSpacing.lg),
        )
    }
    // 078 §C: a prepared tile opens the viewer on it. [viewing] is the image it
    // opened on; [returnTo] is the tile focus goes back to when it closes (the
    // one that opened it, or the one that took its place after a remove).
    var viewing by remember { mutableStateOf<Long?>(null) }
    var returnTo by remember { mutableStateOf<Long?>(null) }
    var focusReturn by remember { mutableStateOf<ScreenshotFocusReturn?>(null) }
    FeedbackScreenshotsSection(
        model = model,
        tiles = tiles.map { ScreenshotTileView(it.id, it.ready?.thumbnail, it.ready != null) },
        notice = notice,
        onAdd = pick,
        onRemove = { id -> haptic(VelaHaptic.Press); tray.remove(id) },
        enabled = !inert,
        onOpen = { id ->
            if (ScreenshotViewerRules.opens(tray.tiles.value, id, sending = inert)) {
                returnTo = id
                viewing = id
            }
        },
        focusReturn = focusReturn,
    )
    viewing?.let { start ->
        // The prepared bytes — what will be sent — labelled by their place among ALL tiles.
        val images = remember(tiles, model.viewScreenshot) {
            tiles.mapIndexedNotNull { index, tile ->
                tile.ready?.let { ViewerImage(tile.id, it.jpeg, it.width, it.height, model.viewScreenshot.replace("{{index}}", (index + 1).toString())) }
            }
        }
        ScreenshotViewer(
            images = images,
            startId = start,
            closeLabel = model.closeViewer,
            removeLabel = model.removeFromViewer,
            onRemove = { id ->
                returnTo = ScreenshotViewerRules.returnAfterRemove(tray.tiles.value.map { it.id }, returnTo, id)
                tray.remove(id)
            },
            onClosed = {
                viewing = null
                focusReturn = ScreenshotFocusReturn(returnTo, (focusReturn?.seq ?: 0) + 1)
            },
        )
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    FeedbackPreview(model, open = previewOpen, onToggle = { previewOpen = !previewOpen })
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    // The promise, quiet and next to the thing it is about (v2 A4: no tinted
    // box — blue on blue failed contrast).
    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
        FeedbackLineIcon(VelaIcons.Info, colors.infoBase, VelaTextSize.sm * 1.2f, Modifier.padding(top = 1.dp))
        Text(
            text = model.consent,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaTextSize.sm * 1.4f,
        )
    }
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    // 发送 while a tile is still being prepared waits for it, busy all the
    // while — an image the person saw added is never silently left out.
    val busy = sending || awaitingTiles
    val send: () -> Unit = {
        if (!busy) {
            awaitingTiles = true
            scope.launch {
                val shots = tray.settled()
                awaitingTiles = false
                onSend(what, steps, shots.map { it.jpeg })
            }
        }
    }
    // The fallback, once shown, STAYS through a retry: 重试 turns its own
    // spinner in place. It used to vanish while sending (the form's 发送
    // came back) and reappear on the next refusal, so the buttons jumped
    // under a thumb — a tap right after a refusal could land on 打开 GitHub
    // 表单 instead (device pass, 078 round 3).
    val fallbackNow = (status as? FeedbackStatus.Fallback)?.url
    var lastFallback by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(fallbackNow) { if (fallbackNow != null) lastFallback = fallbackNow }
    val fallbackUrl = fallbackNow ?: lastFallback?.takeIf { sending }
    if (fallbackUrl != null) {
        // Not an error message: the other road, with the person's typing
        // already in the URL — amber, because nothing has been lost except the
        // images, which a URL cannot carry, and the block says so.
        // v3 B6: never stranded below the fold — the block and its button are
        // brought into view, and TalkBack is moved to (and reads) its title.
        // INSTANTLY, and once (when it first appears): nothing may move while
        // a thumb is on its way to 重试.
        val intoView = remember { BringIntoViewRequester() }
        val titleFocus = remember { FocusRequester() }
        val sheetScroll = LocalSheetScroll.current
        var revealed by remember { mutableStateOf(false) }
        LaunchedEffect(revealed) {
            if (!revealed) return@LaunchedEffect
            if (sheetScroll == null) intoView.bringIntoView()
            withFrameNanos { } // then focus — the title is already on screen, so no second scroll
            runCatching { titleFocus.requestFocus() }
        }
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .bringIntoViewRequester(intoView)
                // In the very layout pass that first places the block — the
                // scroll's end already includes it — jump there, so it is
                // drawn in place one frame later, not two or three.
                .onGloballyPositioned {
                    if (!revealed) {
                        revealed = true
                        sheetScroll?.let { scroll -> scroll.dispatchRawDelta((scroll.maxValue - scroll.value).toFloat()) }
                    }
                },
        ) {
            FeedbackFallbackBlock(model, withScreenshots = tiles.isNotEmpty(), titleFocus = titleFocus)
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaPrimaryButton(model.openGithub, onClick = { onOpen(fallbackUrl) }, modifier = Modifier.fillMaxWidth())
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            VelaSecondaryButton(
                text = if (busy) model.sending else model.tryAgain,
                onClick = send,
                modifier = Modifier.fillMaxWidth(),
                enabled = what.isNotBlank(),
                loading = busy,
                busyLabel = true,
            )
        }
        // The block already offers the form: no second "Prefer GitHub?" road.
    } else {
        VelaPrimaryButton(
            text = if (busy) model.sending else model.send,
            onClick = send,
            modifier = Modifier.fillMaxWidth(),
            enabled = what.isNotBlank(),
            loading = busy,
            busyLabel = true,
        )
        FeedbackLink(model.githubLink, onClick = onGithub)
    }
    }
}

/**
 * "What will be sent" — a disclosure, open by default: plain rows, each line
 * split at its first ": " into a muted label and its value, wrapped lines
 * indented under the value (v2 A4; no sunken box, no monospace).
 */
@Composable
private fun FeedbackPreview(model: FeedbackModel, open: Boolean, onToggle: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .clickable(role = Role.Button, onClick = onToggle)
            .padding(vertical = VelaSpacing.xs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
    ) {
        Text(text = model.previewToggle, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
        FeedbackLineIcon(VelaIcons.ChevronDown, colors.fgMuted, VelaTextSize.sm, Modifier.rotate(if (open) 180f else 0f))
    }
    if (open) {
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        // Each line is split at its first ": " into a label column and a value
        // column — no colon on screen (v3 B2: zh never shows a half-width ": "
        // after Chinese). The PAYLOAD keeps "label: value".
        val rows = model.previewLines.map { line ->
            val cut = line.indexOf(": ")
            if (cut > 0) line.substring(0, cut) to line.substring(cut + 2) else "" to line
        }
        PreviewTable(
            rows = rows,
            gap = VelaSpacing.md,
            rowGap = VelaSpacing.xs,
            label = { text ->
                Text(text = text, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm, lineHeight = VelaTextSize.sm * 1.4f)
            },
            value = { text ->
                Text(text = text, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm, lineHeight = VelaTextSize.sm * 1.4f)
            },
        )
    }
}

/**
 * Two columns: every label as wide as the widest (at most 45% of the row), every
 * value in the rest, wrapped lines staying under their value.
 */
@Composable
private fun PreviewTable(
    rows: List<Pair<String, String>>,
    gap: androidx.compose.ui.unit.Dp,
    rowGap: androidx.compose.ui.unit.Dp,
    label: @Composable (String) -> Unit,
    value: @Composable (String) -> Unit,
) {
    androidx.compose.ui.layout.Layout(
        modifier = Modifier.fillMaxWidth(),
        content = {
            rows.forEach { (l, _) -> Box { label(l) } }
            rows.forEach { (_, v) -> Box { value(v) } }
        },
    ) { measurables, constraints ->
        val width = constraints.maxWidth
        val gapPx = gap.roundToPx()
        val rowGapPx = rowGap.roundToPx()
        val labels = measurables.take(rows.size)
        val values = measurables.drop(rows.size)
        val column = labels.maxOfOrNull { it.maxIntrinsicWidth(androidx.compose.ui.unit.Constraints.Infinity) }?.coerceAtMost((width * 0.45f).toInt()) ?: 0
        val labelPlaced = labels.map { it.measure(androidx.compose.ui.unit.Constraints(maxWidth = column)) }
        val valuePlaced = values.map { it.measure(androidx.compose.ui.unit.Constraints(maxWidth = (width - column - gapPx).coerceAtLeast(0))) }
        val heights = rows.indices.map { maxOf(labelPlaced[it].height, valuePlaced[it].height) }
        val total = heights.sum() + rowGapPx * (rows.size - 1).coerceAtLeast(0)
        layout(width, total) {
            var y = 0
            rows.indices.forEach { i ->
                labelPlaced[i].placeRelative(0, y)
                valuePlaced[i].placeRelative(column + gapPx, y)
                y += heights[i] + rowGapPx
            }
        }
    }
}

/** v2 A2: title, body and the screenshots line — three parts, never one glued string. */
@Composable
private fun FeedbackFallbackBlock(model: FeedbackModel, withScreenshots: Boolean, titleFocus: FocusRequester? = null) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.warningSoft)
            .padding(VelaSpacing.lg),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        FeedbackLineIcon(VelaIcons.TriangleAlert, colors.warningBase, VelaTextSize.base * 1.2f, Modifier.padding(top = 2.dp))
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            // Calm (design review): the amber is the block's and its icon's;
            // the words are ordinary text.
            Text(
                text = model.fallbackTitle,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                fontWeight = VelaFontWeight.semibold,
                modifier = Modifier
                    .then(if (titleFocus != null) Modifier.focusRequester(titleFocus) else Modifier)
                    .semantics { liveRegion = LiveRegionMode.Assertive; heading() }
                    .focusable(),
            )
            Text(
                text = model.fallbackBody,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaTextSize.base * 1.4f,
            )
            if (withScreenshots) {
                Spacer(modifier = Modifier.height(VelaSpacing.xs))
                Text(
                    text = model.fallbackScreenshots,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    lineHeight = VelaTextSize.sm * 1.4f,
                )
            }
        }
    }
}

/**
 * Filed (v2 A7): the success disc (soft fill, a faint success ring so it holds
 * on a light page), the title, which issue it became (or joined) — balanced,
 * at most ~300dp wide — a warning if the images could not be stored, then the
 * buttons. No accent unless screenshots were dropped: then 在 GitHub 查看 is
 * primary, because the issue page is where they get added. The block sits at
 * the sheet's optical centre (2:3), in the height the form had.
 */
@Composable
private fun FeedbackFiled(model: FeedbackModel, status: FeedbackStatus.Filed, heightPx: Int, onOpen: (String) -> Unit, onDone: () -> Unit) {
    val colors = VelaTheme.colors
    // The form's height, but never more than the sheet shows without
    // scrolling: a long form at the largest size put the "centre" below the
    // fold and the block rode high (design review, 078 round 3).
    val formHeight = with(LocalDensity.current) { heightPx.toDp() }
    val height = LocalSheetBodyMax.current?.let { minOf(formHeight, it) } ?: formHeight
    Column(
        modifier = Modifier
            .fillMaxWidth()
            // A floor, not a fixed height: at the largest size the block may
            // outgrow it, and then it simply scrolls.
            .then(if (heightPx > 0) Modifier.heightIn(min = height) else Modifier.padding(bottom = VelaSpacing.xl3)),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        SheetCloseRow()
        if (heightPx > 0) Spacer(modifier = Modifier.weight(2f))
        VelaStatusBadge(
            BadgeVariant.Success,
            modifier = Modifier.border(1.5.dp, colors.successBase.copy(alpha = 0.4f), CircleShape),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        // v3 B7: announced, and TalkBack's focus moved onto it.
        val titleFocus = remember { FocusRequester() }
        LaunchedEffect(Unit) { runCatching { titleFocus.requestFocus() } }
        Text(
            text = model.successTitle,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.xl,
            fontWeight = VelaFontWeight.semibold,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .focusRequester(titleFocus)
                .semantics { liveRegion = LiveRegionMode.Polite; heading() }
                .focusable(),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        Text(
            text = (if (status.deduped) model.successBodyDeduped else model.successBodyNew).replace("{{number}}", status.number.toString()),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            lineHeight = VelaTextSize.base * 1.4f,
            textAlign = TextAlign.Center,
            style = LocalTextStyle.current.copy(lineBreak = LineBreak.Heading),
            modifier = Modifier.widthIn(max = 300.dp),
        )
        if (status.screenshotsDropped > 0) {
            // Centred like the lines above it, no icon: the warning colour says enough.
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = model.screenshotsDropped,
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaTextSize.base * 1.4f,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(max = 300.dp),
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl2))
        // v3 B3: a hierarchy — 在 GitHub 查看 outlined (primary when the images
        // were dropped: the issue page is where they get added), 完成 plain text.
        val dropped = status.screenshotsDropped > 0
        if (status.url.isNotEmpty()) {
            if (dropped) {
                VelaPrimaryButton(model.viewIssue, onClick = { onOpen(status.url) }, modifier = Modifier.fillMaxWidth())
            } else {
                VelaSecondaryButton(model.viewIssue, onClick = { onOpen(status.url) }, modifier = Modifier.fillMaxWidth())
            }
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
        }
        Text(
            text = model.done,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
            fontWeight = VelaFontWeight.medium,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = VelaSizing.controlLg)
                .clip(RoundedCornerShape(VelaRadius.full))
                .clickable(role = Role.Button, onClick = onDone)
                .wrapContentHeight(Alignment.CenterVertically)
                .padding(vertical = VelaSpacing.md),
        )
        if (heightPx > 0) Spacer(modifier = Modifier.weight(3f))
    }
}

/** A link line under the report: centred, info-blue, a real button to TalkBack. */
@Composable
private fun FeedbackLink(text: String, onClick: () -> Unit) {
    Text(
        text = text,
        color = VelaTheme.colors.infoBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.sm)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = VelaSpacing.md),
    )
}

/**
 * The report's text box: the person's own words, in the UI face (not the
 * mono of an endpoint field), several lines tall, the placeholder whole.
 */
@Composable
private fun FeedbackField(value: String, placeholder: String, minLines: Int, enabled: Boolean = true, onValueChange: (String) -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.lg),
    ) {
        // On the app's text style (its tracking and leading), like every other Text here.
        val style = LocalTextStyle.current.merge(TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base))
        if (value.isEmpty()) Text(text = placeholder, style = style.copy(color = colors.fgSubtle))
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            enabled = enabled,
            minLines = minLines,
            textStyle = style,
            cursorBrush = SolidColor(colors.accentBase),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, keyboardType = KeyboardType.Text),
            modifier = Modifier.fillMaxWidth().semantics { contentDescription = placeholder },
        )
    }
}

/**
 * Spec 071: the Trusted Signer page. The address is checked by the core, not
 * here: a refused one leaves the old page in force and says why under the
 * field; an accepted one closes the sheet.
 */
@Composable
private fun SignerPageSheetBody(model: SignerPageModel, onSave: (String) -> Unit, onReset: () -> Unit, onDone: () -> Unit) {
    val colors = VelaTheme.colors
    var text by remember(model.value) { mutableStateOf(model.value) }
    var saving by remember { mutableStateOf(false) }
    LaunchedEffect(model.value, model.error) {
        if (saving) {
            saving = false
            if (model.error == null) onDone()
        }
    }
    SheetTitle(model.title, model.subtitle)
    VelaUrlField(
        label = model.title,
        value = text,
        tone = if (model.error != null) SettingsTone.Error else SettingsTone.Neutral,
        onValueChange = { text = it },
    )
    model.error?.let {
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(it, color = colors.errorBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
    }
    model.foreign?.let {
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaCallout(CalloutModel(tone = CalloutTone.Warning, text = it))
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(
        model.save,
        onClick = {
            if (text.trim() == model.value) {
                onDone()
            } else {
                saving = true
                onSave(text)
            }
        },
        modifier = Modifier.fillMaxWidth(),
    )
    model.reset?.let {
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        VelaSecondaryButton(it, onClick = onReset, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
private fun RpcFixSheetBody(model: RpcFixModel, onPrimary: () -> Unit, onField: ((String) -> Unit)? = null) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Row(
        modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaChainMark(model.mark)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = model.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Text(
                text = model.meta,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
        VelaStatusPill(model.badge)
    }
    VelaCallout(model.callout)
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaUrlField(
        label = model.field.label,
        value = model.field.value,
        badge = model.field.badge,
        tone = model.field.tone,
        onValueChange = onField,
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(model.primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
    if (model.providersLabel != null) {
        Text(
            text = model.providersLabel,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(top = VelaSpacing.xl, bottom = VelaSpacing.md),
        )
        // 087 F07: the four names shared one row, so "Chainlist" — the last,
        // with the least room left — broke mid-word onto a second line. A chip
        // is one word on one line; when the row is full the next chip wraps
        // whole onto the next line (a narrow phone, a large text size).
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            model.providers.forEach { name ->
                Text(
                    text = name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier
                        .background(colors.bgRaised)
                        .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                )
            }
        }
    }
    if (model.report != null) {
        Text(
            text = model.report,
            color = colors.infoBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.xl),
        )
    }
}

@Composable
private fun BalanceDetailSheetBody(model: BalanceDetailModel, onRetry: (String) -> Unit = {}) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Text(
        text = model.summary,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    BalanceDetailSection(model.sectionPending)
    Text(
        text = model.pendingNote,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        modifier = Modifier.padding(vertical = VelaSpacing.md),
    )
    model.pending.forEach { row -> BalanceDetailRow(row, onRetry) }
    // Three sections, as the web draws them (BalanceDetailBody): the settled
    // chains under their own label, and the unpriced holdings only when any.
    BalanceDetailSection(model.sectionDone, top = VelaSpacing.xl)
    model.done.forEach { row -> BalanceDetailRow(row, onRetry) }
    if (model.unpriced.isNotEmpty()) {
        BalanceDetailSection(model.sectionUnpriced, top = VelaSpacing.xl)
        model.unpriced.forEach { row -> BalanceDetailRow(row, onRetry) }
    }
}

/**
 * SR6 (spec 092): every network the wallet cannot reach, one row each — what
 * was last read there, and the network's RPC fix. The rows follow the live
 * view, so one that comes back leaves while the sheet is open.
 */
@Composable
private fun UnreachableSheetBody(model: UnreachableModel, onFix: (Int) -> Unit) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    model.summary?.let { summary ->
        Text(
            text = summary,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(bottom = VelaSpacing.md),
        )
    }
    model.rows.forEach { row ->
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .drawBehind {
                    val y = size.height - VelaBorder.hairline.toPx() / 2
                    drawLine(colors.borderBase, Offset(0f, y), Offset(size.width, y), VelaBorder.hairline.toPx())
                }
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            VelaChainMark(row.mark)
            Column(modifier = Modifier.weight(1f)) {
                Text(text = row.name, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg)
                Text(text = row.line, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
            }
            UnreachableFixAction(row.action) { onFix(row.chainId) }
        }
    }
}

/** A row's 修复: a text action that still answers the finger — it gives and buzzes. */
@Composable
private fun UnreachableFixAction(label: String, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val haptic = rememberVelaHaptic()
    val scale by animateFloatAsState(
        targetValue = if (pressed) VelaMotion.pressScaleButton else 1f,
        animationSpec = VelaMotion.pressSpring,
        label = "unreachableFixPress",
    )
    Text(
        text = label,
        color = VelaTheme.colors.infoBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .clickable(interactionSource = interaction, indication = null) {
                haptic(VelaHaptic.Press)
                onClick()
            }
            .padding(VelaSpacing.sm),
    )
}

@Composable
private fun BalanceDetailSection(text: String, top: androidx.compose.ui.unit.Dp = 0.dp) {
    Text(
        text = text,
        color = VelaTheme.colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(top = top, bottom = VelaSpacing.sm),
    )
}

@Composable
private fun BalanceDetailRow(row: BalanceDetailRowModel, onRetry: (String) -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaChainMark(row.mark)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = row.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.lg,
            )
            if (row.status != null) {
                // Rate-limiting gets a grey line and no button because it
                // resolves itself; a dead RPC gets red and 立即重试.
                Text(
                    text = row.status,
                    color = if (row.tone == SettingsTone.Error) colors.errorBase else colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
        }
        if (row.action != null) {
            // Drawn with no handler before: the retry reached nothing.
            Text(
                text = row.action,
                color = colors.infoBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.clickable { onRetry(row.id) }.padding(VelaSpacing.sm),
            )
        }
        if (row.amount != null) {
            Text(
                text = row.amount,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.lg,
            )
        }
    }
}

@Composable
private fun RelayerSheetBody(model: RelayerModel, onPrimary: () -> Unit) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Text(
        text = model.lead,
        color = colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    Row(
        modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaChainMark(model.mark)
        Column {
            Text(
                text = model.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Text(
                text = model.amountHint,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
    }
    Text(
        text = model.addressDisplay,
        color = colors.fgBase,
        fontFamily = VelaMonoFontFamily,
        fontSize = VelaTextSize.base,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.bgSunken)
            .padding(VelaSpacing.lg),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaCallout(model.callout)
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(model.primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
}
