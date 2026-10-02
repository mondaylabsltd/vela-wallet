package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeSpeedOptionView
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningRow
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.ClearBatchCall
import app.getvela.wallet.feature.signing.core.ClearBatchView
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearProvenance
import app.getvela.wallet.feature.signing.core.ClearSignField
import app.getvela.wallet.feature.signing.core.ClearSignResult
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignRequestView
import app.getvela.wallet.feature.signing.core.SignMethodKind
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.signing.core.ClearOperation
import app.getvela.wallet.feature.signing.core.ClearShellResult
import app.getvela.wallet.feature.signing.core.ClearSigningEvent
import app.getvela.wallet.feature.signing.core.ClearLocale
import app.getvela.wallet.feature.signing.core.ClearProbe
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import uniffi.vela_core_uniffi.ClearSigningCore
import org.junit.Test

/** Spec 044: the signing sheet is the four views, in the corpus's words. */
class SigningLiveTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val drawn = SigningFixtures.build(SigningScreenState.CS1, strings)
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Parallel space", "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
    private fun request(params: String) = IncomingRequest("r1", "eth_sendTransaction", params, "http://127.0.0.1:8137", "tab-1", 100)

    /**
     * The REAL `clear_signing` core, kicked off the way the sheet kicks it off
     * ([SigningController.clearKickoff] and its first-call reader), for one
     * `eth_sendTransaction` whose first call is [call].
     */
    private fun clearOf(call: org.json.JSONObject): ClearSigningView =
        clearOf("eth_sendTransaction", org.json.JSONArray().put(call).toString())

    /** The REAL core, kicked off by [SigningController.clearKickoff] for [method] and [params]. */
    private fun clearOf(method: String, params: String, chainId: Int = 100): ClearSigningView = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val host = CoreHost(
                bridge = ClearSigningCore().asBridge(), scope = scope, initial = ClearSigningView(), serializer = ClearSigningView.serializer(),
                perform = JsonShell.perform(ClearOperation.serializer(), ClearShellResult.serializer()) { op ->
                    when (op) {
                        is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(op.path, null)
                        is ClearOperation.RpcEthCall -> ClearShellResult.RpcAnswer(op.probe, op.chain_id, op.to, null, true)
                        is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
                        is ClearOperation.Timer -> ClearShellResult.TimedOut(op.token)
                        ClearOperation.Now -> ClearShellResult.Clock(1.0e12)
                    }
                },
                escapedFailure = JsonShell.escapedFailure(ClearOperation.serializer(), ClearShellResult.serializer(), fallback = ClearShellResult.Clock(1.0e12)) { ClearShellResult.Clock(1.0e12) },
            )
            host.dispatch(SigningController.clearKickoff(method, params, chainId, "http://127.0.0.1:8137")!!, ClearSigningEvent.serializer())
            withTimeout(10_000) { host.view.first { it.resolved && !it.resolving } }
        } finally {
            scope.cancel()
        }
    }

    private fun plainModel(call: org.json.JSONObject) = run {
        val params = org.json.JSONArray().put(call).toString()
        val sign = SignView(surface = SignSurface.Sheet, request = SignRequestView("r1", "eth_sendTransaction", SignMethodKind.Transaction, params, "http://127.0.0.1:8137", null, 100, null), confirm_gate_open = true)
        SigningLive.model(drawn, request(params), sign, clearOf(call), GuardView(), FeeView(confirm_fee_ready = true), ctx)
    }

    /**
     * Spec 082 RC1 (G14): a dApp's plain value transfer is the CORE's verdict
     * (`ClearSurface::PlainSend`), drawn with the fee row's symbol — the look
     * 079 gave it (Send · −0.001 XDAI · Recipient), no longer this app's own
     * interception of empty calldata.
     */
    @Test
    fun `a plain native transfer reads as Send, not as a blind contract call`() {
        val model = plainModel(org.json.JSONObject().put("to", founder).put("value", "0x38d7ea4c68000"))
        val intent = model.blocks.filterIsInstance<SigningBlock.Intent>().single()
        assertEquals(strings.t("componentsUi.signing.intentSend"), intent.text)
        val amount = model.blocks.filterIsInstance<SigningBlock.Amount>().single()
        assertEquals("−", amount.line.sign)
        assertEquals("0.001", amount.line.value)
        assertEquals("XDAI", amount.line.symbol)
        assertEquals(founder, model.blocks.filterIsInstance<SigningBlock.Party>().single().address)
        assertTrue("no blind warning", model.blocks.none { it is SigningBlock.Warning })
        assertEquals("127.0.0.1:8137", model.dappName)
        assertEquals("the host is said once, as the name (spec 079 F14)", "", model.dappHost)
        assertEquals(strings.t("componentsUi.signing.confirmSend"), model.confirmAction)
        assertTrue(model.confirmEnabled)
    }

    /** RC3: nothing moves — "Send · 0", no minus, and the neutral confirm, never "Confirm send". */
    @Test
    fun `a zero-value empty call is the same card with no minus and a neutral confirm`() {
        for (call in listOf(
            org.json.JSONObject().put("to", founder).put("value", "0x0"),
            org.json.JSONObject().put("to", founder),
            // A JSON null is absent (RC6: `optString` would have read "null").
            org.json.JSONObject().put("to", founder).put("value", org.json.JSONObject.NULL),
        )) {
            val model = plainModel(call)
            val amount = model.blocks.filterIsInstance<SigningBlock.Amount>().single()
            assertEquals(call.toString(), "", amount.line.sign)
            assertEquals(call.toString(), "0", amount.line.value)
            assertEquals(call.toString(), strings.t("componentsUi.signing.confirmLabel"), model.confirmAction)
        }
    }

    /** RC4/RC6: a value the core cannot print exactly is refused — the blind rung, never a calm "0". */
    @Test
    fun `a number where the value should be is blind, never a calm zero`() {
        val model = plainModel(org.json.JSONObject().put("to", founder).put("value", 1000))
        assertTrue("blind: ${model.blocks}", model.blocks.none { it is SigningBlock.Amount })
        assertTrue(model.blocks.any { it is SigningBlock.Warning })
        assertEquals("the reader passes the number as text", "1000", SigningController.firstCall("""[{"to":"$founder","value":1000}]""")?.third)
    }

    /**
     * RC6 on the submit side: a leg the reader refuses (a number where the
     * value should be, no `to`, a value that is not hex) refuses the whole
     * batch. Dropping it sent the others alone — a batch the dApp never asked
     * for, answered as if it had run.
     */
    @Test
    fun `a batch with an unreadable leg is refused whole, never sent a leg short`() {
        val ok = """{"to":"$founder","value":"0x1"}"""
        assertEquals(2, SignExecutor.callsOf("wallet_sendCalls", """[{"calls":[$ok,$ok]}]""")?.size)
        for (bad in listOf("""{"to":"$founder","value":1000}""", """{"value":"0x1"}""", """{"to":"$founder","value":"0x12zz"}""")) {
            assertNull(bad, SignExecutor.callsOf("wallet_sendCalls", """[{"calls":[$ok,$bad]}]"""))
            assertNull(bad, SignExecutor.callsOf("wallet_sendCalls", """[{"calls":[$bad,$ok]}]"""))
        }
        // One call: a number is refused, a JSON null is zero.
        assertNull(SignExecutor.callsOf("eth_sendTransaction", """[{"to":"$founder","value":1000}]"""))
        assertEquals("0", SignExecutor.callsOf("eth_sendTransaction", """[{"to":"$founder","value":null}]""")?.single()?.value)
        assertEquals("1000", SignExecutor.callsOf("eth_sendTransaction", """[{"to":"$founder","value":"0x3e8"}]""")?.single()?.value)
    }

    // Spec 096 (part B): what you see is what you sign.
    private val bnb = ctx.copy(chainName = "BNB Chain", nativeSymbol = "BNB", chainId = 56)

    private fun rowsOf(blocks: List<SigningBlock>): List<SigningRow> =
        blocks.filterIsInstance<SigningBlock.Rows>().flatMap { it.rows }

    /**
     * F4 on the real core: Aave's `depositETH` of 0.003 BNB (the 096 pass's
     * own request) reads "Supply · 0.003 BNB" — not "Value 0" — and a call
     * nobody could read still says the coin it sends.
     */
    @Test
    fun `the coin a call sends is in the readable part (096 F4)`() {
        val deposit = """[{"to":"0x0c2c95b24529664fe55d4437d7a31175cfe6c4f7","data":"0x474cf53d0000000000000000000000006807dc923806fe8fd134338eabca509979a7e0cb00000000000000000000000088cca0eedbf2c4426110bbfc998f0486892668940000000000000000000000000000000000000000000000000000000000000000","value":"0xaa87bee538000"}]"""
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val read = clearOf("eth_sendTransaction", deposit, chainId = 56)
        assertEquals(ClearSurface.ClearSign, read.surface)
        assertNull("the reading's own row says it — once", read.native_value)
        val model = SigningLive.model(drawn, request(deposit), sign, read, GuardView(), FeeView(confirm_fee_ready = true), bnb)
        val rows = rowsOf(model.blocks).map { "${it.label} ${it.value}" }
        assertEquals("${strings.t("componentsUi.signing.intentSupply")} 0.003 BNB", rows.first())
        assertTrue(rows.toString(), rows.none { "Value" in it || "Number" in it })

        val blindParams = """[{"to":"0x7777777777777777777777777777777777777777","data":"0xdeadbeef","value":"0xaa87bee538000"}]"""
        val blind = clearOf("eth_sendTransaction", blindParams, chainId = 56)
        assertEquals(ClearSurface.BlindTransaction, blind.surface)
        assertEquals("0.003", blind.native_value?.amount)
        val coin = rowsOf(SigningLive.model(drawn, request(blindParams), sign, blind, GuardView(), FeeView(confirm_fee_ready = true), bnb).blocks)
        assertEquals(strings.t("componentsUi.signing.labelAmount"), coin.first().label)
        assertEquals("−0.003 BNB", coin.first().value)
    }

    /** F5: an order whose terms are off chain says so; a known contract is named in its batch card. */
    @Test
    fun `an order says its terms are not shown and a known target is named (096 F5)`() {
        val order = ClearSignResult(
            intent = "Swap", risk = ClearRisk.Caution, provenance = ClearProvenance.BuiltIn, terms_off_chain = true,
            fields = listOf(ClearSignField(label = "Order", value = "0x09e95d9c...6abfb9fe", format = "raw")),
        )
        val warning = strings.t("componentsUi.signing.warnOrderTerms")
        assertFalse("the corpus has the sentence", warning.contains("warnOrderTerms"))
        val params = """[{"to":"0x9008D19f58AAbD9eD0D60971565AA8510560ab41","data":"0xec6cb13f"}]"""
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val lone = SigningLive.model(drawn, request(params), sign, ClearSigningView(resolved = true, surface = ClearSurface.ClearSign, result = order), GuardView(), FeeView(confirm_fee_ready = true), bnb)
        assertTrue(lone.blocks.any { it is SigningBlock.Warning && it.text == warning })

        val batch = ClearSigningView(
            resolved = true, surface = ClearSurface.Batch,
            batch = ClearBatchView(calls = listOf(
                ClearBatchCall(index = 1, surface = ClearSurface.ClearSign, result = order, to = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41", to_name = "CoW Protocol", value_wei = "0", amount = "0", risk = ClearRisk.Caution),
                ClearBatchCall(index = 2, surface = ClearSurface.BlindTransaction, to = "0x7777777777777777777777777777777777777777", data_bytes = 4, value_wei = "0", amount = "0"),
            )),
        )
        val drawnBatch = SigningLive.model(drawn, request(params), sign, batch, GuardView(), FeeView(confirm_fee_ready = true), bnb).blocks
        assertTrue(drawnBatch.any { it is SigningBlock.Warning && it.text == warning })
        val cards = drawnBatch.filterIsInstance<SigningBlock.Card>()
        val interacting = strings.t("componentsUi.signing.interactingLabel")
        val named = cards[0].rows.single { it.label == interacting }
        assertEquals("CoW Protocol", named.value)
        assertFalse("a name is not monospace", named.mono)
        assertTrue(cards[1].rows.single { it.label == interacting }.mono)
    }

    // Spec 097 (part A): nothing false or unknown stated as certain.

    /**
     * The REAL core reading one of the 097 pass's requests, its shell
     * scripted: the chain names WBNB and USDC on BNB Chain unless [chainDown],
     * nothing else answers. The event is dispatched as given — at UTC, so a
     * date reads against the chain's clock.
     */
    private fun passReading(event: ClearSigningEvent, chainDown: Boolean = false): ClearSigningView = runBlocking {
        val symbols = mapOf(
            "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c" to "WBNB",
            "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d" to "USDC",
        )
        fun word(n: Int) = "0x" + n.toString(16).padStart(64, '0')
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val host = CoreHost(
                bridge = ClearSigningCore().asBridge(), scope = scope, initial = ClearSigningView(), serializer = ClearSigningView.serializer(),
                perform = JsonShell.perform(ClearOperation.serializer(), ClearShellResult.serializer()) { op ->
                    when (op) {
                        is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(op.path, null)
                        is ClearOperation.RpcEthCall -> {
                            val symbol = symbols[op.to.lowercase()]?.takeUnless { chainDown }
                            val result = symbol?.let {
                                if (op.probe == ClearProbe.Decimals) word(18)
                                else word(32) + word(it.length).drop(2) + it.toByteArray().joinToString("") { b -> "%02x".format(b) }.padEnd(64, '0')
                            }
                            ClearShellResult.RpcAnswer(op.probe, op.chain_id, op.to, result, result == null)
                        }
                        is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
                        is ClearOperation.Timer -> ClearShellResult.TimedOut(op.token)
                        ClearOperation.Now -> ClearShellResult.Clock(1.790957e12)
                    }
                },
                escapedFailure = JsonShell.escapedFailure(ClearOperation.serializer(), ClearShellResult.serializer(), fallback = ClearShellResult.Clock(1.0e12)) { ClearShellResult.Clock(1.0e12) },
            )
            host.dispatch(event, ClearSigningEvent.serializer())
            withTimeout(10_000) { host.view.first { it.resolved && !it.resolving } }
        } finally {
            scope.cancel()
        }
    }

    private val utc = ClearLocale(tz_offset_minutes = 0)

    /** The 1inch order the 097 pass signed (USDC → BNB), verbatim. */
    private val oneInchOrder = """{"types":{"Order":[{"name":"salt","type":"uint256"},{"name":"maker","type":"address"},{"name":"receiver","type":"address"},{"name":"makerAsset","type":"address"},{"name":"takerAsset","type":"address"},{"name":"makingAmount","type":"uint256"},{"name":"takingAmount","type":"uint256"},{"name":"makerTraits","type":"uint256"}],"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"chainId","type":"uint256"},{"name":"verifyingContract","type":"address"}]},"domain":{"name":"1inch Aggregation Router","version":"6","chainId":56,"verifyingContract":"0x111111125421ca6dc452d289314280a0f8842a65"},"primaryType":"Order","message":{"salt":"33701748006248133072184610391074722838607073826819837756152271511067157162131","maker":"0x88cca0eedbf2c4426110bbfc998f048689266894","receiver":"0x0000000000000000000000000000000000000000","makerAsset":"0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d","takerAsset":"0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c","makingAmount":"1160000000000000000","takingAmount":"1430509396956033","makerTraits":"62645329528782394789705396186445553993653435352530577117757868129563663400960"}}"""

    /**
     * N6 (and N3 on this shell — the minimum is said by its row's label): the
     * pass's 1inch order reads as the swap it signs — the zero receiver is the
     * account itself, BNB arrives (the unwrap flag), and the expiry
     * `makerTraits` encodes is said: 16:42 UTC on 2026-10-02.
     */
    @Test
    fun `a 1inch order says where its proceeds go, the coin that arrives and until when (097 N6)`() {
        val read = passReading(ClearSigningEvent.ResolveTypedData(typed_data_json = oneInchOrder, chain_id = 56, locale = utc))
        assertEquals(ClearSurface.ClearSign, read.surface)
        val params = org.json.JSONArray().put("0x88cca0eedbf2c4426110bbfc998f048689266894").put(oneInchOrder).toString()
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val rows = rowsOf(SigningLive.model(drawn, request(params).copy(method = "eth_signTypedData_v4"), sign, read, GuardView(), FeeView(confirm_fee_ready = true), bnb).blocks)
        fun value(key: String) = rows.firstOrNull { it.label == strings.t("componentsUi.signing.$key") }?.value
        assertEquals(rows.toString(), "1.16 USDC", value("labelYouPay"))
        assertEquals(rows.toString(), "0.001430509396956033 BNB", value("labelYouReceiveMin"))
        assertEquals(rows.toString(), "0x88cca0...266894", value("labelRecipient"))
        assertEquals(rows.toString(), "2026-10-02, 16:42", value("labelValidUntil"))
        assertTrue(rows.none { "0x00000000" in it.value })
    }

    /**
     * N1 + N8: an amount the chain never scaled is no figure and the reading
     * says it is incomplete; a batch call on USDC names the token, with its
     * address beside a name only the chain gave.
     */
    @Test
    fun `an unscaled amount is incomplete and a token target is named (097 N1, N8)`() {
        val borrow = passReading(
            ClearSigningEvent.ResolveTransaction(
                to = "0x6807dc923806fe8fd134338eabca509979a7e0cb",
                data = "0xa415bcad0000000000000000000000008ac76a51cc950d9822d68b83fe1ad97b32cd580d0000000000000000000000000000000000000000000000000429d069189e00000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000088cca0eedbf2c4426110bbfc998f048689266894",
                value = "0x0", chain_id = 56, locale = utc,
            ),
            chainDown = true,
        )
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val params = """[{"to":"0x6807dc923806fe8fd134338eabca509979a7e0cb","data":"0xa415bcad"}]"""
        val blocks = SigningLive.model(drawn, request(params), sign, borrow, GuardView(), FeeView(confirm_fee_ready = true), bnb).blocks
        val amount = rowsOf(blocks).first { it.label == strings.t("componentsUi.signing.intentBorrow") }
        assertTrue("no figure: $amount", amount.value.startsWith("\u2014"))
        assertEquals(SigningTone.Caution, amount.valueTone)
        assertTrue(blocks.any { it is SigningBlock.Warning && it.text == strings.t("componentsUi.signing.partialWarning") })

        val usdc = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"
        val approve = "0x095ea7b300000000000000000000000031c2f6fcff4f8759b3bd5bf0e1084a055615c7680000000000000000000000000000000000000000000000001018f0e6fdc80000"
        val batchParams = """[{"chainId":"0x38","calls":[{"to":"$usdc","data":"$approve"},{"to":"0x7777777777777777777777777777777777777777","value":"0x1"}]}]"""
        val batch = passReading(ClearSigningEvent.ResolveBatch(params_json = batchParams, chain_id = 56, locale = utc))
        val cards = SigningLive.model(drawn, request(batchParams).copy(method = "wallet_sendCalls"), sign, batch, GuardView(), FeeView(confirm_fee_ready = true), bnb)
            .blocks.filterIsInstance<SigningBlock.Card>()
        val interacting = strings.t("componentsUi.signing.interactingLabel")
        assertEquals("USDC (0x8ac76a...cd580d)", cards[0].rows.single { it.label == interacting }.value)
    }

    /** F7: every machine says yes and the request is still being read — the slide stays shut, under "Loading…". */
    @Test
    fun `the slide waits for the reading (096 F7)`() {
        val params = """[{"to":"$founder","data":"0xdeadbeef"}]"""
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val reading = ClearSigningView(resolving = true, surface = ClearSurface.Loading)
        val model = SigningLive.model(drawn, request(params), sign, reading, GuardView(), FeeView(confirm_fee_ready = true), ctx)
        assertFalse(model.confirmEnabled)
        val sentence = model.blocks.filterIsInstance<SigningBlock.Sentence>().single()
        assertEquals(strings.t("componentsUi.signing.loading"), sentence.text)
        val read = SigningLive.model(drawn, request(params), sign, ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction), GuardView(), FeeView(confirm_fee_ready = true), ctx)
        assertTrue(read.confirmEnabled)
    }

    /**
     * Spec 096 F1: the core's value table, through this shell's reading —
     * PancakeSwap's `0xaa87bee538000` is 0.003 BNB; decimal text, bare hex,
     * a sign (this shell took "-1" as minus one) and an overflow are refused.
     */
    @Test
    fun `a call value is the core's reading`() {
        val table = listOf(
            "\"0xaa87bee538000\"" to "3000000000000000",
            "\"0xAA87BEE538000\"" to "3000000000000000",
            "\"0x\"" to "0",
            "\"\"" to "0",
            "\"0\"" to "0",
            "0" to "0",
            "\"0x${"f".repeat(64)}\"" to "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "\"0x1${"0".repeat(64)}\"" to null,
            "\"1000\"" to null,
            "\"aa87bee538000\"" to null,
            "\"-1\"" to null,
            "\"0X1f\"" to null,
        )
        for ((value, wei) in table) {
            val calls = SignExecutor.callsOf(
                "eth_sendTransaction",
                """[{"to":"0x13f4EA83D0bd40E75C8222255bc855a974568Dd4","value":$value,"data":"0x3593564c"}]""",
            )
            assertEquals(value, wei, calls?.single()?.value)
        }
    }

    @Test
    fun `the slide waits for the guard and the fee, and a contract call with bytes stays blind`() {
        val params = """[{"to":"$founder","data":"0xdeadbeef"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.Confirm)
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val blocked = SigningLive.model(drawn, request(params), sign, clear, GuardView(confirm_allowed = false), FeeView(confirm_fee_ready = true), ctx)
        assertFalse(blocked.confirmEnabled)
        val noFee = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = false), ctx)
        assertFalse(noFee.confirmEnabled)
        val blind = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = true), ctx)
        assertTrue(blind.blocks.any { it is SigningBlock.Warning })
        assertEquals(strings.t("componentsUi.signing.confirmLabel"), blind.confirmAction)
    }

    /**
     * Spec 081 FR-008: the sheet says where a description came from, and only
     * where there is something to say. A fetched descriptor is the descriptor
     * service's word over plain HTTP; everything else either IS the app's own
     * word or already has its own line.
     */
    @Test
    fun `a fetched description says it was never authenticated, and the others say nothing`() {
        val params = """[{"to":"$founder","data":"0xdeadbeef"}]"""
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        fun warningsFor(provenance: ClearProvenance): List<String> {
            val result = ClearSignResult(
                intent = "Stake",
                fields = listOf(ClearSignField(label = "To", value = "0x…", format = "addressName", address = founder)),
                contract_address = founder,
                provenance = provenance,
            )
            val clear = ClearSigningView(resolved = true, result = result, surface = ClearSurface.ClearSign, confirm = ClearConfirm.Confirm)
            val model = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = true), ctx)
            return model.blocks.filterIsInstance<SigningBlock.Warning>().map { it.text }
        }
        assertEquals(
            listOf(strings.t("componentsUi.signing.descriptorFetchedWarning")),
            warningsFor(ClearProvenance.Fetched),
        )
        for (quiet in listOf(ClearProvenance.BuiltIn, ClearProvenance.PinnedMatch, ClearProvenance.Standard, ClearProvenance.None)) {
            assertEquals(quiet.name, emptyList<String>(), warningsFor(quiet))
        }
    }

    private fun estimate(tier: FeeTier, totalWei: String) = FeeEstimateView(
        chain_id = 100, total_wei = totalWei, max_fee_per_gas = "2000000000", network_fee_per_gas = "0",
        relayer_fee_per_gas = "0", bundler_gas_price = "0", in_band_gas_basis = "0", effective_gas_price = "1000000000",
        max_gas_price = "2000000000", total_gas = "0", deployed = true, tier = tier, quoted = true,
        fee_asset = FeeAssetView.Native, fee_recipient = "0xfee",
    )

    /** Spec 069: the sheet's fee card carries the send form's speed control, drawn by the same builder. */
    @Test
    fun `the fee card carries the speed control, each option its own fee`() {
        val params = """[{"to":"$founder","value":"0x38d7ea4c68000"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val fast = estimate(FeeTier.Fast, "2100000000000000")
        val speed = FeeSpeedView(
            tier = FeeTier.Fast,
            open = true,
            options = listOf(
                FeeSpeedOptionView(FeeTier.Fast, selected = true, fee = fast, gas_price = "1 ~ 2 gwei"),
                FeeSpeedOptionView(FeeTier.Standard, measuring = true),
                FeeSpeedOptionView(FeeTier.Slow, fee = estimate(FeeTier.Slow, "1000000000000000")),
            ),
        )
        val model = SigningLive.model(
            drawn, request(params), sign, clear, GuardView(), FeeView(fee = fast, confirm_fee_ready = true), ctx,
            speed = SendLive.SpeedInputs(speed) { null },
        )
        val fee = model.fee as FeeModel.OnChain
        val control = fee.speed ?: error("the sheet draws the speed control")
        assertEquals(strings.t(I18nKeys.Flows.FEE_SPEED_LABEL), control.label)
        assertEquals(strings.t(I18nKeys.Flows.GAS_TIER_FAST), control.value)
        assertEquals(listOf("fast", "standard", "slow"), control.options.map { it.id })
        // Each option in its row's own words, minus the row's "~".
        assertEquals(fee.value, "~" + control.options[0].value)
        assertEquals("…", control.options[1].value)
        assertTrue(control.options[2].value, control.options[2].value.startsWith("0.001 XDAI"))
        assertEquals("1 ~ 2 gwei", control.options[0].gasPrice)
        assertTrue(model.confirmEnabled)
    }

    @Test
    fun `a speed just picked says estimating, and the slide waits for its own figure (issue 681)`() {
        val params = """[{"to":"$founder","value":"0x38d7ea4c68000"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        // The core's gate is still open — on the speed just left.
        val left = FeeView(fee = estimate(FeeTier.Fast, "2100000000000000"), confirm_fee_ready = true)
        val picked = SendLive.SpeedInputs(FeeSpeedView(tier = FeeTier.Slow, picked = true)) { null }
        val model = SigningLive.model(drawn, request(params), sign, clear, GuardView(), left, ctx, speed = picked)
        assertEquals(strings.t("componentsUi.gas.estimating"), (model.fee as FeeModel.OnChain).value)
        assertFalse("the slide never signs the speed walked away from", model.confirmEnabled)
        // Its own figure lands: the row and the slide follow.
        val landed = FeeView(fee = estimate(FeeTier.Slow, "1000000000000000"), confirm_fee_ready = true)
        val settled = SigningLive.model(drawn, request(params), sign, clear, GuardView(), landed, ctx, speed = picked)
        assertTrue((settled.fee as FeeModel.OnChain).value.startsWith("~0.001 XDAI"))
        assertTrue(settled.confirmEnabled)
    }

    @Test
    fun `the guard's verdict draws the cap, the chips the core offers, and the custom field`() {
        val detected = app.getvela.wallet.feature.signing.core.GuardDetectedApproval(
            kind = app.getvela.wallet.feature.signing.core.GuardApprovalKind.Erc20Approve, token_address = "0xdd", spender = "0x1111111111111111111111111111111111111111",
            amount_raw = null, is_unbounded = true, editable = true, locus = app.getvela.wallet.feature.signing.core.GuardLocus.CalldataWord(1),
        )
        // Kept as the site asked (2026-09-26): the requested chip, the
        // `unlimited` choice, the danger said, and the consent in the opts.
        val editor = app.getvela.wallet.feature.signing.core.GuardEditorView(
            mode = app.getvela.wallet.feature.signing.core.GuardEditorMode.Requested,
            choice = app.getvela.wallet.feature.signing.core.GuardChoice.Unlimited,
            requested_finite = false, requested_unlimited = true, has_balance_cap = false,
        )
        val kept = GuardView(surface = app.getvela.wallet.feature.signing.core.GuardSurface.ApprovalEditor, detected = detected, meta = app.getvela.wallet.feature.signing.core.GuardTokenMetaView("USDC", 6, true, false), editor = editor, confirm_allowed = true, unlimited_consented = true, unlimited_warning = true)
        val blocks = SigningLive.guardBlocks(kept, strings)
        val allowance = blocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(strings.t("componentsUi.signingApprove.unlimitedValue"), allowance.value)
        assertEquals(SigningTone.Danger, allowance.valueTone)
        assertEquals(app.getvela.wallet.feature.signing.AllowanceChip.ChipState.Selected, allowance.chips.first { it.id == "requested" }.state)
        assertNull("no 'unlimited is disabled' note — it is not", allowance.note)
        assertTrue(blocks.any { it is SigningBlock.Warning && it.text == strings.t("componentsUi.signing.unlimitedWarning") })
        val opts = SigningController.approveOpts(FeeView(), ClearSigningView(), kept)
        assertTrue(opts.unlimited_approved)
        assertNull("the site's own bytes", opts.params_override_json)
        assertFalse(SigningController.approveOpts(FeeView(), ClearSigningView(), GuardView()).unlimited_approved)

        val custom = kept.copy(editor = editor.copy(mode = app.getvela.wallet.feature.signing.core.GuardEditorMode.Custom, custom_text = "1", display_amount_raw = "1000000", choice = app.getvela.wallet.feature.signing.core.GuardChoice.Amount("1000000")), unlimited_consented = false, unlimited_warning = false)
        val cappedBlocks = SigningLive.guardBlocks(custom, strings)
        val bounded = cappedBlocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals("1 USDC", bounded.value)
        assertEquals(SigningTone.Neutral, bounded.valueTone)
        assertEquals("1", bounded.custom!!.value)
        assertEquals(app.getvela.wallet.feature.signing.AllowanceChip.ChipState.Selected, bounded.chips.first { it.id == "custom" }.state)
        assertFalse("a capped approval is not unlimited", cappedBlocks.any { it is SigningBlock.Warning })
    }

    @Test
    fun `a capped unlimited approval reads the cap, not the request's Unlimited`() {
        val result = app.getvela.wallet.feature.signing.core.ClearSignResult(
            intent = "Approve",
            fields = listOf(
                app.getvela.wallet.feature.signing.core.ClearSignField(label = "Amount", value = "Unlimited", format = "tokenAmount", warning = true),
                app.getvela.wallet.feature.signing.core.ClearSignField(label = "Spender", value = "0x1111", format = "addressName"),
            ),
            risk = app.getvela.wallet.feature.signing.core.ClearRisk.Danger,
        )
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.ClearSign, result = result)
        val detected = app.getvela.wallet.feature.signing.core.GuardDetectedApproval(
            kind = app.getvela.wallet.feature.signing.core.GuardApprovalKind.Erc20Approve, spender = "0x1111", is_unbounded = true, editable = true,
            locus = app.getvela.wallet.feature.signing.core.GuardLocus.CalldataWord(1),
        )
        val kept = GuardView(
            surface = app.getvela.wallet.feature.signing.core.GuardSurface.ApprovalEditor, detected = detected,
            meta = app.getvela.wallet.feature.signing.core.GuardTokenMetaView("USDC", 6, true, false),
            editor = app.getvela.wallet.feature.signing.core.GuardEditorView(choice = app.getvela.wallet.feature.signing.core.GuardChoice.Unlimited, requested_unlimited = true),
        )
        // Kept as asked: the decode is the truth, untouched.
        assertEquals(clear, SigningLive.cappedApproval(clear, kept))
        val capped = kept.copy(editor = kept.editor!!.copy(choice = app.getvela.wallet.feature.signing.core.GuardChoice.Amount("250000000"), display_amount_raw = "250000000"))
        val shown = SigningLive.cappedApproval(clear, capped).result!!
        assertEquals("250 USDC", shown.fields[0].value)
        assertFalse(shown.fields[0].warning)
        assertEquals("0x1111", shown.fields[1].value)
        assertEquals("an approve is caution once its only warning is gone", app.getvela.wallet.feature.signing.core.ClearRisk.Caution, shown.risk)

        // A bundle decodes from its first leg, so a capped first leg is what that decode reads too.
        val batch = GuardView(
            surface = app.getvela.wallet.feature.signing.core.GuardSurface.Batch,
            batch = app.getvela.wallet.feature.signing.core.GuardBatchView(
                legs = listOf(app.getvela.wallet.feature.signing.core.GuardLegView(to = "0xdd", approval = detected, meta = capped.meta, editor = capped.editor, choice = capped.editor!!.choice, needs_editor = true)),
                all_settled = true,
            ),
        )
        assertEquals("250 USDC", SigningLive.cappedApproval(clear, batch).result!!.fields[0].value)
    }

    @Test
    fun `a batch leg's card carries its leg, so its chips reach the leg`() {
        val leg = app.getvela.wallet.feature.signing.core.GuardLegView(
            to = "0xdd",
            approval = app.getvela.wallet.feature.signing.core.GuardDetectedApproval(
                kind = app.getvela.wallet.feature.signing.core.GuardApprovalKind.Erc20Approve, spender = "0x1111", is_unbounded = true, editable = true,
                locus = app.getvela.wallet.feature.signing.core.GuardLocus.CalldataWord(1),
            ),
            editor = app.getvela.wallet.feature.signing.core.GuardEditorView(
                mode = app.getvela.wallet.feature.signing.core.GuardEditorMode.Requested,
                choice = app.getvela.wallet.feature.signing.core.GuardChoice.Unlimited, requested_unlimited = true,
            ),
            needs_editor = true,
        )
        val batch = GuardView(
            surface = app.getvela.wallet.feature.signing.core.GuardSurface.Batch,
            batch = app.getvela.wallet.feature.signing.core.GuardBatchView(legs = listOf(transferLeg(), leg), any_uncapped = true, all_settled = true),
        )
        val cards = SigningLive.guardBlocks(batch, strings).filterIsInstance<SigningBlock.Allowance>()
        assertEquals(listOf<Int?>(1), cards.map { it.leg })
        assertEquals(app.getvela.wallet.feature.signing.core.GuardEditorMode.Balance, SigningLive.chipMode("balance"))
        assertNull(SigningLive.chipMode("grant"))
    }

    /** Spec 094 S8: an unbounded off-chain permit is warned about from the core's flag, and gets no editor. */
    @Test
    fun `an unlimited permit is said like an unlimited approval and still cannot be capped`() {
        val permit = GuardView(
            surface = app.getvela.wallet.feature.signing.core.GuardSurface.PermitSign,
            detected = app.getvela.wallet.feature.signing.core.GuardDetectedApproval(
                kind = app.getvela.wallet.feature.signing.core.GuardApprovalKind.Permit2Single, spender = "0x1111",
                is_unbounded = true, editable = false,
                locus = app.getvela.wallet.feature.signing.core.GuardLocus.TypedPath("details.amount"),
            ),
            unlimited_warning = true,
        )
        val blocks = SigningLive.guardBlocks(permit, strings)
        val warnings = blocks.filterIsInstance<SigningBlock.Warning>().map { it.text }
        assertTrue(warnings.contains(strings.t("componentsUi.signing.unlimitedWarning")))
        assertTrue(warnings.contains(strings.t("componentsUi.signingApprove.permitCantCap")))
        assertTrue(blocks.none { it is SigningBlock.Allowance })
        val bounded = SigningLive.guardBlocks(permit.copy(unlimited_warning = false), strings)
            .filterIsInstance<SigningBlock.Warning>().map { it.text }
        assertEquals(listOf(strings.t("componentsUi.signingApprove.permitCantCap")), bounded)
    }

    private fun transferLeg() = app.getvela.wallet.feature.signing.core.GuardLegView(to = "0xdd")

    // -- Spec 046 US1: the balance-change block --------------------------------

    @Test
    fun `the balance block says what moves, as the trust machine judged it`() {
        val ready = SigningController.SimOutcome.Ready(
            listOf(
                TrustSimJudgment.Native("-1000000000000000"),
                TrustSimJudgment.Erc20Trusted(token = "0xddaf", delta = "12000000", symbol = "USDC", decimals = 6),
                TrustSimJudgment.Erc20Unverified(token = "0xbad", delta = "5"),
            ),
        )
        val blocks = SigningLive.simBlocks(ready, ctx)
        val balances = blocks.single() as SigningBlock.Balances
        assertEquals(strings.t("componentsUi.signing.balanceChangesTitle"), balances.title)
        assertEquals(listOf("XDAI", "USDC", strings.t("componentsUi.signing.balanceUnverifiedToken")), balances.rows.map { it.symbol })
        assertEquals(listOf("−0.001", "+12", "+5"), balances.rows.map { it.delta })
        assertEquals(listOf(SigningTone.Neutral, SigningTone.Success, SigningTone.Caution), balances.rows.map { it.tone })
        assertEquals(strings.t("componentsUi.signing.unverifiedWarning"), balances.note)

        val none = SigningLive.simBlocks(SigningController.SimOutcome.Ready(emptyList()), ctx).single() as SigningBlock.Balances
        assertEquals(strings.t("componentsUi.signing.simResultNoChange"), none.note)
        assertTrue(none.rows.isEmpty())
        val unavailable = SigningLive.simBlocks(SigningController.SimOutcome.Notice(ClearRisk.Caution, "componentsUi.signing.simUnavailableWarning"), ctx).single() as SigningBlock.Warning
        assertEquals(strings.t("componentsUi.signing.simUnavailableWarning"), unavailable.text)
        assertEquals("could not check is a caution, never a danger", SigningTone.Caution, unavailable.tone)
        val reverts = SigningLive.simBlocks(SigningController.SimOutcome.Notice(ClearRisk.Danger, "componentsUi.signing.simWillFailReason", "STF"), ctx).single() as SigningBlock.Warning
        assertEquals(SigningTone.Danger, reverts.tone)
        assertEquals(strings.t("componentsUi.signing.simWillFailReason", mapOf("reason" to "STF")), reverts.text)
        assertTrue(SigningLive.simBlocks(null, ctx).isEmpty())
    }

    /**
     * Issue #314: the wallet's own key backup leads with its outcome. Its
     * intent is the headline (the sheet draws `dappOwn` that way) and a
     * simulation that moves nothing is folded into the technical details — it
     * was a bordered "Balance changes · No asset changes" card weighing as much
     * as the outcome. A dApp's sheet keeps the card, and a simulation that has
     * something to say is never folded away, not even on the wallet's own.
     */
    @Test
    fun `the wallet's own backup folds a simulation that moves nothing into the technical details`() {
        val registry = "0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9"
        val call = org.json.JSONObject().put("to", registry).put("data", "0xcd438f9b").put("value", "0x0")
        val params = org.json.JSONArray().put(call).toString()
        val sign = SignView(surface = SignSurface.Sheet, request = SignRequestView("r1", "eth_sendTransaction", SignMethodKind.Transaction, params, "https://getvela.app", null, 1, null), confirm_gate_open = true)
        val backup = ClearSigningView(
            resolved = true,
            surface = ClearSurface.ClearSign,
            result = ClearSignResult(
                intent = "Back up public keys",
                contract_name = "Vela passkey registry",
                fields = listOf(
                    ClearSignField("Registered as", "Parallel space"),
                    ClearSignField("Address", "0x88cCA0Ee…266894", format = "addressName", address = founder.lowercase()),
                    ClearSignField("Public keys", "1"),
                ),
                risk = ClearRisk.Safe,
                contract_address = registry,
                verified = true,
                provenance = ClearProvenance.BuiltIn,
            ),
        )
        val own = IncomingRequest("r1", "eth_sendTransaction", params, "https://getvela.app", SigningLive.WALLET_TRANSPORT, 1)
        val nothingMoves = SigningController.SimOutcome.Ready(emptyList())
        fun sheet(request: IncomingRequest, sim: SigningController.SimOutcome) =
            SigningLive.model(drawn, request, sign, backup, GuardView(), FeeView(confirm_fee_ready = true), ctx, sim)

        val ownSheet = sheet(own, nothingMoves)
        assertTrue(ownSheet.dappOwn)
        assertEquals(strings.t("settingsModals.backup.intent"), (ownSheet.blocks.first() as SigningBlock.Intent).text)
        assertTrue("the no-change card is still on the sheet", ownSheet.blocks.none { it is SigningBlock.Balances })
        assertEquals(strings.t("componentsUi.signing.simResultLabel"), ownSheet.tech.simResult?.label)
        assertEquals(strings.t("componentsUi.signing.simResultNoChange"), ownSheet.tech.simResult?.value)
        assertFalse(ownSheet.tech.isEmpty)

        val dappSheet = sheet(request(params), nothingMoves)
        assertTrue("a dApp keeps its balance card", dappSheet.blocks.any { it is SigningBlock.Balances })
        assertNull(dappSheet.tech.simResult)

        val reverts = sheet(own, SigningController.SimOutcome.Notice(ClearRisk.Danger, "componentsUi.signing.simWillFail"))
        assertTrue("a revert is never folded away", reverts.blocks.any { it is SigningBlock.Warning && it.tone == SigningTone.Danger })
        assertNull(reverts.tech.simResult)
        val moves = sheet(own, SigningController.SimOutcome.Ready(listOf(TrustSimJudgment.Native("-1000000000000000"))))
        assertTrue("a balance that would move stays on the sheet", moves.blocks.any { it is SigningBlock.Balances })
        assertNull(moves.tech.simResult)
    }

    /**
     * Spec 082 RJ15 (G49): a signed amount is the core's (`formatSignedTokenAmount`)
     * — a dust delta is written exactly, never `−0`, the minus is U+2212, and a
     * zero is not drawn at all.
     */
    @Test
    fun `a signed amount never reads minus zero and a zero is not drawn`() {
        val dust = SigningLive.simBlocks(
            SigningController.SimOutcome.Ready(listOf(TrustSimJudgment.Native("-1000"), TrustSimJudgment.Erc20Trusted(token = "0xddaf", delta = "0", symbol = "USDC", decimals = 6))),
            ctx,
        ).single() as SigningBlock.Balances
        assertEquals("the zero USDC move is not a row", listOf("XDAI"), dust.rows.map { it.symbol })
        assertEquals("\u22120.000000000000001", dust.rows.single().delta)
        assertTrue("never −0", dust.rows.none { it.delta == "\u22120" || it.delta == "-0" })

        val zeros = SigningLive.simBlocks(SigningController.SimOutcome.Ready(listOf(TrustSimJudgment.Native("0"))), ctx).single() as SigningBlock.Balances
        assertTrue(zeros.rows.isEmpty())
        assertEquals("nothing of theirs moves", strings.t("componentsUi.signing.simResultNoChange"), zeros.note)
    }

    /**
     * Spec 082 RJ13 (G48): the reason under a failed fee is the core's
     * (`feeFailureReasonKey`) — the relay's failure, or the chain's node,
     * rate-limited or out of reach and named — never Vela's words for a node
     * that did not answer, and nothing at all for a failure no network caused.
     */
    @Test
    fun `the fee row's reason is the core's, naming the chain's node when that is what failed`() {
        fun warning(failure: app.getvela.wallet.feature.send.core.FeeFailure) =
            (SigningLive.feeModel(ClearSigningView(), FeeView(failed = failure), ctx) as FeeModel.OnChain).warning
        assertEquals(strings.t("home.balanceDetailStatusRetrying"), warning(app.getvela.wallet.feature.send.core.FeeFailure.ChainRead(rate_limited = true)))
        assertEquals(strings.t("explore.chainDown", mapOf("chain" to "Gnosis")), warning(app.getvela.wallet.feature.send.core.FeeFailure.ChainRead(rate_limited = false)))
        assertTrue("the chain is named", warning(app.getvela.wallet.feature.send.core.FeeFailure.ChainRead(rate_limited = false))!!.contains("Gnosis"))
        assertEquals(strings.t("componentsUi.funding.denialNetworkError"), warning(app.getvela.wallet.feature.send.core.FeeFailure.QuoteUnavailable))
        assertNull(warning(app.getvela.wallet.feature.send.core.FeeFailure.CalculationFailed))
        val down = SigningLive.feeModel(ClearSigningView(), FeeView(failed = app.getvela.wallet.feature.send.core.FeeFailure.ChainRead(true)), ctx) as FeeModel.OnChain
        assertTrue("a chain read that failed is tapped to ask again", down.tappable)
    }

    // -- Issue #262: the coin that pays --------------------------------------

    private fun estimate(asset: FeeAssetView, totalWei: String) = FeeEstimateView(
        chain_id = 1, total_wei = totalWei, max_fee_per_gas = "0", network_fee_per_gas = "1", relayer_fee_per_gas = "0",
        bundler_gas_price = "1", in_band_gas_basis = "1", total_gas = "300000", deployed = false, tier = FeeTier.Fast,
        quoted = true, fee_asset = asset, fee_recipient = "0x3e59292e18417f814112f731e7163534c6d2fe3c",
    )

    private val eth = FeeOptionView(symbol = "ETH", decimals = 18, balance = "0", recipient = "0x3e59", usd_balance = "0", amount = "400000000000000", insufficient = true, selected = true)
    private val usdt = FeeOptionView(symbol = "USDT", contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7", decimals = 6, balance = "2000000", recipient = "0x3e59", usd_balance = "2", amount = "1020000")

    /** The report's own wallet: 0 ETH and 2 USDT on Ethereum, quoted in ETH. */
    @Test
    fun `a fee in a coin the account does not hold says so, and the list offers the one it has`() {
        val fee = FeeView(fee = estimate(FeeAssetView.Native, "400000000000000"), options = listOf(eth, usdt), confirm_fee_ready = false)

        val closed = SigningLive.feeModel(ClearSigningView(), fee, ctx) as FeeModel.OnChain
        assertEquals(strings.t("send.warnInsufficientGas", mapOf("sym" to "ETH")), closed.warning)
        assertTrue("two coins: the row opens a list", closed.tappable)
        assertTrue(closed.options.isEmpty())

        val open = SigningLive.feeModel(ClearSigningView(), fee, ctx.copy(feeOpen = true)) as FeeModel.OnChain
        assertEquals(listOf("native", usdt.contract), open.options.map { it.id })
        assertTrue("ETH cannot pay: shown, not pickable", open.options[0].disabled)
        assertFalse(open.options[1].disabled)
    }

    /**
     * Spec 096 F2: the person chose a coin the transaction itself spends (the
     * PancakeSwap USDC swap, fee in USDC). The core flags it; the sheet says
     * so under the fee while that coin pays, and nothing while another does.
     */
    @Test
    fun `a fee coin the transaction spends is warned while it pays`() {
        val usdc = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"
        val spent = FeeOptionView(
            symbol = "USDC", contract = usdc, decimals = 18, balance = "2341700000000000000",
            recipient = "0x3e59", usd_balance = "2.34", amount = "280000000000000000",
            selected = true, spent_by_operation = true,
        )
        val bnb = eth.copy(symbol = "BNB", selected = false)
        val paying = FeeView(
            fee = estimate(FeeAssetView.Erc20(token = usdc, decimals = 18, amount = "280000000000000000", symbol = "USDC"), "0"),
            options = listOf(bnb, spent),
            confirm_fee_ready = true,
        )
        val model = SigningLive.feeModel(ClearSigningView(), paying, ctx) as FeeModel.OnChain
        assertEquals(strings.t("componentsUi.gas.feeCoinSpent", mapOf("sym" to "USDC")), model.warning)
        assertFalse(model.warning!!.contains("{{"))

        val inUsdt = paying.copy(options = listOf(bnb, spent.copy(selected = false), usdt.copy(selected = true)))
        assertNull((SigningLive.feeModel(ClearSigningView(), inUsdt, ctx) as FeeModel.OnChain).warning)
        val measuring = paying.copy(busy = true)
        assertNull((SigningLive.feeModel(ClearSigningView(), measuring, ctx) as FeeModel.OnChain).warning)
    }

    /** A coin that pays is not a warning, and one coin alone has no list to open. */
    @Test
    fun `a fee the account can pay carries no warning`() {
        val fee = FeeView(fee = estimate(FeeAssetView.Native, "1"), options = listOf(eth.copy(balance = "1", amount = "1", insufficient = false)), confirm_fee_ready = true)

        val model = SigningLive.feeModel(ClearSigningView(), fee, ctx) as FeeModel.OnChain
        assertNull(model.warning)
        assertFalse(model.tappable)
    }

    /** Spec 079: the fee can always be asked again, and a fee that could not be quoted says why. */
    @Test
    fun `the fee row carries the send form's refresh and says why a quote failed`() {
        val shown = SigningLive.feeModel(ClearSigningView(), FeeView(fee = estimate(FeeAssetView.Native, "1"), options = listOf(eth.copy(balance = "1", amount = "1", insufficient = false)), confirm_fee_ready = true), ctx) as FeeModel.OnChain
        assertEquals(strings.t(I18nKeys.Flows.FEE_REFRESH), shown.refreshLabel)
        assertFalse("one coin: no list, so no chevron", shown.chevron)

        val measuring = SigningLive.feeModel(ClearSigningView(), FeeView(busy = true), ctx) as FeeModel.OnChain
        assertTrue(measuring.refreshing)

        val down = SigningLive.feeModel(ClearSigningView(), FeeView(failed = app.getvela.wallet.feature.send.core.FeeFailure.QuoteUnavailable), ctx) as FeeModel.OnChain
        assertEquals(strings.t("componentsUi.funding.denialNetworkError"), down.warning)
        assertTrue("a failed quote is tapped to ask again", down.tappable)

        val broken = SigningLive.feeModel(ClearSigningView(), FeeView(failed = app.getvela.wallet.feature.send.core.FeeFailure.MissingPublicKey), ctx) as FeeModel.OnChain
        assertNull("no network sentence for a failure the network did not cause", broken.warning)
    }

    /** The approve signs the coin that was picked, in that coin's units (the send core's own rule). */
    @Test
    fun `the approve carries the picked stablecoin and its own amount`() {
        val picked = FeeView(
            fee = estimate(FeeAssetView.Erc20(token = usdt.contract!!, decimals = 6, amount = "1020000", symbol = "USDT"), "0"),
            fee_token = usdt.contract,
            confirm_fee_ready = true,
        )
        val opts = SigningController.approveOpts(picked, ClearSigningView(), GuardView())
        assertEquals(usdt.contract, opts.gas_fee_token)
        assertEquals("1020000", opts.quoted_fee?.amount)

        val native = SigningController.approveOpts(FeeView(fee = estimate(FeeAssetView.Native, "400000000000000"), confirm_fee_ready = true), ClearSigningView(), GuardView())
        assertNull(native.gas_fee_token)
        assertEquals("400000000000000", native.quoted_fee?.amount)
    }

    // Spec 081's "a refusal says what happened, and carries a kind" moved with
    // the code it tests: spec 070 made the CORE write the page's answer
    // (`dapp_browser` → `dapp_rpc::sign_error_json`), so this shell no longer
    // has a `responseJson` to check. The assertion is now
    // `BrowserMachineTest."a refused request tells the page what happened and
    // why"`, against the real machine, plus the core's own
    // `dapp_rpc::tests::a_refused_request_is_answered_with_a_sentence_and_a_kind`.

    /**
     * The core names a descriptor's words; the sheet says them in the reader's
     * language (the founder read "Approve / Amount / Spender / Unlimited" in
     * English on a Chinese sheet, 2026-09-27). Unnamed words stay as written.
     */
    @Test
    fun namedDescriptorWordsReadInTheReadersLanguage() {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val zh: VelaStrings = I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("zh") }
        val clear = app.getvela.wallet.feature.signing.core.ClearSigningView(
            resolved = true,
            surface = ClearSurface.ClearSign,
            confirm = ClearConfirm.ConfirmIntent("Approve", intent_term = "intentApprove"),
            result = app.getvela.wallet.feature.signing.core.ClearSignResult(
                intent = "Approve",
                intent_term = "intentApprove",
                fields = listOf(
                    app.getvela.wallet.feature.signing.core.ClearSignField(
                        label = "Amount", label_term = "labelAmount",
                        value = "Unlimited", value_term = "valueUnlimited",
                        format = "tokenAmount", warning = true,
                    ),
                    app.getvela.wallet.feature.signing.core.ClearSignField(label = "Referral code", value = "abc"),
                ),
            ),
        )
        val result = SigningLive.localizedTerms(clear, zh).result!!
        assertEquals(zh.t("componentsUi.signing.intentApprove"), result.intent)
        assertTrue("translated, not the English", result.intent != "Approve")
        assertEquals(listOf(zh.t("componentsUi.signing.labelAmount"), "Referral code"), result.fields.map { it.label })
        assertEquals(listOf(zh.t("componentsUi.signing.valueUnlimited"), "abc"), result.fields.map { it.value })
        assertEquals(zh.t("componentsUi.signing.intentApprove"), SigningLive.confirmLabel(clear, zh))
    }

    // ---------------------------------------------------------------------
    // 089 S1 — a batch's sheet shows every call
    // ---------------------------------------------------------------------

    private val other = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"

    private fun batchModel(calls: String, clear: ClearSigningView? = null, guard: GuardView = GuardView()) = run {
        val params = """[{"version":"2.0.0","chainId":"0x64","calls":$calls}]"""
        val sign = SignView(surface = SignSurface.Sheet, request = SignRequestView("r1", "wallet_sendCalls", SignMethodKind.Batch, params, "http://127.0.0.1:8137", null, 100, null), confirm_gate_open = true)
        val incoming = IncomingRequest("r1", "wallet_sendCalls", params, "http://127.0.0.1:8137", "tab-1", 100)
        SigningLive.model(drawn, incoming, sign, clear ?: clearOf("wallet_sendCalls", params), guard, FeeView(confirm_fee_ready = true), ctx)
    }

    /**
     * THE defect: `[1 wei → A, 1 xDAI → B]` read "Send 0.000…1 xDAI to A" and
     * signed both. The real core, kicked off as the sheet kicks it off, hands
     * every call; every call is a card; the total is said; the headline is the
     * batch's, never call 1's "Send".
     */
    @Test
    fun `a two-call batch draws both calls and their total, never call 1 alone`() {
        val model = batchModel("""[{"to":"$founder","value":"0x1"},{"to":"$other","value":"0xde0b6b3a7640000"}]""")
        val intent = model.blocks.first() as SigningBlock.Intent
        assertEquals(strings.t("componentsUi.signing.batchIntent"), intent.text)
        assertEquals(
            strings.t("componentsUi.signing.batchSubtitle", mapOf("count" to "2")),
            (model.blocks[1] as SigningBlock.Sentence).text,
        )
        val cards = model.blocks.filterIsInstance<SigningBlock.Card>()
        assertEquals(2, cards.size)
        val send = strings.t("componentsUi.signing.intentSend")
        assertEquals(strings.t("componentsUi.signing.batchStep", mapOf("index" to "1", "action" to send)), cards[0].title)
        assertEquals("−0.000000000000000001 XDAI", cards[0].rows[0].value)
        assertEquals(founder.lowercase(), cards[0].rows[1].value.lowercase())
        assertEquals(strings.t("componentsUi.signing.batchStep", mapOf("index" to "2", "action" to send)), cards[1].title)
        assertEquals("−1 XDAI", cards[1].rows[0].value)
        assertEquals(other.lowercase(), cards[1].rows[1].value.lowercase())
        val total = model.blocks.filterIsInstance<SigningBlock.Rows>().single().rows.single()
        assertEquals(strings.t("send.splitTotalLabel"), total.label)
        assertEquals("−1.000000000000000001 XDAI", total.value)
        assertTrue("no lone Send headline", model.blocks.filterIsInstance<SigningBlock.Intent>().none { it.text == send })
        assertEquals(strings.t("componentsUi.signing.confirmLabel"), model.confirmAction)
        assertEquals("the technical details are the whole batch", true, model.tech.rawHex?.contains(other))
    }

    /**
     * A call nobody could read is a card that says so (the "unable to decode"
     * words, its byte count), with whom it calls — never dropped; and a batch
     * whose SECOND call is an unlimited approve reads danger in that call's
     * card and in the headline, with the guard's warning under them.
     */
    @Test
    fun `an unreadable call says so, and a second-call unlimited approve warns`() {
        val approve = ClearSignResult(
            intent = "Approve", intent_term = "intentApprove", contract_name = null, owner = null,
            fields = listOf(
                ClearSignField(label = "Amount", value = "Unlimited", format = "tokenAmount", warning = true, label_term = "labelAmount", value_term = "valueUnlimited"),
            ),
            risk = ClearRisk.Danger, contract_address = founder, verified = false, provenance = ClearProvenance.Standard,
        )
        val clear = ClearSigningView(
            resolved = true, surface = ClearSurface.Batch,
            batch = app.getvela.wallet.feature.signing.core.ClearBatchView(
                calls = listOf(
                    app.getvela.wallet.feature.signing.core.ClearBatchCall(index = 1, surface = ClearSurface.BlindTransaction, to = other, data_bytes = 36, value_wei = "0", amount = "0", risk = ClearRisk.Caution),
                    app.getvela.wallet.feature.signing.core.ClearBatchCall(index = 2, surface = ClearSurface.ClearSign, result = approve, to = founder, data_bytes = 68, value_wei = "0", amount = "0", risk = ClearRisk.Danger),
                ),
                total_value_wei = "0", total_amount = "0", risk = ClearRisk.Danger,
            ),
        )
        val guard = GuardView(
            surface = app.getvela.wallet.feature.signing.core.GuardSurface.Batch,
            batch = app.getvela.wallet.feature.signing.core.GuardBatchView(any_uncapped = true),
            unlimited_warning = true,
        )
        val model = batchModel("""[{"to":"$other","data":"0xdeadbeef"},{"to":"$founder","data":"0x095ea7b3"}]""", clear, guard)
        assertEquals(SigningTone.Danger, (model.blocks.first() as SigningBlock.Intent).tone)
        val cards = model.blocks.filterIsInstance<SigningBlock.Card>()
        val undecoded = strings.t("componentsUi.signing.blindDecodeWarning", mapOf("bytes" to "36"))
        assertEquals(strings.t("componentsUi.signing.batchStep", mapOf("index" to "1", "action" to undecoded)), cards[0].title)
        assertEquals(other, cards[0].rows.single().value)
        assertEquals(SigningTone.Danger, cards[1].tone)
        assertEquals(strings.t("componentsUi.signing.batchStep", mapOf("index" to "2", "action" to strings.t("componentsUi.signing.intentApprove"))), cards[1].title)
        assertEquals(strings.t("componentsUi.signing.valueUnlimited"), cards[1].rows[0].value)
        // …and whom the call goes to: inside a batch nothing else says it for this call.
        assertEquals(strings.t("componentsUi.signing.interactingLabel"), cards[1].rows[1].label)
        assertEquals(founder, cards[1].rows[1].value)
        assertTrue(
            "the unlimited warning",
            model.blocks.any { it is SigningBlock.Warning && it.text == strings.t("componentsUi.signing.unlimitedWarning") },
        )
        assertTrue("nothing to total", model.blocks.none { it is SigningBlock.Rows })
    }

    /** The kickoff: a batch goes over whole; a transaction is its own call, never a stray `calls` beside it. */
    @Test
    fun `the kickoff hands a batch over whole and reads a transaction from its own call`() {
        val batch = """[{"calls":[{"to":"$founder","value":"0x1"},{"to":"$other","value":"0x2"}]}]"""
        assertEquals(
            ClearSigningEvent.ResolveBatch(params_json = batch, chain_id = 100, locale = (SigningController.clearKickoff("wallet_sendCalls", batch, 100, null) as ClearSigningEvent.ResolveBatch).locale),
            SigningController.clearKickoff("wallet_sendCalls", batch, 100, null),
        )
        val stray = """[{"to":"$founder","data":"0x095ea7b3ffff","calls":[{"to":"$other","value":"0x1"}]}]"""
        val event = SigningController.clearKickoff("eth_sendTransaction", stray, 100, null) as ClearSigningEvent.ResolveTransaction
        assertEquals(founder, event.to)
        assertEquals("0x095ea7b3ffff", event.data)
    }
}
