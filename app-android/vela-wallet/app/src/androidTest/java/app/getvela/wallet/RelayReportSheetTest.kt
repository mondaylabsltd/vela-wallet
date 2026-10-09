package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.RELAY_REPORT_TAG
import app.getvela.wallet.feature.flows.SendFormBody
import app.getvela.wallet.feature.send.RelayReport
import app.getvela.wallet.feature.send.RelayReportSheet
import app.getvela.wallet.feature.send.core.SendRelayReport
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsScreenState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #466 on a real renderer: the relay stop's "Report this" is a button
 * on the form, and it opens the app's own report sheet with the core's words
 * already in its boxes — never an empty form. Nothing is sent here: the
 * sheet's 发送 hands its words to a lambda.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.RelayReportSheetTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class RelayReportSheetTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
        .apply { initialize("en") }

    private val report = SendRelayReport(
        what = "Relayer out of gas on Unichain (130)\nTreasury 0x3e59292e18417f814112f731e7163534c6d2fe3c. " +
            "Has 0 ETH of its 0.0001 ETH floor (short 0.0001 ETH).",
        steps = "1. Send ETH on Unichain (130).\n2. Press Continue.",
        area = "Send",
        fingerprint = "relay-gas-130",
    )

    @Test
    fun theStopsReportButtonOpensTheSheetWithTheCoresWords() {
        val form = (FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model.copy(
            warning = "Relayer out of gas",
            report = "Report this",
        )
        var open by mutableStateOf<SendRelayReport?>(null)
        val sent = mutableListOf<Pair<String, String>>()
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SendFormBody(model = form, onReport = { open = report })
                    open?.let { shown ->
                        RelayReportSheet(
                            model = SettingsFixtures.buildState(SettingsScreenState.ST15, strings),
                            seed = RelayReport.seed(shown),
                            onDismiss = { open = null },
                            onSend = { what, steps, _ -> sent += what to steps },
                            onGithub = {},
                            onOpenLink = {},
                            onOpened = {},
                            onClosed = {},
                        )
                    }
                }
            }
        }
        compose.onNodeWithTag(RELAY_REPORT_TAG).performClick()
        compose.waitForIdle()
        // Both boxes hold the core's words, steps unfolded.
        val boxes = compose.onAllNodes(hasSetTextAction()).fetchSemanticsNodes()
        val texts = boxes.map { node ->
            node.config.getOrNull(androidx.compose.ui.semantics.SemanticsProperties.EditableText)?.text.orEmpty()
        }
        assertTrue(texts.toString(), texts.contains(report.what))
        assertTrue(texts.toString(), texts.contains(report.steps))
        // 发送 files exactly what the boxes hold.
        compose.onAllNodes(hasText(strings.t(app.getvela.wallet.core.i18n.I18nKeys.SettingsUi.BUG_SEND)))[0].performScrollTo().performClick()
        compose.waitUntil(5_000) { sent.isNotEmpty() }
        assertEquals(listOf(report.what to report.steps), sent)
    }

    /**
     * Issue #478: a tap in the sheet that is not on a control leaves the box —
     * focus goes, and the keyboard with it. The sheet is its own window: the
     * page's tap-to-leave never reached it, and the keyboard's own hide key
     * was the only way to see Send again.
     */
    @Test
    fun aTapOutsideTheBoxLeavesIt() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST15, strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    RelayReportSheet(
                        model = model,
                        seed = RelayReport.seed(report),
                        onDismiss = {},
                        onSend = { _, _, _ -> },
                        onGithub = {},
                        onOpenLink = {},
                        onOpened = {},
                        onClosed = {},
                    )
                }
            }
        }
        compose.waitForIdle()
        val box = compose.onAllNodes(hasSetTextAction())[0]
        box.performClick()
        box.assertIsFocused()
        // The sheet's title: words, not a control.
        compose.onNodeWithText(model.feedback.title).performClick()
        compose.waitForIdle()
        box.assertIsNotFocused()
        // What was typed is still there.
        val kept = box.fetchSemanticsNode().config.getOrNull(androidx.compose.ui.semantics.SemanticsProperties.EditableText)?.text
        assertEquals(report.what, kept)
    }
}
