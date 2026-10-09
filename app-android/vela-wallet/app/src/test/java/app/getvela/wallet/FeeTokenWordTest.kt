package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeTokenWord
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEstimateFailure
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendFeeCoin
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.formChain
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.SendCore

/**
 * The fee → send bridge's word about the fee card's coin, under the core's
 * one rule (the doc on `send::Event::FeeTokenChanged`; iOS
 * `SendStore.feeTokenChanged`): the card's coin reaches the send machine
 * whenever the pair (chain, coin) differs from what this journey was last
 * told, only while the fee session prices the form's own chain — the core
 * files the word against the form's chain at the moment it is said, and drops
 * it while the form has none. A fresh journey has been told nothing.
 *
 * Over the REAL send machine, driven by hand on this thread (the web's
 * `realSend`, the desktop's `Journey`): every warm quote fails, so the form
 * has no estimate in hand and its fee row names the coin in force.
 */
class FeeTokenWordTest {

    private val usdt = "0x55d398326f99059ff775485246999027b3197955"

    private fun holding(chainId: Int, network: String, symbol: String, contract: String? = null) = SendToken(
        network = network, chain_id = chainId, symbol = symbol, balance = "5", decimals = 18,
        token_address = contract, price_usd = 1.0,
    )

    private val bnb = holding(56, "bsc", "BNB")
    private val eth = holding(1, "ethereum", "ETH")

    /** One send journey on the real machine, with the controller's bridge in front of it. */
    private inner class Journey(private val word: FeeTokenWord) {
        private val core = SendCore().asBridge()
        val told = mutableListOf<SendEvent>()

        init {
            dispatch(
                SendEvent.Open(
                    account = SendAccountRef(id = "cred0", address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"),
                    display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2),
                ),
            )
        }

        val view: SendView get() = Wire.json.decodeFromString(SendView.serializer(), core.view())
        val coin: SendFeeCoin? get() = view.fee_coin

        fun dispatch(event: SendEvent) = drain(core.dispatch(Wire.json.encodeToString(SendEvent.serializer(), event)))

        fun pick(token: SendToken) {
            dispatch(SendEvent.SelectToken(SendLive.tokenId(token)))
            assertEquals(SendStage.EnterDetails, view.stage)
        }

        /** What `SendController.tellFeeToken` does: the card's coin [token], its session pricing [pricing]. */
        fun tell(token: String?, pricing: Int?): SendEvent? =
            word.news(token, pricing, formChain(view))?.also {
                told += it
                dispatch(it)
            }

        /** Answer what the machine asks, depth first, on this thread; anything else stays unanswered. */
        private fun drain(resultJson: String) {
            val effects = JSONObject(resultJson).optJSONArray("effects") ?: return
            for (i in 0 until effects.length()) {
                val effect = effects.getJSONObject(i)
                val answer: SendShellResult = when (effect.getJSONObject("operation").getString("type")) {
                    "fetch_tokens" -> SendShellResult.TokensLoaded(
                        tokens = listOf(bnb, holding(56, "bsc", "USDT", usdt), eth),
                        chains = listOf(SendChainInfo(56, "bsc", "BNB"), SendChainInfo(1, "ethereum", "ETH")),
                    )
                    "load_account_credential" -> SendShellResult.AccountCredential(public_key_hex = "04aa")
                    "prewarm_fees" -> SendShellResult.FeesPrewarmed
                    "estimate_fee" -> SendShellResult.FeeEstimated(SendFeeOutcome.Failed(SendEstimateFailure.QuoteUnavailable))
                    else -> continue
                }
                drain(core.resolveEffect(effect.getLong("id").toULong(), Wire.json.encodeToString(SendShellResult.serializer(), answer)))
            }
        }
    }

    @Test
    fun `the card's coin is told once per change, only about the form's own chain, and again to a new journey`() {
        val word = FeeTokenWord()
        var send = Journey(word)
        // No chain on the form yet: a word now is about no chain at all.
        assertNull(send.tell(usdt, 56))
        send.pick(bnb)
        assertNull("nothing was told before the form had a chain", send.coin?.contract)

        assertEquals(SendEvent.FeeTokenChanged(usdt), send.tell(usdt, 56))
        assertEquals("the card's coin names the row", SendFeeCoin(symbol = "USDT", contract = usdt, chain_id = 56), send.coin)

        // The person picks the chain's own coin: newer than the card's word.
        send.dispatch(SendEvent.ChooseFeeToken(null))
        assertNull(send.coin?.contract)
        // The card has not spoken again — the same word is not said twice, or
        // it would undo the person's pick.
        assertNull("an unchanged coin is not told again", send.tell(usdt, 56))
        assertNull(send.coin?.contract)
        assertEquals(SendEvent.FeeTokenChanged(null), send.tell(null, 56))
        // A session still pricing another chain says nothing about this one.
        assertNull("another chain's coin is not told", send.tell(usdt, 1))
        assertNull("nor is a session that has priced nothing", send.tell(usdt, null))
        assertNull(send.coin?.contract)
        assertEquals(SendEvent.FeeTokenChanged(usdt), send.tell(usdt, 56))
        assertEquals("a change is told", usdt, send.coin?.contract)
        assertEquals(3, send.told.size)

        // A new journey has been told nothing: the same coin is told again.
        word.forget()
        send = Journey(word)
        send.pick(bnb)
        assertEquals(SendEvent.FeeTokenChanged(usdt), send.tell(usdt, 56))
        assertEquals("a fresh journey hears the card's coin", usdt, send.coin?.contract)
    }

    @Test
    fun `the same coin is told again when the form moves to another chain`() {
        val send = Journey(FeeTokenWord())
        send.pick(bnb)
        assertEquals(SendEvent.FeeTokenChanged(null), send.tell(null, 56))

        send.dispatch(SendEvent.ChangeToken)
        send.pick(eth)
        assertNull("the network just left says nothing about this one", send.tell(null, 56))
        assertEquals(SendEvent.FeeTokenChanged(null), send.tell(null, 1))
        assertNull(send.tell(null, 1))
        assertEquals(2, send.told.size)
        assertEquals(SendFeeCoin(symbol = "ETH", contract = null, chain_id = 1), send.coin)
    }

    @Test
    fun `the form's chain is the selected token's, else the sweep's`() {
        assertNull(formChain(SendView()))
        assertEquals(56, formChain(SendView(selected_token = bnb, multi_chain_id = 1)))
        assertEquals(1, formChain(SendView(multi_chain_id = 1)))
    }
}
