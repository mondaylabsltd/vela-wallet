package app.getvela.wallet

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.flows.ContactEntryModel
import app.getvela.wallet.feature.flows.ContactPickBody
import app.getvela.wallet.feature.flows.ContactPickModel
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #467: a contact is picked by its ADDRESS — the row says who it is,
 * and the tap sends that, whatever order the book is in when the finger lands.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ContactPickByAddressTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ContactPickByAddressTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun aPickedContactIsNamedByAddress() {
        val alice = "0x1111111111111111111111111111111111111111"
        val bob = "0x2222222222222222222222222222222222222222"
        val picked = mutableListOf<String>()
        compose.setContent {
            VelaTheme(darkTheme = false) {
                ContactPickBody(
                    model = ContactPickModel(
                        title = "Choose a contact",
                        closeLabel = "Close",
                        searchPlaceholder = "Search",
                        groupsTitle = "Groups",
                        groups = emptyList(),
                        contactsTitle = "Contacts",
                        contacts = listOf(
                            ContactEntryModel(name = "Alice", addressDisplay = "0x1111…1111", identiconSeed = alice, address = alice),
                            ContactEntryModel(name = "Bob", addressDisplay = "0x2222…2222", identiconSeed = bob, address = bob),
                        ),
                    ),
                    onSelect = { picked += it },
                )
            }
        }
        compose.onNodeWithText("Bob").performClick()
        compose.onNodeWithText("Alice").performClick()
        assertEquals(listOf(bob, alice), picked)
    }
}
