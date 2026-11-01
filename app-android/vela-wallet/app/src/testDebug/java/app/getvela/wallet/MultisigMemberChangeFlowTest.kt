package app.getvela.wallet

import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.obj
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A member change as a FLOW (scenario `member-change`), through the real
 * machines over the host dylib: the hub boots on a simulated world
 * ([MultisigWorld], the core's own world in Kotlin), Settings' members sheet
 * is shown, a removal / a new rule is composed, the members machine plans one
 * proposal per deployed network (none where it is not), the proposals machine
 * draws its review, the person confirms it, ONE passkey signs, and the
 * mailbox holds the Safe's own `removeOwner` / `changeThreshold` call at the
 * multisig's next number. The removal is then approved by the second member
 * here, executed, and Settings reads the members the chain now holds.
 *
 * The same verbs the members sheet sends (`MultisigMembersSheet`): compose,
 * step, proposing, then `hub.propose` for each network's proposal.
 */
class MultisigMemberChangeFlowTest {

    private val scopes = mutableListOf<CoroutineScope>()
    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))

    @After
    fun stop() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun world() = MultisigWorld { name ->
        File(repoRoot, "rust/crates/vela-core/tests/vectors/safe-code/$name.hex").readText().trim()
    }

    private fun hub(world: MultisigWorld, networks: List<Int> = listOf(100, 8453)): MultisigHub {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        return MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), networks, networks, emptyList()) },
            scope = scope,
        )
    }

    /** The first non-null [pick], polled — the hub's typed views land after the machine's commit. */
    private suspend fun <T : Any> poll(timeoutMs: Long = 30_000, what: String, pick: () -> T?): T =
        withTimeoutOrNull(timeoutMs) {
            var found = pick()
            while (found == null) {
                delay(25)
                found = pick()
            }
            found
        } ?: error("never came: $what")

    private class Setup(val world: MultisigWorld, val hub: MultisigHub, val safe: String, val owners: List<String>)

    /** A 2-of-3 of the three fixture accounts, deployed on Gnosis only (used on Gnosis and Base); [here] on this device. */
    private suspend fun setup(here: List<Int>): Setup {
        val world = world()
        world.locals += here
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x31")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100, 8453))}]"
        val hub = hub(world)
        withTimeout(30_000) { hub.boot() }
        hub.activate(safe)
        withTimeout(20_000) { while (hub.proposalsView.value.followed?.equals(safe, ignoreCase = true) != true) delay(20) }
        return Setup(world, hub, safe, owners)
    }

    /** What the members sheet shows, once the machine read the chain. */
    private suspend fun shown(s: Setup): JsonObject {
        s.hub.showMembers(s.safe)
        return withTimeout(30_000) { s.hub.members.until { v -> v.takeIf { it["rows"].arr.isNotEmpty() } }!! }
    }

    /**
     * The members sheet's Propose: each network's proposal through
     * `hub.propose`, its review confirmed as the person does on the review
     * sheet (`multisig.draft.propose`) — [MultisigHub.confirmDraft].
     */
    private suspend fun proposeEach(s: Setup, plan: JsonElement?): Map<Int, JsonElement?> {
        val out = linkedMapOf<Int, JsonElement?>()
        s.hub.sendMembers(ev("proposing"))
        for (network in plan["networks"].arr.filter { it["proposal"] != null }) {
            val chainId = network["chain_id"].int!!
            val settled = CoroutineScope(Dispatchers.Default).async {
                s.hub.propose(chainId, network["proposal"].obj.mapValues { it.value }, address = s.safe)
            }
            // The review sheet is up, with the draft the machine drew: the person proposes.
            withTimeout(30_000) { while (s.hub.sheet.value != MultisigSheet.Review(chainId)) delay(20) }
            assertNotNull("the review is drawn before anything is signed", s.hub.proposalsView.value.draft)
            assertTrue("no passkey before the review is confirmed", s.world.passkeys.isEmpty() || out.isNotEmpty())
            s.hub.confirmDraft(null)
            out[chainId] = withTimeout(60_000) { settled.await() }
        }
        return out
    }

    @Test
    fun a_member_removed_in_settings_is_proposed_approved_executed_and_read_back() = runBlocking {
        val s = setup(here = listOf(0, 1))
        val carol = s.owners[2]
        val members = shown(s)
        assertEquals("the three members, read on the deployed network", 3, members["rows"].arr.size)

        s.hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "remove", "owner" to carol)))
        val composed = withTimeout(30_000) {
            s.hub.members.until { v -> v.takeIf { it["plan"]["networks"].arr.isNotEmpty() && it["can_propose"].bool == true } }!!
        }
        val plan = composed["plan"]
        // One proposal where it is deployed; Base, where it is not, is said and proposes nothing.
        val gnosis = plan["networks"].arr.single { it["chain_id"].int == 100 }
        assertEquals("2 of 3 before", 3, gnosis["verdict"]["before"]["owners"].arr.size)
        assertEquals("2 of 2 after", 2, gnosis["verdict"]["after"]["owners"].arr.size)
        assertEquals(2, gnosis["verdict"]["after"]["threshold"].int)
        assertEquals("member_change", gnosis["proposal"].kind)
        assertTrue("Base is not deployed: nothing proposed there", plan["networks"].arr.none { it["chain_id"].int == 8453 && it["proposal"] != null })

        val settled = proposeEach(s, plan)
        assertEquals(setOf(100), settled.keys)
        val done = settled.getValue(100)
        assertEquals("done", done.kind)
        val hash = done["safe_tx_hash"].str ?: done["safeTxHash"].str
        assertNotNull(hash)
        assertEquals("one passkey for the proposal", 1, s.world.passkeys.size)

        // The mailbox holds the Safe's own removeOwner(prev, carol, 2) on itself, at its next number.
        val stored = s.world.mailbox.single()
        assertEquals(100L, stored["chainId"].int?.toLong())
        val tx = stored["safeTx"]
        assertTrue("to the multisig itself", tx["to"].str.equals(s.safe, ignoreCase = true))
        val data = tx["data"].str!!.lowercase()
        assertTrue("removeOwner(address,address,uint256): $data", data.startsWith("0xf8dc5dd9"))
        assertTrue("names the member removed", data.contains(carol.removePrefix("0x").lowercase()))
        assertEquals("0", tx["nonce"].str?.let { if (it.startsWith("0x")) it.removePrefix("0x").toLong(16).toString() else it })

        // The proposals machine reads it as a member change, one approval of two — its review asked as the sheet asks it.
        s.hub.sendProposal(ev("review", "chain_id" to 100, "safe_tx_hash" to hash, "blind" to false))
        val item = poll(what = "the proposal reviewed") {
            s.hub.proposalsView.value.item(100, hash!!)?.takeIf { it.approved == 1 && it.action != null }
        }
        assertEquals("removeOwner", item.action.kind)
        assertEquals(listOf(s.owners[1].lowercase()), item.canApprove.map { it.lowercase() })

        // The second member here approves: ready.
        s.hub.sendProposal(ev("approve", "chain_id" to 100, "safe_tx_hash" to hash, "owner" to s.owners[1]))
        val ready = poll(what = "ready") { s.hub.proposalsView.value.item(100, hash!!)?.takeIf { it.status == "ready" } }
        assertEquals(2, ready.approved)

        // Executed: the chain then holds the new members, and Settings reads them.
        s.world.onSend = MultisigWorld.Landing(
            chain = 100, safe = s.safe, after = MultisigWorld.Safe(s.owners.take(2), 2, nonce = 1),
            safeTxHash = hash!!, txHash = "0x" + "cd".repeat(32), userOpHash = "0x" + "ab".repeat(32),
        )
        s.hub.sendProposal(ev("execute", "chain_id" to 100, "safe_tx_hash" to hash, "executor" to s.owners[0]))
        val verdict = poll(60_000, "executed") {
            s.hub.proposalsView.value.let { v -> v.lookup(v.executions, 100, hash)?.takeIf { it.kind != "working" } }
        }
        assertEquals("executed", verdict.kind)
        assertEquals("one user operation", 1, s.world.sent.size)
        s.hub.sendMembers(ev("compose", "compose" to null))
        s.hub.showMembers(s.safe)
        val after = withTimeout(30_000) { s.hub.members.until { v -> v.takeIf { it["rows"].arr.size == 2 } } }
        assertNotNull("Settings reads the two members the chain now holds", after)
        assertTrue(after!!["rows"].arr.none { it["address"].str.equals(carol, ignoreCase = true) })
    }

    @Test
    fun the_approvals_needed_changed_in_settings_is_one_change_threshold_proposal() = runBlocking {
        val s = setup(here = listOf(0))
        shown(s)
        s.hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "threshold")))
        // The stepper starts at the rule in force: 2 of 3. One fewer: 1 of 3.
        val stepper = withTimeout(30_000) { s.hub.members.until { v -> v["stepper"]?.takeIf { it["value"].int != null } }!! }
        assertEquals(2, stepper["value"].int)
        s.hub.sendMembers(ev("step", "by" to -1))
        val composed = withTimeout(30_000) {
            s.hub.members.until { v -> v.takeIf { it["stepper"]["value"].int == 1 && it["can_propose"].bool == true } }!!
        }
        val gnosis = composed["plan"]["networks"].arr.single { it["chain_id"].int == 100 }
        assertEquals(2, gnosis["verdict"]["before"]["threshold"].int)
        assertEquals(1, gnosis["verdict"]["after"]["threshold"].int)
        assertEquals("the members stay", 3, gnosis["verdict"]["after"]["owners"].arr.size)

        val settled = proposeEach(s, composed["plan"])
        assertEquals("done", settled.getValue(100).kind)
        val data = s.world.mailbox.single()["safeTx"]["data"].str!!.lowercase()
        assertTrue("changeThreshold(1): $data", data.startsWith("0x694e80c3") && data.endsWith("1".padStart(64, '0')))
        assertEquals(1, s.world.passkeys.size)
        assertEquals(s.owners[0].lowercase(), s.world.passkeys.single().first.lowercase())
    }
}
