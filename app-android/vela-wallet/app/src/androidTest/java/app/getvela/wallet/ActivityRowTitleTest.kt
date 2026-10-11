package app.getvela.wallet

import android.util.Log
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.wallet.ActivityKind
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.components.ActivityTitleLinesKey
import app.getvela.wallet.feature.wallet.core.FeedDapp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 1: 「在 arcorbs.game3.app 合 / 约交互」 on home
 * and History — a dApp row's title broke inside its action.
 *
 * The real row ([ActivityRow]), its title built by the live builder
 * (`WalletLive.dappTitle` / `dappTitleParts`, from the core's sentence in
 * each language), drawn with the app's fonts: the owner's own host, the
 * test dApp's and two more, in Chinese and English, in the list of a 1080 px
 * phone and of a 320 dp one, with and without a figure beside the title, at
 * the default text size and the largest.
 *
 * At every one of them: the title is on one line, or on two that break
 * between the action and the site — 「在 arcorbs.game3.app」 over 「合约交互」,
 * "Contract interaction" over "on arcorbs.game3.app"; the action is whole on
 * a line of its own; and the title is exactly as tall as that many single
 * lines, so nothing wrapped where the eye cannot see a rule.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ActivityRowTitleTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ActivityRowTitleTest {

    @get:Rule
    val compose = createComposeRule()

    private fun strings(lang: String) = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize(lang) }
    }

    private val hosts = listOf(
        // The owner's row.
        "arcorbs.game3.app",
        "app.uniswap.org",
        "a-very-long-subdomain.of.some-protocol.example.finance",
        "127.0.0.1:8137",
    )

    private fun row(dapp: FeedDapp, words: I18nRuntime, figure: Boolean) = ActivityRowModel(
        id = dapp.place.orEmpty() + figure,
        kind = ActivityKind.Dapp,
        title = WalletLive.dappTitle(dapp, words),
        subtitle = "0x88cC…6894 · Arc Testnet",
        amount = if (figure) "−0.05" else "",
        unit = if (figure) "BNB" else "",
        positive = false,
        masked = false,
        badgeColor = Color.Gray,
        titlePlace = WalletLive.dappTitleParts(dapp, words),
    )

    private fun titles(lang: String) = SemanticsMatcher("a dApp row's title") { it.config.contains(ActivityTitleLinesKey) }

    private fun breaksOnlyBetweenItsParts(lang: String, rowDp: Int, scale: TextScaleLevel) {
        val widthPx = (rowDp * compose.density.density).toInt()
        val words = strings(lang)
        // The owner's row: a contract call, as the core names it (`intentContractCall` — 「合约交互」).
        val action = words.t(I18nKeys.Wallet.SIGNING_TERM_PREFIX + TERM)
        // A title that fits one line whatever the width: what one line is tall.
        val short = FeedDapp(place = "a.b", intent = "Go")
        val rows = listOf(row(short, words, figure = false)) +
            hosts.flatMap { host -> listOf(true, false).map { figure -> row(FeedDapp(place = host, intent_term = TERM), words, figure) } }
        compose.setContent {
            val device = LocalDensity.current
            CompositionLocalProvider(
                LocalVelaStrings provides words,
                // The app's own text size is a font scale over the device's (`VelaTheme`).
                LocalDensity provides Density(device.density, device.fontScale * scale.factor),
            ) {
                VelaTheme(darkTheme = false) {
                    Box(modifier = Modifier.width(rowDp.dp)) {
                        Column { rows.forEach { ActivityRow(it) } }
                    }
                }
            }
        }
        compose.waitForIdle()
        val drawn: List<SemanticsNode> = compose.onAllNodes(titles(lang), useUnmergedTree = true).fetchSemanticsNodes()
        assertEquals("every row drew a title in its parts", rows.size, drawn.size)
        val oneLine = drawn.first().size.height
        assertEquals("the reference title is one line", 1, drawn.first().config[ActivityTitleLinesKey].size)
        val report = StringBuilder("$lang · row ${widthPx}px · text ${scale.wire} (one line = $oneLine px)")
        var wrapped = 0
        drawn.drop(1).zip(rows.drop(1)).forEach { (node, model) ->
            val lines = node.config[ActivityTitleLinesKey]
            val texts = lines.map { it.text }
            val place = model.titlePlace!!.place
            report.append("\n  ${if (model.hasFigure) "with a figure" else "no figure   "} | ${texts.joinToString(" ⏎ ")} | ${node.size.height} px")
            assertTrue("one line or two: $texts\n$report", lines.size in 1..2)
            // The action is whole, among one line's own words — never split across two.
            assertEquals("the action 「$action」 is on exactly one line: $texts\n$report", 1, texts.count { action in it })
            // A break is at an edge of the place: the place is whole on its line too (cut only by drawing, in its middle).
            assertEquals("the place is on exactly one line: $texts\n$report", 1, lines.count { it.place == place })
            // Nothing lost at the break but its space.
            assertEquals(model.title.filterNot { it.isWhitespace() }, texts.joinToString("").filterNot { it.isWhitespace() })
            assertTrue("the title is no wider than its row\n$report", node.size.width <= widthPx)
            // Broken, the action has a line to itself and what joins it to the place is on the place's line.
            if (lines.size == 2) assertEquals("the action's line: $texts\n$report", action, lines.single { it.place == null }.text)
            // And no line wrapped by itself: the title is that many single lines tall. The one exception is said by
            // the row itself — the action alone, wider than the whole of the room beside a figure, wraps between its
            // own words (never the line holding the place) — and it is one more line, no more.
            val wraps = lines.count { it.wraps }
            wrapped += wraps
            assertTrue("a line with the place wrapped: $lines\n$report", lines.none { it.wraps && it.place != null })
            assertEquals("the title is ${lines.size + wraps} line(s) tall: $texts\n$report", oneLine * (lines.size + wraps), node.size.height)
        }
        Log.i(TAG, report.toString())
        // Chinese: the action is one word. It never wraps, at any of these widths and sizes.
        if (lang == "zh") assertEquals("「$action」 took a second line\n$report", 0, wrapped)
        // On the 1080 px phone at the default text size nothing wraps by itself in either language. (Narrower, or
        // larger, "Contract interaction" beside a figure is wider than the room it has, and takes a line per word.)
        if (scale == TextScaleLevel.Standard && rowDp == PHONE) assertEquals("words wrapped at the default text size\n$report", 0, wrapped)
    }

    @Test fun chineseAt1080() = breaksOnlyBetweenItsParts("zh", PHONE, TextScaleLevel.Standard)
    @Test fun chineseAt1080AtTheLargestText() = breaksOnlyBetweenItsParts("zh", PHONE, TextScaleLevel.XLarge)
    @Test fun chineseInANarrowRow() = breaksOnlyBetweenItsParts("zh", NARROW, TextScaleLevel.Standard)
    @Test fun chineseInANarrowRowAtTheLargestText() = breaksOnlyBetweenItsParts("zh", NARROW, TextScaleLevel.XLarge)
    @Test fun englishAt1080() = breaksOnlyBetweenItsParts("en", PHONE, TextScaleLevel.Standard)
    @Test fun englishAt1080AtTheLargestText() = breaksOnlyBetweenItsParts("en", PHONE, TextScaleLevel.XLarge)
    @Test fun englishInANarrowRow() = breaksOnlyBetweenItsParts("en", NARROW, TextScaleLevel.Standard)
    @Test fun englishInANarrowRowAtTheLargestText() = breaksOnlyBetweenItsParts("en", NARROW, TextScaleLevel.XLarge)

    private companion object {
        const val TAG = "pr4-row"

        /** The list's width on the owner's phone: 1080 px at 440 dpi is 393 dp, less the screen's 24 dp margins — 948 px. */
        const val PHONE = 345

        /** And on a 320 dp phone: 272 dp — 748 px at 440 dpi. */
        const val NARROW = 272

        /** The core's term for a contract call: `componentsUi.signing.intentContractCall`. */
        const val TERM = "intentContractCall"
    }
}
