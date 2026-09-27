package app.getvela.wallet

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.diagnostics.JpegFacts
import app.getvela.wallet.core.diagnostics.ScreenshotPrep
import java.io.ByteArrayOutputStream
import java.util.Base64
import kotlin.math.max
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The report's screenshots through the REAL Android codecs (spec 078 round 3):
 * what leaves the device is a JPEG, with no EXIF, never longer than 1920 on
 * its longest edge, in tile order — the privacy guarantee is the re-encode,
 * so it is proved on the codec that does it, not on a stand-in.
 *
 * ```bash
 * adb shell am instrument -w -e class app.getvela.wallet.ScreenshotPrepDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ScreenshotPrepDeviceTest {

    /** A picture with detail in it (JPEG of a flat colour is trivially small). */
    private fun picture(width: Int, height: Int): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(Color.WHITE)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        for (i in 0 until 40) {
            paint.color = Color.rgb((i * 37) % 255, (i * 91) % 255, (i * 53) % 255)
            canvas.drawCircle((i * 97 % width).toFloat(), (i * 61 % height).toFloat(), (20 + i * 7).toFloat(), paint)
        }
        return bitmap
    }

    private fun encode(bitmap: Bitmap, format: Bitmap.CompressFormat, quality: Int = 95): ByteArray =
        ByteArrayOutputStream().use { out -> bitmap.compress(format, quality, out); out.toByteArray() }

    /**
     * [jpeg] with a camera-style EXIF block spliced in after SOI: big-endian
     * TIFF, IFD0 with Orientation (0x0112) = [orientation] and a Make string.
     */
    private fun withExif(jpeg: ByteArray, orientation: Int): ByteArray {
        val make = "VelaCam\u0000".toByteArray()
        val tiff = java.nio.ByteBuffer.allocate(8 + 2 + 12 * 2 + 4 + make.size).apply {
            put("MM".toByteArray()); putShort(42); putInt(8)
            putShort(2)
            // Make (0x010F), ASCII, count, offset → after the IFD.
            putShort(0x010F); putShort(2); putInt(make.size); putInt(8 + 2 + 24 + 4)
            // Orientation (0x0112), SHORT, 1, value in the first two bytes.
            putShort(0x0112); putShort(3); putInt(1); putShort(orientation.toShort()); putShort(0)
            putInt(0)
            put(make)
        }.array()
        val payload = "Exif".toByteArray() + byteArrayOf(0, 0) + tiff
        val length = payload.size + 2
        val segment = byteArrayOf(0xFF.toByte(), 0xE1.toByte(), (length shr 8).toByte(), length.toByte()) + payload
        return jpeg.copyOfRange(0, 2) + segment + jpeg.copyOfRange(2, jpeg.size)
    }

    private fun assertSendable(prepared: ScreenshotPrep.Prepared?, width: Int, height: Int) {
        assertNotNull(prepared)
        prepared!!
        assertTrue("FF D8 FF", JpegFacts.isJpeg(prepared.jpeg))
        assertFalse("no EXIF APP1 segment", JpegFacts.hasExif(prepared.jpeg))
        assertEquals(width to height, JpegFacts.size(prepared.jpeg))
        assertEquals(width, prepared.width)
        assertEquals(height, prepared.height)
        assertTrue(max(width, height) <= ScreenshotPrep.MAX_EDGE)
        assertTrue(prepared.jpeg.size <= BugReport.MAX_SCREENSHOT_BYTES)
    }

    @Test
    fun aLargePngComesOutAJpegAt1920() {
        val prepared = ScreenshotPrep.prepare(encode(picture(3000, 2000), Bitmap.CompressFormat.PNG))
        assertSendable(prepared, 1920, 1280)
    }

    @Test
    fun aSmallImageIsReEncodedButNeverUpscaled() {
        // Even a PNG that is already small goes through the re-encode — that is the guarantee.
        val prepared = ScreenshotPrep.prepare(encode(picture(400, 300), Bitmap.CompressFormat.PNG))
        assertSendable(prepared, 400, 300)
    }

    @Test
    fun aCameraJpegLosesItsExifAndKeepsItsOrientation() {
        val source = withExif(encode(picture(2400, 1600), Bitmap.CompressFormat.JPEG), orientation = 6)
        assertTrue("the fixture carries EXIF", JpegFacts.hasExif(source))
        val prepared = ScreenshotPrep.prepare(source)
        // Orientation 6 = rotate 90°: the decoder turns it upright, THEN the tag is dropped.
        assertSendable(prepared, 1280, 1920)
        assertFalse(String(prepared!!.jpeg, Charsets.ISO_8859_1).contains("VelaCam"))
    }

    @Test
    fun aTransparentPngIsFlattenedNotBlackened() {
        // Clear everywhere but a dark square in the middle.
        val clear = Bitmap.createBitmap(200, 200, Bitmap.Config.ARGB_8888).apply {
            Canvas(this).drawRect(80f, 80f, 120f, 120f, Paint().apply { color = Color.BLACK })
        }
        val prepared = ScreenshotPrep.prepare(encode(clear, Bitmap.CompressFormat.PNG))
        assertSendable(prepared, 200, 200)
        val decoded = BitmapFactory.decodeByteArray(prepared!!.jpeg, 0, prepared.jpeg.size)
        val middle = decoded.getPixel(100, 100)
        assertTrue("the drawn part survives", Color.red(middle) < 40)
        val corner = decoded.getPixel(5, 5)
        assertTrue("clear became white, not black", Color.red(corner) > 240 && Color.green(corner) > 240 && Color.blue(corner) > 240)
    }

    @Test
    fun aWebpIsReadToo() {
        @Suppress("DEPRECATION")
        val prepared = ScreenshotPrep.prepare(encode(picture(640, 480), Bitmap.CompressFormat.WEBP, 90))
        assertSendable(prepared, 640, 480)
    }

    @Test
    fun somethingThatIsNotAnImageIsRefused() {
        assertNull(ScreenshotPrep.prepare("%PDF-1.7 not a picture".toByteArray()))
    }

    @Test
    fun thePayloadCarriesTheSentBytesInTileOrder() {
        val sources = listOf(
            encode(picture(3000, 2000), Bitmap.CompressFormat.PNG),
            withExif(encode(picture(1000, 800), Bitmap.CompressFormat.JPEG), orientation = 1),
            encode(picture(500, 2500), Bitmap.CompressFormat.PNG),
        )
        val prepared = sources.map { ScreenshotPrep.prepare(it, thumbnailPx = 144)!! }
        prepared.forEach { assertNotNull("a tile-sized thumbnail from the same bytes", it.thumbnail) }
        val labels = BugReport.EnvironmentLabels("v", "p", "l", "r", "f", "none")
        val facts = BugReport.DeviceFacts("1.0.0", "abc", "Android", "en", emptyList(), emptyList())
        val payload = BugReport.build("x", "", BugReport.AREA_OTHER, labels, facts, prepared.map { it.jpeg })
        val sent = payload.screenshots!!.map { Base64.getDecoder().decode(it) }
        assertEquals(3, sent.size)
        sent.forEachIndexed { i, bytes ->
            assertTrue("tile ${i + 1} is the prepared bytes", bytes.contentEquals(prepared[i].jpeg))
            assertTrue(JpegFacts.isJpeg(bytes))
            assertFalse(JpegFacts.hasExif(bytes))
            val (w, h) = JpegFacts.size(bytes)!!
            assertTrue(max(w, h) <= 1920)
        }
        assertEquals(1920 to 1280, JpegFacts.size(sent[0]))
        assertEquals(1000 to 800, JpegFacts.size(sent[1]))
        assertEquals(384 to 1920, JpegFacts.size(sent[2]))
    }
}
