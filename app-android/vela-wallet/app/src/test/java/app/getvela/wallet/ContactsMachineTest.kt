package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.contacts.core.ContactEvent
import app.getvela.wallet.feature.contacts.core.ContactOperation
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.contacts.core.ContactShellResult
import app.getvela.wallet.feature.contacts.core.ContactsExecutor
import app.getvela.wallet.feature.contacts.core.ContactsView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The address book end to end, on the real machine.
 *
 * The case this file exists for is the last one: **deleting the middle
 * contact**. Every wiring bug in this area looks identical on screen until the
 * moment it removes the wrong person, and it is the exact shape the desktop
 * sibling shipped.
 */
class ContactsMachineTest {

    private fun host(store: KeyValueStore): CoreHost<ContactsView> {
        val executor = ContactsExecutor(store)
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.ContactsCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
            initial = ContactsView(),
            serializer = ContactsView.serializer(),
            perform = JsonShell.perform(
                ContactOperation.serializer(),
                ContactShellResult.serializer(),
                executor::perform,
            ),
            escapedFailure = JsonShell.escapedFailure(
                ContactOperation.serializer(),
                ContactShellResult.serializer(),
                fallback = ContactShellResult.StoreLoaded(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<ContactsView>.settle(
        predicate: (ContactsView) -> Boolean,
    ): ContactsView = runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    private fun CoreHost<ContactsView>.send(event: ContactEvent) =
        dispatch(event, ContactEvent.serializer())

    private fun FakeStore.await(key: String, predicate: (String?) -> Boolean): String? =
        runBlocking {
            withTimeout(TIMEOUT_MS) {
                while (!predicate(values[key])) delay(10)
                values[key]
            }
        }

    private fun save(address: String, name: String) =
        ContactEvent.Save(
            input = ContactSaveInput(address = address, name = name),
            now_ms = 1_725_000_000_000.0,
        )

    @Test
    fun anEmptyDeviceShowsAnEmptyBook() {
        val host = host(FakeStore())
        host.send(ContactEvent.AccountSwitched(ME))

        val view = host.settle { it.loaded }
        assertTrue("no fixture people may appear", view.contacts.isEmpty())
    }

    @Test
    fun aSavedContactComesBack() {
        val store = FakeStore()
        val host = host(store)
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }

        host.send(save(ALICE, "Alice"))
        val view = host.settle { it.contacts.isNotEmpty() }

        assertEquals("Alice", view.contacts.single().name)
        assertTrue(
            "and it reached the shared key",
            store.await(KeyValueStore.Keys.CONTACTS) { it?.contains("Alice") == true } != null,
        )
    }

    @Test
    fun aSecondBookOverTheSameStoreAgrees() {
        val store = FakeStore()
        host(store).let { first ->
            first.send(ContactEvent.AccountSwitched(ME))
            first.settle { it.loaded }
            first.send(save(ALICE, "Alice"))
            first.settle { it.contacts.isNotEmpty() }
            store.await(KeyValueStore.Keys.CONTACTS) { it?.contains("Alice") == true }
        }

        val second = host(store)
        second.send(ContactEvent.AccountSwitched(ME))
        assertEquals("Alice", second.settle { it.loaded }.contacts.single().name)
    }

    @Test
    fun deletingTheMiddleContactRemovesThatOne() {
        // Three saved, and the one deleted is neither first nor last — the only
        // arrangement in which an off-by-one target survives the assertion.
        val store = FakeStore()
        val host = host(store)
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }

        host.send(save(ALICE, "Alice"))
        host.send(save(BOB, "Bob"))
        host.send(save(CAROL, "Carol"))
        host.settle { it.contacts.size == 3 }

        host.send(ContactEvent.Delete(BOB, 1_725_000_100_000.0))
        val view = host.settle { it.contacts.size == 2 }

        assertEquals(setOf("Alice", "Carol"), view.contacts.mapNotNull { it.name }.toSet())

        val stored = JSONArray(
            store.await(KeyValueStore.Keys.CONTACTS) { it?.contains("Bob") == false }!!,
        )
        val names = (0 until stored.length()).map { stored.getJSONObject(it).optString("name") }
        assertEquals(setOf("Alice", "Carol"), names.toSet())
    }

    @Test
    fun aGroupKnowsItsMembers() {
        val store = FakeStore()
        val host = host(store)
        host.send(ContactEvent.AccountSwitched(ME))
        host.settle { it.loaded }
        host.send(save(ALICE, "Alice"))
        host.send(save(BOB, "Bob"))
        host.settle { it.contacts.size == 2 }

        host.send(
            ContactEvent.GroupSave(
                app.getvela.wallet.feature.contacts.core.ContactGroupInput(
                    id = null,
                    name = "Team",
                    color = null,
                    members = listOf(ALICE, BOB),
                ),
            ),
        )

        val view = host.settle { it.groups.isNotEmpty() }
        val group = view.groups.single()
        assertEquals("Team", group.name)
        assertEquals(2, group.members.size)
        // Members are resolved to whole contacts, in membership order — the
        // core synthesises one for a member with no saved entry so sending to a
        // group never silently drops a payee.
        assertEquals(setOf("Alice", "Bob"), group.members.mapNotNull { it.name }.toSet())
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L

        // Twenty bytes each. The core validates an address before it will save
        // one, so a convenient "0xaaaa" is not a contact — it is a rejected
        // event, and a test written with one asserts nothing.
        const val ME = "0x1111111111111111111111111111111111111111"
        const val ALICE = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        const val BOB = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        const val CAROL = "0xcccccccccccccccccccccccccccccccccccccccc"
    }
}
