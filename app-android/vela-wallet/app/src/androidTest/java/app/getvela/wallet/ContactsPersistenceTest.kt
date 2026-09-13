package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.contacts.core.ContactsController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * SC-004 on a device, across two real process deaths.
 *
 * Three phases, each meant to be a **separate `am instrument` invocation** with
 * a `force-stop` between them:
 *
 * ```bash
 * P=app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * T=app.getvela.wallet.ContactsPersistenceTest
 * adb shell am instrument -w -e class "$T#threeContactsAreSaved" "$P"
 * adb shell am force-stop app.getvela.wallet
 * adb shell am instrument -w -e class "$T#theMiddleOneIsDeleted" "$P"
 * adb shell am force-stop app.getvela.wallet
 * adb shell am instrument -w -e class "$T#onlyTheOtherTwoRemain" "$P"
 * ```
 *
 * The middle one is the target because it is the only position where an
 * off-by-one still fails the assertion. Deleting the first or the last passes
 * under several wrong implementations.
 */
@RunWith(AndroidJUnit4::class)
class ContactsPersistenceTest {

    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    private fun controller() = ContactsController(
        context = context,
        scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    )

    private fun awaitStore(predicate: (String?) -> Boolean): String? = runBlocking {
        val store = VelaStore(context)
        withTimeout(TIMEOUT_MS) {
            var value = store.read(KeyValueStore.Keys.CONTACTS)
            while (!predicate(value)) {
                delay(20)
                value = store.read(KeyValueStore.Keys.CONTACTS)
            }
            value
        }
    }

    @Test
    fun threeContactsAreSaved() = runBlocking {
        VelaStore(context).remove(KeyValueStore.Keys.CONTACTS)

        val contacts = controller()
        contacts.open(ME)
        withTimeout(TIMEOUT_MS) { contacts.view.first { it.loaded } }

        contacts.save(ContactSaveInput(address = ALICE, name = "Alice"))
        contacts.save(ContactSaveInput(address = BOB, name = "Bob"))
        contacts.save(ContactSaveInput(address = CAROL, name = "Carol"))
        withTimeout(TIMEOUT_MS) { contacts.view.first { it.contacts.size == 3 } }

        val stored = awaitStore { it?.contains("Carol") == true }!!
        assertTrue(stored.contains("Alice"))
        assertTrue(stored.contains("Bob"))
    }

    @Test
    fun theMiddleOneIsDeleted() = runBlocking {
        val contacts = controller()
        contacts.open(ME)
        // A fresh process: this must read three back before deleting anything,
        // or the delete would be operating on a book it never loaded.
        val before = withTimeout(TIMEOUT_MS) { contacts.view.first { it.contacts.size == 3 } }
        assertEquals(
            setOf("Alice", "Bob", "Carol"),
            before.contacts.mapNotNull { it.name }.toSet(),
        )

        contacts.delete(BOB)
        withTimeout(TIMEOUT_MS) { contacts.view.first { it.contacts.size == 2 } }
        val stored = awaitStore { it?.contains("Bob") == false }!!
        assertTrue("the other two are still on disk", stored.contains("Alice"))
        assertTrue(stored.contains("Carol"))
    }

    @Test
    fun onlyTheOtherTwoRemain() = runBlocking {
        val contacts = controller()
        contacts.open(ME)
        val view = withTimeout(TIMEOUT_MS) { contacts.view.first { it.loaded } }

        assertEquals(2, view.contacts.size)
        assertEquals(setOf("Alice", "Carol"), view.contacts.mapNotNull { it.name }.toSet())
        assertFalse(
            "the deleted contact must not come back on a cold start",
            view.contacts.any { it.address == BOB },
        )
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
        const val ME = "0x1111111111111111111111111111111111111111"
        const val ALICE = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        const val BOB = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        const val CAROL = "0xcccccccccccccccccccccccccccccccccccccccc"
    }
}
