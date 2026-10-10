package app.getvela.wallet

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.CreateView
import app.getvela.wallet.feature.onboarding.flow.Fixture
import app.getvela.wallet.feature.onboarding.flow.FlowFixtures
import app.getvela.wallet.feature.onboarding.flow.KEY_COUNT_TAG
import app.getvela.wallet.feature.onboarding.flow.KeysScreen
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The integration's notes 17 and 22 on the keys screen, as drawn.
 *
 * With no key the heading over the three places is "Choose where it lives"
 * — not the screen's title said again — and there is no "Added 0 / 7": a
 * count of nothing over an empty list. The counter is the core's
 * (`CreateView.key_count_shown`) and arrives with the first key, heading the
 * list it counts. Its arrival moves nothing ABOVE it — the title and the
 * subtitle stay exactly where they were — and what is below moves down by
 * the list that appeared, counter and key together.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.KeysCounterTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class KeysCounterTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private fun board(code: String): CreateView =
        (FlowFixtures.all.first { it.code == code }.fixture as Fixture.Flow).view

    @Test
    fun theCounterArrivesWithTheFirstKeyAndMovesNothingAboveIt() {
        val none = board("keys · signing page offered")
        // The same screen once two keys are in — the title unchanged (nothing blocks it).
        val some = board("keys · two, ready")
        assertTrue(none.keys.isEmpty() && !none.keyCountShown)
        assertTrue(some.keys.size == 2 && some.keyCountShown)

        var view by mutableStateOf(none)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    Column(modifier = Modifier.fillMaxSize()) {
                        KeysScreen(
                            keys = view.keys,
                            canAddKey = view.canAddKey,
                            canFinish = view.canFinish,
                            needsSecondKey = view.needsSecondKey,
                            busy = view.busy,
                            addHeadingKey = view.addHeadingKey,
                            methodsPinned = view.methodsPinned,
                            keyCountShown = view.keyCountShown,
                            addMethods = view.addMethods,
                            onAddKey = {},
                            onConfirmKey = {},
                            onRemoveKey = {},
                            onFinish = {},
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
        val title = strings.t(I18nKeys.Create.KEYS_TITLE)
        val subtitle = strings.t(I18nKeys.Create.KEYS_SUBTITLE)
        fun bounds(text: String) = compose.onNodeWithText(text).fetchSemanticsNode().boundsInRoot
        fun counters() = compose.onAllNodesWithTag(KEY_COUNT_TAG, useUnmergedTree = true).fetchSemanticsNodes()

        // No key: the new heading, and no counter at all.
        compose.onNodeWithText("Choose where it lives").assertExists()
        assertEquals("the heading is not the title said again", "Add passkeys", title)
        assertEquals(0, counters().size)
        compose.onNodeWithText(strings.t(I18nKeys.Create.KEYS_LABEL)).assertDoesNotExist()
        val titleAt = bounds(title)
        val subtitleAt = bounds(subtitle)
        val hintAt = bounds(strings.t(I18nKeys.Create.KEYS_HINT))

        // Keys: the counter heads their list.
        view = some
        compose.waitForIdle()
        val counter = counters().single().boundsInRoot
        compose.onNodeWithText("2 / 7").assertExists()
        assertEquals("the title did not move", titleAt, bounds(title))
        assertEquals("the subtitle did not move", subtitleAt, bounds(subtitle))
        assertTrue("the counter is under the subtitle", counter.top > subtitleAt.bottom)
        val first = bounds(some.keys.first().name)
        assertTrue("…and over the first key", counter.bottom <= first.top)
        android.util.Log.i(
            "KeysCounter",
            "title ${titleAt.top} subtitle ${subtitleAt.top} (both frames) · counter ${counter.top}..${counter.bottom} · first key ${first.top} · hint ${hintAt.top} -> ${bounds(strings.t(I18nKeys.Create.KEYS_HINT)).top}",
        )

        // Back to none: the counter goes with the list.
        view = none
        compose.waitForIdle()
        assertEquals(0, counters().size)
        assertEquals(titleAt, bounds(title))
        assertEquals(hintAt, bounds(strings.t(I18nKeys.Create.KEYS_HINT)))
    }
}
