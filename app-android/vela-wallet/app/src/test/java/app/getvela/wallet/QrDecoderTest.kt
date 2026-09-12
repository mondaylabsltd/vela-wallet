package app.getvela.wallet

import app.getvela.wallet.feature.scan.QrDecoder
import com.google.zxing.BarcodeFormat
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.qrcode.QRCodeWriter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The same decoder for a frame and a photo (spec 046 D4): a code written by ZXing reads back through it. */
class QrDecoderTest {
    private fun pixelsOf(text: String, size: Int): IntArray {
        val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size)
        return IntArray(size * size) { i -> if (matrix.get(i % size, i / size)) 0xFF000000.toInt() else 0xFFFFFFFF.toInt() }
    }

    @Test
    fun `a payment request round-trips through the decoder`() {
        val text = "ethereum:0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141@100?value=1000000000000000"
        val size = 240
        val pixels = pixelsOf(text, size)
        assertEquals(text, QrDecoder.decode(RGBLuminanceSource(size, size, pixels)))
        val luminance = ByteArray(size * size) { i -> if (pixels[i] == 0xFF000000.toInt()) 0 else 0xFF.toByte() }
        assertEquals(text, QrDecoder.decodeYuv(luminance, size, size, size))
    }

    @Test
    fun `a blank image is no code`() {
        val size = 120
        assertNull(QrDecoder.decode(RGBLuminanceSource(size, size, IntArray(size * size) { 0xFFFFFFFF.toInt() })))
    }
}
