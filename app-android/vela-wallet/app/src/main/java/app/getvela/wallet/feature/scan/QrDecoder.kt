package app.getvela.wallet.feature.scan

import android.graphics.Bitmap
import com.sun.jna.Library
import com.sun.jna.Native
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.CharacterCodingException
import java.nio.charset.Charset
import java.nio.charset.CodingErrorAction

/**
 * Reads a QR code out of a camera frame or a picked photo with vela-qr
 * (rust/vela-qr) — the decoder the web and desktop scanners ship — loaded as
 * `libvela_qr.so` over JNA, which the app already carries for the core.
 *
 * It replaced ZXing (spec 046 D4's one decoder for both paths, still one): in
 * rust/qr-lab's 572 images, vela-qr reads 97.2% of photos and 97.4% of
 * frames, with no wrong reads. Its own strategy — each size dark-on-light and
 * light-on-dark, halved until too small, small pictures doubled — is what the
 * still ladder (spec 090, `stillQrSizes`) used to do around ZXing here.
 *
 * Text in a charset the crate leaves to the platform (Shift_JIS, GB18030,
 * Big5, …, named by WHATWG labels) is decoded with java.nio, strictly. One
 * part of a structured-append sequence, or data that is not text, is not a
 * message: both read as no code. `null` = no QR in this image.
 */
object QrDecoder {
    /** A picked photo wider than this is averaged down to it first, as the web scanner does. */
    const val STILL_MAX_WIDTH = 1200

    private interface VelaQr : Library {
        fun vela_qr_read_luma(pixels: ByteArray, width: Int, height: Int): Int

        fun vela_qr_result(out: ByteArray, capacity: Int): Int
    }

    private val native: VelaQr by lazy { Native.load("vela_qr", VelaQr::class.java) }

    /** 8-bit luminance, row-major. */
    fun decodeLuma(luma: ByteArray, width: Int, height: Int): String? {
        if (width <= 0 || height <= 0 || luma.size != width * height) return null
        val n = native.vela_qr_read_luma(luma, width, height)
        if (n <= 0) return null
        val out = ByteArray(n)
        if (native.vela_qr_result(out, n) != n) return null
        return message(out)
    }

    /** A camera frame's Y plane (YUV_420_888) is already luminance. */
    fun decodeYuv(y: ByteArray, width: Int, height: Int, rowStride: Int): String? {
        val packed = if (rowStride == width) y else ByteArray(width * height).also { out ->
            for (row in 0 until height) System.arraycopy(y, row * rowStride, out, row * width, width)
        }
        return decodeLuma(packed, width, height)
    }

    /** A picked photo: [decodeStill] over its pixels. */
    fun decode(bitmap: Bitmap): String? {
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        return decodeStill(pixels, bitmap.width, bitmap.height)
    }

    /**
     * A still image (ARGB), read once: as it is, or averaged down to
     * [STILL_MAX_WIDTH] wide first — a downscale is a low-pass filter, and the
     * crate halves further itself (rust/qr-lab: one read at ≤ 1200 wide is as
     * good as a ladder of sizes, and several times faster).
     */
    fun decodeStill(pixels: IntArray, width: Int, height: Int): String? {
        if (width <= 0 || height <= 0) return null
        val (w, h, argb) = if (width <= STILL_MAX_WIDTH) {
            Triple(width, height, pixels)
        } else {
            val dh = maxOf(1, Math.round(STILL_MAX_WIDTH.toDouble() * height / width).toInt())
            Triple(STILL_MAX_WIDTH, dh, downscale(pixels, width, height, STILL_MAX_WIDTH, dh))
        }
        return decodeLuma(luminance(argb), w, h)
    }

    /** Integer luminance, the weights every Vela scanner uses. */
    internal fun luminance(argb: IntArray): ByteArray = ByteArray(argb.size) { i ->
        val p = argb[i]
        ((((p shr 16) and 0xFF) * 77 + ((p shr 8) and 0xFF) * 150 + (p and 0xFF) * 29) shr 8).toByte()
    }

    /**
     * Area-average downscale: every source pixel counts toward the one it lands
     * in, which is the low-pass filter the read relies on (a bilinear
     * `createScaledBitmap` aliases at large ratios). Pure Kotlin, so a JVM test
     * runs exactly what the phone runs.
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

    // ------------------------------------------------------------------------
    // vela_qr::wire — the result format (rust/vela-qr/src/wire.rs)
    // ------------------------------------------------------------------------

    /** What one read found. */
    sealed interface Read {
        data class Text(val text: String) : Read

        /** One symbol of several: never the whole message. */
        data class Part(val index: Int, val count: Int, val text: String?) : Read

        /** A code, but not text this platform can read. */
        data object Unreadable : Read
    }

    private fun message(bytes: ByteArray): String? = (parse(bytes) as? Read.Text)?.text

    /** Exported for its unit. */
    internal fun parse(bytes: ByteArray): Read {
        val buf = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        fun u8() = buf.get().toInt() and 0xFF
        fun take(n: Int) = ByteArray(n).also { buf.get(it) }
        return try {
            if (u8() != 1) return Read.Unreadable
            val part = if (u8() == 1) u8() to u8() else null
            val texts = List(buf.int) {
                val candidates = List(u8()) {
                    if (u8() == 0) Candidate.Text(String(take(buf.int), Charsets.UTF_8)) else Candidate.Label(String(take(u8()), Charsets.US_ASCII))
                }
                segmentText(candidates, take(buf.int))
            }
            val text = if (texts.all { it != null }) texts.joinToString("") else null
            when {
                part != null -> Read.Part(part.first, part.second, text)
                text == null -> Read.Unreadable
                else -> Read.Text(text)
            }
        } catch (_: RuntimeException) {
            // A truncated or malformed buffer: never a guess.
            Read.Unreadable
        }
    }

    private sealed interface Candidate {
        data class Text(val text: String) : Candidate

        data class Label(val label: String) : Candidate
    }

    /** The first candidate that decodes the bytes strictly. */
    private fun segmentText(candidates: List<Candidate>, raw: ByteArray): String? {
        for (c in candidates) {
            when (c) {
                is Candidate.Text -> return c.text
                is Candidate.Label -> try {
                    return Charset.forName(c.label).newDecoder()
                        .onMalformedInput(CodingErrorAction.REPORT)
                        .onUnmappableCharacter(CodingErrorAction.REPORT)
                        .decode(ByteBuffer.wrap(raw))
                        .toString()
                } catch (_: CharacterCodingException) {
                    // Not valid in that charset: the next candidate.
                } catch (_: IllegalArgumentException) {
                    // A label this runtime has no charset for: the next candidate.
                }
            }
        }
        return null
    }
}
