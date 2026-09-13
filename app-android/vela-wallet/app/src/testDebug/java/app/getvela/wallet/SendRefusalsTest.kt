package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcPool
import java.io.File
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * Spec 043 phase 5 (T043): every refusal reaches the screen in the core's
 * words, and the cancel checkpoint holds — one prompt per attempt.
 *
 * Same harness as [SendMachineTest]: the real `send` + `fee_policy`
 * machines, a scripted relay, the fixture keyset as the signer.
 */
class SendRefusalsTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val recipient = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private var signs = 0
    private var relaySends = 0

    /** When set, the signer parks here until the test lets go (or the executor cancels it). */
    private var gate: CompletableDeferred<Unit>? = null

    @After
    fun stop() = scope.cancel()

    private fun ctx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = mapOf(100 to "https://gnosisscan.io"),
        money = WalletLive.Money.of(CurrencyView(code = "USD")),
        fromName = "Parallel space",
        fromAddress = safe,
    )

    private fun seedAccount() {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject()
            .put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
        store.values["vela.accounts"] = JSONArray().put(account).toString()
        store.values["vela.activeAccountIndex"] = "0"
    }

    private fun scriptRelay() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            FakeRelayPort.body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", "0x77359400").put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.always("eth_sendUserOperation") { relaySends += 1; FakeRelayPort.body("0xhash") }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            signs += 1
            gate?.await()
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private fun controller(): SendController {
        val pool = RpcPool(store = FakeStore(), endpoints = FakeEndpointSource(listOf("https://rpc.test")), scope = scope, transport = FakeRpcTransport { _, _ -> FakeRpcTransport.network() })
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        return SendController(
            scope = scope,
            relay = relay,
            pool = pool,
            feed = feed,
            accountStore = AccountStore(store),
            balances = { BalanceView(tokens = listOf(BalanceToken(chain_id = 100, symbol = "XDAI", name = "xDAI", balance = "0.71697", decimals = 18, token_address = null, price_usd = 1.0))) },
            networks = { NetView(loaded = true, networks = listOf(NetNetworkRow(id = "gnosis", chain_id = 100, display_name = "Gnosis", native_symbol = "xDAI", rpc_url = "https://rpc.test", explorer_url = "https://gnosisscan.io", bundler_url = "https://relay.test/100"))) },
            signer = { fixtureSigner },
            haptic = {},
            refreshBalances = {},
        )
    }

    private suspend fun SendController.toForm() {
        open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { send.first { it.tokens.isNotEmpty() } }
        selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { send.first { it.stage == SendStage.EnterDetails } }
    }

    private suspend fun SendController.toConfirm() {
        toForm()
        setRecipient(recipient); setAmount("0.001")
        withTimeout(10_000) { send.first { it.can_continue } }
        continueTapped()
        withTimeout(30_000) { send.first { it.stage == SendStage.Confirm && it.can_confirm } }
    }

    @Test
    fun `a malformed address is refused in the core's words`() = runBlocking {
        seedAccount(); scriptRelay()
        val c = controller()
        c.toForm()
        c.setRecipient("0xabc"); c.setAmount("0.001")
        // The core validates the address at Continue (the web's rule too):
        // the form does not second-guess a half-typed address.
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        val alert = withTimeout(10_000) {
            while (c.alert.value == null) delay(20)
            c.alert.value!!
        }
        assertEquals(SendAlertKind.InvalidAddress, alert)
        val (title, body) = SendLive.alertText(alert, strings)
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INVALID_ADDRESS_TITLE), title)
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INVALID_ADDRESS_BODY), body)
        assertTrue("the corpus has words for it, not a key", title.isNotBlank() && !title.startsWith("send."))
        c.dismissAlert()
        assertNull(c.alert.value)
    }

    @Test
    fun `more than the balance is refused on the form, before Continue`() = runBlocking {
        seedAccount(); scriptRelay()
        val c = controller()
        c.toForm()
        c.setRecipient(recipient); c.setAmount("5")
        val view = withTimeout(10_000) { c.send.first { it.amount_warning != null } }
        val warning = SendLive.formWarning(view, ctx())
        assertNotNull("the form says what is wrong while the person is still typing", warning)
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY), warning)
        val drawn = (app.getvela.wallet.feature.flows.FlowFixtures.build(app.getvela.wallet.feature.flows.FlowState.SD2, strings).base as app.getvela.wallet.feature.flows.FlowBase.SendForm).model
        val form = SendLive.form(drawn, view, app.getvela.wallet.feature.send.core.FeeView(), ctx())
        assertEquals(warning, form.warning)
        // Continue is the checkpoint: the same refusal, as the alert, in the same words.
        c.continueTapped()
        val alert = withTimeout(10_000) {
            while (c.alert.value == null) delay(20)
            c.alert.value!!
        }
        assertTrue("$alert", alert is SendAlertKind.InsufficientBalance)
        assertEquals(warning, SendLive.alertText(alert, strings).second)
        assertEquals("still on the form", SendStage.EnterDetails, c.send.value.stage)
    }

    @Test
    fun `cancelling the ceremony spends one attempt and returns to confirm`() = runBlocking {
        seedAccount(); scriptRelay()
        val c = controller()
        c.toConfirm()
        gate = CompletableDeferred()
        c.slideConfirm()
        val signing = withTimeout(10_000) { c.send.first { it.tx_status == SendTxStatus.Signing } }
        // The stage is still Confirm while the prompt is up: the person is
        // looking at the confirm page, under the system's passkey sheet.
        assertEquals(app.getvela.wallet.feature.flows.FlowState.SD3, SendLive.flowState(signing, feeSheetOpen = false))
        withTimeout(10_000) { while (signs == 0) delay(20) }
        val confirm = SendLive.confirm(
            (app.getvela.wallet.feature.flows.FlowFixtures.build(app.getvela.wallet.feature.flows.FlowState.SD3, strings).base as app.getvela.wallet.feature.flows.FlowBase.SendConfirm).model,
            signing, ctx(),
        )
        assertEquals("while the prompt is up the button under it says Cancel", strings.t(I18nKeys.Flows.CANCEL), confirm.noticeAction)
        assertEquals(strings.t(I18nKeys.Flows.TX_PREPARING_BIOMETRIC), confirm.notice)
        assertFalse(confirm.ctaEnabled)
        c.cancelSigning()
        val back = withTimeout(10_000) { c.send.first { it.tx_status != SendTxStatus.Signing } }
        assertEquals("cancel returns to the confirm page", SendStage.Confirm, back.stage)
        assertNull("a cancel is not an error", back.tx_error)
        assertEquals(1, signs)
        assertEquals("nothing reached the relay", 0, relaySends)
        // A second attempt is a second prompt — never a replay of the first.
        gate = null
        withTimeout(10_000) { c.send.first { it.can_confirm } }
        c.slideConfirm()
        withTimeout(30_000) { c.send.first { it.receipt != null } }
        assertEquals(2, signs)
        assertEquals(1, relaySends)
    }

    @Test
    fun `the relay's refusal is the confirm page's notice, with a retry`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { FakeRelayPort.error("AA25 invalid account nonce") }
        val c = controller()
        c.toConfirm()
        c.slideConfirm()
        val failed = withTimeout(30_000) { c.send.first { it.tx_error != null } }
        assertEquals(SendStage.Confirm, failed.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_ERROR_GENERIC), SendLive.confirmNotice(failed, ctx()))
        val drawn = (app.getvela.wallet.feature.flows.FlowFixtures.build(app.getvela.wallet.feature.flows.FlowState.SD3, strings).base as app.getvela.wallet.feature.flows.FlowBase.SendConfirm).model
        val confirm = SendLive.confirm(drawn, failed, ctx())
        assertEquals(strings.t(I18nKeys.Flows.TX_RETRY), confirm.noticeAction)
        assertFalse("the slider waits for the retry", confirm.ctaEnabled)
        c.retryAfterError()
        val cleared = withTimeout(10_000) { c.send.first { it.tx_error == null } }
        assertTrue(cleared.can_confirm)
    }

    @Test
    fun `the relay being out of gas money is named as such`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { FakeRelayPort.error("insufficient funds for gas * price + value: relayer balance too low") }
        val c = controller()
        c.toConfirm()
        c.slideConfirm()
        val failed = withTimeout(30_000) { c.send.first { it.tx_error != null } }
        val notice = SendLive.confirmNotice(failed, ctx())
        assertNotNull(notice)
        assertTrue(notice == strings.t(I18nKeys.Flows.TX_ERROR_BUNDLER_FUND) || notice == strings.t(I18nKeys.Flows.TX_ERROR_GENERIC))
    }
}
