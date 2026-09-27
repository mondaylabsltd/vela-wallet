package app.getvela.wallet.core.diagnostics

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ImageDecoder
import android.net.Uri
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.ByteBuffer
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * A picked image, made fit to send (spec 078 round 3): decoded, scaled so its
 * longest edge is at most [MAX_EDGE] (never up), and re-encoded as a JPEG at
 * quality [QUALITY].
 *
 * **The re-encode is the privacy guarantee, not an optimisation.** A photo
 * straight off a camera carries EXIF — the phone's model, the time, often the
 * place it was taken — and `Bitmap.compress` writes none of it. So every image
 * goes through here, even a PNG that is already small, and only the bytes that
 * come out are kept: the tile's thumbnail is decoded from them too, so what the
 * person looks at is exactly what is sent.
 *
 * `ImageDecoder` applies the EXIF orientation while decoding, so dropping the
 * tag afterwards never leaves a photo on its side; it also reads HEIC and WebP.
 *
 * Over the endpoint's 2 MB per image: quality [FALLBACK_QUALITY], then a
 * [FALLBACK_EDGE] edge. Blocking — call it off the main thread.
 */
object ScreenshotPrep {

    const val MAX_EDGE: Int = 1920
    const val FALLBACK_EDGE: Int = 1440
    const val QUALITY: Int = 85
    const val FALLBACK_QUALITY: Int = 70

    /** The bytes that will be sent, their size, and a tile-sized picture of those same bytes. */
    class Prepared(val jpeg: ByteArray, val width: Int, val height: Int, val thumbnail: Bitmap?)

    /** A picked `content://` image; null when it is not an image this device can decode. */
    fun prepare(resolver: ContentResolver, uri: Uri, thumbnailPx: Int): Prepared? {
        val type = runCatching { resolver.getType(uri) }.getOrNull()
        if (type != null && !type.startsWith("image/")) return null
        return prepare(ImageDecoder.createSource(resolver, uri), thumbnailPx)
    }

    /** Raw image bytes (the tests' road in). */
    fun prepare(bytes: ByteArray, thumbnailPx: Int = 0): Prepared? =
        prepare(ImageDecoder.createSource(ByteBuffer.wrap(bytes)), thumbnailPx)

    private fun prepare(source: ImageDecoder.Source, thumbnailPx: Int): Prepared? = try {
        var bitmap = flattened(decode(source, MAX_EDGE))
        var jpeg = encode(bitmap, QUALITY)
        if (jpeg.size > BugReport.MAX_SCREENSHOT_BYTES) jpeg = encode(bitmap, FALLBACK_QUALITY)
        if (jpeg.size > BugReport.MAX_SCREENSHOT_BYTES) {
            bitmap = scaledDown(bitmap, FALLBACK_EDGE)
            jpeg = encode(bitmap, FALLBACK_QUALITY)
        }
        val width = bitmap.width
        val height = bitmap.height
        bitmap.recycle()
        when {
            // Belt and braces on the one promise this file exists for.
            !JpegFacts.isJpeg(jpeg) || JpegFacts.hasExif(jpeg) -> null
            jpeg.size > BugReport.MAX_SCREENSHOT_BYTES -> null
            else -> Prepared(jpeg, width, height, if (thumbnailPx > 0) thumbnail(jpeg, thumbnailPx) else null)
        }
    } catch (fault: IOException) {
        // ImageDecoder.DecodeException is an IOException: not an image, or one this device cannot read.
        VelaLog.event("feedback", "screenshot unreadable", "error" to fault.javaClass.simpleName)
        null
    } catch (fault: IllegalArgumentException) {
        null
    } catch (fault: IllegalStateException) {
        null
    } catch (fault: SecurityException) {
        null
    } catch (fault: OutOfMemoryError) {
        VelaLog.event("feedback", "screenshot too large to decode")
        null
    }

    /** Decoded straight to at most [edge] on its longest side — the full-size image is never held. */
    private fun decode(source: ImageDecoder.Source, edge: Int): Bitmap = ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        val w = info.size.width
        val h = info.size.height
        val longest = max(w, h)
        if (longest > edge) {
            val scale = edge.toDouble() / longest
            decoder.setTargetSize(max(1, (w * scale).roundToInt()), max(1, (h * scale).roundToInt()))
        }
    }.let { scaledDown(it, edge) }

    /** A decoder that ignored its target size still never sends more than [edge]. */
    private fun scaledDown(bitmap: Bitmap, edge: Int): Bitmap {
        val longest = max(bitmap.width, bitmap.height)
        if (longest <= edge) return bitmap
        val scale = edge.toDouble() / longest
        val out = Bitmap.createScaledBitmap(bitmap, max(1, (bitmap.width * scale).roundToInt()), max(1, (bitmap.height * scale).roundToInt()), true)
        if (out !== bitmap) bitmap.recycle()
        return out
    }

    /** JPEG has no alpha: a transparent PNG would come out black where it was clear. */
    private fun flattened(bitmap: Bitmap): Bitmap {
        if (!bitmap.hasAlpha()) return bitmap
        val out = Bitmap.createBitmap(bitmap.width, bitmap.height, Bitmap.Config.ARGB_8888)
        Canvas(out).apply {
            drawColor(Color.WHITE)
            drawBitmap(bitmap, 0f, 0f, null)
        }
        bitmap.recycle()
        return out
    }

    private fun encode(bitmap: Bitmap, quality: Int): ByteArray =
        ByteArrayOutputStream().use { out ->
            check(bitmap.compress(Bitmap.CompressFormat.JPEG, quality, out)) { "jpeg encode refused" }
            out.toByteArray()
        }

    /** The tile's picture, from the bytes that will be sent; its SHORTER edge is [px] (the tile crops). */
    private fun thumbnail(jpeg: ByteArray, px: Int): Bitmap? = runCatching {
        ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(jpeg))) { decoder, info, _ ->
            val w = info.size.width
            val h = info.size.height
            val shortest = minOf(w, h)
            if (shortest > px) {
                val scale = px.toDouble() / shortest
                decoder.setTargetSize(max(1, (w * scale).roundToInt()), max(1, (h * scale).roundToInt()))
            }
        }
    }.getOrNull()
}

/**
 * What a JPEG's own markers say — no Android types, so the JVM tests and the
 * device tests read the sent bytes the same way.
 */
object JpegFacts {

    /** `FF D8 FF`: the start-of-image marker and the first segment's. */
    fun isJpeg(bytes: ByteArray): Boolean =
        bytes.size >= 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte() && bytes[2] == 0xFF.toByte()

    /** An APP1 segment that opens with `Exif\0\0` anywhere before the image data. */
    fun hasExif(bytes: ByteArray): Boolean = segments(bytes).any { (marker, start, length) ->
        marker == 0xE1 && length >= 6 &&
            String(bytes, start, 4, Charsets.ISO_8859_1) == "Exif" && bytes[start + 4] == 0.toByte() && bytes[start + 5] == 0.toByte()
    }

    /** Width × height from the first start-of-frame segment, or null. */
    fun size(bytes: ByteArray): Pair<Int, Int>? = segments(bytes)
        .firstOrNull { (marker, _, length) -> marker in SOF_MARKERS && length >= 5 }
        ?.let { (_, start, _) ->
            val height = (bytes[start + 1].toInt() and 0xFF shl 8) or (bytes[start + 2].toInt() and 0xFF)
            val width = (bytes[start + 3].toInt() and 0xFF shl 8) or (bytes[start + 4].toInt() and 0xFF)
            width to height
        }

    private val SOF_MARKERS = setOf(0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF)

    /** (marker, payload start, payload length) for every segment up to start-of-scan. */
    private fun segments(bytes: ByteArray): Sequence<Triple<Int, Int, Int>> = sequence {
        if (!isJpeg(bytes)) return@sequence
        var i = 2
        while (i + 4 <= bytes.size) {
            if (bytes[i] != 0xFF.toByte()) return@sequence
            val marker = bytes[i + 1].toInt() and 0xFF
            if (marker == 0xFF) { i += 1; continue }
            if (marker == 0xD9 || marker == 0xDA) return@sequence
            val length = (bytes[i + 2].toInt() and 0xFF shl 8) or (bytes[i + 3].toInt() and 0xFF)
            if (length < 2 || i + 2 + length > bytes.size) return@sequence
            yield(Triple(marker, i + 4, length - 2))
            i += 2 + length
        }
    }
}
