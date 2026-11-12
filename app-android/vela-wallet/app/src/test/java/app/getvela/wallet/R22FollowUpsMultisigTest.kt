package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.draftDoor
import app.getvela.wallet.feature.multisig.offersOwnNode
import app.getvela.wallet.feature.multisig.stateUnreadKey
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * §39 (Round 22 follow-ups) on Android's multisig, through the REAL
 * proposals machine (the host core over uniffi) and [MultisigWorld]:
 *
 * - r22-2: a network whose pool holds ONE provider (Arc with its default
 *   RPC) never locks the member out — the review says only one node could
 *   check (`review.stateOneNode`, never "checked") with "Add your own node",
 *   the door is "… anyway", and the press goes through.
 * - Two independent nodes that disagree sign nothing, said as the
 *   disagreement (`authority.signDisagreed`), never "can't reach".
 * - The gate's unchecked state is `authority.signUnchecked`, never
 *   "Can't reach {{network}}".
 */
class R22FollowUpsMultisigTest {

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
        accounts = MsAccountsView(),
        localNames = emptyMap(),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"

    private suspend fun <T : Any> poll(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T =
        withTimeoutOrNull(timeoutMs) {
            var found = pick()
            while (found == null) {
                delay(25)
                found = pick()
            }
            found
        } ?: error("never came: $what")

    /** A 2-of-3 deployed on Gnosis, members 0 and 1 here; [shape] sets how the network answers. */
    private fun withDraft(shape: (MultisigWorld) -> Unit, body: suspend (MultisigWorld, MultisigHub, List<String>) -> Unit) {
        val world = MultisigWorld { name -> File(code, "$name.hex").readText().trim() }
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x5f")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        shape(world)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val hub = MultisigHub(
                live = world,
                facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
                scope = scope,
            )
            try {
                runBlocking {
                    withTimeout(120_000) {
                        hub.boot()
                        hub.activate(safe)
                        poll("followed") { hub.proposalsView.value.followed }
                        val transfer = mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to bob, "amount" to "1000")))
                        hub.proposals.request(ev("draft", "chain_id" to 100, "proposal" to transfer)) { v -> v["draft"] as? JsonObject }
                        body(world, hub, owners)
                    }
                }
            } finally {
                hub.dispose()
            }
        } finally {
            scope.cancel()
        }
    }

    private suspend fun proposed(hub: MultisigHub, owner: String): JsonElement =
        // §41 (P2-5): pressed as the sheet presses it — "… anyway" when its door says so.
        hub.proposals.request<JsonElement>(ev("confirm_draft", "as_member" to owner, "anyway_confirmed" to hub.proposalsView.value.draft?.let { draftDoor(it, refuseBlind = false).anyway })) { v ->
            (v["actions"] as? JsonObject)?.get("propose")?.takeIf { it.kind != "working" }
        } ?: error("the press never settled")

    @Test
    fun `one provider - the review says only one node checked, offers the own node, and the press goes through anyway`() =
        withDraft({ it.soleProvider = true }) { world, hub, owners ->
            val draft = poll("the draft's review read") { hub.proposalsView.value.draft?.takeIf { it.reviewState.kind == "ready" } }
            val verdict = draft.review!!["state"]["verdict"]
            assertEquals("never \"checked\": $verdict", "unavailable", verdict.kind)
            assertEquals("oneNode", verdict["reason"].str)
            // Said as one node, never "couldn't check" — and with the way to add one's own.
            assertEquals("review.stateOneNode", stateUnreadKey(verdict))
            assertEquals(
                "Only one node could confirm who can act on Gnosis and what running it changes in the multisig. Add your own node in Settings to have it checked twice.",
                words.t(stateUnreadKey(verdict), "network" to "Gnosis"),
            )
            assertTrue(offersOwnNode(verdict))
            assertEquals("Add your own node", words.t("review.addOwnNode"))
            // The door: "… anyway", held.
            val door = draftDoor(draft, refuseBlind = false)
            assertTrue("… anyway: $door", door.anyway)
            assertTrue(door.holdMs > 0)
            // The press goes through, signed.
            val done = proposed(hub, owners[0])
            assertEquals("proposed: $done", "done", done.kind)
            assertEquals(1, world.passkeys.size)
        }

    @Test
    fun `two independent nodes that disagree sign nothing, said as the disagreement`() =
        withDraft({}) { world, hub, owners ->
            poll("the draft's review read") { hub.proposalsView.value.draft?.takeIf { it.reviewState.kind == "ready" } }
            world.disagreeing = true
            val refused = proposed(hub, owners[0])
            assertEquals("$refused", "signingDisagreed", refused.kind)
            assertTrue("nothing signed", world.passkeys.isEmpty())
            val said = words.actionFailure(refused, 100)
            assertEquals(
                "Two independent nodes on Gnosis gave different answers about who can act for this multisig, so Vela won't sign there. Add your own node in Settings to check with one you trust, or try again later.",
                said,
            )
            assertFalse("never can't reach: $said", said!!.contains("reach", ignoreCase = true))
        }

    @Test
    fun `the gate's unchecked state is its own words, never can't reach`() {
        val said = words.actionFailure(kotlinx.serialization.json.buildJsonObject { put("kind", kotlinx.serialization.json.JsonPrimitive("signingUnchecked")) }, 100)
        assertEquals(strings.t("home.multisig.authority.signUnchecked", mapOf("network" to "Gnosis")), said)
        assertFalse(said!!.contains("Can't reach"))
    }

    @Test
    fun `a single node without the sole-provider flag is uncorroborated, not one node`() {
        val verdict = kotlinx.serialization.json.Json.parseToJsonElement("""{"kind":"unavailable","reason":"uncorroborated"}""")
        assertEquals("review.stateUnconfirmed", stateUnreadKey(verdict))
        assertFalse(offersOwnNode(verdict))
    }
}
