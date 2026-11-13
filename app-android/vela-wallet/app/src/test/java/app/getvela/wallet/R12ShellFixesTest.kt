package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.format.ShortAddress
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.deltaText
import app.getvela.wallet.feature.multisig.readingAddresses
import app.getvela.wallet.feature.multisig.readingLines
import app.getvela.wallet.feature.multisig.recipientsLeft
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread
import kotlinx.serialization.json.JsonElement
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 12's Android shell fixes, read through the builders the screens
 * draw from — the core's views in (as JSON), what the app says out — and the
 * transport the multisig service and the relay are read through.
 */
class R12ShellFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val carol = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"
    private val safe = "0x726726d55Aa06DB856d4E6a9e65e8773829BC302"
    private val usdc = "0x3600000000000000000000000000000000000000"
    private val funder = "0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D"
    private val hash = "0x" + "ab".repeat(32)

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private fun words(tokens: Map<String, JsonElement> = emptyMap()) = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Alice"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
        tokenFacts = { id, token -> tokens["$id:${token.lowercase()}"] },
    )

    // -- r12-1: a shortfall is said in the token's own decimals, never 18 -----------------

    /**
     * r12-1 (round 12): MS-B #1 sent 0.01 USDC through Arc's ERC-20 interface;
     * the "would fail" line read "the multisig holds 0, and this needs
     * <0.000001" — the shortfall named the token by address alone and the
     * shell assumed 18 decimals. Now: the core's facts when it gives them, the
     * review's token facts else, and base units, "decimals unknown", when
     * nobody said them.
     */
    @Test
    fun a_token_shortfall_never_assumes_18_decimals() {
        val why = json("""{"kind":"short","token":"$usdc","have":"0","need":"10000"}""")
        // Nobody said its decimals: the base units, and that they are unknown.
        val unknown = words().whyFails(why, 5042002)
        assertTrue(unknown, unknown.contains("10000 base units · decimals unknown"))
        assertFalse(unknown, unknown.contains("<0.000001"))
        assertFalse(unknown, unknown.contains("0.00000000000001"))
        // The core read the token for the review: its decimals and symbol.
        val known = words(mapOf("5042002:$usdc" to json("""{"token":"$usdc","symbol":"USDC","decimals":6,"known":true}""")))
        assertEquals("the multisig holds 0 USDC, and this needs 0.01 USDC", known.whyFails(why, 5042002))
        // The core's own payload (`OutsideTokenFacts` on the shortfall) wins.
        val payload = json("""{"kind":"short","token":{"token":"$usdc","symbol":"USDC","decimals":6,"known":true},"have":"0","need":"1000000000000"}""")
        assertTrue(words().whyFails(payload, 5042002).endsWith("this needs 1,000,000 USDC"))
        val payloadUnknown = json("""{"kind":"short","token":{"token":"$usdc","symbol":"USDC","decimals":null,"known":false},"have":"0","need":"5"}""")
        assertTrue(words().whyFails(payloadUnknown, 5042002).contains("5 base units · decimals unknown USDC"))
        // The core's amounts (§13 `MultisigAmount`): drawn as it says them, nothing worded here.
        val said = json(
            """{"kind":"short","token":"$usdc",
                "have":{"base_units":"0","token":"$usdc","said":{"kind":"known","text":"0 USDC","number":"0","symbol":"USDC","decimals":6}},
                "need":{"base_units":"10000","token":"$usdc","said":{"kind":"decimals_unknown","key":"home.multisig.review.reading.decimalsUnknown","raw":"10000"}}}""",
        )
        assertEquals("the multisig holds 0 USDC, and this needs 10000 base units · decimals unknown", words().whyFails(said, 5042002))
        // The network's own coin is counted in wei.
        val coin = json("""{"kind":"short","token":null,"have":"0","need":"500000000000000000"}""")
        assertEquals("the multisig holds 0 USDC, and this needs 0.5 USDC", words().whyFails(coin, 5042002))
    }

    @Test
    fun a_balance_change_of_a_token_not_read_yet_is_its_base_units() {
        val delta = json("""{"token":"$usdc","delta":"-10000"}""")
        val said = deltaText(delta, 5042002, words(), emptyMap())
        assertTrue(said, said.contains("10000 base units · decimals unknown"))
        assertEquals("−0.01 USDC", deltaText(delta, 5042002, words(), mapOf(usdc to ("USDC" to 6))))
    }

    @Test
    fun no_multisig_source_assumes_18_decimals_for_a_token() {
        val dir = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig")
        val offenders = dir.walkTopDown().filter { it.isFile && it.extension == "kt" }.flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                val code = line.substringBefore("//")
                if (Regex("""\?: ?18\b|to 18\b|, 18\)|1e18|pow\(18\)""").containsMatchIn(code)) "${file.name}:${i + 1}: ${line.trim()}" else null
            }
        }.toList()
        assertEquals("18 assumed for an amount: $offenders", emptyList<String>(), offenders)
    }

    // -- r12-4: a refused transaction asks nobody --------------------------------------

    private fun proposals(
        status: String = "pending",
        approvers: List<String> = listOf(bob),
        reviews: String = "{}",
        canApprove: List<String> = listOf(alice),
        refusal: String? = null,
        extra: String = "",
    ): MsProposalsView = MsProposalsView.of(
        json(
            """
            {"followed":"$safe","standings":{},"reviews":$reviews $extra,
             "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"6",
              "proposer":"$bob","approvals":[${approvers.joinToString(",") { "{\"owner\":\"$it\"}" }}],"approved":${approvers.size},"threshold":2,"status":"$status",
              "canApprove":[${canApprove.joinToString(",") { "\"$it\"" }}],"canExecute":[],"cancelledBy":null,"sameNonce":[],
              "executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,
              "review":{"action":{"kind":"batch","calls":[]},"refusal":${refusal?.let { "\"$it\"" } ?: "null"}}}],
             "chains":[{"chainId":5042002,"members":{"deployed":true,"owners":["$alice","$bob","$carol"],"threshold":2,"nonce":"0"},"items":[],"mailbox":true}]}
            """.trimIndent(),
        ),
    )

    private fun model(view: MsProposalsView, w: MultisigWords = words()) = MsProposalModel(view.items.single(), view, w, null)

    /**
     * r12-4 (round 12): deep4 (#6) and a delegatecall (#7) said "Vela won't
     * sign this transaction" and still offered "Ask members to approve", Send
     * link, Copy link and the outside-wallet link. The sheet read `refused`
     * on the item's thin review, which never carries it; it is the ONE
     * review's (`reviews[…]`).
     */
    @Test
    fun a_transaction_the_core_refuses_asks_nobody_to_approve_it() {
        val open = model(proposals())
        assertTrue("an open one asks the members still to approve", open.asksMembers)

        val refusedReview = """{"5042002:$hash":{"refused":{"why":"tooDeep","key":"home.multisig.review.refusedWhy.tooDeep"},"sign":{"allowed":false}}}"""
        val deep = model(proposals(reviews = refusedReview, canApprove = emptyList()))
        assertTrue(deep.nobodySigns)
        assertFalse("no Ask, no Send link, no Copy link, no outside link", deep.asksMembers)

        // A sign rule shut alone says the same.
        val shut = model(proposals(reviews = """{"5042002:$hash":{"sign":{"allowed":false}}}"""))
        assertFalse(shut.asksMembers)

        // Before the review lands: the core left nobody who can act, and said why.
        val delegate = model(proposals(canApprove = emptyList(), refusal = "a delegatecall"))
        assertFalse(delegate.asksMembers)

        // Done, or enough approvals: nobody asked either.
        assertFalse(model(proposals(approvers = listOf(bob, carol))).asksMembers)
    }

    // -- r12 S4: said once ---------------------------------------------------------------

    /** r12 S4: the executed sheet's chip says "Executed"; the green box under it said it again. */
    @Test
    fun an_executed_sheet_says_executed_once() {
        val view = proposals(
            status = "done",
            approvers = listOf(bob, alice),
            canApprove = emptyList(),
            extra = ""","executions":{"5042002:$hash":{"kind":"executed","txHash":"0xtx"}},"timelines":{"5042002:$hash":[{"kind":"executed","tx_hash":"0xtx"}]}""",
        )
        val m = model(view)
        val executed = strings.t("home.multisig.proposal.executed")
        assertEquals(executed, m.statusLine)
        assertEquals("the chip says it; no note again", 0, m.sheetNotes.count { it.text == executed })
        assertFalse("nor the sheet's own line", m.saysExecuted)
    }

    /** r12 S4: "To 0x3e9b…" in the reading, then "To 0x3e9b…" again in the recipients. */
    @Test
    fun an_address_the_reading_names_is_not_named_again() {
        val review = json(
            """{"reading":{"calls":[{"path":[],"to":"$usdc","value":{"raw":"0","decimals":18,"plain":"0","unlimited":false},
                 "intent":{"kind":"send","token":{"token":"$usdc","symbol":"USDC","decimals":6,"known":true},"to":"$funder",
                 "amount":{"raw":"10000","decimals":6,"plain":"0.01","unlimited":false}}}]},
               "recipients":[{"address":"${funder.lowercase()}","role":"recipient","firstTimeCue":true},{"address":"$carol","role":"contract"}]}""",
        )
        val drawn = readingAddresses(readingLines(review, 5042002, words()))
        assertTrue(drawn.contains(funder.lowercase()))
        val left = recipientsLeft(review, drawn)
        assertEquals("only the one the reading did not name", listOf(carol), left.map { (it as kotlinx.serialization.json.JsonObject).getValue("address").let { a -> (a as kotlinx.serialization.json.JsonPrimitive).content } })
    }

    // -- r12-7: the one short form ------------------------------------------------------

    /**
     * r12-7 (round 12): the header, Settings, Activity and the confirm said
     * `0x88cC…6894`, Activity a lowercase `0x3e9b…692d`. Every one now is the
     * core's `names.short`: EIP-55, 6+6.
     */
    @Test
    fun every_short_address_is_the_cores() {
        val want = "0x3e9b95…31692D"
        assertEquals(want, ShortAddress.of(funder.lowercase()))
        assertEquals(want, WalletLive.shortenAddress(funder.lowercase()))
        assertEquals(want, SendLive.shortAddress(funder))
        assertEquals(want, ExploreLive.shortAddress(funder.lowercase()))
        assertEquals(want, ContactsLive.shortenAddress(funder))
        assertEquals(want, MultisigWords.short(funder.lowercase()))
        assertEquals("0x88cCA0…266894", ShortAddress.of("0x88cca0eedbf2c4426110bbfc998f048689266894"))
    }

    @Test
    fun no_android_source_keeps_a_4_plus_4_short_form() {
        val dir = File(repoRoot, "app-android/vela-wallet/app/src")
        val offenders = dir.walkTopDown().filter { it.isFile && it.extension == "kt" && !it.path.contains("/test/") && !it.path.contains("/androidTest/") }
            .flatMap { file ->
                file.readLines().mapIndexedNotNull { i, line ->
                    if (line.contains("take(6)}…") || line.contains("take(6) + \"…\"") || line.contains("take(6) + \"\\u2026\"")) "${file.name}:${i + 1}" else null
                }
            }.toList()
        assertEquals(emptyList<String>(), offenders)
    }

    // -- r12-5 / r12-6: a pooled connection a server closed is not an outage ---------------

    /**
     * A one-thread HTTP server on 127.0.0.1: answers each request with [status]
     * line [version], then — [closeAfter] — closes the connection, as an HTTP/1.0
     * server (the fork's relay) or an idle keep-alive timeout (node's 5 s; the
     * dev fault proxy) does. Counts the connections it took.
     */
    private class Server(val version: String, val closeAfter: Boolean) : AutoCloseable {
        val socket = ServerSocket(0)
        val connections = AtomicInteger()
        val port get() = socket.localPort
        private val loop = thread(isDaemon = true) {
            while (!socket.isClosed) {
                val client = runCatching { socket.accept() }.getOrNull() ?: break
                connections.incrementAndGet()
                thread(isDaemon = true) { serve(client) }
            }
        }

        private fun serve(client: Socket) = client.use { c ->
            val input = c.getInputStream().bufferedReader()
            while (true) {
                val first = input.readLine() ?: return
                if (first.isEmpty()) continue
                while (true) { val h = input.readLine() ?: return; if (h.isEmpty()) break }
                val body = """{"ok":true}"""
                c.getOutputStream().write("$version 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${body.length}\r\n\r\n$body".toByteArray())
                c.getOutputStream().flush()
                if (closeAfter) return
            }
        }

        override fun close() = socket.close()
    }

    private fun get(port: Int) = Request.Builder().url("http://127.0.0.1:$port/api/proposals").get().build()

    /**
     * r12-5 (round 12): every mailbox read the service took answered 200, yet
     * the home said "Multisig service unreachable" through Retry and after the
     * faults cleared. The read that failed was on a POOLED connection the
     * server side had already closed — answered `http_unreachable`, which the
     * core counts as the service away. An HTTP/1.0 answer now ends its
     * connection, and a read that meets a dead pooled one is asked again on a
     * new one.
     */
    @Test
    fun an_http_1_0_answer_is_never_reused_and_every_read_is_answered() {
        Server("HTTP/1.0", closeAfter = true).use { server ->
            // The plain client, no asking again: the close itself is what keeps the pool clean.
            repeat(5) {
                VelaHttp.client.newCall(get(server.port)).execute().use { assertEquals(200, it.code) }
                Thread.sleep(50)
            }
            assertEquals("a connection each: none reused after the server closed it", 5, server.connections.get())
        }
    }

    @Test
    fun a_read_on_a_pooled_connection_the_server_closed_is_asked_again() {
        // HTTP/1.1, keep-alive by default — and the server closes anyway (an idle timeout).
        Server("HTTP/1.1", closeAfter = true).use { server ->
            VelaHttp.execute(VelaHttp.client, get(server.port)).use { assertEquals(200, it.code) }
            Thread.sleep(200) // the server's close lands; okhttp still holds the socket
            VelaHttp.execute(VelaHttp.client, get(server.port)).use { assertEquals("answered, never an outage", 200, it.code) }
            assertEquals(2, server.connections.get())
        }
    }

    @Test
    fun a_write_goes_on_a_connection_of_its_own() {
        Server("HTTP/1.1", closeAfter = true).use { server ->
            VelaHttp.execute(VelaHttp.client, get(server.port)).use { assertEquals(200, it.code) }
            Thread.sleep(200)
            val post = Request.Builder().url("http://127.0.0.1:${server.port}/api/proposals")
                .post("{}".toRequestBody(null)).build()
            VelaHttp.execute(VelaHttp.client, post).use { assertEquals("sent once, on a new connection", 200, it.code) }
        }
    }

    @Test
    fun the_pool_keeps_an_idle_connection_under_common_server_idle_timeouts() {
        assertTrue(VelaHttp.POOL_KEEP_ALIVE_SECONDS < 5)
        assertFalse("okhttp never sends a write twice on its own", VelaHttp.client.retryOnConnectionFailure)
    }
}
