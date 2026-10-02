package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.scan.QrDecoder
import app.getvela.wallet.feature.wallet.core.PaymentRequestEvent
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import com.google.zxing.BarcodeFormat
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.qrcode.QRCodeWriter
import javax.imageio.ImageIO
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.PaymentRequestCore
import uniffi.vela_core_uniffi.stillQrSizes

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

    /** The receive code's value, from the real core: the bare address, or with the switch on, the URI for Polygon. */
    private fun coreCode(includeNetwork: Boolean): String {
        val core = PaymentRequestCore()
        fun send(event: PaymentRequestEvent): PaymentRequestView {
            val out = core.dispatch(Wire.json.encodeToString(PaymentRequestEvent.serializer(), event))
            val view = Wire.json.parseToJsonElement(out).jsonObject["view"] as JsonObject
            return Wire.json.decodeFromJsonElement(PaymentRequestView.serializer(), view)
        }
        val me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        send(PaymentRequestEvent.Start(account = me, recipient = me, base_url = "https://getvela.app/pay"))
        send(PaymentRequestEvent.AssetPicked(chain_id = 137, token_address = null, symbol = "POL", decimals = 18, network_name = "Polygon"))
        return send(PaymentRequestEvent.IncludeNetworkChanged(includeNetwork)).qr_value
    }

    /**
     * Spec 090: Vela reads its OWN receive code from a picture of an Android
     * screen — the code cropped out of a 1080×2400 screenshot, as a person
     * would pick it from the album after it went through a chat.
     *
     * Without the ladder this fails: each module is ~24 px across, wider than
     * HybridBinarizer's local window, and ZXing finds nothing in the image as
     * it is (asserted below). The core's ladder shrinks it until it reads.
     */
    @Test
    fun `vela reads its own receive code from a screenshot`() {
        for ((file, includeNetwork) in listOf("receive-code-on.png" to true, "receive-code-off.png" to false)) {
            val image = ImageIO.read(javaClass.getResource("/qr/$file"))
            val pixels = image.getRGB(0, 0, image.width, image.height, null, 0, image.width)
            assertNull("$file reads as it is — the ladder is no longer what makes it work", QrDecoder.decode(RGBLuminanceSource(image.width, image.height, pixels)))
            assertEquals(file, coreCode(includeNetwork), QrDecoder.decodeStill(pixels, image.width, image.height))
        }
    }

    /** The ladder is the core's: as is, then each smaller rung, never enlarged. */
    @Test
    fun `the still ladder comes from the core`() {
        assertEquals(
            listOf(1080 to 2400, 461 to 1024, 288 to 640, 180 to 400),
            stillQrSizes(1080u, 2400u).map { it.width.toInt() to it.height.toInt() },
        )
        assertEquals(listOf(300 to 300), stillQrSizes(300u, 300u).map { it.width.toInt() to it.height.toInt() })
    }

    /** A shrink averages: a 2×2 block of black and white becomes one grey pixel. */
    @Test
    fun `the downscale averages every source pixel`() {
        val black = 0xFF000000.toInt()
        val white = 0xFFFFFFFF.toInt()
        val out = QrDecoder.downscale(intArrayOf(black, white, white, black), 2, 2, 1, 1)
        assertEquals(0xFF7F7F7F.toInt(), out.single())
    }
}
