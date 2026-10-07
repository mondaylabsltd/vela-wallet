package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.hasSetTextAction
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.components.MultiPickSheet
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactsView
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #437: a book of 59 imported contacts, and Add member showed the 14
 * that fit the screen — the menu it used could not scroll, and its own Cancel
 * was below the edge. Here the sheet is drawn as the contacts page draws it,
 * over 59 people: the last can be scrolled to, ticked and saved, and Save
 * stays on screen throughout.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.MultiPickSheetTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class MultiPickSheetTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings by lazy {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private val book = ContactsView(
        loaded = true,
        contacts = (1..59).map { i -> Contact(address = "0x" + i.toString(16).padStart(40, '0'), name = "Person $i") },
    )

    @Test
    fun theFiftyNinthContactCanBeReachedTickedAndSaved() {
        val group = ContactGroupView(id = "g1", name = "gy", members = listOf(book.contacts[0]))
        var ticks by mutableStateOf(setOf(ContactsLive.pickKey(book.contacts[0].address)))
        var query by mutableStateOf("")
        var saved: List<String>? = null
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    MultiPickSheet(
                        model = ContactsLive.memberPick(book, ticks, query, strings),
                        onToggle = { address ->
                            val key = ContactsLive.pickKey(address)
                            ticks = if (key in ticks) ticks - key else ticks + key
                        },
                        onQueryChange = { query = it },
                        onSave = { saved = ContactsLive.membersAfterPick(group, book, ticks) },
                        onDismiss = {},
                    )
                }
            }
        }
        val save = strings.t(I18nKeys.Contacts.SAVE)

        compose.onNodeWithText(save).assertIsDisplayed()
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText("Person 59"))
        compose.onNodeWithText("Person 59").assertIsDisplayed().performClick()
        compose.waitForIdle()
        compose.onNodeWithText(save).assertIsDisplayed().performClick()
        compose.waitForIdle()

        assertEquals(
            listOf(book.contacts[0].address, book.contacts[58].address).map(ContactsLive::pickKey),
            saved,
        )
    }

    @Test
    fun aLongBookCanBeSearched() {
        var ticks by mutableStateOf(emptySet<String>())
        var query by mutableStateOf("")
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    MultiPickSheet(
                        model = ContactsLive.memberPick(book, ticks, query, strings),
                        onToggle = { address -> ticks = ticks + ContactsLive.pickKey(address) },
                        onQueryChange = { query = it },
                        onSave = {},
                        onDismiss = {},
                    )
                }
            }
        }
        compose.onNode(hasSetTextAction()).performTextInput("Person 42")
        compose.waitForIdle()
        // The field holds the query too: the row is the checkbox that says it.
        val row = isToggleable() and hasText("Person 42")
        compose.onNode(row).assertIsDisplayed().performClick()
        compose.waitForIdle()
        assertEquals(setOf(ContactsLive.pickKey(book.contacts[41].address)), ticks)
        // A checkbox to accessibility, and the search hides the rest.
        compose.onNode(row).assertIsOn()
        compose.onNodeWithText("Person 1").assertDoesNotExist()
    }
}
