package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.PayLinkRefusalWords
import app.getvela.wallet.feature.wallet.core.PayLink
import app.getvela.wallet.feature.wallet.core.PayRefusal
import app.getvela.wallet.feature.wallet.core.PayVerdict
import app.getvela.wallet.feature.wallet.core.PaymentRequestEvent
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import java.io.File
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.PaymentRequestCore

/**
 * Spec 039 Part B on the phone: a `/pay` link the core refuses is said, in
 * the web's words — "Invalid payment link" and why (`pay_refusal`: the
 * recipient, the network, the token or the amount) — whether it came by an
 * App Link, a share or a scan. It used to be dropped in silence.
 */
class PayLinkRefusalTest {
    private val en: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    @Test
    fun `the core says why each link is refused, and the words are the web's`() {
        for ((link, refusal, key) in listOf(
            Triple("https://getvela.app/pay?chain=100&amount=1", PayRefusal.Recipient, I18nKeys.PayLink.INVALID_BODY),
            Triple("https://getvela.app/pay?to=$ME&chain=zz&amount=1", PayRefusal.Network, I18nKeys.PayLink.INVALID_BODY),
            Triple("https://getvela.app/pay?to=$ME&chain=100&token=USDC&amount=1", PayRefusal.Token, I18nKeys.PayLink.INVALID_TOKEN),
            Triple("https://getvela.app/pay?to=$ME&chain=100&amount=1e3", PayRefusal.Amount, I18nKeys.PayLink.INVALID_AMOUNT),
        )) {
            val verdict = PayVerdict.of(opened(link))
            assertEquals(link, PayVerdict.Refused(refusal), verdict)
            val model = PayLinkRefusalWords.model(refusal, en)
            assertEquals(en.t(I18nKeys.PayLink.INVALID_TITLE), model.title)
            assertEquals(en.t(key), model.body)
            assertTrue("a sentence, not a key: ${model.body}", !model.body.startsWith("receive."))
        }
        assertEquals("Invalid payment link", PayLinkRefusalWords.model(PayRefusal.Amount, en).title)
        assertTrue(PayLinkRefusalWords.body(PayRefusal.Amount, en).contains("amount"))
        // No reason (a core that did not answer): the general sentence is still true.
        assertEquals(en.t(I18nKeys.PayLink.INVALID_BODY), PayLinkRefusalWords.body(null, en))
    }

    @Test
    fun `a good link is a request, with no refusal`() {
        val verdict = PayVerdict.of(opened("https://getvela.app/pay?to=$ME&chain=100&amount=0.5"))
        assertTrue("$verdict", verdict is PayVerdict.Valid)
        assertEquals(100, (verdict as PayVerdict.Valid).pay.chain_id)
        // The view decodes the reason as the core spells it.
        val wire = Wire.json.decodeFromString(PaymentRequestView.serializer(), """{"pay_valid":false,"pay_refusal":"amount"}""")
        assertEquals(PayVerdict.Refused(PayRefusal.Amount), PayVerdict.of(wire))
    }

    // -- the rig ------------------------------------------------------------------

    /** The link as the wallet route reads it: tokenized here, ruled on by the real core. */
    private fun opened(link: String): PaymentRequestView {
        val pay = PayLink.parse(link) as PayLink.Pay
        val core = PaymentRequestCore()
        try {
            val event = PaymentRequestEvent.LinkOpened(to = pay.to, chain = pay.chain, token = pay.token, amount = pay.amount, sym = pay.sym, dec = pay.dec, net = pay.net)
            val out = core.dispatch(Wire.json.encodeToString(PaymentRequestEvent.serializer(), event))
            val view = Wire.json.parseToJsonElement(out).jsonObject["view"] as JsonObject
            return Wire.json.decodeFromJsonElement(PaymentRequestView.serializer(), view)
        } finally {
            core.close()
        }
    }

    private companion object {
        const val ME = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    }
}
