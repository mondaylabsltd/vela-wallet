package app.getvela.wallet

import android.graphics.Bitmap
import android.util.Log
import android.view.View
import android.view.WindowManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.ViewRootForTest
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.designsystem.components.VELA_SHEET_CONTENT_TAG
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.diagnostics.CrashReport
import app.getvela.wallet.core.diagnostics.CrashSheet
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.browser.ExternalPageSheet
import app.getvela.wallet.feature.browser.core.BrowserController
import app.getvela.wallet.feature.contacts.ActionMenuModel
import app.getvela.wallet.feature.contacts.ContactNoticeModel
import app.getvela.wallet.feature.contacts.ContactsIcon
import app.getvela.wallet.feature.contacts.DeleteConfirmModel
import app.getvela.wallet.feature.contacts.MenuItemModel
import app.getvela.wallet.feature.contacts.components.ActionMenuSheet
import app.getvela.wallet.feature.contacts.components.ContactNoticeSheet
import app.getvela.wallet.feature.contacts.components.DeleteConfirmSheet
import app.getvela.wallet.feature.onboarding.ThemeSettingsSheet
import app.getvela.wallet.feature.onboarding.core.HYBRID_PRODUCT
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.EndpointSheet
import app.getvela.wallet.feature.onboarding.flow.FlowSheet
import app.getvela.wallet.feature.onboarding.flow.InsertKeySheet
import app.getvela.wallet.feature.onboarding.flow.LocationAskSheet
import app.getvela.wallet.feature.onboarding.flow.SignOutSheet
import app.getvela.wallet.feature.onboarding.flow.UsbTouchIndicator
import app.getvela.wallet.feature.onboarding.flow.UsbWalletPicker
import app.getvela.wallet.feature.wallet.ChainRowModel
import app.getvela.wallet.feature.wallet.SheetModel
import app.getvela.wallet.feature.wallet.components.ChainSelectSheet
import app.getvela.wallet.feature.wallet.components.IdenticonViewerSheet
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.vela_core_uniffi.CtapCredentialChoice

/**
 * Device walk 2026-10-11, item 6: the identicon sheet opened at HALF height —
 * 「复制地址」 under the system's navigation bar, 「关闭」 off screen until the
 * sheet was dragged up. It used the sheet host's default state, which stops a
 * sheet taller than half the window half-way, and fourteen other sheets used
 * the same default.
 *
 * The rule, for each of the fifteen: **a sheet opens at its content's
 * height; its last action is whole, inside the window and above the
 * navigation bar, with no drag of the sheet.** A sheet whose content is
 * taller than the window has (a short window, a long list) is capped under
 * the status bar and scrolls INSIDE itself to that action.
 *
 * Whatever screen and navigation mode this runs on is what it checks, so it
 * is run three ways (emulator only):
 *
 * ```bash
 * adb -s <emulator> shell cmd overlay enable com.android.internal.systemui.navbar.threebutton
 * adb -s <emulator> shell cmd overlay enable com.android.internal.systemui.navbar.gestural
 * adb -s <emulator> shell wm size 1080x1300        # a short window: the tall sheets must scroll inside
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SheetOpensWholeFitTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SheetOpensWholeFitTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    /** The walk's language: its sentences are the ones that were cut. */
    private val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
        .apply { initialize("zh") }

    @After
    fun tearDown() {
        // The crash sheet's fixture must never meet the app's next launch.
        CrashReport.clear(context)
    }

    @Test
    fun theIdenticonSheet() = opensWhole("identicon") {
        IdenticonViewerSheet(address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894", onDismiss = {})
    }

    @Test
    fun theChainSelectSheetWithEveryNetwork() = opensWhole("chain-select") {
        // "All networks" and the wallet's 25: a list longer than any phone.
        val rows = List(26) { index -> ChainRowModel(name = if (index == 0) "所有网络" else "Network $index", dot = if (index == 0) null else Color.Gray, count = index, selected = index == 0) }
        ChainSelectSheet(model = SheetModel(title = "选择链", rows = rows), onDismiss = {})
    }

    @Test
    fun theCrashSheet() {
        CrashReport.recordFault(context, "pr4-fit", IllegalStateException("a fault, for the sheet's sake"), "0.0.0")
        opensWhole("crash") { CrashSheet(strings = strings, version = "0.0.0") }
    }

    @Test
    fun theExternalPageSheet() = opensWhole("external-page") {
        ExternalPageSheet(
            page = BrowserController.ExternalPage(url = "https://app.example.org/a/long/path?with=a&query=that&goes=on", host = "app.example.org"),
            strings = strings,
            onAnswer = {},
        )
    }

    @Test
    fun theActionMenuSheet() = opensWhole("action-menu") {
        ActionMenuSheet(
            model = ActionMenuModel(
                items = listOf(
                    MenuItemModel("contacts.add", ContactsIcon.AddContact, "添加联系人"),
                    MenuItemModel("contacts.import", ContactsIcon.Import, "导入"),
                    MenuItemModel("contacts.export", ContactsIcon.Export, "导出"),
                    MenuItemModel("contacts.edit", ContactsIcon.Edit, "编辑"),
                    MenuItemModel("contacts.move", ContactsIcon.MoveGroup, "移动到分组"),
                    MenuItemModel("contacts.delete", ContactsIcon.Delete, "删除", destructive = true, dividerBefore = true),
                ),
                cancel = "取消",
            ),
            onDismiss = {},
        )
    }

    @Test
    fun theDeleteConfirmSheet() = opensWhole("delete-confirm") {
        DeleteConfirmSheet(
            model = DeleteConfirmModel(title = "删除联系人？", body = "「jxjjx测试」将从通讯录中移除。这不会影响任何交易记录。", confirm = "删除", cancel = "取消"),
            onDismiss = {},
        )
    }

    @Test
    fun theContactNoticeSheet() = opensWhole("contact-notice") {
        ContactNoticeSheet(
            model = ContactNoticeModel(title = "导入完成", body = List(12) { "第 ${it + 1} 行：已导入 0x7000…000${it % 10}" }.joinToString("\n"), close = "关闭"),
            onDismiss = {},
        )
    }

    @Test
    fun theThemeSettingsSheet() = opensWhole("theme") {
        ThemeSettingsSheet(current = ThemePreference.Light, onSelect = {}, onDismiss = {})
    }

    @Test
    fun theEndpointSheet() = opensWhole("endpoint") {
        EndpointSheet(current = "https://registry.example.org", defaultUrl = "https://registry.getvela.app", onSave = {}, onDismiss = {})
    }

    @Test
    fun theFlowSheet() = opensWhole("flow-prompt") {
        FlowSheet(kind = PromptKind(type = "recover_offer", detail = null), confirmable = true, onAnswer = {})
    }

    @Test
    fun theInsertKeySheet() = opensWhole("insert-key") {
        InsertKeySheet(otgLooksOff = true, onCancel = {})
    }

    @Test
    fun theLocationAskSheet() = opensWhole("location-ask") {
        LocationAskSheet(onAnswer = {})
    }

    @Test
    fun theSignOutSheet() = opensWhole("sign-out") {
        SignOutSheet(pendingUploadWarning = true, accountCount = 6, onConfirm = {}, onDismiss = {})
    }

    @Test
    fun theWalletPickerOfASecurityKey() = opensWhole("usb-wallet-picker") {
        UsbWalletPicker(
            choices = List(4) { CtapCredentialChoice(name = "Wallet ${it + 1}", credentialId = "cred-$it", product = "YubiKey 5C NFC") },
            onPick = {},
        )
    }

    @Test
    fun theTouchPromptOfAPhoneLink() = opensWhole("usb-touch") {
        UsbTouchIndicator(kind = "touch", product = HYBRID_PRODUCT, onCancel = {})
    }

    /**
     * Opens [sheet] and checks the rule above against the real window: the
     * display's own height and the system's own navigation-bar inset, never a
     * number written here.
     */
    private fun opensWhole(name: String, sheet: @Composable () -> Unit) {
        var activityRoot: View? = null
        compose.setContent {
            activityRoot = LocalView.current
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { sheet() }
            }
        }
        val content = compose.onNodeWithTag(VELA_SHEET_CONTENT_TAG)
        // The sheet slides in; the test clock runs its animation to the end.
        compose.waitUntil(HANG_MS) { runCatching { content.fetchSemanticsNode() }.isSuccess }
        compose.waitForIdle()

        val dp = compose.density.density
        // The raw insets, from the activity's window: never consumed, whichever window asks.
        val insets = ViewCompat.getRootWindowInsets(activityRoot!!)!!
        val navigationBar = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
        val statusBar = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
        val screen = context.getSystemService(WindowManager::class.java).currentWindowMetrics.bounds
        val floor = screen.height() - navigationBar

        val column = content.fetchSemanticsNode()
        val sheetTop = onScreenTop(column)
        // The host caps a sheet's content at the window less the status bar, the navigation bar and its handle's
        // room (48dp): at the cap, the content is taller than the room there is.
        val handleRoom = 48 * dp
        val atCap = column.size.height >= screen.height() - statusBar - navigationBar - handleRoom - 1

        val actions = compose.onAllNodes(hasClickAction() and hasAnyAncestor(hasTestTag(VELA_SHEET_CONTENT_TAG)))
        val count = actions.fetchSemanticsNodes().size
        assertTrue("$name: a sheet with nothing to tap", count > 0)
        // The last action: the one laid out lowest in the content, on or off screen.
        val index = (0 until count).maxBy { i -> actions[i].fetchSemanticsNode().let { it.positionInRoot.y + it.size.height } }
        val last = actions[index]

        fun whole(node: SemanticsNode): Boolean {
            val top = onScreenTop(node)
            val shown = node.boundsInRoot
            return shown.height >= node.size.height - 0.5f && top >= statusBar && top + node.size.height <= floor + 0.5f
        }

        var scrolledInside = false
        if (!whole(last.fetchSemanticsNode())) {
            // Never a drag of the sheet: the content's own scroll, or nothing.
            runCatching { last.performScrollTo() }
            compose.waitForIdle()
            scrolledInside = true
        }
        val node = last.fetchSemanticsNode()
        val top = onScreenTop(node)
        val bottom = top + node.size.height
        save(name)
        Log.i(
            TAG,
            "$name: screen ${screen.width()}x${screen.height()} navBar $navigationBar statusBar $statusBar | sheet top $sheetTop " +
                "content ${column.size.height}px atCap=$atCap | last action y ${top.toInt()}..${bottom.toInt()} floor $floor " +
                "(${((floor - bottom) / dp).toInt()}dp above the bar) scrolledInside=$scrolledInside",
        )
        // The sheet's own top edge is its content's top less the handle's room (a custom handle takes less, never more).
        assertTrue(
            "$name: the sheet's top edge (${(sheetTop - handleRoom).toInt()}) is under the status bar ($statusBar)",
            sheetTop - handleRoom >= statusBar - 1,
        )
        assertTrue(
            "$name: the last action is at y ${top.toInt()}..${bottom.toInt()} — not whole inside the window above the navigation bar " +
                "(floor $floor, screen ${screen.height()}, sheet top $sheetTop), and no scroll inside the sheet reaches it",
            whole(node),
        )
        last.assertIsDisplayed()
        if (!atCap) {
            assertFalse("$name: the content fits the window, yet its last action had to be scrolled to", scrolledInside)
        }
    }

    /** [node]'s top on the SCREEN: its place in its own window's root, plus where that root is. */
    private fun onScreenTop(node: SemanticsNode): Float {
        val view = (node.root as ViewRootForTest).view
        val at = IntArray(2).also(view::getLocationOnScreen)
        return at[1] + node.positionInRoot.y
    }

    /** Evidence for the review: the screen as measured, in the app's cache. */
    private fun save(name: String) {
        val shot = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
        java.io.File(context.cacheDir, "sheet-fit-$name.png").outputStream().use {
            shot.compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }

    private companion object {
        const val TAG = "pr4-fit"

        /** A hang limit for the sheet to appear, not a measure of anything. */
        const val HANG_MS = 60_000L
    }
}
