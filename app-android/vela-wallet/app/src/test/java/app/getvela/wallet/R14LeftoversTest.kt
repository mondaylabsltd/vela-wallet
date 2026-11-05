package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.designsystem.components.AddressBreaks
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.addPressLabel
import app.getvela.wallet.feature.multisig.importStatus
import app.getvela.wallet.feature.multisig.core.MsAddView
import app.getvela.wallet.feature.multisig.core.MsImportView
import app.getvela.wallet.feature.multisig.core.MsPhase
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonNull
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 14's leftovers on Android (docs/multisig-machines.md §21), at the
 * shell's pure seams and through the REAL machines (the host core over
 * uniffi, [MultisigWorld] behind it):
 *
 * - the add sheet reads the core's Try again (`view.add.retry`) and its
 *   `busy` answer;
 * - a rate-limited import is the core's `busy`, with Try again, never
 *   "unavailable";
 * - the fee signed is the fee shown by the core's rule alone — no shell copy;
 * - a 6+6 short form inside prose is bound whole for drawing.
 */
class R14LeftoversTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val code = File(repoRoot, "rust/crates/vela-core/tests/vectors/safe-code")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val words = MultisigWords(
        strings = strings,
        accounts = app.getvela.wallet.feature.multisig.core.MsAccountsView(),
        localNames = emptyMap(),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    // -- item 5: a short form is never broken at its ellipsis ----------------------

    @Test
    fun `a short form inside prose is bound whole, and nothing else is touched`() {
        val joined = AddressBreaks.wholeShortForms("0x777E98…61875a · Not shared · 2 of 3")
        assertEquals("0x777E98⁠…⁠61875a · Not shared · 2 of 3", joined)
        // Two in one line (a member change), CJK around them.
        assertEquals(
            "将 0xD40086⁠…⁠de130b 换成 0x88cCA0⁠…⁠266894",
            AddressBreaks.wholeShortForms("将 0xD40086…de130b 换成 0x88cCA0…266894"),
        )
        // Prose ellipses, full addresses and names stay as they are.
        for (text in listOf("Reading the transaction…", "0x777E989C821a7F1b031696f044a2fF114261875a", "Ops…", "…")) {
            assertEquals(text, AddressBreaks.wholeShortForms(text))
        }
        // Bound once: drawing it again changes nothing.
        assertEquals(joined, AddressBreaks.wholeShortForms(joined))
    }

    // -- item 2: the add sheet reads the core's Try again -------------------------

    @Test
    fun `the add view carries the core's Try again, and Add says it`() {
        val raw = Wire.json.parseToJsonElement("""{"phase":"done","address":"0xab","outcome":{"kind":"busy","chain_id":100},"retry":true}""")
        val view = MsAddView.of(raw)
        assertTrue(view.retry)
        assertFalse(MsAddView.of(Wire.json.parseToJsonElement("""{"phase":"done"}""")).retry)
        assertEquals("Try again", addPressLabel(words, retry = true, failed = true))
        assertEquals(words.t("add.add"), addPressLabel(words, retry = true, failed = false))
        assertEquals(words.t("add.add"), addPressLabel(words, retry = false, failed = true))
    }

    @Test
    fun `the import view carries the rows the core offers Try again for`() {
        val raw = Wire.json.parseToJsonElement("""{"safes":[],"error":null,"busy":null,"outcomes":{"0xab":{"kind":"busy","chain_id":100}},"retry":["0xab"]}""")
        val view = MsImportView.of(raw)
        assertEquals(listOf("0xab"), view.retry)
        assertTrue(view.offersRetry("0xAB"))
        assertFalse(view.offersRetry("0xcd"))
        assertEquals("Gnosis", words.network(100))
        assertEquals(words.t("add.busy", "network" to "Gnosis"), importStatus(words, view.outcomes["0xab"]))
        assertNull(importStatus(words, null))
    }

    // -- items 2 and 4, through the real machines -----------------------------------

    private fun world() = MultisigWorld { name -> File(code, "$name.hex").readText().trim() }

    private fun <T> withHub(world: MultisigWorld, body: suspend (MultisigHub) -> T): T {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val hub = MultisigHub(
                live = world,
                facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
                scope = scope,
            )
            try {
                return runBlocking { withTimeout(60_000) { hub.boot(); body(hub) } }
            } finally {
                hub.dispose()
            }
        } finally {
            scope.cancel()
        }
    }

    /** A 1-of-1 of fixture account 0, its genesis with the service, deployed on Gnosis — not kept here. */
    private fun recorded(world: MultisigWorld, salt: String): String {
        val owner = world.address(0)
        val g = world.genesis(listOf(owner), 1, "Ops", salt)
        val address = g["address"].str!!
        world.records[address.lowercase()] = Wire.json.parseToJsonElement(
            """{"found":true,"pending":false,"genesis":${g["genesis"]},"recordedOn":[100],"readFrom":100}""",
        )
        world.deploy(100, address, MultisigWorld.Safe(listOf(owner), 1))
        return address
    }

    @Test
    fun `a rate-limited add is the core's busy, and its Try again adds once the limit lifts`() {
        val world = world()
        val address = recorded(world, "0x61")
        world.rateLimited = setOf(100)
        withHub(world) { hub ->
            val add = ev("add", "address" to address, "network" to 100, "accept_risks" to false)
            val settled: (kotlinx.serialization.json.JsonElement) -> kotlinx.serialization.json.JsonElement? = { v ->
                val a = app.getvela.wallet.feature.multisig.core.MsAccountsView.of(v).add
                if (a.phase == MsPhase.Done && a.address.equals(address, ignoreCase = true)) a.outcome ?: JsonNull else null
            }
            val outcome = hub.accounts.request(add, 60_000, settled)!!
            assertEquals("busy", outcome.kind)
            assertEquals(100, outcome["chain_id"].int)
            assertTrue("Try again offered", app.getvela.wallet.feature.multisig.core.MsAccountsView.of(hub.accounts.view.value).add.retry)
            world.rateLimited = emptySet()
            assertEquals("added", hub.accounts.request(add, 60_000, settled).kind)
            assertFalse(app.getvela.wallet.feature.multisig.core.MsAccountsView.of(hub.accounts.view.value).add.retry)
        }
    }

    @Test
    fun `a rate-limited import is the core's busy with Try again, never unavailable`() {
        val world = world()
        val address = recorded(world, "0x62")
        val export = """{"version":"3.0","data":{"addedSafes":{"100":{"$address":{"owners":[{"value":"${world.address(0)}"}],"threshold":1}}},"undeployedSafes":{},"addressBook":{"100":{"$address":"Ops"}}}}"""
        withHub(world) { hub ->
            hub.send(ev("parse_export", "json" to export))
            hub.accounts.until(30_000) { v -> app.getvela.wallet.feature.multisig.core.MsAccountsView.of(v).importing.safes.takeIf { it.isNotEmpty() } }
            world.rateLimited = setOf(100)
            val key = address.lowercase()
            val import = ev("import", "address" to address, "accept_risks" to false)
            val settled: (kotlinx.serialization.json.JsonElement) -> kotlinx.serialization.json.JsonElement? = { v ->
                val i = app.getvela.wallet.feature.multisig.core.MsAccountsView.of(v).importing
                if (i.busy == null) i.outcomes[key] else null
            }
            val outcome = hub.accounts.request(import, 60_000, settled)!!
            assertEquals("busy", outcome.kind)
            val view = app.getvela.wallet.feature.multisig.core.MsAccountsView.of(hub.accounts.view.value).importing
            assertTrue("Try again offered", view.offersRetry(address))
            assertEquals(words.t("add.busy", "network" to "Gnosis"), importStatus(words, outcome))
            world.rateLimited = emptySet()
            assertEquals("imported", hub.accounts.request(import, 60_000, settled).kind)
            assertFalse(app.getvela.wallet.feature.multisig.core.MsAccountsView.of(hub.accounts.view.value).importing.offersRetry(address))
        }
    }

    // -- item 3: the fee signed is the fee shown — the core's rule, once -------------

    @Test
    fun `no shell copy of the fee rule is left`() {
        val main = File(repoRoot, "app-android/vela-wallet/app/src/main/java")
        val copies = main.walkTopDown().filter { it.isFile && it.extension == "kt" }
            .filter { f -> f.readText().let { "ShownFee" in it || "MultisigShellFailure.FeeMoved(" in it } }
            .map { it.relativeTo(main).path }.toList()
        assertEquals("the core alone refuses a fee other than the one shown (§19.11)", emptyList<String>(), copies)
        // The executor hands an execution to the send pipeline as the core asked it.
        val executor = File(main, "app/getvela/wallet/feature/multisig/core/MultisigExecutor.kt").readText()
        assertTrue(Regex("""is MultisigOperation\.SendCalls -> ports\.sendCalls\(op\)""").containsMatchIn(executor))
    }

    @Test
    fun `the core's fee_moved answer is said with the new fee and the executor's name`() {
        // The core's `fee_moved_detail` shape (multisig_proposals.rs).
        val ana = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val named = MultisigWords(
            strings = strings,
            localNames = mapOf(ana.lowercase() to "Ana"),
            networkName = { if (it == 100) "Gnosis" else null },
            nativeSymbol = { if (it == 100) "XDAI" else null },
        )
        val detail = """{"kind":"fee_moved","amount":"97200000000000000","chainId":100,"executor":"$ana"}"""
        assertEquals("That didn't go through: Network fee ≈ 0.0972 XDAI, paid by Ana", named.failureText(detail))
    }
}
