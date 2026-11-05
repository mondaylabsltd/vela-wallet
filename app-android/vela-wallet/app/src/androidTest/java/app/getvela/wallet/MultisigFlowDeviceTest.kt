package app.getvela.wallet

import android.util.Log
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.ComposeUiTestFlags
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.multisig.MsLocal
import app.getvela.wallet.feature.multisig.MultisigHost
import app.getvela.wallet.feature.multisig.MultisigHostContext
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedView
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.vela_core_uniffi.ActivityFeedCore

/**
 * The multisig on a device, hermetic (scenarios `create`,
 * `propose-approve-execute`, `reject`, `member-change`, `outside-owner`,
 * `send-then-activity`): the REAL sheets ([MultisigHost] — the one the app
 * draws), over the REAL machines (the app's own core .so), over a simulated
 * world ([MultisigWorld]: the genesis service and its mailbox, Safe's code,
 * deployed Safes, passkeys signed by the parallel space's fixture keys).
 * Every press is a tap on the sheet the person sees; what landed is read
 * back from the world (the service, the mailbox, the relay) and from the
 * screen.
 *
 * No network, no harness, no funds: it runs wherever a debug build runs.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.MultisigFlowDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class MultisigFlowDeviceTest {

    /**
     * The sheets compose on the main thread, as they do in the app. The hub
     * works on `Dispatchers.IO` (the app's too) and the sheets collect its
     * flows in their composition's effect context. In the app that context is
     * the main thread's. Under the default test rule it is an UNCONFINED test
     * dispatcher: the collector resumed on the IO thread that set the flow,
     * and now and then the recomposition ran right there with it. A
     * `ModalBottomSheet` built off the main thread fails with "Can't create
     * handler inside thread … that has not called Looper.prepare()" (the reject
     * flow, once in three suite runs). A standard test dispatcher only queues
     * the resumption; the clock runs it on the main thread.
     */
    init {
        ComposeUiTestFlags.isStandardTestDispatcherSupportEnabled = true
    }

    @get:Rule
    val compose = createComposeRule(effectContext = StandardTestDispatcher())

    /** On a failure: where the world and the machines stood (logcat, tag ms-device). */
    @get:Rule
    val watcher = object : org.junit.rules.TestWatcher() {
        override fun failed(e: Throwable?, description: org.junit.runner.Description?) {
            val w = shownWorld ?: return
            Log.e(TAG, "FAILED ${description?.methodName}: mailbox=${w.mailbox.size} ${w.mailbox.map { it["safeTx"]["nonce"].str + ":" + it["approvals"].arr.size }} passkeys=${w.passkeys.map { it.first.take(8) }} sent=${w.sent.size} imports=${w.imports.size} userOp=${w.userOp}")
            Log.e(TAG, "http: ${w.httpLog.takeLast(15)}")
            for (h in hubs) {
                val v = h.proposalsView.value
                Log.e(TAG, "hub sheet=${h.sheet.value} draft=${v.draft != null} actions=${v.actions} executions=${v.executions} previews=${v.previews.keys}")
                Log.e(TAG, "items=${v.items.map { "${it.nonce}:${it.status}:${it.approved}/${it.threshold} ex=${it.canExecute} ap=${it.canApprove}" }}")
                Log.e(TAG, "members=${h.membersView.value}".take(3000))
            }
        }
    }
    private var shownWorld: MultisigWorld? = null

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val scopes = mutableListOf<CoroutineScope>()
    private val hubs = mutableListOf<MultisigHub>()

    @After
    fun stop() {
        hubs.forEach { it.dispose() }
        scopes.forEach { it.cancel() }
    }

    private val strings: I18nRuntime = instrumentation.targetContext.let { context ->
        I18nRuntime(
            readOnDemand = { namespace, tag -> runCatching { context.assets.open(I18nCatalogs.assetPath(tag, namespace)).use { it.readBytes() } }.getOrNull() },
        ) { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply {
            initialize("en")
            include(I18nCatalogs.MULTISIG)
        }
    }

    /** Safe's canonical code, from the test APK's assets (the core's vectors). */
    private fun world() = MultisigWorld { name ->
        instrumentation.context.assets.open("$name.hex").use { it.readBytes().toString(Charsets.UTF_8).trim() }
    }

    private fun scope() = CoroutineScope(SupervisorJob() + Dispatchers.IO).also { scopes += it }

    private fun hub(world: MultisigWorld, writeRows: suspend (List<org.json.JSONObject>) -> Boolean = { true }): MultisigHub =
        MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), NETWORKS, NETWORKS, emptyList()) },
            writeRows = writeRows,
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = scope(),
        ).also { hubs += it }

    private fun words(world: MultisigWorld) = MultisigWords(
        strings = strings,
        localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
        networkName = { mapOf(100 to "Gnosis", 8453 to "Base")[it] },
        nativeSymbol = { mapOf(100 to "XDAI", 8453 to "ETH")[it] },
    )

    /** The app's multisig host, drawn over [hub] — the very composable the root draws. */
    private fun show(hub: MultisigHub, world: MultisigWorld, below: (@androidx.compose.runtime.Composable () -> Unit)? = null) {
        val context = MultisigHostContext(
            words = words(world),
            accounts = world.locals.map { MsLocal(world.fixtures[it].address, world.fixtures[it].name) },
            contacts = emptyList(),
            spaceNetworks = listOf(100),
            otherSpaceName = "Testnets",
            origin = "https://wallet.getvela.app",
            locale = "en",
            tokenInfo = { _, _ -> "XDAI" to 18 },
            explorerTx = { _, _ -> null },
            onAddNetwork = {},
            onFund = {},
            askNotifications = {},
        )
        shownWorld = world
        val first = drawn.value == null
        drawn.value = Drawn(hub, context, below)
        if (!first) {
            compose.waitForIdle()
            return
        }
        compose.setContent {
            val now by drawn
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    now?.let { d ->
                        // Another device's hub is another composition: keyed, nothing carried over.
                        androidx.compose.runtime.key(d.hub) {
                            d.below?.invoke()
                            MultisigHost(d.hub, d.context)
                        }
                    }
                }
            }
        }
    }

    private class Drawn(val hub: MultisigHub, val context: MultisigHostContext, val below: (@androidx.compose.runtime.Composable () -> Unit)?)
    private val drawn = androidx.compose.runtime.mutableStateOf<Drawn?>(null)

    // -- driving the screen ---------------------------------------------------------------

    private fun waitFor(tag: String, timeoutMs: Long = 30_000) {
        compose.waitUntil(timeoutMs) { compose.onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
    }

    private fun node(tag: String): SemanticsNodeInteraction = compose.onAllNodesWithTag(tag)[0]

    private fun tap(tag: String, timeoutMs: Long = 30_000) {
        waitFor(tag, timeoutMs)
        // Enabled: the machine said it may be pressed.
        compose.waitUntil(timeoutMs) {
            compose.onAllNodesWithTag(tag).fetchSemanticsNodes().firstOrNull()
                ?.config?.contains(androidx.compose.ui.semantics.SemanticsProperties.Disabled) == false
        }
        runCatching { node(tag).performScrollTo() }
        node(tag).performClick()
        // This world answers neither of the review's safety checks (no
        // `debug_traceCall`, no probe, no simulation), so since r13-2 the core
        // lets a signature through only "… anyway", with its confirmation —
        // which the person would press, and so does the test.
        compose.waitForIdle()
        val anyway = compose.onAllNodesWithTag("multisig.sign.anywayConfirm")
        if (anyway.fetchSemanticsNodes().isNotEmpty()) anyway[0].performClick()
        compose.waitForIdle()
    }

    private fun type(tag: String, text: String) {
        waitFor(tag)
        val field = compose.onAllNodes(hasTestTag(tag) and androidx.compose.ui.test.hasSetTextAction(), useUnmergedTree = false)
            .fetchSemanticsNodes().let { if (it.isNotEmpty()) compose.onAllNodes(hasTestTag(tag) and androidx.compose.ui.test.hasSetTextAction())[0] else null }
            ?: compose.onAllNodes(androidx.compose.ui.test.hasAnyAncestor(hasTestTag(tag)) and androidx.compose.ui.test.hasSetTextAction())[0]
        runCatching { field.performScrollTo() }
        field.performTextInput(text)
        compose.waitForIdle()
    }

    /** Every text drawn in the node [tag] names, its own and its children's, in order. */
    private fun textOf(tag: String): String {
        fun texts(n: androidx.compose.ui.semantics.SemanticsNode): List<String> =
            n.config.getOrElse(androidx.compose.ui.semantics.SemanticsProperties.Text) { emptyList() }.map { it.text } +
                n.children.flatMap(::texts)
        return texts(compose.onAllNodesWithTag(tag, useUnmergedTree = true)[0].fetchSemanticsNode()).joinToString(" ")
    }

    private fun <T : Any> eventually(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            pick()?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("never: $what")
    }

    private fun booted(hub: MultisigHub, safe: String? = null) = runBlocking {
        withTimeout(30_000) { hub.boot() }
        if (safe != null) {
            hub.activate(safe)
            eventually("the multisig followed") { hub.proposalsView.value.followed?.takeIf { it.equals(safe, ignoreCase = true) } }
        }
    }

    /** A 2-of-3 of fixture accounts 0, 1, 2, kept here, used on Gnosis; deployed there when [deployed]. */
    private fun twoOfThree(world: MultisigWorld, deployed: Boolean, chains: List<Int> = listOf(100)): Pair<String, List<String>> {
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x31")
        val safe = g["address"].str!!
        if (deployed) for (c in chains) world.deploy(c, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, chains)}]"
        return safe to owners
    }

    /**
     * A transfer proposed the way the send screen proposes it
     * (`MultisigWiring.proposeFromSend` → `hub.propose`): its review sheet
     * comes up and the person presses Propose there. Answers its hash.
     */
    private fun proposeTransfer(hub: MultisigHub, safe: String, to: String, amount: String = "1000000000000000000"): String {
        var settled: JsonElement? = null
        val job = scope().launch {
            settled = hub.propose(100, mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to to, "amount" to amount))), address = safe)
        }
        waitFor("multisig.draft.title")
        tap("multisig.draft.propose")
        eventually("proposed", 60_000) { settled?.takeIf { job.isCompleted } }
        assertEquals("done", settled.kind)
        return (settled["safe_tx_hash"].str ?: settled["safeTxHash"].str)!!
    }

    // -- create -----------------------------------------------------------------------------

    @Test
    fun aMultisigIsCreatedFromItsSheetAndRecordedOnTheIndexChain() {
        val world = world()
        world.locals += 0
        val hub = hub(world)
        booted(hub)
        show(hub, world)
        hub.sheet.value = MultisigSheet.Create

        type("multisig.create.name", "Team")
        type("multisig.create.address", world.address(1))
        tap("multisig.create.addMember")
        tap("multisig.create.next")
        // The machine's sensible start for two members, on the one network offered.
        waitFor("multisig.create.threshold.value")
        waitFor("multisig.create.network.100")
        tap("multisig.create.toReview")
        waitFor("multisig.create.draftAddress")
        tap("multisig.create.create")
        waitFor("multisig.create.created", 60_000)
        waitFor("multisig.create.shared")

        // One passkey, the record sent to the index chain, kept here and shown.
        assertEquals("one passkey prompt", 1, world.passkeys.size)
        assertEquals(world.address(0).lowercase(), world.passkeys.single().first.lowercase())
        val import = world.imports.single()
        assertEquals(100L, import["chainId"].long)
        assertEquals("Team", import["genesis"]["name"].str)
        assertTrue(import["ownerProof"]["clientDataJSON"].str!!.startsWith("{\"type\":\"webauthn.get\""))
        val row = world.storedAccounts().single()
        assertEquals(true, row["recorded"]?.let { (it as? kotlinx.serialization.json.JsonPrimitive)?.content == "true" })
        val address = row["address"].str!!
        eventually("shown") { hub.accountsView.value.active?.takeIf { it.equals(address, ignoreCase = true) } }
        assertEquals(2, row["owners"].arr.size)

        tap("multisig.create.done")
        eventually("the sheet closed") { if (hub.sheet.value == null) true else null }
    }

    // -- propose, approve, execute; and the Activity row ------------------------------------

    @Test
    fun aTransferIsProposedApprovedAndExecutedFromItsSheetAndIsAnActivityRowAtOnce() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, owners) = twoOfThree(world, deployed = false)
        // Member 1's own fee look fails: its approval is an approval, never the one press —
        // this walks propose → approve → execute as three presses.
        world.quoteRefusedFor += owners[1].lowercase()

        // The Activity: the real feed machine over the real store the hub writes executed rows to.
        val store = MemoryStore()
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val feedHost = CoreHost(
            bridge = ActivityFeedCore().asBridge(),
            scope = scope(),
            initial = FeedView(),
            serializer = FeedView.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), feed::perform),
            escapedFailure = JsonShell.escapedFailure(FeedOperation.serializer(), FeedShellResult.serializer(), fallback = FeedShellResult.HapticPlayed, answer = feed::neutralAnswer),
        ).also { it.start() }
        val hub = hub(world, writeRows = { rows -> feed.writeRecords(rows) })
        hub.onRecorded = { feedHost.dispatch(FeedEvent.ReconcileCompleted(resolved_count = 1), FeedEvent.serializer()) }
        booted(hub, safe)
        feedHost.dispatch(FeedEvent.AccountSwitched(safe), FeedEvent.serializer())
        show(hub, world) {
            val view by feedHost.view.collectAsState()
            Column {
                for (group in WalletLive.activity(view, strings)) for (row in group.rows) ActivityRow(row)
            }
        }

        val recipient = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
        val hash = proposeTransfer(hub, safe, recipient)
        assertEquals("one passkey to propose", 1, world.passkeys.size)
        assertEquals(1, world.mailbox.size)

        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.title")
        // The second member here approves: one passkey, two of two.
        tap("multisig.proposal.approve", 60_000)
        eventually("approved") { world.mailbox.single()["approvals"].arr.size.takeIf { it == 2 } }
        assertEquals(owners[1].lowercase(), world.passkeys[1].first.lowercase())

        // Ready: Execute, as the first member (its fee look priced).
        world.onSend = MultisigWorld.Landing(
            chain = 100, safe = safe, after = MultisigWorld.Safe(owners, 2, nonce = 1),
            safeTxHash = hash, txHash = "0x" + "cd".repeat(32), userOpHash = "0x" + "ab".repeat(32),
        )
        tap("multisig.proposal.execute", 90_000)
        // "Executed": its own line, the sheet's executed note, or — said once
        // (r12 S4) — the heading's chip (whichever the sheet says it with).
        val executedWord = strings.t("home.multisig.proposal.executed")
        compose.waitUntil(90_000) {
            listOf("multisig.proposal.executed", "multisig.proposal.note.executed")
                .any { compose.onAllNodesWithTag(it, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() } ||
                compose.onAllNodes(hasTestTag("multisig.proposal.status") and (hasText(executedWord) or hasAnyDescendant(hasText(executedWord))), useUnmergedTree = true)
                    .fetchSemanticsNodes().isNotEmpty()
        }
        assertEquals("one user operation: the deployment, then execTransaction", 1, world.sent.size)
        assertTrue(world.sent.single().third.size >= 2)

        // At once: the Activity holds the transfer, confirmed, read by the feed machine and drawn.
        val record = eventually("the Activity row") {
            runBlocking { (feed.perform(FeedOperation.ReadTxStore(safe, 1)) as? FeedShellResult.StoreLoaded)?.records?.singleOrNull() }
        }
        assertEquals("multisig:100:${hash.lowercase()}:0", record.id.lowercase())
        assertEquals(app.getvela.wallet.feature.wallet.core.FeedTxKind.Send, record.kind)
        assertEquals(app.getvela.wallet.feature.wallet.core.FeedTxStatus.Confirmed, record.status)
        // Drawn as Activity draws it: a "Sent" row of 1 XDAI, under the sheet.
        val sent = strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.LABEL_SENT)
        compose.waitUntil(30_000) {
            compose.onAllNodes(hasText(sent), useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
        }
        val rows = WalletLive.activity(feedHost.view.value, strings).flatMap { it.rows }
        assertEquals("one row for the one transfer", 1, rows.size)
        assertEquals(sent, rows.single().title)
        assertTrue("1 XDAI: ${rows.single().amount} ${rows.single().unit}", rows.single().amount.contains("1") && rows.single().unit == "XDAI")
        Log.i(TAG, "executed $hash; activity ${record.id}")
    }

    // -- reject --------------------------------------------------------------------------------

    @Test
    fun anotherMembersProposalIsRejectedWithASameNumberCancellation() {
        val world = world()
        val (safe, owners) = twoOfThree(world, deployed = true)
        // Member 1 proposes on its own device.
        world.locals += 1
        val theirs = hub(world)
        booted(theirs, safe)
        show(theirs, world)
        val hash = proposeTransfer(theirs, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        theirs.dispose()
        hubs -= theirs

        // This device: member 0 (its own store: only the multisig is kept).
        world.locals.clear()
        world.locals += 0
        world.store.keys.removeAll { it != "vela.multisigAccounts" }
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.title")
        // Another member's proposal: Reject (not withdraw), confirmed, then its review, then one passkey.
        tap("multisig.proposal.reject", 60_000)
        tap("multisig.proposal.rejectConfirm")
        waitFor("multisig.draft.title")
        tap("multisig.draft.propose")
        eventually("the cancellation in the mailbox", 60_000) { world.mailbox.takeIf { it.size == 2 } }

        val cancel = world.mailbox.first { !it["safeTxHash"].str.equals(hash, ignoreCase = true) }
        val tx = cancel["safeTx"]
        assertTrue("to the multisig itself", tx["to"].str.equals(safe, ignoreCase = true))
        assertEquals("no value", "0", tx["value"].str?.removePrefix("0x")?.ifEmpty { "0" }?.trimStart('0')?.ifEmpty { "0" })
        assertEquals("no call", "0x", tx["data"].str)
        assertEquals("the same number", world.mailbox.first { it["safeTxHash"].str.equals(hash, ignoreCase = true) }["safeTx"]["nonce"].str, tx["nonce"].str)
        assertEquals("approved by the member who rejected", owners[0].lowercase(), cancel["approvals"].arr.single()["owner"].str!!.lowercase())
        assertEquals(owners[0].lowercase(), world.passkeys.last().first.lowercase())
        // The proposal knows it: the cancellation waiting at its number, one tap away.
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.openCancellation", 60_000)
    }

    // -- member change -------------------------------------------------------------------------

    @Test
    fun aMemberRemovedInSettingsIsProposedOnEveryDeployedNetwork() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, owners) = twoOfThree(world, deployed = true, chains = listOf(100, 8453))
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        hub.sheet.value = MultisigSheet.Members(safe)

        compose.waitUntil(30_000) { compose.onAllNodesWithTag("multisig.members.menu").fetchSemanticsNodes().size == 3 }
        val carol = owners[2]
        compose.onAllNodes(hasTestTag("multisig.members.menu") and hasText(MultisigWords.short(carol), substring = true))[0]
            .also { runCatching { it.performScrollTo() } }.performClick()
        tap("multisig.members.remove")
        waitFor("multisig.members.plan.100")
        waitFor("multisig.members.plan.8453")
        assertTrue("2 of 3 → 2 of 2", textOf("multisig.members.plan.100").contains("2 of 2"))
        tap("multisig.members.propose")
        // Each deployed network's change, its review, one passkey each.
        for (chain in listOf(100, 8453)) {
            waitFor("multisig.draft.title", 60_000)
            tap("multisig.draft.propose")
            eventually("proposed on $chain", 60_000) { world.mailbox.firstOrNull { it["chainId"].long == chain.toLong() } }
        }
        assertEquals(2, world.mailbox.size)
        for (p in world.mailbox) {
            val data = p["safeTx"]["data"].str!!.lowercase()
            assertTrue("removeOwner on ${p["chainId"]}: $data", data.startsWith("0xf8dc5dd9") && data.contains(carol.removePrefix("0x").lowercase()))
            assertTrue(p["safeTx"]["to"].str.equals(safe, ignoreCase = true))
        }
        assertEquals("one passkey per network", 2, world.passkeys.size)
    }

    /**
     * r13-5 (r12-8): an executed member change's Activity row is said in the
     * language in front when it is DRAWN — the language switched after the
     * row exists, the row follows (zh Activity said "Add 0x78e6AE…44f0E3").
     */
    @Test
    fun anExecutedMemberChangesActivityRowFollowsALanguageSwitchedAfterIt() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, owners) = twoOfThree(world, deployed = true)
        world.quoteRefusedFor += owners[1].lowercase()
        val store = MemoryStore()
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val feedHost = CoreHost(
            bridge = ActivityFeedCore().asBridge(),
            scope = scope(),
            initial = FeedView(),
            serializer = FeedView.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), feed::perform),
            escapedFailure = JsonShell.escapedFailure(FeedOperation.serializer(), FeedShellResult.serializer(), fallback = FeedShellResult.HapticPlayed, answer = feed::neutralAnswer),
        ).also { it.start() }
        val hub = hub(world, writeRows = { rows -> feed.writeRecords(rows) })
        hub.onRecorded = { feedHost.dispatch(FeedEvent.ReconcileCompleted(resolved_count = 1), FeedEvent.serializer()) }
        booted(hub, safe)
        feedHost.dispatch(FeedEvent.AccountSwitched(safe), FeedEvent.serializer())
        show(hub, world) {
            val view by feedHost.view.collectAsState()
            Column {
                for (group in WalletLive.activity(view, strings)) for (row in group.rows) ActivityRow(row)
            }
        }
        // What the app does whenever the language or the names move (MultisigWiring).
        val reword: () -> Int = {
            val w = MultisigWords(strings, hub.accountsView.value, world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name })
            runBlocking { feed.rewordRecords { row -> app.getvela.wallet.feature.multisig.MemberRowTitles.reword(row) { w.memberChangeTitle(it) } } }
                .also { if (it > 0) feedHost.dispatch(FeedEvent.ReconcileCompleted(resolved_count = it), FeedEvent.serializer()) }
        }

        // Remove member 2: proposed, approved by member 1, executed by member 0.
        hub.sheet.value = MultisigSheet.Members(safe)
        compose.waitUntil(30_000) { compose.onAllNodesWithTag("multisig.members.menu").fetchSemanticsNodes().size == 3 }
        val carol = owners[2]
        compose.onAllNodes(hasTestTag("multisig.members.menu") and hasText(MultisigWords.short(carol), substring = true))[0]
            .also { runCatching { it.performScrollTo() } }.performClick()
        tap("multisig.members.remove")
        tap("multisig.members.propose")
        waitFor("multisig.draft.title", 60_000)
        tap("multisig.draft.propose")
        val hash = eventually("proposed", 60_000) { world.mailbox.singleOrNull()?.get("safeTxHash")?.str }
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.title")
        tap("multisig.proposal.approve", 60_000)
        eventually("approved") { world.mailbox.single()["approvals"].arr.size.takeIf { it == 2 } }
        world.onSend = MultisigWorld.Landing(
            chain = 100, safe = safe, after = MultisigWorld.Safe(owners.take(2), 2, nonce = 1),
            safeTxHash = hash, txHash = "0x" + "ce".repeat(32), userOpHash = "0x" + "ac".repeat(32),
        )
        tap("multisig.proposal.execute", 90_000)
        val stored = eventually("the member change's row, with its action", 90_000) {
            runBlocking { store.read(KeyValueStore.Keys.TRANSACTIONS) }
                ?.let { org.json.JSONArray(it) }?.takeIf { it.length() == 1 }?.getJSONObject(0)?.takeIf { it.has("multisigChange") }
        }
        assertEquals("removeOwner", stored.getJSONObject("multisigChange").getString("kind"))
        hub.sheet.value = null

        // Worded in English, and drawn so.
        reword()
        val action = Wire.json.parseToJsonElement(stored.getJSONObject("multisigChange").toString())
        val en = MultisigWords(strings, hub.accountsView.value, world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name }).memberChangeTitle(action)!!
        assertTrue(en, en.startsWith("Remove "))
        compose.waitUntil(30_000) { compose.onAllNodes(hasText(en), useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }

        // The language switches AFTER the row exists: the row is drawn in it.
        strings.setLocale("zh")
        try {
            assertEquals(1, reword())
            val zh = MultisigWords(strings, hub.accountsView.value, world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name }).memberChangeTitle(action)!!
            assertTrue(zh, zh.startsWith("移除 "))
            compose.waitUntil(30_000) { compose.onAllNodes(hasText(zh), useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
            assertTrue("no English title left", compose.onAllNodes(hasText(en), useUnmergedTree = true).fetchSemanticsNodes().isEmpty())
            Log.i(TAG, "r13-5: '$en' -> '$zh'")
        } finally {
            strings.setLocale("en")
        }
    }

    /**
     * r13-6: a proposal link handed over while the boot still waits for the
     * app's networks is read with them once it is done — never "a network
     * this device doesn't have", and the multisig is listed.
     */
    @Test
    fun aLinkDeliveredBeforeTheBootCompletesIsReadWithTheNetworks() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, _) = twoOfThree(world, deployed = true)
        val networksRead = kotlinx.coroutines.CompletableDeferred<Unit>()
        val hub = MultisigHub(
            live = world,
            facts = {
                networksRead.await()
                MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), NETWORKS, NETWORKS, emptyList())
            },
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = scope(),
        ).also { hubs += it }
        val boot = scope().launch { hub.boot() }
        show(hub, world)
        // The link, as MultisigAppLayer hands it over: boot, open, the sheet.
        val link = scope().launch {
            hub.boot()
            hub.openLink("#ms=100.$safe.0x" + "ef".repeat(32))
            hub.sheet.value = MultisigSheet.Link
        }
        Thread.sleep(800)
        assertTrue("the link waits for the boot", !link.isCompleted)
        networksRead.complete(Unit)
        runBlocking { withTimeout(30_000) { boot.join(); link.join() } }
        // A member here: its transaction opens (the router's `open_proposal`);
        // else the link's own sheet says where it stands.
        compose.waitUntil(30_000) {
            listOf("multisig.link", "multisig.proposal").any {
                compose.onAllNodesWithTag(it, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
            }
        }
        Thread.sleep(2_000)
        compose.waitForIdle()
        assertTrue(
            "never 'a network this device doesn't have'",
            compose.onAllNodesWithTag("multisig.link.missingNetwork", useUnmergedTree = true).fetchSemanticsNodes().isEmpty(),
        )
        assertTrue("the link's stand: ${hub.accountsView.value.link?.kind}", hub.accountsView.value.link?.kind != "missingNetwork")
        assertEquals(safe.lowercase(), hub.accountsView.value.accounts.single().account.address.lowercase())
    }

    // -- an outside member ----------------------------------------------------------------------

    @Test
    fun anOutsideMemberIsShownTheConnectLinkForTheTransaction() {
        val world = world()
        world.locals += 0
        val outside = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
        val owners = listOf(world.address(0), outside)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x32")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        val row = jo(*world.storedRow(g, listOf(100)).entries.map { it.key to it.value }.toTypedArray(), "members" to mapOf(outside.lowercase() to mapOf("name" to "Dana", "kind" to "wallet")))
        world.store["vela.multisigAccounts"] = "[$row]"
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)

        val hash = proposeTransfer(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.outsideLink", 60_000)
        val link = textOf("multisig.proposal.outsideLink")
        assertTrue(link, link.startsWith("https://wallet.getvela.app/en/connect#"))
        assertTrue("names the outside member: $link", link.lowercase().contains(outside.lowercase()))
        assertTrue("names the transaction: $link", link.lowercase().contains(hash.lowercase()))
        waitFor("multisig.proposal.outsideQr")
    }

    /** An in-memory store for the Activity (the device's own is the app's, not a test's). */
    private class MemoryStore : KeyValueStore {
        val values = ConcurrentHashMap<String, String>()
        override suspend fun read(key: String): String? = values[key]
        override suspend fun write(key: String, value: String): Boolean { values[key] = value; return true }
        override suspend fun remove(vararg keys: String): Boolean { keys.forEach { values.remove(it) }; return true }
    }

    private companion object {
        const val TAG = "ms-device"
        val NETWORKS = listOf(100, 8453)
    }
}
