package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.JpegFacts
import app.getvela.wallet.core.diagnostics.ScreenshotTray
import app.getvela.wallet.core.diagnostics.ScreenshotTray.Notice
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The picker's rules (spec 078 round 3), without a picker: at most five, a
 * refusal that stands until the next change, a tile that could not be read
 * taken back out, and 发送 carrying the prepared ones in tile order.
 */
class ScreenshotTrayTest {
    /** Unconfined: a preparation runs at once and resumes where its gate is opened — no timing in these tests. */
    private val backgroundScope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = backgroundScope.cancel()

    @Test
    fun `picking beyond five takes the first that fit and says so`() {
        val tray = ScreenshotTray<String, String>(backgroundScope) { "jpeg:$it" }
        tray.add(listOf("a", "b", "c"))
        assertNull(tray.notice.value)
        assertEquals(2, tray.room)
        // The document picker a no-GMS phone falls back to does not honour a maximum.
        tray.add(listOf("d", "e", "f", "g"))
        assertEquals(Notice.Limit, tray.notice.value)
        assertEquals(listOf("jpeg:a", "jpeg:b", "jpeg:c", "jpeg:d", "jpeg:e"), tray.ready())
        assertEquals(0, tray.room)
        // Full: a further pick adds nothing and says so again.
        tray.add(listOf("h"))
        assertEquals(5, tray.tiles.value.size)
        assertEquals(Notice.Limit, tray.notice.value)
    }

    @Test
    fun `remove re-indexes and clears the refusal`() {
        val tray = ScreenshotTray<String, String>(backgroundScope) { it }
        tray.add(listOf("a", "b", "c", "d", "e", "f"))
        assertEquals(Notice.Limit, tray.notice.value)
        val second = tray.tiles.value[1].id
        tray.remove(second)
        assertNull("the next change clears it", tray.notice.value)
        assertEquals(listOf("a", "c", "d", "e"), tray.ready())
        assertEquals(1, tray.room)
    }

    @Test
    fun `an image that cannot be read is taken back out and named`() {
        val tray = ScreenshotTray<String, String>(backgroundScope) { if (it == "heic") null else it }
        tray.add(listOf("a", "heic", "b"))
        assertEquals(Notice.Unsupported, tray.notice.value)
        assertEquals(listOf("a", "b"), tray.ready())
        assertEquals(2, tray.tiles.value.size)
        tray.add(listOf("c"))
        assertNull(tray.notice.value)
    }

    @Test
    fun `send carries the prepared ones in tile order, whatever order they finish in`() {
        val gates = mapOf("a" to CompletableDeferred<Unit>(), "b" to CompletableDeferred(), "c" to CompletableDeferred())
        val tray = ScreenshotTray<String, String>(backgroundScope) { gates.getValue(it).await(); it }
        tray.add(listOf("a", "b", "c"))
        assertEquals("placeholders at once", 3, tray.tiles.value.size)
        assertTrue(tray.tiles.value.all { it.ready == null })
        assertEquals("a tile still preparing never blocks a send", emptyList<String>(), tray.ready())
        gates.getValue("c").complete(Unit)
        gates.getValue("a").complete(Unit)
        assertEquals(listOf("a", "c"), tray.ready())
        gates.getValue("b").complete(Unit)
        assertEquals(listOf("a", "b", "c"), tray.ready())
    }

    @Test
    fun `send waits for tiles still being prepared, and never drops one silently`() = runBlocking {
        val gates = mapOf("a" to CompletableDeferred<Unit>(), "b" to CompletableDeferred(), "c" to CompletableDeferred())
        val tray = ScreenshotTray<String, String>(backgroundScope) { gates.getValue(it).await(); it }
        tray.add(listOf("a", "b", "c"))
        gates.getValue("a").complete(Unit)
        assertTrue(tray.preparing)
        // 发送 pressed now: it must wait, not send just "a".
        val sent = async(Dispatchers.Unconfined) { tray.settled() }
        assertFalse("still waiting on b and c", sent.isCompleted)
        gates.getValue("c").complete(Unit)
        assertFalse("still waiting on b", sent.isCompleted)
        gates.getValue("b").complete(Unit)
        assertEquals(listOf("a", "b", "c"), sent.await())
        assertFalse(tray.preparing)
    }

    @Test
    fun `send waits, and a tile that fails becomes the refusal while the rest go`() = runBlocking {
        val gates = mapOf("a" to CompletableDeferred<Boolean>(), "heic" to CompletableDeferred(), "b" to CompletableDeferred())
        val tray = ScreenshotTray<String, String>(backgroundScope) { if (gates.getValue(it).await()) it else null }
        tray.add(listOf("a", "heic", "b"))
        val sent = async(Dispatchers.Unconfined) { tray.settled() }
        gates.getValue("a").complete(true)
        gates.getValue("b").complete(true)
        assertFalse("the failing one is still in flight", sent.isCompleted)
        gates.getValue("heic").complete(false)
        assertEquals(listOf("a", "b"), sent.await())
        assertEquals(Notice.Unsupported, tray.notice.value)
        assertEquals(2, tray.tiles.value.size)
    }

    @Test
    fun `send with nothing in flight goes at once`() = runBlocking {
        val tray = ScreenshotTray<String, String>(backgroundScope) { it }
        assertEquals(emptyList<String>(), tray.settled())
        tray.add(listOf("a", "b"))
        assertEquals(listOf("a", "b"), tray.settled())
    }

    @Test
    fun `a tile removed while preparing stays removed`() {
        val gate = CompletableDeferred<Unit>()
        val tray = ScreenshotTray<String, String>(backgroundScope) { gate.await(); it }
        tray.add(listOf("a"))
        tray.remove(tray.tiles.value.single().id)
        gate.complete(Unit)
        assertTrue(tray.tiles.value.isEmpty())
        assertEquals(emptyList<String>(), tray.ready())
    }

    // --- Reading a JPEG's markers (the same reader the device test uses) ---

    private fun jpeg(width: Int, height: Int): ByteArray {
        val image = BufferedImage(width, height, BufferedImage.TYPE_INT_RGB)
        return ByteArrayOutputStream().use { out -> ImageIO.write(image, "jpg", out); out.toByteArray() }
    }

    /** [bytes] with an APP1 `Exif` segment spliced in straight after SOI. */
    private fun withExif(bytes: ByteArray): ByteArray {
        val payload = "Exif".toByteArray() + byteArrayOf(0, 0) + "MM".toByteArray() + byteArrayOf(0, 42, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0)
        val length = payload.size + 2
        val segment = byteArrayOf(0xFF.toByte(), 0xE1.toByte(), (length shr 8).toByte(), length.toByte()) + payload
        return bytes.copyOfRange(0, 2) + segment + bytes.copyOfRange(2, bytes.size)
    }

    @Test
    fun `jpeg facts read the start marker, exif and the frame size`() {
        val plain = jpeg(320, 200)
        assertTrue(JpegFacts.isJpeg(plain))
        assertFalse(JpegFacts.hasExif(plain))
        assertEquals(320 to 200, JpegFacts.size(plain))
        val tagged = withExif(plain)
        assertTrue(JpegFacts.hasExif(tagged))
        assertEquals(320 to 200, JpegFacts.size(tagged))
        assertFalse(JpegFacts.isJpeg(byteArrayOf(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte())))
    }
}
