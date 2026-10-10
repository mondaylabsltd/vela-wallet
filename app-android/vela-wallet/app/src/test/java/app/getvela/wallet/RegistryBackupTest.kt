package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.KeyPillTone
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.RowTrailing
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.settings.core.RegistryBackup
import app.getvela.wallet.feature.settings.core.WalletKeys
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 062: the Android transport for `vela_core::registry_backup`, and the
 * settings row it feeds. The walk is the core's and is tested there; pinned
 * here is that requests are carried faithfully (a bare `0x` is an ANSWER, a
 * `null` a silence), that a call is only ever offered with "not backed up",
 * and that the row is a button only while there is something to do.
 */
class RegistryBackupTest {
    private val transcripts = ArrayList<JSONArray>()

    private fun ask(id: String, chain: Int) = JSONObject().put("type", "ask").put(
        "requests",
        JSONArray().put(JSONObject().put("type", "eth_call").put("id", id).put("chain_id", chain).put("to", "0xreg").put("data", "0xdata")),
    ).toString()

    /** A `done` as the core writes it; [row] is the core's row for the state (`BackupState::row`), absent where it draws none. */
    private fun done(state: String, withCall: Boolean = false, row: Triple<String, String, String>? = null, explained: Boolean = state != "not_copyable") = JSONObject()
        .put("type", "done").put("state", state)
        .put("call", if (withCall) JSONObject().put("chain_id", 1).put("to", "0xreg").put("value", "0").put("data", "0xcd438f9b") else JSONObject.NULL)
        .put("unit_id", 10)
        .apply {
            if (row != null) {
                put(
                    "row",
                    JSONObject().put("title_key", "settingsModals.backup.title").put("subtitle_key", "settingsModals.backup.${row.first}")
                        .put("tone", row.second).put("action", row.third)
                        // As the core writes it: the key is LEFT OUT where no paragraph applies.
                        .apply { if (explained) put("explain_key", "settingsModals.backup.explain") },
                )
            }
        }.toString()

    private fun backup(script: List<String>, chain: (Int) -> String? = { "0x" }): RegistryBackup {
        var round = 0
        return RegistryBackup(
            ethCall = { chainId, _, _ -> chain(chainId) },
            step = { _, _, answers, _ -> transcripts += JSONArray(answers); script[round++] },
        )
    }

    @Test
    fun `answers are handed back verbatim - 0x is an answer, null a silence`() = runBlocking {
        val check = backup(listOf(ask("target", 1), ask("groups", 100), done("not_backed_up", withCall = true))) { if (it == 1) "0x" else null }
            .check("0xsafe", "04ab")
        assertEquals(RegistryBackup.State.NotBackedUp, check.state)
        assertEquals(RegistryBackup.Call(1, "0xreg", "0xcd438f9b"), check.call)
        val last = transcripts.last()
        assertEquals("ok" to "0x", last.getJSONObject(0).let { it.getString("outcome") to it.getString("body") })
        assertEquals("failed", last.getJSONObject(1).getString("outcome"))
        assertTrue(last.getJSONObject(1).isNull("body"))
    }

    @Test
    fun `every state maps, and a call without its state is not believed`() = runBlocking {
        for ((wire, state) in listOf(
            "unavailable" to RegistryBackup.State.Unavailable,
            "not_registered" to RegistryBackup.State.NotRegistered,
            "backed_up" to RegistryBackup.State.BackedUp,
            "could_not_check" to RegistryBackup.State.CouldNotCheck,
            // A wallet from before registry V13: its own state, not a retry for ever.
            "not_copyable" to RegistryBackup.State.NotCopyable,
            "something new" to RegistryBackup.State.CouldNotCheck,
        )) {
            val check = backup(listOf(done(wire))).check("0xsafe", "04ab")
            assertEquals(state, check.state)
            assertNull(check.call)
        }
        // "Not backed up" with nothing to send is not something to show a fee for.
        assertEquals(RegistryBackup.State.CouldNotCheck, backup(listOf(done("not_backed_up"))).check("0xsafe", "04ab").state)
    }

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val model by lazy { SettingsFixtures.buildState(SettingsScreenState.ST1, strings) }

    /** The row is the core's: its words, its tone, its tap (`BackupState::row`) — read off the step, never re-mapped. */
    private fun rowFor(doneJson: String) = runBlocking {
        SettingsLive.ethereumBackupRow(backup(listOf(doneJson)).check("0xsafe", "04ab"), strings)
    }

    @Test
    fun `the backup row - the core's words, tone and tap, and nothing where it draws none`() {
        // No registry on Ethereum, no record to copy: the core sends no row.
        assertNull(rowFor(done("unavailable")))
        assertNull(rowFor(done("not_registered")))
        // Still asking: the title and "Checking…", inert.
        val asking = SettingsLive.ethereumBackupRow(null, strings)!!
        assertEquals(SettingsLive.ETHEREUM_BACKUP_ROW, asking.id)
        assertEquals("Copy this wallet's record to Ethereum", asking.title)
        assertEquals("Checking…" to RowTrailing.None, asking.subtitle to asking.trailing)

        val copied = rowFor(done("backed_up", row = Triple("backedUp", "positive", "none")))!!
        assertEquals("Copied to Ethereum" to RowTrailing.None, copied.subtitle to copied.trailing)
        assertTrue("said in the positive tone", copied.subtitlePositive)

        // Not a warning: a copy is optional and costs a fee. A tap opens the sheet.
        val notYet = rowFor(done("not_backed_up", withCall = true, row = Triple("notBackedUp", "neutral", "copy")))!!
        assertEquals("Not copied yet (optional)" to RowTrailing.Chevron, notYet.subtitle to notYet.trailing)
        assertFalse(notYet.subtitlePositive)

        // The retry (dead-controls #8): the glyph that says "ask again", not a
        // chevron that would promise a page.
        val couldNot = rowFor(done("could_not_check", row = Triple("couldNotCheck", "neutral", "retry")))!!
        assertEquals("Couldn't check. Tap to try again." to RowTrailing.Retry, couldNot.subtitle to couldNot.trailing)

        // A wallet from before registry V13: a calm end, nothing to tap. It
        // used to read "Couldn't check" and retry for ever.
        val never = rowFor(done("not_copyable", row = Triple("cannotCopy", "neutral", "none")))!!
        assertEquals("This older wallet can't be copied" to RowTrailing.None, never.subtitle to never.trailing)
        assertFalse(never.subtitlePositive)

        // The ripple goes exactly where the action is.
        assertEquals(
            listOf(false, false, true, true, false),
            listOf(asking, copied, notYet, couldNot, never).map { it.actionable },
        )
    }

    /** A walk that never reaches the core's `done` is this transport's own "couldn't check" — the core's row for it, a retry. */
    @Test
    fun `a walk that never ends is could not check, with the retry`() = runBlocking {
        val check = backup(List(32) { ask("again", 1) }).check("0xsafe", "04ab")
        assertEquals(RegistryBackup.State.CouldNotCheck, check.state)
        assertEquals(RegistryBackup.Action.Retry, check.row?.action)
        val row = SettingsLive.ethereumBackupRow(check, strings)!!
        assertEquals("Couldn't check. Tap to try again." to RowTrailing.Retry, row.subtitle to row.trailing)
    }

    /**
     * The REAL core, both ends of the wire: with nobody answering it ends on
     * `could_not_check` and sends the row this shell draws; with a chain that
     * has no registry it sends no row, and nothing is drawn.
     */
    @Test
    fun `the core's own done carries the row`() = runBlocking {
        val address = "0x88cCA0e4a4bE5c1A2C1b6bA0a6bE4e2D80266894"
        val key = "04" + "ab".repeat(64)
        val real = { chain: (Int) -> String? ->
            RegistryBackup(
                ethCall = { chainId, _, _ -> chain(chainId) },
                step = { a, k, answers, target -> uniffi.vela_core_uniffi.registryBackupStep(a, k, answers, target) },
            )
        }
        val silent = real { null }.check(address, key)
        assertEquals(RegistryBackup.State.CouldNotCheck, silent.state)
        assertEquals(
            RegistryBackup.Row(
                "settingsModals.backup.title", "settingsModals.backup.couldNotCheck", RegistryBackup.Tone.Neutral, RegistryBackup.Action.Retry,
                explainKey = "settingsModals.backup.explain",
            ),
            silent.row,
        )
        // The transport's own give-up is that row, word for word.
        assertEquals(silent.row, RegistryBackup.COULD_NOT.row)
        val noRegistry = real { "0x" }.check(address, key)
        assertEquals(RegistryBackup.State.Unavailable, noRegistry.state)
        assertNull(noRegistry.row)
        assertNull(SettingsLive.ethereumBackupRow(noRegistry, strings))
    }

    /**
     * The integration's note 6: the paragraph under the row is the core's per
     * state (`BackupRow.explain_key`). While the walk runs, and under every
     * state a copy can still be made or checked in, it is the explanation;
     * under a wallet that can NEVER be copied there is none — it described
     * making a copy the row above had just said cannot be made.
     */
    @Test
    fun `the explanation under the row is the core's, and absent where a copy can never be made`() = runBlocking {
        val keys = WalletKeys.Result(WalletKeys.Source.Registry, listOf(key("Mine")))
        fun explain(check: RegistryBackup.Check?) = SettingsLive.withWalletKeys(model, keys, check, strings).keys!!.backupExplain
        val words = strings.t("settingsModals.backup.explain")

        assertEquals("still asking: the explanation itself", words, explain(null))
        for ((state, row) in listOf(
            "backed_up" to Triple("backedUp", "positive", "none"),
            "not_backed_up" to Triple("notBackedUp", "neutral", "copy"),
            "could_not_check" to Triple("couldNotCheck", "neutral", "retry"),
        )) {
            val check = backup(listOf(done(state, withCall = state == "not_backed_up", row = row))).check("0xsafe", "04ab")
            assertEquals(state, "settingsModals.backup.explain", check.row?.explainKey)
            assertEquals(state, words, explain(check))
        }
        // The transport's own give-up is the core's could-not-check row, paragraph included.
        assertEquals(words, explain(RegistryBackup.COULD_NOT))

        val never = backup(listOf(done("not_copyable", row = Triple("cannotCopy", "neutral", "none")))).check("0xsafe", "04ab")
        assertEquals(RegistryBackup.State.NotCopyable, never.state)
        assertNull("the core leaves the key out", never.row?.explainKey)
        assertNull("no paragraph, and no room kept for one", explain(never))

        // A JSON null reads as absent too — never the word "null" as a key.
        val nulled = JSONObject(done("not_copyable", row = Triple("cannotCopy", "neutral", "none")))
        nulled.getJSONObject("row").put("explain_key", JSONObject.NULL)
        assertNull(backup(listOf(nulled.toString())).check("0xsafe", "04ab").row?.explainKey)

        // The boards: SK1–SK3 explain, SK4 (the older wallet) does not.
        assertEquals(
            listOf(true, true, true, false),
            listOf(SettingsScreenState.SK1, SettingsScreenState.SK2, SettingsScreenState.SK3, SettingsScreenState.SK4)
                .map { SettingsFixtures.buildState(it, strings).keys!!.backupExplain != null },
        )
    }

    /** The SK boards draw each state with the core's keys — every one in the corpus, none echoed. */
    @Test
    fun `the keys boards carry a row for each state`() {
        val titles = listOf(SettingsScreenState.SK1, SettingsScreenState.SK2, SettingsScreenState.SK3, SettingsScreenState.SK4)
            .map { SettingsFixtures.buildState(it, strings).keys!!.backup!! }
        assertEquals(
            listOf("Not copied yet (optional)", "Copied to Ethereum", "Couldn't check. Tap to try again.", "This older wallet can't be copied"),
            titles.map { it.subtitle },
        )
        assertEquals(listOf(RowTrailing.Chevron, RowTrailing.None, RowTrailing.Retry, RowTrailing.None), titles.map { it.trailing })
        assertTrue(titles.all { it.title == "Copy this wallet's record to Ethereum" })
    }

    /** "Not copied yet": the core's row, and the call a tap sends. */
    private val notCopied = SettingsFixtures.backupCheck(SettingsScreenState.SK1)!!

    private fun key(name: String = "", provider: String = "", method: KeyMethod = KeyMethod.Platform, synced: Boolean? = true) = WalletKeys.Row(
        key = CreateKeyRow(name, "platform", "internal", true, synced ?: true, "", provider, method),
        synced = synced,
        publicKeyHex = "04" + "ab".repeat(64),
    )

    @Test
    fun `the keys block - every key named, its holder said, badges only where someone can vouch`() {
        val registry = WalletKeys.Result(
            WalletKeys.Source.Registry,
            listOf(key("Interleave", "Apple Passwords"), key(method = KeyMethod.SecurityKey, synced = false), key(method = KeyMethod.Hybrid)),
        )
        val block = SettingsLive.withWalletKeys(model, registry, notCopied, strings).keys!!
        assertEquals("Keys" to "3", block.title to block.count)
        assertEquals(listOf("Interleave", "Key 2", "Key 3"), block.rows.map { it.name })
        // The holder line now names WHERE the key lives (issue 207): a hybrid
        // key is reached on a phone or tablet, not a nameless "Passkey".
        assertEquals(listOf("Apple Passwords", "Security key", "Phone or tablet"), block.rows.map { it.holder })
        assertEquals(listOf(listOf("Cloud-synced"), listOf("Device-bound"), listOf("Cloud-synced")), block.rows.map { row -> row.pills.map { it.text } })
        assertEquals(listOf("Public key", "Transport"), block.rows.first().details.map { it.label })
        // The copy's own explanation: what becomes public, that it costs a fee,
        // and what a copy cannot do — never "only public keys".
        val explain = block.backupExplain!!
        assertTrue(explain, explain.contains("is public"))
        assertTrue(explain, explain.contains("credential ID and authenticator model"))
        assertTrue(explain, explain.contains("pay its network fee"))
        assertTrue(explain, explain.contains("can't move money or bring back a lost passkey"))
        assertEquals("abab…abab", block.rows.first().fingerprint)
        assertNull(block.note)
        assertEquals(RowTrailing.Chevron, block.backup!!.trailing)
        // The sections below are untouched: the block is its own thing, not a row smuggled into one.
        assertSame(model.sections, SettingsLive.withWalletKeys(model, registry, null, strings).sections)
    }

    /**
     * Spec 102: a key is captioned by where it LIVES — never by the page it
     * was made on. A key made on a signing page is an ordinary passkey (the
     * page ran the ceremony in a browser), and the row says so; there is no
     * "Trusted Signer" holder or detail row any more.
     */
    @Test
    fun `the keys block - a key is captioned by its place, never by a page`() {
        val block = SettingsLive.withWalletKeys(
            model,
            WalletKeys.Result(WalletKeys.Source.Device, listOf(key("Made on a page"), key("Built in"))),
            notCopied,
            strings,
        ).keys!!
        assertEquals(listOf("Built-in passkey", "Built-in passkey"), block.rows.map { it.holder })
        assertTrue(block.rows.all { row -> row.details.none { it.label.contains("Trusted") } })
        // A wallet on the apps' own domain says nothing about domains.
        assertNull(block.domain)
    }

    /** Spec 102: a wallet whose keys belong to its own domain says which (`settings.signing.keysOn`). */
    @Test
    fun `the keys block - a custom-domain wallet names its domain`() {
        val block = SettingsLive.withWalletKeys(
            model,
            WalletKeys.Result(WalletKeys.Source.Device, listOf(key("Mine"))),
            notCopied,
            strings,
            signingDomain = "example.com",
        ).keys!!
        assertEquals("Keys on example.com", block.domain)
    }

    @Test
    fun `the keys block - still asking, registry silent, and registry empty are three different things`() {
        val asking = SettingsLive.withWalletKeys(model, null, null, strings).keys!!
        assertTrue(asking.loading && asking.count.isEmpty() && asking.rows.isEmpty())

        val silent = SettingsLive.withWalletKeys(model, WalletKeys.Result(WalletKeys.Source.Device, listOf(key("Mine", synced = null))), RegistryBackup.COULD_NOT, strings).keys!!
        assertEquals("Couldn't reach the registry. Showing what this device remembers.", silent.note)
        assertTrue(silent.rows.first().pills.isEmpty())

        // A registry that answered with nothing was not unreachable.
        val empty = SettingsLive.withWalletKeys(model, WalletKeys.Result(WalletKeys.Source.NotRegistered, listOf(key("Mine", synced = null))), RegistryBackup.Check(RegistryBackup.State.NotRegistered, null), strings).keys!!
        assertNull(empty.note)
        assertNull(empty.backup)
    }

    @Test
    fun `the keys transport - requests carried, the three sources told apart, a null badge kept null`() = runBlocking {
        val ask = JSONObject().put("type", "ask").put("requests", JSONArray().put(JSONObject().put("type", "eth_call").put("id", "groups@100").put("chain_id", 100).put("to", "0xreg").put("data", "0xd"))).toString()
        fun done(source: String) = JSONObject().put("type", "done").put("source", source).put("chain_id", JSONObject.NULL)
            .put("keys", JSONArray().put(JSONObject().put("name", "A").put("method", "security_key").put("synced", JSONObject.NULL).put("public_key_hex", "04ab").put("provider_name", "").put("aaguid", "").put("transports", "usb").put("authenticator_attachment", ""))).toString()
        for ((wire, source) in listOf("registry" to WalletKeys.Source.Registry, "device" to WalletKeys.Source.Device, "not_registered" to WalletKeys.Source.NotRegistered)) {
            val script = ArrayDeque(listOf(ask, done(wire)))
            val seen = ArrayList<String>()
            val result = WalletKeys(ethCall = { _, _, _ -> null }, step = { _, _, answers, _ -> seen += answers; script.removeFirst() })
                .read("0xsafe", listOf(WalletKeys.DeviceKey("04ab", "A", "")), "")
            assertEquals(source, result.source)
            assertEquals(KeyMethod.SecurityKey, result.rows.single().key.method)
            assertNull(result.rows.single().synced)
            assertEquals("failed", JSONArray(seen.last()).getJSONObject(0).getString("outcome"))
        }
    }

    /**
     * Founder, 2026-09-26: the key this device signs with stands out. It wears
     * the one FILLED pill, first in the row, labelled from the corpus; every
     * other row keeps only the registry's outlined facts.
     */
    @Test
    fun `the keys block - the key this device signs with wears the filled pill, first`() {
        val rows = listOf(key("First"), key("Second").copy(signsHere = true), key("Third", synced = false))
        val block = SettingsLive.withWalletKeys(model, WalletKeys.Result(WalletKeys.Source.Registry, rows), null, strings).keys!!
        assertEquals(
            listOf(listOf("Cloud-synced"), listOf("Signed in", "Cloud-synced"), listOf("Device-bound")),
            block.rows.map { row -> row.pills.map { it.text } },
        )
        assertEquals(KeyPillTone.SignsHere, block.rows[1].pills.first().tone)
        assertTrue(
            "only the signing row is marked",
            block.rows.filterIndexed { index, _ -> index != 1 }.all { row -> row.pills.none { it.tone == KeyPillTone.SignsHere } },
        )
    }

    /** Every step of the walk — the fallback too — is asked with the account's sign-in credential. */
    @Test
    fun `the keys walk - carries the sign-in credential on every step, and each key's credential`() = runBlocking<Unit> {
        val ask = JSONObject().put("type", "ask").put("requests", JSONArray().put(JSONObject().put("type", "eth_call").put("id", "groups@100").put("chain_id", 100).put("to", "0xreg").put("data", "0xd"))).toString()
        val credentials = ArrayList<String>()
        val devices = ArrayList<String>()
        // Asks forever: the walk gives up after its rounds and falls back to the device alone.
        WalletKeys(ethCall = { _, _, _ -> null }, step = { _, device, _, credential -> credentials += credential; devices += device; ask })
            .read("0xsafe", listOf(WalletKeys.DeviceKey("04ab", "A", "", credentialId = "c2")), "c2")
        assertTrue(credentials.size > 1)
        assertTrue("every call: $credentials", credentials.all { it == "c2" })
        assertEquals("c2", JSONArray(devices.first()).getJSONObject(0).getString("credential_id"))
    }

    /** Through the real core: the row whose public key the sign-in credential names, and no other. */
    @Test
    fun `the keys walk - the core marks the sign-in key's row, and none for a record without one`() = runBlocking<Unit> {
        val device = listOf(
            WalletKeys.DeviceKey("04" + "11".repeat(64), "One", "internal", credentialId = "c1"),
            WalletKeys.DeviceKey("04" + "22".repeat(64), "Two", "internal", credentialId = "c2"),
        )
        // No address: the device's own memory, and nobody to ask.
        val walk = WalletKeys(ethCall = { _, _, _ -> null })
        assertEquals(listOf(false, true), walk.read("", device, "c2").rows.map { it.signsHere })
        assertEquals(listOf(false, false), walk.read("", device, "").rows.map { it.signsHere })
    }
}
