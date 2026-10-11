package app.getvela.wallet

import android.graphics.Color as AndroidColor
import android.util.Log
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.platform.testTag
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.TX_DELETE_TAG
import app.getvela.wallet.feature.flows.TxDetailBody
import app.getvela.wallet.feature.flows.TxDetailModel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 10: on a transfer's detail 「删除记录」 was the
 * heaviest button on the sheet — full width, filled with the error colour —
 * under the hash and "View on explorer".
 *
 * The real detail body, for a transfer and for a dApp record (the same body,
 * with and without an explorer link), on a real renderer: the delete is a
 * text action — its box carries the error colour on its letters only, never
 * as a fill — it is the LAST thing on the sheet, under the explorer button,
 * a finger tall, and it still deletes. A pending record's is plain ink.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.TxDetailDeleteQuietTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class TxDetailDeleteQuietTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("zh") }
    }
    private val delete = strings.t(I18nKeys.Flows.DELETE_RECORD)

    private val BODY = "tx-detail-body"

    private fun detail(state: FlowState) = (FlowFixtures.build(state, strings).sheet as FlowSheet.TxDetail).model

    /** How much of the delete action's own box is drawn in [ink] — a fill is most of it, letters are a sliver. */
    private fun share(ink: Int): Float {
        val image = compose.onNodeWithTag(TX_DELETE_TAG).captureToImage().asAndroidBitmap()
        var hits = 0
        for (y in 0 until image.height) for (x in 0 until image.width) {
            val pixel = image.getPixel(x, y)
            val near = kotlin.math.abs(AndroidColor.red(pixel) - AndroidColor.red(ink)) < 40 &&
                kotlin.math.abs(AndroidColor.green(pixel) - AndroidColor.green(ink)) < 40 &&
                kotlin.math.abs(AndroidColor.blue(pixel) - AndroidColor.blue(ink)) < 40
            if (near) hits++
        }
        return hits.toFloat() / (image.width * image.height)
    }

    @Test
    fun deleteIsAQuietTextActionUnderTheUsefulOnes() {
        var deleted = 0
        var error = 0
        var muted = 0
        val transfer = detail(FlowState.A3).copy(deleteLabel = delete)
        var current by mutableStateOf<TxDetailModel>(transfer)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    error = VelaTheme.colors.errorBase.toArgb()
                    muted = VelaTheme.colors.fgMuted.toArgb()
                    Column(Modifier.padding(24.dp)) {
                        Box(Modifier.testTag(BODY)) { TxDetailBody(model = current, onDelete = { deleted++ }) }
                    }
                }
            }
        }
        val dp = compose.density.density
        val cases = listOf(
            "a transfer" to transfer,
            // A dApp record: no hash's explorer to open, a note, and nothing else useful under its facts.
            "a dApp record with no explorer link" to detail(FlowState.A2).copy(deleteLabel = delete, explorerShown = false),
            "a dApp record with its explorer link" to detail(FlowState.A2).copy(deleteLabel = delete),
        )
        val measured = StringBuilder()
        for ((name, model) in cases) {
            current = model
            compose.waitForIdle()
            val action = compose.onNodeWithTag(TX_DELETE_TAG)
            action.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            compose.onNodeWithText(delete).assertExists()
            val box = action.fetchSemanticsNode().boundsInRoot
            val red = share(error)
            measured.append("\n$name: delete ${box.top.toInt()}..${box.bottom.toInt()} (${(box.height / dp).toInt()}dp tall), error-colour ink ${"%.1f".format(red * 100)}% of its box")
            assertTrue("$name: the delete is filled with the error colour (${red * 100}% of its box)$measured", red < 0.12f)
            assertTrue("$name: the delete's words are in the error colour$measured", red > 0.002f)
            assertTrue("$name: a finger's height$measured", box.height >= 44 * dp - 1)
            if (model.explorerShown) {
                val explorer = compose.onNodeWithText(model.viewOnExplorer).fetchSemanticsNode().boundsInRoot
                assertTrue("$name: the delete is not under the explorer button$measured", box.top >= explorer.bottom)
            }
            // Nothing of the sheet is under it: it is the last thing in the body.
            val body = compose.onNodeWithTag(BODY).fetchSemanticsNode().boundsInRoot
            assertEquals("$name: something stands under the delete$measured", body.bottom, box.bottom, 1f)
        }
        Log.i("pr4-delete", measured.toString())

        // Spec 082 RJ18: a pending record's delete is quieter again — plain ink, not the error colour.
        current = transfer.copy(deleteQuiet = true)
        compose.waitForIdle()
        assertTrue("a pending record's delete is still red", share(error) < 0.002f)
        assertTrue("…and is drawn in plain ink", share(muted) > 0.002f)

        compose.onNodeWithTag(TX_DELETE_TAG).performClick()
        compose.waitForIdle()
        assertEquals("it still deletes", 1, deleted)
    }
}
