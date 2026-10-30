package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.AppLinks
import app.getvela.wallet.feature.wallet.core.AppLinks.Route
import app.getvela.wallet.feature.wallet.core.AppLinks.WebKind
import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element

/**
 * The verified App Links (native parity Phase 3): what each claimed link
 * routes to, read with the REAL core's link codec (`multisig_link_call` over
 * JNA) — the links below are the web's own shapes (`tests/vectors/multisig/links.json`).
 */
class AppLinksTest {
    private val safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private val txHash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    @Test
    fun `a pay link on either host is a payment request`() {
        val a = AppLinks.classify("https://getvela.app/pay?to=$founder&chain=100&amount=0.001") as Route.Pay
        assertEquals(founder, a.link.to)
        assertEquals("100", a.link.chain)
        val b = AppLinks.classify("https://wallet.getvela.app/pay?to=$founder&chain=1&sym=USDC") as Route.Pay
        assertEquals("USDC", b.link.sym)
    }

    @Test
    fun `a transaction to approve goes to the web wallet as a proposal`() {
        val url = "https://wallet.getvela.app/zh/wallet#ms=100.$safe.$txHash"
        assertEquals(Route.Web(url, WebKind.Proposal), AppLinks.classify(url))
        // The web's own tolerance: an encoded `=` still reads.
        val encoded = "https://wallet.getvela.app/en/wallet#ms%3D100.$safe.$txHash"
        assertEquals(WebKind.Proposal, (AppLinks.classify(encoded) as Route.Web).kind)
    }

    @Test
    fun `a multisig to add goes to the web wallet as an invite, any locale`() {
        val url = "https://wallet.getvela.app/pt-BR/wallet#msa=100,137:$safe"
        assertEquals(Route.Web(url, WebKind.Invite), AppLinks.classify(url))
        val bare = "https://wallet.getvela.app/en/wallet#msa=$safe"
        assertEquals(WebKind.Invite, (AppLinks.classify(bare) as Route.Web).kind)
    }

    @Test
    fun `a damaged multisig link still goes to the web, which says what is wrong`() {
        val shortHash = "https://wallet.getvela.app/en/wallet#ms=100.$safe.0x12"
        assertEquals(WebKind.Broken, (AppLinks.classify(shortHash) as Route.Web).kind)
        val badChecksum = "https://wallet.getvela.app/en/wallet#msa=0x0cb9aef9713c4a2ea2995fca68d7355ce5e27D83"
        assertEquals(WebKind.Broken, (AppLinks.classify(badChecksum) as Route.Web).kind)
    }

    @Test
    fun `the outside signer's connect page goes to the browser`() {
        val url = "https://wallet.getvela.app/en/connect#wss%3A%2F%2Frelay.example%23abc&for=$founder"
        assertEquals(Route.Web(url, WebKind.Connect), AppLinks.classify(url))
    }

    @Test
    fun `the wallet page with nothing to act on just opens the app`() {
        assertEquals(Route.Home, AppLinks.classify("https://wallet.getvela.app/en/wallet"))
        assertEquals(Route.Home, AppLinks.classify("https://wallet.getvela.app/zh/wallet#settings"))
    }

    @Test
    fun `anything else is not ours`() {
        assertNull(AppLinks.classify(null))
        assertNull(AppLinks.classify("not a url"))
        assertNull(AppLinks.classify("http://wallet.getvela.app/en/wallet#ms=100.$safe.$txHash"))
        assertNull(AppLinks.classify("https://evil.example/en/wallet#ms=100.$safe.$txHash"))
        // getvela.app is the marketing site: only its /pay is the wallet's.
        assertNull(AppLinks.classify("https://getvela.app/en/wallet#ms=100.$safe.$txHash"))
        assertNull(AppLinks.classify("https://wallet.getvela.app/en/settings"))
        assertNull(AppLinks.classify("https://wallet.getvela.app/wallet"))
        assertNull(AppLinks.classify("https://wallet.getvela.app/english/wallet"))
        assertNull(AppLinks.classify("https://wallet.getvela.app/en/wallet/extra"))
        // velawallet:// stays PayLink's.
        assertNull(AppLinks.classify("velawallet://pay?to=$founder"))
    }

    @Test
    fun `a codec that refuses reads as a broken link, never a crash`() {
        val refusing = AppLinks.LinkCodec { method, _ -> if (method == "isMultisigLink") "true" else error("refused") }
        val url = "https://wallet.getvela.app/en/wallet#ms=100.$safe.$txHash"
        assertEquals(Route.Web(url, WebKind.Broken), AppLinks.classify(url, refusing))
    }

    /**
     * The manifest claims exactly these, with `autoVerify`, one host per
     * filter (Android 12+ verifies each host alone, so getvela.app's pay link
     * never waits on the wallet host's `assetlinks.json`).
     */
    @Test
    fun `the manifest claims each host with autoVerify`() {
        val root = File(System.getProperty("vela.repo.root"))
        val manifest = root.resolve("app-android/vela-wallet/app/src/main/AndroidManifest.xml")
        val doc = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder().parse(manifest)
        val ns = "http://schemas.android.com/apk/res/android"
        val filters = doc.getElementsByTagName("intent-filter").let { list -> (0 until list.length).map { list.item(it) as Element } }
        val verified = filters.filter { it.getAttributeNS(ns, "autoVerify") == "true" }
        val claims = verified.associate { filter ->
            val data = filter.getElementsByTagName("data").let { list -> (0 until list.length).map { list.item(it) as Element } }
            val hosts = data.mapNotNull { it.getAttributeNS(ns, "host").takeIf(String::isNotEmpty) }
            assertEquals("one host per verified filter", 1, hosts.size)
            assertEquals(listOf("https"), data.mapNotNull { it.getAttributeNS(ns, "scheme").takeIf(String::isNotEmpty) })
            val actions = filter.getElementsByTagName("category").let { list -> (0 until list.length).map { (list.item(it) as Element).getAttributeNS(ns, "name") } }
            assertTrue(actions.containsAll(listOf("android.intent.category.DEFAULT", "android.intent.category.BROWSABLE")))
            hosts.single() to data.flatMap { d -> listOf("path", "pathPattern").mapNotNull { a -> d.getAttributeNS(ns, a).takeIf(String::isNotEmpty) } }.toSet()
        }
        assertEquals(AppLinks.HOSTS, claims.keys)
        assertEquals(setOf("/pay"), claims["getvela.app"])
        assertEquals(setOf("/pay", "/..*/wallet", "/..*/connect"), claims["wallet.getvela.app"])
    }

    /** The file the owner hosts names this package and the keys getvela.app's already names. */
    @Test
    fun `the assetlinks file to host names the package and its release key`() {
        val root = File(System.getProperty("vela.repo.root"))
        val file = root.resolve("app-android/vela-wallet/app-links/assetlinks.json")
        val entries = org.json.JSONArray(file.readText())
        val wallet = (0 until entries.length()).map { entries.getJSONObject(it) }
            .single { it.getJSONObject("target").getString("package_name") == "app.getvela.wallet" }
        val relations = wallet.getJSONArray("relation").let { r -> (0 until r.length()).map(r::getString) }
        assertTrue(relations.contains("delegate_permission/common.handle_all_urls"))
        val prints = wallet.getJSONObject("target").getJSONArray("sha256_cert_fingerprints").let { r -> (0 until r.length()).map(r::getString) }
        assertTrue("the release key", prints.contains(RELEASE_KEY))
        prints.forEach { assertTrue("a SHA-256 fingerprint: $it", Regex("^([0-9A-F]{2}:){31}[0-9A-F]{2}$").matches(it)) }
    }

    private companion object {
        const val RELEASE_KEY = "A3:8E:36:FE:5A:99:AE:30:73:F7:91:4C:3D:43:58:32:AE:BD:D9:7D:C1:A4:DF:EF:9E:4B:3A:6E:10:31:9D:53"
    }
}
