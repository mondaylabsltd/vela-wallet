package app.getvela.wallet

import app.getvela.wallet.feature.scan.ScanCamera
import app.getvela.wallet.navigation.DEVELOPER_ROUTES
import app.getvela.wallet.navigation.VelaDestinations
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * PRIVACY — the integration's note 7: in a gallery, a board, a preview or a
 * developer route the scanner draws a fixture frame and NEVER opens a camera.
 * A sweep of the desktop's gallery bound the real camera on its scanner state
 * and photographed the person at the machine.
 *
 * Three things hold that here, and each is checked:
 *
 * 1. which launches are fixture sessions is one pure rule
 *    ([ScanCamera.isFixtureSession]) over the extras a build honours — and a
 *    release build honours none of them;
 * 2. the camera is started in ONE file, and nowhere else in the app;
 * 3. in that file, the gate comes before any of the camera API.
 *
 * (That a fixture session really draws the fixture frame and reaches no
 * camera start is the instrumented `ScanFixtureFrameTest`.)
 */
class ScanCameraGateTest {
    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val sources = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")

    @Test
    fun `a gallery, a board or a developer route is a fixture session, and the real routes are not`() {
        for (route in DEVELOPER_ROUTES) assertTrue(route, ScanCamera.isFixtureSession(route, gallery = false))
        // Every gallery by name, so a new one cannot be added without being one.
        for (route in VelaDestinations.ALL.filter { it.endsWith("gallery") }) assertTrue(route, route in DEVELOPER_ROUTES)
        // The onboarding gallery replaces the nav host altogether (`vela.gallery`).
        assertTrue(ScanCamera.isFixtureSession(VelaDestinations.WELCOME, gallery = true))
        // The app itself: a person's own scanner opens their own camera.
        for (route in VelaDestinations.ALL - DEVELOPER_ROUTES) assertFalse(route, ScanCamera.isFixtureSession(route, gallery = false))
        assertEquals(setOf(VelaDestinations.WELCOME, VelaDestinations.CREATE, VelaDestinations.WALLET, VelaDestinations.SETTINGS), VelaDestinations.ALL - DEVELOPER_ROUTES)
    }

    @Test
    fun `a release build is never a fixture session - it honours no developer extra`() {
        for (route in DEVELOPER_ROUTES) {
            val honoured = LaunchExtras.startDestination(route, debug = false)
            assertEquals(VelaDestinations.WELCOME, honoured)
            val gallery = LaunchExtras.honoured("vela.gallery", true, debug = false) == true
            assertFalse(ScanCamera.isFixtureSession(honoured, gallery))
        }
        // …and a debug build's pinned gallery is one.
        assertTrue(ScanCamera.isFixtureSession(LaunchExtras.startDestination(VelaDestinations.FLOWS_GALLERY, debug = true), gallery = false))
    }

    @Test
    fun `a preview draws the fixture too`() {
        val before = ScanCamera.fixtureSession
        try {
            ScanCamera.fixtureSession = false
            assertTrue("Compose's inspection mode", ScanCamera.fixtureOnly(inspection = true))
            assertFalse(ScanCamera.fixtureOnly(inspection = false))
            ScanCamera.fixtureSession = true
            assertTrue(ScanCamera.fixtureOnly(inspection = false))
        } finally {
            ScanCamera.fixtureSession = before
        }
    }

    /** The camera API's own entry points: the provider, and the bind that turns a lens on. */
    private val starters = listOf("ProcessCameraProvider", "bindToLifecycle", "CameraX.", "Camera2", "camera2", "android.hardware.Camera")

    @Test
    fun `one file starts a camera, and it asks the gate first`() {
        val naming = sources.walkTopDown().filter { it.isFile && it.extension == "kt" }
            .filter { file -> file.readLines().any { line -> !line.trimStart().startsWith("*") && !line.trimStart().startsWith("//") && starters.any(line::contains) } }
            .map { it.relativeTo(sources).path }.toList()
        assertEquals("a second place that can start a camera is a second place the gate must be", listOf("feature/scan/CameraScanner.kt"), naming)

        val scanner = File(sources, "feature/scan/CameraScanner.kt").readText()
        val body = scanner.substringAfter("fun CameraScanner(")
        val gate = body.indexOf("ScanCamera.fixtureOnly(")
        val leaves = body.indexOf("return", gate)
        val firstCameraCall = starters.map { body.indexOf(it) }.filter { it >= 0 }.min()
        assertTrue("the gate is in the scanner", gate >= 0)
        assertTrue("the fixture frame is what it draws instead", body.substring(gate, leaves).contains("ScanFixtureFrame("))
        assertTrue("…and it leaves before the first camera call", leaves in (gate + 1) until firstCameraCall)
        // The start is counted where it happens, so the instrumented test can show it is not reached.
        val counted = body.indexOf("ScanCamera.starts.incrementAndGet()")
        assertTrue(counted in (leaves + 1) until body.indexOf("ProcessCameraProvider.getInstance("))
    }
}
