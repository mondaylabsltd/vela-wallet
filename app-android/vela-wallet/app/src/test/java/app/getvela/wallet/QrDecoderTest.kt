package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.scan.QrDecoder
import app.getvela.wallet.feature.wallet.core.PaymentRequestEvent
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import javax.imageio.ImageIO
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.PaymentRequestCore
import uniffi.vela_core_uniffi.qrMatrix

/**
 * The scanner's decoder (vela-qr over JNA, the host build of libvela_qr) reads
 * back what the core's encoder wrote — one decoder for a frame and a photo
 * (spec 046 D4), independent of the encoder it checks.
 */
class QrDecoderTest {
    private val black = 0xFF000000.toInt()
    private val white = 0xFFFFFFFF.toInt()

    /** The core's code for `text`, drawn `scale` px a module with a four-module quiet zone. */
    private fun pixelsOf(text: String, scale: Int = 6, quiet: Int = 4): Pair<IntArray, Int> {
        val code = qrMatrix(text)!!
        val n = code.width.toInt()
        val side = (n + 2 * quiet) * scale
        val pixels = IntArray(side * side) { white }
        for (y in 0 until n) for (x in 0 until n) {
            if (!code.modules[y * n + x]) continue
            for (dy in 0 until scale) for (dx in 0 until scale) {
                pixels[((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx] = black
            }
        }
        return pixels to side
    }

    @Test
    fun `a payment request round-trips through the decoder`() {
        val text = "ethereum:0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141@100?value=1000000000000000"
        val (pixels, side) = pixelsOf(text)
        assertEquals(text, QrDecoder.decodeStill(pixels, side, side))
        // A camera frame's Y plane, rows padded past the width as CameraX hands them over.
        val stride = side + 16
        val luminance = QrDecoder.luminance(pixels)
        val padded = ByteArray(stride * side).also { out -> for (row in 0 until side) System.arraycopy(luminance, row * side, out, row * stride, side) }
        assertEquals(text, QrDecoder.decodeYuv(padded, side, side, stride))
    }

    @Test
    fun `a code shown light-on-dark reads`() {
        val address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val (pixels, side) = pixelsOf(address)
        val inverted = IntArray(pixels.size) { i -> if (pixels[i] == black) white else black }
        assertEquals(address, QrDecoder.decodeStill(inverted, side, side))
    }

    @Test
    fun `a photo wider than 1200 is read after averaging it down`() {
        val address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val (code, side) = pixelsOf(address, scale = 30)
        val (w, h) = 3024 to 4032
        val photo = IntArray(w * h) { white }
        for (y in 0 until side) System.arraycopy(code, y * side, photo, (1200 + y) * w + 600, side)
        assertEquals(address, QrDecoder.decodeStill(photo, w, h))
    }

    @Test
    fun `a blank image is no code`() {
        val size = 120
        assertNull(QrDecoder.decodeStill(IntArray(size * size) { white }, size, size))
        assertNull(QrDecoder.decodeLuma(ByteArray(10), 4, 4))
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
     * would pick it from the album after it went through a chat. Each module
     * is ~24 px across, wider than a binarizer's local window — what made
     * ZXing need a ladder here; vela-qr halves the image itself.
     */
    @Test
    fun `vela reads its own receive code from a screenshot`() {
        for ((file, includeNetwork) in listOf("receive-code-on.png" to true, "receive-code-off.png" to false)) {
            val image = ImageIO.read(javaClass.getResource("/qr/$file"))
            val pixels = image.getRGB(0, 0, image.width, image.height, null, 0, image.width)
            assertEquals(file, coreCode(includeNetwork), QrDecoder.decodeStill(pixels, image.width, image.height))
        }
    }

    /** A shrink averages: a 2×2 block of black and white becomes one grey pixel. */
    @Test
    fun `the downscale averages every source pixel`() {
        val out = QrDecoder.downscale(intArrayOf(black, white, white, black), 2, 2, 1, 1)
        assertEquals(0xFF7F7F7F.toInt(), out.single())
    }

    // ---- the result format (rust/vela-qr/src/wire.rs) --------------------------

    private fun wire(part: Pair<Int, Int>? = null, vararg segments: Pair<List<Any>, ByteArray>): ByteArray {
        val out = java.io.ByteArrayOutputStream()
        fun u32(v: Int) = repeat(4) { out.write((v shr (8 * it)) and 0xFF) }
        out.write(1)
        if (part == null) out.write(0) else { out.write(1); out.write(part.first); out.write(part.second) }
        u32(segments.size)
        for ((candidates, raw) in segments) {
            out.write(candidates.size)
            for (c in candidates) {
                val bytes = c.toString().toByteArray()
                if (c is String) { out.write(0); u32(bytes.size) } else { out.write(1); out.write(bytes.size) }
                out.write(bytes)
            }
            u32(raw.size)
            out.write(raw)
        }
        return out.toByteArray()
    }

    /** A WHATWG label, not decoded text. */
    private data class Label(val name: String) {
        override fun toString() = name
    }

    @Test
    fun `GBK bytes are decoded by java nio, strictly`() {
        val gbk = byteArrayOf(0xC4.toByte(), 0xE3.toByte(), 0xBA.toByte(), 0xC3.toByte())
        assertEquals(QrDecoder.Read.Text("你好"), QrDecoder.parse(wire(null, listOf<Any>(Label("gb18030"), "Äãºà") to gbk)))
        // 0x80 alone is not Shift_JIS: the next candidate stands.
        assertEquals(QrDecoder.Read.Text("\u0080"), QrDecoder.parse(wire(null, listOf<Any>(Label("shift_jis"), "\u0080") to byteArrayOf(0x80.toByte()))))
    }

    @Test
    fun `one part of several is never the message, and non-text is unreadable`() {
        assertEquals(QrDecoder.Read.Part(0, 3, "ab"), QrDecoder.parse(wire(0 to 3, listOf<Any>("ab") to "ab".toByteArray())))
        assertEquals(QrDecoder.Read.Unreadable, QrDecoder.parse(wire(null, listOf<Any>() to byteArrayOf(0, 1))))
        assertEquals(QrDecoder.Read.Unreadable, QrDecoder.parse(byteArrayOf(1, 0, 9)))
    }
}
