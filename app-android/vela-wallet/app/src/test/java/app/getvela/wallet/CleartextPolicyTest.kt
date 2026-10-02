package app.getvela.wallet

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Who may speak plain http (spec 091; owner ruling 2026-10-02: "正式版应该都是
 * 禁止的吧，只有调试开发的时候能就行").
 *
 * - **Release** carries no network security config and targets SDK ≥ 28, so
 *   the platform refuses every cleartext connection — the app's own and the
 *   in-app browser's. Nothing in `main` or `release` may grant any.
 * - **Debug** may: Settings' hidden debug mode (developer builds only) offers
 *   the wallet to an http page on the developer's own network, and that page
 *   has to load first. A config cannot name IP ranges, so its base allows
 *   cleartext; which pages get the wallet stays the core's rule. The loopback
 *   entries for the cable-served test dApp (spec 044) stay.
 *
 * Read from the source sets, as the build merges them: a release config added
 * in `main` would reach the store build, and this fails first.
 */
class CleartextPolicyTest {

    private val app: File by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        File(root, "app-android/vela-wallet/app")
    }

    @Test
    fun `a release build grants no cleartext`() {
        val manifest = File(app, "src/main/AndroidManifest.xml").readText()
        assertFalse("main manifest names a network security config", manifest.contains("networkSecurityConfig"))
        assertFalse("main manifest allows cleartext", Regex("""usesCleartextTraffic\s*=\s*"true"""").containsMatchIn(manifest))
        assertFalse(
            "a network security config in main would reach the release build",
            File(app, "src/main/res/xml/network_security_config.xml").exists(),
        )
        val release = File(app, "src/release")
        val granting = release.walkTopDown()
            .filter { it.isFile && (it.extension == "xml") }
            .filter { it.readText().let { text -> text.contains("cleartextTrafficPermitted") || text.contains("networkSecurityConfig") || text.contains("usesCleartextTraffic") } }
            .map { it.relativeTo(app).path }
            .toList()
        assertEquals("release source set grants cleartext", emptyList<String>(), granting)
        // Without a config, cleartext is off only from targetSdk 28.
        val target = Regex("""targetSdk\s*=\s*(\d+)""").find(File(app, "build.gradle.kts").readText())
            ?.groupValues?.get(1)?.toInt()
        assertTrue("targetSdk $target — cleartext is on by default below 28", (target ?: 0) >= 28)
    }

    @Test
    fun `a debug build allows cleartext, the loopback test dApp included`() {
        val manifest = File(app, "src/debug/AndroidManifest.xml").readText()
        assertTrue(manifest.contains("""android:networkSecurityConfig="@xml/network_security_config""""))
        val config = File(app, "src/debug/res/xml/network_security_config.xml").readText()
        assertTrue(
            "debug base config allows cleartext (a LAN http page for debug mode)",
            Regex("""<base-config\s+cleartextTrafficPermitted="true"""").containsMatchIn(config),
        )
        for (host in listOf("127.0.0.1", "localhost")) {
            assertTrue("$host stays allowed", config.contains(""">$host</domain>"""))
        }
    }
}
