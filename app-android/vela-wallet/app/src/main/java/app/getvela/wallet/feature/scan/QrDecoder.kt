package app.getvela.wallet.feature.scan

import android.graphics.Bitmap
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.LuminanceSource
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import uniffi.vela_core_uniffi.stillQrSizes

/**
 * ZXing over one luminance source (spec 046 D4): the same decoder for a
 * camera frame and a picked photo, so what the lens reads and what a photo
 * reads can never differ. `null` = no QR in this image.
 */
object QrDecoder {
    private val hints = mapOf(
        DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
        DecodeHintType.TRY_HARDER to true,
    )

    fun decode(source: LuminanceSource): String? {
        val reader = MultiFormatReader().apply { setHints(hints) }
        return try {
            reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text
        } catch (_: NotFoundException) {
            // Rotated codes: a second try on the transposed frame.
            runCatching { reader.decodeWithState(BinaryBitmap(HybridBinarizer(source.rotateCounterClockwise()))).text }.getOrNull()
        } catch (_: Exception) {
            null
        } finally {
            reader.reset()
        }
    }

    /** A camera frame's Y plane (YUV_420_888) is already luminance. */
    fun decodeYuv(y: ByteArray, width: Int, height: Int, rowStride: Int): String? {
        val packed = if (rowStride == width) y else ByteArray(width * height).also { out ->
            for (row in 0 until height) System.arraycopy(y, row * rowStride, out, row * width, width)
        }
        return decode(PlanarYUVLuminanceSource(packed, width, height, 0, 0, width, height, false))
    }

    /** A picked photo: [decodeStill] over its pixels. */
    fun decode(bitmap: Bitmap): String? {
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        return decodeStill(pixels, bitmap.width, bitmap.height)
    }

    /**
     * A still image (spec 090): as it is, then at each smaller size the core's
     * ladder names (`stillQrSizes`: longest side 1024, 640, 400), first hit wins.
     *
     * A screenshot of Vela's own receive code draws each module ~24 px across,
     * wider than [HybridBinarizer]'s local window, and came back "no code" from
     * the album. Shrinking puts the edges back inside the window. Camera frames
     * keep [decodeYuv]: they arrive at the size the analyser asked for.
     */
    fun decodeStill(pixels: IntArray, width: Int, height: Int): String? {
        for (size in stillQrSizes(width.toUInt(), height.toUInt())) {
            val w = size.width.toInt()
            val h = size.height.toInt()
            val scaled = if (w == width && h == height) pixels else downscale(pixels, width, height, w, h)
            decode(RGBLuminanceSource(w, h, scaled))?.let { return it }
        }
        return null
    }

    /**
     * Area-average downscale: every source pixel counts toward the one it lands
     * in, which is the low-pass filter the ladder relies on. Pure Kotlin, so a
     * JVM test runs exactly what the phone runs.
     */
    internal fun downscale(src: IntArray, sw: Int, sh: Int, dw: Int, dh: Int): IntArray {
        val out = IntArray(dw * dh)
        for (dy in 0 until dh) {
            val y0 = dy * sh / dh
            val y1 = maxOf(y0 + 1, (dy + 1) * sh / dh)
            for (dx in 0 until dw) {
                val x0 = dx * sw / dw
                val x1 = maxOf(x0 + 1, (dx + 1) * sw / dw)
                var r = 0
                var g = 0
                var b = 0
                for (y in y0 until y1) {
                    val row = y * sw
                    for (x in x0 until x1) {
                        val p = src[row + x]
                        r += (p shr 16) and 0xFF
                        g += (p shr 8) and 0xFF
                        b += p and 0xFF
                    }
                }
                val n = (y1 - y0) * (x1 - x0)
                out[dy * dw + dx] = (0xFF shl 24) or ((r / n) shl 16) or ((g / n) shl 8) or (b / n)
            }
        }
        return out
    }
}
