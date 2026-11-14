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
    fun `the front page's carried multisig link is one too, and nothing else of the front page is ours`() {
        val url = "https://wallet.getvela.app/en#msa=$safe"
        assertEquals(Route.Web(url, WebKind.Invite), AppLinks.classify(url))
        assertEquals(WebKind.Proposal, (AppLinks.classify("https://wallet.getvela.app/zh#ms=100.$safe.$txHash") as Route.Web).kind)
        assertNull(AppLinks.classify("https://wallet.getvela.app/en"))
        assertNull(AppLinks.classify("https://wallet.getvela.app/en#settings"))
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

    private val ns = "http://schemas.android.com/apk/res/android"

    private fun manifest(): Element {
        val root = File(System.getProperty("vela.repo.root"))
        val file = root.resolve("app-android/vela-wallet/app/src/main/AndroidManifest.xml")
        return DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder().parse(file).documentElement
    }

    private fun Element.children(tag: String): List<Element> =
        getElementsByTagName(tag).let { list -> (0 until list.length).map { list.item(it) as Element } }

    /**
     * MainActivity claims `/pay` only, one host per verified filter (Android
     * 12+ verifies each host alone) — never a bare wallet page, which would
     * take every web-wallet visit from the browser, and never the connect
     * page, which belongs to members who sign with another wallet.
     */
    @Test
    fun `the activity claims the pay link on each host, and nothing else`() {
        val activity = manifest().children("activity").single { it.getAttributeNS(ns, "name") == ".MainActivity" }
        val claims = activity.children("intent-filter").filter { it.getAttributeNS(ns, "autoVerify") == "true" }.associate { filter ->
            val data = filter.children("data")
            val hosts = data.mapNotNull { it.getAttributeNS(ns, "host").takeIf(String::isNotEmpty) }
            assertEquals("one host per verified filter", 1, hosts.size)
            assertEquals(listOf("https"), data.mapNotNull { it.getAttributeNS(ns, "scheme").takeIf(String::isNotEmpty) })
            val categories = filter.children("category").map { it.getAttributeNS(ns, "name") }
            assertTrue(categories.containsAll(listOf("android.intent.category.DEFAULT", "android.intent.category.BROWSABLE")))
            hosts.single() to data.flatMap { d -> PATH_ATTRS.mapNotNull { a -> d.getAttributeNS(ns, a).takeIf(String::isNotEmpty) } }.toSet()
        }
        assertEquals(AppLinks.HOSTS, claims.keys)
        assertEquals(setOf("/pay"), claims["getvela.app"])
        assertEquals(setOf("/pay"), claims["wallet.getvela.app"])
        assertTrue(
            "the connect page is claimed nowhere",
            manifest().children("data").none { d -> PATH_ATTRS.any { "connect" in d.getAttributeNS(ns, it) } },
        )
    }

    /**
     * The multisig links are matched on the fragment (Android 15's
     * `uri-relative-filter-group`) by an alias that is OFF before API 35 —
     * where the groups are ignored and the path alone would claim every
     * wallet page.
     */
    @Test
    fun `multisig links are claimed on the fragment, on Android 15 and later only`() {
        val alias = manifest().children("activity-alias").single { it.getAttributeNS(ns, "name") == ".MultisigLinks" }
        assertEquals(".MainActivity", alias.getAttributeNS(ns, "targetActivity"))
        assertEquals("@bool/vela_fragment_links", alias.getAttributeNS(ns, "enabled"))
        val res = File(System.getProperty("vela.repo.root"), "app-android/vela-wallet/app/src/main/res")
        assertTrue(res.resolve("values/bools.xml").readText().contains("""<bool name="vela_fragment_links">false</bool>"""))
        assertTrue(res.resolve("values-v35/bools.xml").readText().contains("""<bool name="vela_fragment_links">true</bool>"""))
        val filter = alias.children("intent-filter").single()
        assertEquals("true", filter.getAttributeNS(ns, "autoVerify"))
        val groups = filter.children("uri-relative-filter-group")
        assertEquals(4, groups.size)
        val pages = mutableSetOf<String>()
        groups.forEach { group ->
            val data = group.children("data")
            assertTrue("every group names a fragment", data.any { it.getAttributeNS(ns, "fragmentPrefix") in setOf("ms=", "msa=") })
            pages += data.mapNotNull { it.getAttributeNS(ns, "pathAdvancedPattern").takeIf(String::isNotEmpty) }
        }
        // The two pages the classifier reads: a locale's wallet page and its front page.
        val wallet = Regex(pages.single { it.endsWith("/wallet") })
        val front = Regex(pages.single { !it.endsWith("/wallet") })
        assertTrue(wallet.matches("/en/wallet") && wallet.matches("/pt-BR/wallet") && !wallet.matches("/en/connect"))
        assertTrue(front.matches("/zh") && front.matches("/zh-TW") && !front.matches("/en/wallet"))
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
        val PATH_ATTRS = listOf("path", "pathPrefix", "pathPattern", "pathAdvancedPattern", "pathSuffix")
        const val RELEASE_KEY = "7D:74:AF:4A:A4:78:CD:B1:C4:02:01:7F:7D:12:7A:99:46:5E:DF:93:25:40:73:0F:5A:19:EE:4D:B9:BA:C8:8B"
    }
}
