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

    fun decode(bitmap: Bitmap): String? {
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        return decode(RGBLuminanceSource(bitmap.width, bitmap.height, pixels))
    }
}
