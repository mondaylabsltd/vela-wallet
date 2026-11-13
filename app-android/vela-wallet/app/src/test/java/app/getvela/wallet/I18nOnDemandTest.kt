package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.I18n
import uniffi.vela_core_uniffi.TOptions

/**
 * P2.6: the on-demand namespaces (`assets/i18n/<name>/<lng>.json`, gen-i18n's
 * `ON_DEMAND`) reach this app, and merged the web's way.
 *
 * The last test is `scripts/verify-i18n-parity.mjs` carried to Android: the
 * web proves its deep-merged assets render every corpus key as i18next does;
 * this proves Android's own reading of the same files — the main catalog, plus
 * each namespace merged in by [I18nRuntime.include] — renders every corpus key
 * as an engine loaded with the corpus itself, in all 15 languages. A key the
 * Android catalogs leave out, or a merge that drops a sibling, fails it.
 */
class I18nOnDemandTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )
    private val assets = File(repoRoot, "assets/i18n")
    private val corpus = File(repoRoot, "rust/crates/vela-core/i18n/locales")

    private val locales: List<String> =
        assets.listFiles { f -> f.isFile && f.name.endsWith(".json") }!!.map { it.name.removeSuffix(".json") }.sorted()

    /** The app's reader, on the repo's files: the same layout `syncVelaI18nAssets` packages. */
    private fun runtime(): I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag ->
            File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes()
        },
    ) { tag -> File(assets, "$tag.json").readBytes() }

    private val sample = "home.multisig.switcher.section"

    private fun multisigValue(tag: String): String =
        JSONObject(File(assets, "multisig/$tag.json").readText())
            .getJSONObject("home").getJSONObject("multisig").getJSONObject("switcher").getString("section")

    @Test
    fun theNamespacesTheAppKnowsAreTheOnesGenI18nWrites() {
        val written = assets.listFiles { f -> f.isDirectory && f.listFiles { c -> c.name.endsWith(".json") }!!.isNotEmpty() }!!
            .map { it.name }.sorted()
        assertEquals(written, I18nCatalogs.ON_DEMAND.sorted())
        for (name in written) {
            for (tag in locales) assertTrue("$name/$tag.json", File(assets, "$name/$tag.json").isFile)
        }
    }

    @Test
    fun aNamespaceIsNotResidentUntilAScreenAsksForIt() {
        val strings = runtime().apply { initialize("en") }
        assertEquals("the main catalog holds no home.multisig", sample, strings.t(sample))
        assertTrue(strings.state.value.namespaces.isEmpty())
        strings.include(I18nCatalogs.MULTISIG)
        assertEquals(multisigValue("en"), strings.t(sample))
        assertEquals(setOf(I18nCatalogs.MULTISIG), strings.state.value.namespaces)
        // The rest of `home` is still there: merged, not replaced.
        assertNotEquals("home.totalBalance", strings.t("home.totalBalance"))
    }

    @Test
    fun anIncludedNamespaceFollowsALanguageChange() {
        val strings = runtime().apply { initialize("en") }
        strings.include(I18nCatalogs.MULTISIG)
        strings.setLocale("zh")
        assertEquals(multisigValue("zh"), strings.t(sample))
        strings.setLocale("ja")
        assertEquals(multisigValue("ja"), strings.t(sample))
    }

    /**
     * r10-2: the per-app language changed while the app runs, zh-CN → en-US
     * (Settings → App languages): every multisig word was a raw key until a
     * restart. The merged English had gone into the core's active slot, not
     * its pinned fallback, so a switch back to English read the fallback
     * without the namespace.
     */
    @Test
    fun anIncludedNamespaceSurvivesASwitchBackToEnglish() {
        val fromChinese = runtime().apply { initialize("zh") }
        fromChinese.include(I18nCatalogs.MULTISIG)
        assertEquals(multisigValue("zh"), fromChinese.t(sample))
        fromChinese.setLocale("en")
        assertEquals(multisigValue("en"), fromChinese.t(sample))
        assertEquals(multisigValue("en"), fromChinese.tMany(listOf(sample)).single())
        assertEquals(multisigValue("en"), fromChinese.t(sample, mapOf("x" to "y")))

        val roundTrip = runtime().apply { initialize("en") }
        roundTrip.include(I18nCatalogs.MULTISIG)
        roundTrip.setLocale("zh")
        roundTrip.setLocale("en")
        assertEquals(multisigValue("en"), roundTrip.t(sample))
    }

    /** Phase 4: plain lookups are remembered per catalog state, never across a change of it. */
    @Test
    fun aRememberedWordFollowsALanguageChange() {
        val strings = runtime().apply { initialize("en") }
        val key = "home.totalBalance"
        val english = strings.t(key)
        assertEquals(english, strings.t(key))
        strings.setLocale("de")
        val german = strings.t(key)
        assertNotEquals(english, german)
        assertEquals(german, strings.t(key))
        strings.setLocale("en")
        assertEquals(english, strings.t(key))
    }

    /**
     * core-perf-1: many keys in one ask answer exactly as `t` does, one by
     * one — plain keys, keys nothing resolves, a branch, and a key with vars —
     * and follow a language change.
     */
    @Test
    fun manyKeysAtOnceAnswerAsOneAtATime() {
        val strings = runtime().apply { initialize("en") }
        val keys = listOf("home.totalBalance", "no.such.key", "home", "home.totalBalance", sample)
        for (tag in listOf("en", "zh", "de")) {
            strings.setLocale(tag)
            assertEquals(tag, keys.map(strings::t), strings.tMany(keys))
        }
        val vars = mapOf("ago" to "5m")
        val withVars = listOf("home.lastUpdated", "home.totalBalance")
        assertEquals(withVars.map { strings.t(it, vars) }, strings.tMany(withVars, vars))
        assertEquals(emptyList<String>(), strings.tMany(emptyList()))
    }

    @Test
    fun aNamespaceAskedForBeforeTheEngineIsBuiltLoadsWithIt() {
        val strings = runtime()
        strings.include(I18nCatalogs.MULTISIG)
        strings.initialize("de")
        assertEquals(multisigValue("de"), strings.t(sample))
    }

    @Test
    fun includingTwiceIsANoOp() {
        val strings = runtime().apply { initialize("fr") }
        strings.include(I18nCatalogs.MULTISIG)
        val before = strings.state.value
        strings.include(I18nCatalogs.MULTISIG)
        assertTrue(before === strings.state.value)
    }

    @Test
    fun theMergeKeepsSiblingsAndLetsTheNamespaceWin() {
        val merged = I18nCatalogs.merged(
            """{"home":{"title":"T","multisig":{"a":"old"}},"common":{"ok":"OK"}}""".toByteArray(),
            listOf("""{"home":{"multisig":{"a":"A","b":{"c":"C"}}}}""".toByteArray()),
        )
        val doc = JSONObject(String(merged))
        assertEquals("T", doc.getJSONObject("home").getString("title"))
        assertEquals("OK", doc.getJSONObject("common").getString("ok"))
        assertEquals("A", doc.getJSONObject("home").getJSONObject("multisig").getString("a"))
        assertEquals("C", doc.getJSONObject("home").getJSONObject("multisig").getJSONObject("b").getString("c"))
        val same = "{}".toByteArray()
        assertTrue("no parts: the main catalog as it is", I18nCatalogs.merged(same, emptyList()) === same)
    }

    // -- verify-i18n-parity, Android's reading ---------------------------------

    /** The corpus for a language, merged the way verify-i18n-parity.mjs merges it (namespace files over the root). */
    private fun corpusDoc(tag: String): JSONObject {
        val out = JSONObject(File(corpus, "$tag.json").readText())
        File(corpus, tag).listFiles { f -> f.name.endsWith(".json") }!!.sortedBy { it.name }.forEach { file ->
            val ns = JSONObject(file.readText())
            for (key in ns.keys()) out.put(key, ns.get(key))
        }
        return out
    }

    private fun leaves(node: JSONObject, prefix: String, out: MutableList<String>): MutableList<String> {
        for (key in node.keys()) {
            val path = if (prefix.isEmpty()) key else "$prefix.$key"
            val value = node.get(key)
            if (value is JSONObject) leaves(value, path, out) else out += path
        }
        return out
    }

    private val noOptions = TOptions(count = null, context = null, defaultValue = null, lng = null, ordinal = false, vars = emptyList())

    @Test
    fun everyCorpusKeyRendersOnAndroidAsTheCorpusSaysInEveryLanguage() {
        val enCorpus = corpusDoc("en").toString().toByteArray()
        var compared = 0
        val divergences = mutableListOf<String>()
        for (tag in locales) {
            val android = runtime().apply {
                include(I18nCatalogs.MULTISIG)
                initialize(tag)
            }
            val doc = corpusDoc(tag)
            val reference = I18n(enCorpus).apply {
                if (tag != "en") loadCatalog(tag, doc.toString().toByteArray())
                changeLanguage(tag)
            }
            for (key in leaves(doc, "", mutableListOf())) {
                compared++
                val want = reference.t(key, noOptions)
                val got = android.t(key)
                if (want != got && divergences.size < 25) divergences += "$tag::$key\n  corpus:  $want\n  android: $got"
            }
        }
        assertTrue("compared $compared", compared > 15 * 2000)
        assertTrue("${divergences.size} divergence(s):\n" + divergences.joinToString("\n"), divergences.isEmpty())
    }
}
