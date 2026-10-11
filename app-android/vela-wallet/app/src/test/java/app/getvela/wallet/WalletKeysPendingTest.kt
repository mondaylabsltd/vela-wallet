package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.settings.RowTrailing
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.RegistryBackup
import app.getvela.wallet.feature.settings.core.WalletKeys
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device walk, 2026-10-11: Settings dropped when its keys block loaded. Until
 * the registry answered the block was one skeleton row; then the wallet's keys
 * arrived and everything under the block moved down by their height (151 px on
 * the phone for a wallet of one key), and the header gained its count.
 *
 * The block now opens on what this device's own record says — the core's walk
 * run with no address, which asks no chain — and the registry's answer fills
 * those rows in place. What is pinned here is the MODEL: which rows there are
 * while asking, what they may say (only what the record knows for certain —
 * the same fix on the iPhone read "Built-in passkey" for three seconds until
 * the registry said "Security key"), and that the rooms a row and the
 * copy-record row keep are there from the first model. That the drawn page
 * then does not move is `SettingsKeysSteadyTest`, on a device.
 */
class WalletKeysPendingTest {
    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val model by lazy { SettingsFixtures.buildState(SettingsScreenState.ST1, strings) }

    // Three points on P-256 (1·G, 2·G, 3·G): keys the core reads as keys.
    private val keyA = "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
    private val keyB = "047cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc4766997807775510db8ed040293d9ac69f7430dbba7dade63ce982299e04b79d227873d1"
    private val keyC = "045ecbe4d1a6330a44c8f7ef951d4bf165e6c6b721efada985fb41661bc6e7fd6c8734640c4998ff7e374b06ce1a64a2ecd82ab036384fb83d9a79b127a27d5032"

    /** The record as `VelaWalletApplication.deviceKeysOf` hands it over: only the first key is named, no transports. */
    private val record = listOf(
        WalletKeys.DeviceKey(keyA, "F", "", "cred-a"),
        WalletKeys.DeviceKey(keyB, "", "", "cred-b"),
        WalletKeys.DeviceKey(keyC, "", "", "cred-c"),
    )

    private var asked = 0

    /** The real core, and a chain that counts every call it is asked for. */
    private val keys = WalletKeys(ethCall = { _, _, _ -> asked++; null })

    private val kinds = listOf("Built-in passkey", "Phone or tablet", "Security key")

    /** What the registry says of the same three keys: who holds each, and its badges. */
    private fun registry(): WalletKeys.Result {
        fun row(hex: String, name: String, provider: String, method: KeyMethod, synced: Boolean, signsHere: Boolean = false) = WalletKeys.Row(
            key = CreateKeyRow(name, if (method == KeyMethod.SecurityKey) "cross-platform" else "platform", if (method == KeyMethod.SecurityKey) "usb" else "internal", true, synced, "aaguid", provider, method),
            synced = synced,
            publicKeyHex = hex,
            credentialId = "credential",
            attestationHex = "0x01",
            userVerified = true,
            signsHere = signsHere,
        )
        return WalletKeys.Result(
            WalletKeys.Source.Registry,
            listOf(
                row(keyA, "F", "Google Password Manager", KeyMethod.Platform, synced = true),
                row(keyB, "Backup", "", KeyMethod.SecurityKey, synced = false, signsHere = true),
                row(keyC, "", "", KeyMethod.Hybrid, synced = true),
            ),
        )
    }

    @Test
    fun `the device's own record answers at once, and no chain is asked`() {
        val seen = ArrayList<List<String>>()
        val spied = WalletKeys(
            ethCall = { _, _, _ -> asked++; null },
            step = { address, device, answers, credential ->
                seen += listOf(address, answers, credential)
                uniffi.vela_core_uniffi.walletKeysStep(address, device, answers, credential)
            },
        )
        val own = spied.deviceOnly(record, "cred-b")

        // No address, so the core can ask nobody; one step, and it is done.
        assertEquals(listOf(listOf("", "[]", "cred-b")), seen)
        assertEquals("no chain was asked", 0, asked)
        assertEquals(WalletKeys.Source.Device, own.source)
        // Every key the wallet has, in founding order, named where the record names it.
        assertEquals(listOf("F", "", ""), own.rows.map { it.key.name })
        assertEquals(listOf(keyA, keyB, keyC).map { it.takeLast(128) }, own.rows.map { it.publicKeyHex.lowercase().takeLast(128) })
        // The sign-in key is the record's own fact.
        assertEquals(listOf(false, true, false), own.rows.map { it.signsHere })
        // Nobody has vouched for anything else.
        assertTrue(own.rows.all { it.synced == null && it.userVerified == null && it.key.providerName.isEmpty() })
    }

    @Test
    fun `while the registry is asked - the rows are the device's keys, and say no kind words`() {
        val block = SettingsLive.withWalletKeys(model, null, null, strings, device = keys.deviceOnly(record, "cred-b")).keys!!

        assertTrue("still asking", block.loading)
        // The rows and the count stand from the first model: nothing to grow into.
        assertEquals("3", block.count)
        assertEquals(3, block.rows.size)
        assertTrue(block.rows.all { it.pending })

        // The record's facts: the name it holds, the fingerprint, the sign-in key.
        assertEquals(listOf("F", "", ""), block.rows.map { it.name })
        assertEquals(listOf("6b17…51f5", "7cf2…73d1", "5ecb…5032"), block.rows.map { it.fingerprint })
        assertEquals(listOf(emptyList(), listOf("Signed in"), emptyList()), block.rows.map { row -> row.pills.map { it.text } })

        // …and nothing the registry may contradict. Not what holds the key —
        // the record does not know, and the core's device row calls every key
        // "platform" when it is told no transports…
        assertTrue("no holder is stated while asking", block.rows.all { it.holder.isEmpty() })
        // …not a name nobody chose ("Key 2": the registry may name it "Backup")…
        assertTrue(block.rows.none { row -> row.name.startsWith("Key ") })
        assertEquals(listOf("Key 1", "Key 2", "Key 3"), block.rows.map { it.nameRoom })
        // …and nothing to open onto yet.
        assertTrue(block.rows.all { it.details.isEmpty() })
        val said = block.rows.flatMap { row -> listOf(row.name, row.holder) + row.pills.map { it.text } }
        assertTrue("a kind word is said while asking: $said", said.none { it in kinds || it == "Verify to use" || it == "Cloud-synced" || it == "Device-bound" })
        // The device speaking for itself is not "couldn't reach the registry".
        assertNull(block.note)
    }

    @Test
    fun `when the registry answers - its rows fill the same places`() {
        val own = keys.deviceOnly(record, "cred-b")
        val asking = SettingsLive.withWalletKeys(model, null, null, strings, device = own).keys!!
        val answered = SettingsLive.withWalletKeys(model, registry(), null, strings, device = own).keys!!

        assertFalse(answered.loading)
        assertTrue(answered.rows.none { it.pending })
        // The same keys in the same order, and the same count: the answer adds no row.
        assertEquals(asking.count, answered.count)
        assertEquals(asking.rows.map { it.fingerprint }, answered.rows.map { it.fingerprint })
        // Now the registry's words: who holds each key, the name it recorded, its badges, its facts.
        assertEquals(listOf("Google Password Manager", "Security key", "Phone or tablet"), answered.rows.map { it.holder })
        assertEquals(listOf("F", "Backup", "Key 3"), answered.rows.map { it.name })
        assertEquals(
            listOf(listOf("Verify to use", "Cloud-synced"), listOf("Signed in", "Verify to use", "Device-bound"), listOf("Verify to use", "Cloud-synced")),
            answered.rows.map { row -> row.pills.map { it.text } },
        )
        assertTrue(answered.rows.all { it.details.isNotEmpty() })
    }

    @Test
    fun `a registry that never answers leaves the same rows, and says so in the room kept for it`() {
        val own = keys.deviceOnly(record, "cred-b")
        val asking = SettingsLive.withWalletKeys(model, null, null, strings, device = own).keys!!
        // The walk's own fallback when every chain is silent: the device's rows, as an ANSWER.
        val silent = SettingsLive.withWalletKeys(model, own, RegistryBackup.COULD_NOT, strings, device = own).keys!!

        assertFalse(silent.loading)
        assertEquals(asking.count, silent.count)
        assertEquals(asking.rows.map { it.fingerprint }, silent.rows.map { it.fingerprint })
        assertEquals("Couldn't reach the registry. Showing what this device remembers.", silent.note)
        // The sentence has had its room since the first model — the caption
        // under the title keeps it — so saying it moves nothing.
        assertEquals(silent.note, asking.noteRoom)
        assertNull(asking.note)
    }

    @Test
    fun `before the record itself is read - one skeleton row, and no guessed count`() {
        val block = SettingsLive.withWalletKeys(model, null, null, strings).keys!!
        assertTrue(block.loading && block.rows.isEmpty() && block.count.isEmpty())
    }

    @Test
    fun `a row keeps the room of what it can end with, from the first model`() {
        val own = keys.deviceOnly(record, "cred-b")
        val asking = SettingsLive.withWalletKeys(model, null, null, strings, device = own).keys!!
        val answered = SettingsLive.withWalletKeys(model, registry(), null, strings, device = own).keys!!

        // The fullest sets of badges the registry can vouch for on one key.
        assertEquals(listOf(listOf("Verify to use", "Cloud-synced"), listOf("Verify to use", "Device-bound")), asking.badgeRoom)
        // Every line a holder can read when the catalog names no vault, and a vault's name.
        assertEquals(kinds + SettingsLive.KEY_VAULT_ROOM, asking.holderRoom)
        // The same rooms before and after: that is what keeps a row one height.
        assertEquals(asking.badgeRoom, answered.badgeRoom)
        assertEquals(asking.holderRoom, answered.holderRoom)
        // Every badge and every kind line a row can show has room kept for it.
        val shown = answered.rows.flatMap { row -> row.pills.filter { it.text != "Signed in" }.map { it.text } }
        assertTrue(shown.all { text -> asking.badgeRoom.any { text in it } })
        assertTrue(answered.rows.map { it.holder }.all { it in asking.holderRoom })
    }

    @Test
    fun `the copy-record row keeps the room of every line it can read, and of its glyph`() {
        fun check(subtitle: String, action: RegistryBackup.Action, tone: RegistryBackup.Tone = RegistryBackup.Tone.Neutral) = RegistryBackup.Check(
            RegistryBackup.State.NotBackedUp,
            null,
            RegistryBackup.Row("settingsModals.backup.title", "settingsModals.backup.$subtitle", tone, action, "settingsModals.backup.explain"),
        )
        val asking = SettingsLive.ethereumBackupRow(null, strings)!!
        val states = listOf(
            SettingsLive.ethereumBackupRow(check("backedUp", RegistryBackup.Action.None, RegistryBackup.Tone.Positive), strings)!!,
            SettingsLive.ethereumBackupRow(check("notBackedUp", RegistryBackup.Action.Copy), strings)!!,
            SettingsLive.ethereumBackupRow(check("couldNotCheck", RegistryBackup.Action.Retry), strings)!!,
            SettingsLive.ethereumBackupRow(check("cannotCopy", RegistryBackup.Action.None), strings)!!,
        )
        assertEquals("Checking…" to RowTrailing.None, asking.subtitle to asking.trailing)
        // "Checking…" already holds the room of the line that replaces it…
        for (row in states) {
            assertTrue("no room kept for \"${row.subtitle}\"", row.subtitle in asking.subtitleRoom)
            assertEquals(asking.subtitleRoom, row.subtitleRoom)
            assertTrue(row.trailingRoom)
        }
        assertTrue(asking.subtitle in asking.subtitleRoom)
        // …and of the chevron or the retry glyph that arrives with it.
        assertTrue(asking.trailingRoom)
        assertEquals(listOf(RowTrailing.None, RowTrailing.Chevron, RowTrailing.Retry, RowTrailing.None), states.map { it.trailing })

        // The lines are the corpus's: every state line it has under
        // `settingsModals.backup` is one the row keeps room for. A state the
        // core gains later fails here instead of moving the page.
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        val backup = JSONObject(File(root, "assets/i18n/en.json").readText()).getJSONObject("settingsModals").getJSONObject("backup")
        val lines = backup.keys().asSequence().filter { it != "title" && it != "explain" }.map { "settingsModals.backup.$it" }.toSet()
        assertEquals(lines, SettingsLive.BACKUP_LINES.filter { it.startsWith("settingsModals.backup.") }.toSet())
    }

    /** A settings row whose second line is not a state keeps no room and no empty glyph place. */
    @Test
    fun `an ordinary settings row is unchanged`() {
        val rows = model.sections.flatMap { it.rows }
        assertTrue(rows.isNotEmpty())
        assertTrue(rows.all { it.subtitleRoom.isEmpty() && !it.trailingRoom })
    }
}
