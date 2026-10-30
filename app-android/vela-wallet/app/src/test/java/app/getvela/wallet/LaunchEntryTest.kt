package app.getvela.wallet

import app.getvela.wallet.core.platform.LaunchEntry
import app.getvela.wallet.core.platform.LaunchEntry.Ask
import app.getvela.wallet.core.platform.LauncherShortcuts
import app.getvela.wallet.feature.flows.WalletFlowEntry
import app.getvela.wallet.navigation.launchFlowEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The ways in from outside the app (native parity Phase 3): launcher
 * shortcuts, the Quick Settings Scan tile, and text shared into Vela.
 */
class LaunchEntryTest {
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    @Test
    fun `each shortcut and the tile name their flow by action`() {
        assertEquals(Ask.Open(LaunchEntry.Flow.Send), LaunchEntry.read("app.getvela.wallet.action.SEND", null, null))
        assertEquals(Ask.Open(LaunchEntry.Flow.Receive), LaunchEntry.read("app.getvela.wallet.action.RECEIVE", null, null))
        assertEquals(Ask.Open(LaunchEntry.Flow.Scan), LaunchEntry.read("app.getvela.wallet.action.SCAN", null, null))
        // The launcher's own start asks nothing beyond opening.
        assertNull(LaunchEntry.read("android.intent.action.MAIN", null, null))
        assertNull(LaunchEntry.read(null, null, null))
        // An action string that only looks like ours is not.
        assertNull(LaunchEntry.read("app.getvela.wallet.action.STEAL", null, null))
    }

    @Test
    fun `a shortcut's flow is the dock's own flow`() {
        assertEquals(WalletFlowEntry.Send, launchFlowEntry(LaunchEntry.Flow.Send))
        assertEquals(WalletFlowEntry.Receive, launchFlowEntry(LaunchEntry.Flow.Receive))
        assertEquals(WalletFlowEntry.Scan, launchFlowEntry(LaunchEntry.Flow.Scan))
        // Labelled with the dock's words, in the app's language.
        assertEquals("componentsUi.dock.send", LauncherShortcuts.labelKey(LaunchEntry.Flow.Send))
        assertEquals("componentsUi.dock.receive", LauncherShortcuts.labelKey(LaunchEntry.Flow.Receive))
        assertEquals("componentsUi.dock.scan", LauncherShortcuts.labelKey(LaunchEntry.Flow.Scan))
        // Stable ids: a republish replaces, never duplicates, a pinned shortcut.
        assertEquals(listOf("send", "receive", "scan"), LaunchEntry.Flow.entries.map { it.shortcutId })
    }

    @Test
    fun `a shared address or ethereum request goes to Send`() {
        assertEquals(Ask.PaymentShared(founder), LaunchEntry.read(LaunchEntry.ACTION_SEND, "text/plain", founder))
        assertEquals(Ask.PaymentShared(founder), LaunchEntry.shared("Here is my address: $founder."))
        assertEquals(Ask.PaymentShared(founder), LaunchEntry.shared("（$founder）"))
        val request = "ethereum:$founder@100?value=1e15"
        assertEquals(Ask.PaymentShared(request), LaunchEntry.shared("pay me $request please"))
    }

    @Test
    fun `a shared pay link goes to the core's pay link check, before anything else in the text`() {
        val link = "https://getvela.app/pay?to=$founder&chain=100&amount=0.001"
        assertEquals(Ask.PayLinkShared(link), LaunchEntry.shared("Pay me: $link"))
        val wallet = "https://wallet.getvela.app/pay?to=$founder&chain=1"
        assertEquals(Ask.PayLinkShared(wallet), LaunchEntry.shared("$founder or $wallet"))
        val scheme = "velawallet://pay?to=$founder&chain=100"
        assertEquals(Ask.PayLinkShared(scheme), LaunchEntry.shared(scheme))
    }

    @Test
    fun `a shared web-wallet multisig or connect link is read by the core and goes to the browser`() {
        val connect = "https://wallet.getvela.app/en/connect#wss%3A%2F%2Frelay.example%23abc&for=$founder"
        assertEquals(Ask.WebLinkShared(connect), LaunchEntry.shared("Approve here: $connect"))
        val proposal = "https://wallet.getvela.app/en/wallet#ms=100.0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83." +
            "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
        assertEquals(Ask.WebLinkShared(proposal), LaunchEntry.shared(proposal))
        // A plain wallet page is not a link to act on.
        assertNull(LaunchEntry.shared("https://wallet.getvela.app/en/wallet"))
    }

    @Test
    fun `anything else shared asks nothing - no word is guessed into a recipient`() {
        assertNull(LaunchEntry.shared(null))
        assertNull(LaunchEntry.shared(""))
        assertNull(LaunchEntry.shared("hello there"))
        assertNull(LaunchEntry.shared("vitalik.eth"))
        assertNull(LaunchEntry.shared("0x1234"))
        assertNull(LaunchEntry.shared("https://example.com/pay?to=$founder"))
        // Not text: a shared image or file is not ours to read.
        assertNull(LaunchEntry.read(LaunchEntry.ACTION_SEND, "image/png", founder))
        assertNull(LaunchEntry.read(LaunchEntry.ACTION_SEND, null, founder))
    }
}
