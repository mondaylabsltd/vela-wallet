package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactGroup
import app.getvela.wallet.feature.contacts.core.ContactKind
import app.getvela.wallet.feature.contacts.core.ContactOperation
import app.getvela.wallet.feature.contacts.core.ContactShellResult
import app.getvela.wallet.feature.contacts.core.ContactSource
import app.getvela.wallet.feature.contacts.core.ContactTombstone
import app.getvela.wallet.feature.contacts.core.ContactTxKind
import app.getvela.wallet.feature.contacts.core.ContactsExecutor
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The `contacts` shell contract, and above all the **stored bytes**.
 *
 * These three keys have had the same shapes since the Expo client, and every
 * Vela client reads them. The tests that matter here are the ones that would
 * otherwise fail silently on somebody else's device: a tombstone map written as
 * a list, an optional field written as `null` instead of omitted, an address
 * that kept its capitalisation.
 */
class ContactsExecutorTest {

    /** One store per test, so the history cases can write into the same one. */
    private val store = FakeStore()

    private fun executor(store: KeyValueStore = this.store) = ContactsExecutor(store)

    private fun contact(
        address: String,
        name: String? = null,
        favorite: Boolean = false,
    ) = Contact(
        address = address,
        name = name,
        kind = ContactKind.Eoa,
        favorite = favorite,
        tx_count = 3,
        last_used_ms = 1_725_000_000_000.0,
        first_seen_ms = 1_720_000_000_000.0,
        source = ContactSource.Manual,
    )

    // -- read_store ----------------------------------------------------------

    @Test
    fun anEmptyDeviceHasAnEmptyBook() = runBlocking {
        val loaded = executor().perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        assertTrue(loaded.contacts.isEmpty())
        assertTrue(loaded.tombstones.isEmpty())
        assertTrue(loaded.groups.isEmpty())
    }

    @Test
    fun storedContactsAreReadInTheOtherClientsShapes() = runBlocking {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CONTACTS to JSONArray().put(
                    JSONObject()
                        .put("address", "0xAAAAaaaaAAAAaaaaAAAAaaaaAAAAaaaaAAAAaaaa")
                        .put("name", "Alice")
                        .put("resolvedName", "alice.eth")
                        .put("kind", "eoa")
                        .put("favorite", true)
                        .put("txCount", 7)
                        .put("lastUsed", 1_725_000_000_000L)
                        .put("firstSeen", 1_720_000_000_000L)
                        .put("source", "manual"),
                ).toString(),
            ),
        )

        val loaded = executor(store)
            .perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        val alice = loaded.contacts.single()

        // Lowercased on the way in: the address is the canonical key, and a
        // checksummed copy would be a second contact for the same person.
        assertEquals("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", alice.address)
        assertEquals("Alice", alice.name)
        assertEquals("alice.eth", alice.resolved_name)
        assertEquals(ContactKind.Eoa, alice.kind)
        assertTrue(alice.favorite)
        assertEquals(7, alice.tx_count)
    }

    @Test
    fun tombstonesAreAMapNotAList() = runBlocking {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CONTACTS_DISMISSED to
                    JSONObject().put("0xBBBB", 1_725_000_000_000L).toString(),
            ),
        )
        val loaded = executor(store)
            .perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        assertEquals("0xbbbb", loaded.tombstones.single().address)
        assertEquals(1_725_000_000_000.0, loaded.tombstones.single().dismissed_at_ms, 0.0)
    }

    @Test
    fun aRecordWithNoAddressIsDroppedNotDefaulted() = runBlocking {
        // The whole model is addressed by it. A blank address would collide
        // with any other blank one and merge two strangers into one row.
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CONTACTS to
                    JSONArray().put(JSONObject().put("name", "Nobody")).toString(),
            ),
        )
        val loaded = executor(store)
            .perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        assertTrue(loaded.contacts.isEmpty())
    }

    @Test
    fun aCorruptFileReadsAsEmptyNotAsHalfABook() = runBlocking {
        // Half an address book is worse than none, because a person cannot tell
        // it is half — they conclude a contact was lost and re-add it.
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CONTACTS to "{ truncated",
                KeyValueStore.Keys.CONTACTS_DISMISSED to "[]",
                KeyValueStore.Keys.CONTACT_GROUPS to "null",
            ),
        )
        val loaded = executor(store)
            .perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        assertTrue(loaded.contacts.isEmpty())
        assertTrue(loaded.tombstones.isEmpty())
        assertTrue(loaded.groups.isEmpty())
    }

    // -- the writes ----------------------------------------------------------

    @Test
    fun anOptionalFieldIsOmittedRatherThanWrittenAsNull() = runBlocking {
        val store = FakeStore()
        executor(store).perform(
            ContactOperation.WriteContacts(listOf(contact("0xabc", name = null))),
        )
        val stored = JSONArray(store.values[KeyValueStore.Keys.CONTACTS]).getJSONObject(0)

        // The stored JSON must stay identical to what the other clients write.
        assertFalse("an absent name is an absent KEY", stored.has("name"))
        assertFalse(stored.has("note"))
        assertEquals("0xabc", stored.getString("address"))
        assertEquals("eoa", stored.getString("kind"))
        assertEquals("manual", stored.getString("source"))
        assertEquals(3, stored.getInt("txCount"))
    }

    @Test
    fun dismissalsAreWrittenAsAnAddressToTimeMap() = runBlocking {
        val store = FakeStore()
        executor(store).perform(
            ContactOperation.WriteDismissed(
                listOf(ContactTombstone("0xabc", 1_725_000_000_000.0)),
            ),
        )
        val stored = JSONObject(store.values[KeyValueStore.Keys.CONTACTS_DISMISSED]!!)
        // A LIST here would make every other client read "nothing dismissed"
        // and resurrect contacts this person removed.
        assertEquals(1_725_000_000_000.0, stored.getDouble("0xabc"), 0.0)
    }

    @Test
    fun groupsRoundTrip() = runBlocking {
        val store = FakeStore()
        val group = ContactGroup("g1", "Team", color = null, members = listOf("0xabc", "0xdef"))
        executor(store).perform(ContactOperation.WriteGroups(listOf(group)))

        val loaded = executor(store)
            .perform(ContactOperation.ReadStore) as ContactShellResult.StoreLoaded
        assertEquals(group, loaded.groups.single())
    }

    @Test
    fun aRefusedWriteStillAnswers() = runBlocking {
        val store = FakeStore()
        store.refuseWrites = true
        assertEquals(
            ContactShellResult.Written,
            executor(store).perform(ContactOperation.WriteContacts(listOf(contact("0xabc")))),
        )
    }

    // -- the fail-closed four ------------------------------------------------

    @Test
    fun historyIsTruthfullyEmptyNotFailed() = runBlocking {
        // Android has no local transaction store yet. Empty and failed are
        // different facts and the core has a variant for each; answering
        // `history_failed` would make the book look broken rather than new.
        assertEquals(
            ContactShellResult.HistoryLoaded(emptyList()),
            executor().perform(ContactOperation.LoadSendHistory),
        )
    }

    @Test
    fun identityIsUnresolvedRatherThanGuessed() = runBlocking {
        val result = executor().perform(ContactOperation.ResolveIdentity("0xabc"))
                as ContactShellResult.IdentityResolved
        assertEquals("0xabc", result.address)
        assertNull(result.identity)
    }

    @Test
    fun anUnclassifiedRecipientIsUnknownNotCleared() = runBlocking {
        // `code = null` is ignorance. Answering `"0x"` would tell the core this
        // address is definitely not a contract — a claim nothing has checked,
        // and one a trust badge is drawn from.
        val result = executor().perform(ContactOperation.ClassifyRecipient(1, "0xabc"))
                as ContactShellResult.RecipientClassified
        assertNull(result.code)
        assertEquals("0xabc", result.address)
    }

    @Test
    fun aFailedHistoryReadHasItsOwnAnswer() {
        // The one place the neutral answer differs from the happy one: an
        // exception reading history is not an empty history.
        assertEquals(
            ContactShellResult.HistoryFailed,
            executor().neutralAnswer(ContactOperation.LoadSendHistory),
        )
    }

    // -- the history-derived half of the book (spec 041 phase 7) -------------

    private suspend fun writeActivity(vararg rows: JSONObject) {
        val array = JSONArray()
        rows.forEach(array::put)
        store.write(KeyValueStore.Keys.TRANSACTIONS, array.toString())
    }

    private fun sent(to: String, seconds: Double, name: String? = null) = JSONObject()
        .put("id", "tx-$to-$seconds")
        .put("type", "send")
        .put("to", to)
        .put("timestamp", seconds)
        .apply { if (name != null) put("toName", name) }

    /**
     * **Seconds in, milliseconds out.**
     *
     * The activity store keeps epoch SECONDS and the core wants MILLISECONDS.
     * They are a thousand apart and both are plain numbers, so nothing catches
     * a missed conversion except this: every "last paid" would read as 1970,
     * and a person's most recent recipient would sort to the bottom of their
     * own address book.
     */
    @Test
    fun `send history crosses in milliseconds, not seconds`() = runBlocking {
        writeActivity(sent("0xAAAA", 1_756_900_000.0, name = "Alice"))

        val result = executor().perform(ContactOperation.LoadSendHistory)

        val tx = (result as ContactShellResult.HistoryLoaded).txs.single()
        assertEquals(1_756_900_000_000.0, tx.timestamp_ms!!, 1.0)
        assertEquals("0xAAAA", tx.to)
        assertEquals("Alice", tx.to_name)
        assertEquals(ContactTxKind.Send, tx.kind)
    }

    @Test
    fun `the address book reads the same store the feed reads`() = runBlocking {
        // Not a copy, not a second key: a payment that shows in Activity is a
        // payment the address book knows about, or the two disagree about
        // whether it happened.
        writeActivity(sent("0xAAAA", 1.0), sent("0xBBBB", 2.0))

        val result = executor().perform(ContactOperation.LoadSendHistory)

        assertEquals(2, (result as ContactShellResult.HistoryLoaded).txs.size)
    }

    @Test
    fun `an untyped legacy row is left for the core to interpret`() = runBlocking {
        writeActivity(JSONObject().put("id", "x").put("to", "0xAAAA").put("timestamp", 1.0))

        val result = executor().perform(ContactOperation.LoadSendHistory)

        assertNull((result as ContactShellResult.HistoryLoaded).txs.single().kind)
    }

    @Test
    fun `an empty store is an empty history, not a failure`() = runBlocking {
        val result = executor().perform(ContactOperation.LoadSendHistory)

        assertEquals(emptyList<Any>(), (result as ContactShellResult.HistoryLoaded).txs)
    }

    // -- identity and classification -----------------------------------------

    @Test
    fun `a name from the index becomes an identity`() = runBlocking {
        val subject = ContactsExecutor(store, registryName = { "Bob" })

        val result = subject.perform(ContactOperation.ResolveIdentity("0xAAAA"))

        val identity = (result as ContactShellResult.IdentityResolved).identity!!
        assertEquals("Bob", identity.name)
        assertEquals("passkey", identity.source)
    }

    @Test
    fun `no name anywhere resolves to nothing, never to an invented one`() = runBlocking {
        val result = executor().perform(ContactOperation.ResolveIdentity("0xAAAA"))

        assertNull((result as ContactShellResult.IdentityResolved).identity)
    }

    @Test
    fun `a code read that failed stays unknown, not "not a contract"`() = runBlocking {
        // `"0x"` means "definitely not a contract" and a trust badge is drawn
        // from it. A read that never answered has not earned that claim.
        val result = executor().perform(ContactOperation.ClassifyRecipient(1, "0xAAAA"))

        assertNull((result as ContactShellResult.RecipientClassified).code)
    }

    @Test
    fun `a code read passes the chain's answer through untouched`() = runBlocking {
        val subject = ContactsExecutor(store, code = { _, _ -> "0x60806040" })

        val result = subject.perform(ContactOperation.ClassifyRecipient(137, "0xAAAA"))

        val classified = result as ContactShellResult.RecipientClassified
        assertEquals("0x60806040", classified.code)
        assertEquals(137, classified.chain_id)
    }
}
