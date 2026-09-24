package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
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

    private fun done(state: String, withCall: Boolean = false) = JSONObject()
        .put("type", "done").put("state", state)
        .put("call", if (withCall) JSONObject().put("chain_id", 1).put("to", "0xreg").put("value", "0").put("data", "0xcd438f9b") else JSONObject.NULL)
        .put("unit_id", 10).toString()

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

    @Test
    fun `the backup row - four states, an affordance only where a tap does something, nothing when dark`() {
        fun row(state: RegistryBackup.State?) = SettingsLive.ethereumBackupRow(state, strings)
        assertNull(row(RegistryBackup.State.Unavailable))
        assertNull(row(RegistryBackup.State.NotRegistered))
        assertEquals(SettingsLive.ETHEREUM_BACKUP_ROW, row(null)!!.id)
        assertEquals("Back up public keys to Ethereum", row(null)!!.title)
        assertEquals("Checking…" to RowTrailing.None, row(null)!!.let { it.subtitle to it.trailing })
        assertEquals("Backed up on Ethereum" to RowTrailing.None, row(RegistryBackup.State.BackedUp)!!.let { it.subtitle to it.trailing })
        assertEquals("Not backed up yet" to RowTrailing.Chevron, row(RegistryBackup.State.NotBackedUp)!!.let { it.subtitle to it.trailing })
        // The retry (dead-controls #8): the one state where a tap is worth
        // taking, wearing the glyph that says "ask again" rather than a
        // chevron that would promise a page.
        assertEquals("Could not check" to RowTrailing.Retry, row(RegistryBackup.State.CouldNotCheck)!!.let { it.subtitle to it.trailing })
        // And the ripple goes exactly where the action is: a row with nothing
        // to do takes no taps at all.
        assertEquals(
            listOf(false, false, true, true),
            listOf(null, RegistryBackup.State.BackedUp, RegistryBackup.State.NotBackedUp, RegistryBackup.State.CouldNotCheck)
                .map { row(it)!!.actionable },
        )
    }

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
        val block = SettingsLive.withWalletKeys(model, registry, RegistryBackup.State.NotBackedUp, strings).keys!!
        assertEquals("Keys" to "3", block.title to block.count)
        assertEquals(listOf("Interleave", "Key 2", "Key 3"), block.rows.map { it.name })
        // The holder line now names WHERE the key lives (issue 207): a hybrid
        // key is reached on a phone or tablet, not a nameless "Passkey".
        assertEquals(listOf("Apple Passwords", "Security key", "Phone or tablet"), block.rows.map { it.holder })
        assertEquals(listOf(listOf("Cloud-synced"), listOf("Device-bound"), listOf("Cloud-synced")), block.rows.map { row -> row.pills.map { it.text } })
        assertEquals(listOf("Public key", "Transport"), block.rows.first().details.map { it.label })
        assertTrue(block.backupExplain.contains("Private keys never leave"))
        assertEquals("abab…abab", block.rows.first().fingerprint)
        assertNull(block.note)
        assertEquals(RowTrailing.Chevron, block.backup!!.trailing)
        // The sections below are untouched: the block is its own thing, not a row smuggled into one.
        assertSame(model.sections, SettingsLive.withWalletKeys(model, registry, null, strings).sections)
    }

    /**
     * Spec 075: a key minted on the Trusted Signer page lives behind that page.
     * The page signs in a browser and so reports `platform`; before the core's
     * rule moved, the row called it the built-in passkey of this phone — the
     * one side of the page the wallet cannot reach (device pass, 2026-09-22).
     */
    @Test
    fun `the keys block - a key behind a page names the page, not this device`() {
        val behind = key(name = "On the page", method = KeyMethod.TrustedSigner)
            .copy(signerOrigin = "http://localhost:8140")
        val block = SettingsLive.withWalletKeys(
            model,
            WalletKeys.Result(WalletKeys.Source.Device, listOf(behind, key("Built in"))),
            RegistryBackup.State.NotBackedUp,
            strings,
        ).keys!!
        assertEquals(listOf("Trusted Signer", "Built-in passkey"), block.rows.map { it.holder })
        // …and WHICH page, for somebody running their own deployment.
        assertEquals(
            "http://localhost:8140",
            block.rows.first().details.firstOrNull { it.label == "Trusted Signer" }?.value,
        )
        // A key that lives on an authenticator this device can reach says nothing about pages.
        assertTrue(block.rows[1].details.none { it.label == "Trusted Signer" })
    }

    @Test
    fun `the keys block - still asking, registry silent, and registry empty are three different things`() {
        val asking = SettingsLive.withWalletKeys(model, null, null, strings).keys!!
        assertTrue(asking.loading && asking.count.isEmpty() && asking.rows.isEmpty())

        val silent = SettingsLive.withWalletKeys(model, WalletKeys.Result(WalletKeys.Source.Device, listOf(key("Mine", synced = null))), RegistryBackup.State.CouldNotCheck, strings).keys!!
        assertEquals("Couldn't reach the registry. Showing what this device remembers.", silent.note)
        assertTrue(silent.rows.first().pills.isEmpty())

        // A registry that answered with nothing was not unreachable.
        val empty = SettingsLive.withWalletKeys(model, WalletKeys.Result(WalletKeys.Source.NotRegistered, listOf(key("Mine", synced = null))), RegistryBackup.State.NotRegistered, strings).keys!!
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
            val result = WalletKeys(ethCall = { _, _, _ -> null }, step = { _, _, answers -> seen += answers; script.removeFirst() })
                .read("0xsafe", listOf(WalletKeys.DeviceKey("04ab", "A", "")))
            assertEquals(source, result.source)
            assertEquals(KeyMethod.SecurityKey, result.rows.single().key.method)
            assertNull(result.rows.single().synced)
            assertEquals("failed", JSONArray(seen.last()).getJSONObject(0).getString("outcome"))
        }
    }
}
