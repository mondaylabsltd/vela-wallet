package app.getvela.wallet.feature.contacts.core

import app.getvela.wallet.core.data.KeyValueStore
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `contacts` core touches the outside world.
 *
 * Seven operations: **three storage, four network**. All seven are live; the
 * one remaining gap is named in place, in `resolve_identity`.
 *
 * Port source:
 * `app-web/vela-wallet/src/lib/contacts/core/contacts-executor.ts`, itself a
 * port of the Expo client's — so these bytes have been the same across every
 * Vela client since the address book existed, and that is the point:
 *
 * - `vela.contacts` is an ARRAY of camelCase records with **optional fields
 *   omitted rather than written as null**, byte-identical to what the other
 *   clients produce.
 * - `vela.contacts.dismissed` is a **map** of `address → epoch ms`, not a list.
 *   Writing a list here would make every other client read "no dismissals" and
 *   resurrect contacts a person had removed.
 * - `vela.contactGroups` is an array of `{id, name, color?, members: []}`.
 *
 * Coercion is hygiene, never policy: a record with no address is dropped, a
 * junk file reads as empty. What it must never do is *throw* — a rejected
 * `store_loaded` leaves the core unloaded forever and silently discards every
 * later write.
 */
class ContactsExecutor(
    private val store: KeyValueStore,
    /**
     * A name for an address, from the passkey index.
     *
     * A seam rather than a client, so the storage arms stay testable without a
     * network. The default answers `null` — no identity — which is what the
     * core reads as "look again later", never as "this person has no name".
     */
    private val registryName: suspend (String) -> String? = { null },
    /**
     * `eth_getCode` on one chain. `null` is unknown, and the default is
     * therefore honest: with no chain reader wired, nothing has been checked.
     */
    private val code: suspend (Int, String) -> String? = { _, _ -> null },
) {

    suspend fun perform(operation: ContactOperation): ContactShellResult = when (operation) {

        // -- storage: live ----------------------------------------------------

        is ContactOperation.ReadStore -> ContactShellResult.StoreLoaded(
            contacts = readContacts(),
            tombstones = readTombstones(),
            groups = readGroups(),
        )

        is ContactOperation.WriteContacts -> {
            store.write(
                KeyValueStore.Keys.CONTACTS,
                JSONArray().apply {
                    operation.contacts.forEach { put(toStoredContact(it)) }
                }.toString(),
            )
            ContactShellResult.Written
        }

        is ContactOperation.WriteDismissed -> {
            store.write(
                KeyValueStore.Keys.CONTACTS_DISMISSED,
                JSONObject().apply {
                    operation.tombstones.forEach { put(it.address, it.dismissed_at_ms.toLongOrDouble()) }
                }.toString(),
            )
            ContactShellResult.Written
        }

        is ContactOperation.WriteGroups -> {
            store.write(
                KeyValueStore.Keys.CONTACT_GROUPS,
                JSONArray().apply {
                    operation.groups.forEach { put(toStoredGroup(it)) }
                }.toString(),
            )
            ContactShellResult.Written
        }

        // -- what the rest of the app knows -----------------------------------

        /*
         * The people this person has actually paid.
         *
         * The core builds half the address book out of send history — somebody
         * you have paid twice is somebody worth offering by name. It reads the
         * SAME store the activity feed reads, so a payment that shows in
         * Activity is a payment the address book knows about.
         */
        is ContactOperation.LoadSendHistory -> ContactShellResult.HistoryLoaded(txs = sendHistory())

        /*
         * A name for an address.
         *
         * The passkey index knows the people who have a Vela wallet, and that
         * is the lookup worth having here: it is why paying another Vela user
         * can show a name where every other wallet shows forty hex characters.
         *
         * `null` is "no identity anywhere", never an invented name — the core
         * caches only a positive answer, so a nameless address is asked about
         * again rather than remembered as nameless.
         */
        is ContactOperation.ResolveIdentity -> ContactShellResult.IdentityResolved(
            address = operation.address,
            identity = registryName(operation.address)?.let {
                ContactIdentity(name = it, source = "passkey")
            },
            // The ENS-style reverse waterfall (.bnb, .arb, Basenames) needs a
            // namehash, which needs keccak256 — the core has one for selectors
            // and it should be the owner of this too rather than a third
            // hand-written copy. // live in 042
        )

        /*
         * Is this address a contract?
         *
         * The raw `eth_getCode` answer, untouched. `null` is UNKNOWN, not a
         * verdict: answering `"0x"` would tell the core this address is
         * definitely not a contract — a claim nothing here checked, and one a
         * trust badge gets drawn from.
         */
        is ContactOperation.ClassifyRecipient -> ContactShellResult.RecipientClassified(
            chain_id = operation.chain_id,
            address = operation.address,
            code = code(operation.chain_id, operation.address),
        )
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: ContactOperation): ContactShellResult = when (operation) {
        is ContactOperation.ReadStore -> ContactShellResult.StoreLoaded()
        is ContactOperation.WriteContacts,
        is ContactOperation.WriteDismissed,
        is ContactOperation.WriteGroups,
        -> ContactShellResult.Written
        // A failure to read history is a different fact from an empty history,
        // and the core has a variant for each.
        is ContactOperation.LoadSendHistory -> ContactShellResult.HistoryFailed
        is ContactOperation.ResolveIdentity ->
            ContactShellResult.IdentityResolved(operation.address, null)
        is ContactOperation.ClassifyRecipient ->
            ContactShellResult.RecipientClassified(operation.chain_id, operation.address, null)
    }

    // -- stored ⇄ wire -------------------------------------------------------

    /**
     * Who this person has paid, from the activity store.
     *
     * The SAME key the feed reads, so the address book and Activity can never
     * disagree about whether a payment happened.
     *
     * **The store keeps epoch seconds; the core wants milliseconds.** They sit
     * a thousand apart and both are plain numbers, so the conversion is the
     * whole of this function's risk: forget it and every "last paid" reads as
     * 1970, which sorts a person's most recent recipient to the bottom.
     */
    private suspend fun sendHistory(): List<ContactHistoryTx> {
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { index ->
            val row = array.optJSONObject(index) ?: return@mapNotNull null
            val seconds = row.optDouble("timestamp", Double.NaN)
            ContactHistoryTx(
                kind = when (row.optString("type")) {
                    "send" -> ContactTxKind.Send
                    "receive" -> ContactTxKind.Receive
                    "dapp_tx", "dappTx" -> ContactTxKind.DappTx
                    // A record with no type is a legacy row, and what that
                    // means is the core's rule, not this one's.
                    else -> null
                },
                to = row.optString("to").ifBlank { null },
                to_name = row.optString("toName").ifBlank { null },
                timestamp_ms = if (seconds.isNaN()) null else seconds * 1000,
            )
        }
    }

    private suspend fun readContacts(): List<Contact> =
        objects(store.read(KeyValueStore.Keys.CONTACTS))
            // A record with no address has no key, and the core's whole model
            // is addressed by it. Dropped, not defaulted to "".
            .filter { it.optString("address").isNotEmpty() }
            .map { record ->
                Contact(
                    address = record.optString("address").lowercase(),
                    name = record.optionalString("name"),
                    resolved_name = record.optionalString("resolvedName"),
                    resolved_source = record.optionalString("resolvedSource"),
                    kind = kindOf(record.optString("kind")),
                    favorite = record.optBoolean("favorite", false),
                    note = record.optionalString("note"),
                    tx_count = record.optInt("txCount", 0).coerceAtLeast(0),
                    last_used_ms = record.optDouble("lastUsed", 0.0),
                    first_seen_ms = record.optDouble("firstSeen", 0.0),
                    source = if (record.optString("source") == "auto") {
                        ContactSource.Auto
                    } else {
                        ContactSource.Manual
                    },
                )
            }

    /** Optional fields are OMITTED, never written as null — see the header. */
    private fun toStoredContact(contact: Contact): JSONObject = JSONObject().apply {
        put("address", contact.address)
        contact.name?.let { put("name", it) }
        contact.resolved_name?.let { put("resolvedName", it) }
        contact.resolved_source?.let { put("resolvedSource", it) }
        put("kind", wireKind(contact.kind))
        put("favorite", contact.favorite)
        contact.note?.let { put("note", it) }
        put("txCount", contact.tx_count)
        // Epoch ms as whole numbers. `org.json` renders a Double in scientific
        // notation — `1.725E12` — which is legal JSON that every parser
        // accepts and NO other Vela client has ever written. Storage is a
        // cross-client contract down to how a number looks in the file.
        put("lastUsed", contact.last_used_ms.toLongOrDouble())
        put("firstSeen", contact.first_seen_ms.toLongOrDouble())
        put("source", if (contact.source == ContactSource.Auto) "auto" else "manual")
    }

    /** A MAP of address → epoch ms. Not a list. */
    private suspend fun readTombstones(): List<ContactTombstone> {
        val record = obj(store.read(KeyValueStore.Keys.CONTACTS_DISMISSED)) ?: return emptyList()
        return record.keys().asSequence().mapNotNull { address ->
            val at = record.optDouble(address, Double.NaN)
            if (at.isNaN()) null else ContactTombstone(address.lowercase(), at)
        }.toList()
    }

    private suspend fun readGroups(): List<ContactGroup> =
        objects(store.read(KeyValueStore.Keys.CONTACT_GROUPS)).map { record ->
            ContactGroup(
                id = record.optString("id"),
                name = record.optString("name"),
                color = record.optionalString("color"),
                members = record.optJSONArray("members").strings(),
            )
        }

    private fun toStoredGroup(group: ContactGroup): JSONObject = JSONObject().apply {
        put("id", group.id)
        put("name", group.name)
        group.color?.let { put("color", it) }
        put("members", JSONArray().apply { group.members.forEach { put(it) } })
    }

    private fun kindOf(raw: String): ContactKind = when (raw) {
        "eoa" -> ContactKind.Eoa
        "account" -> ContactKind.Account
        else -> ContactKind.Unknown
    }

    private fun wireKind(kind: ContactKind): String = when (kind) {
        ContactKind.Eoa -> "eoa"
        ContactKind.Account -> "account"
        ContactKind.Unknown -> "unknown"
    }

    // -- json hygiene --------------------------------------------------------

    private fun obj(raw: String?): JSONObject? =
        raw?.let { runCatching { JSONObject(it) }.getOrNull() }

    private fun objects(raw: String?): List<JSONObject> {
        val array = raw?.let { runCatching { JSONArray(it) }.getOrNull() } ?: return emptyList()
        return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
    }

    private fun JSONArray?.strings(): List<String> =
        if (this == null) emptyList() else (0 until length()).mapNotNull { optString(it).ifEmpty { null } }

    private fun JSONObject.optionalString(key: String): String? =
        if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

    /** A whole number stays whole in the file; anything else keeps its point. */
    private fun Double.toLongOrDouble(): Any =
        if (this == toLong().toDouble()) toLong() else this
}
