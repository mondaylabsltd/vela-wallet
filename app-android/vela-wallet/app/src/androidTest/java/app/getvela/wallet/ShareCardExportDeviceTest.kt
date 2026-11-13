package app.getvela.wallet

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.marks.LogoStore
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.ShareCardCapture
import app.getvela.wallet.feature.flows.ShareCardModel
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.feature.flows.shareCardLogo
import app.getvela.wallet.feature.scan.QrDecoder
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.Collections
import kotlin.math.abs
import kotlinx.coroutines.runBlocking
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * 保存图片 in isolation (2026-09-27): the saved share card, produced by the
 * very capture the receive screen uses, for the web's review set — written
 * out so a person LOOKS at it beside the web's
 * (`app-web/vela-wallet/.share-card-review/`), and decoded here so the code
 * in the picture is proved to carry the address, before and after a chat
 * app's downscale-and-JPEG.
 *
 * The logo cases ask for the chain-data endpoint's URLs through the app's
 * own logo fetch ([LogoStore]), answered from the test APK's fixtures
 * (`src/androidTest/assets/share-card-logos/`, the endpoint's own bytes:
 * Gnosis and Tempo serve WebP under a `.png` name) — never the internet, so
 * this passes on a machine or a CI runner with no direct way out. Each first
 * asserts the logo arrived, then that the saved card DRAWS it: against the
 * same card with the lettered disc, it differs only inside the logo's circle
 * in the code's centre — a card that silently fell back to the disc would
 * pass every other check.
 *
 * ```bash
 * adb shell am instrument -w -e class app.getvela.wallet.ShareCardExportDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * adb pull /sdcard/Android/data/app.getvela.wallet/files/share-cards/ .
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ShareCardExportDeviceTest {

    @get:Rule
    val compose = createComposeRule()

    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    /** The chain-data URLs the fixtures answered, in order. */
    private val served = Collections.synchronizedList(mutableListOf<String>())

    /**
     * The chain-data endpoint, from the test APK's assets: a logo it holds is
     * answered with its bytes, as the endpoint serves them; anything else is
     * a 404. Nothing is ever asked of the network.
     */
    private val fixtures = Interceptor { chain ->
        val url = chain.request().url.toString()
        val bytes = url.takeIf { it.startsWith("$LOGOS/") }?.let { logo ->
            runCatching {
                InstrumentationRegistry.getInstrumentation().context.assets
                    .open("$FIXTURES/${logo.substringAfterLast('/')}")
                    .use { it.readBytes() }
            }.getOrNull()
        }
        if (bytes != null) served += url
        Response.Builder()
            .request(chain.request())
            .protocol(Protocol.HTTP_1_1)
            .code(if (bytes != null) 200 else 404)
            .message(if (bytes != null) "OK" else "Not Found")
            .body((bytes ?: ByteArray(0)).toResponseBody("image/png".toMediaType()))
            .build()
    }

    @Before
    fun answerLogosFromFixtures() = LogoStore.answerFrom(fixtures)

    @After
    fun giveTheNetworkBack() = LogoStore.answerFrom(null)

    private fun card(
        headline: String,
        note: String,
        name: String,
        ticker: String,
        badge: Color,
        chainId: Int?,
    ) = ShareCardModel(
        headline = headline,
        name = name,
        lines = FlowFixtures.addressLines(ADDRESS),
        networkNote = note,
        networkMark = TokenMarkModel(ticker, badge),
        identiconSeed = ADDRESS,
        wordmark = "Vela Wallet",
        code = ADDRESS,
        chainLogoUrl = chainId?.let { "$LOGOS/eip155-$it.png" },
    )

    private val zh = card("扫码向我转账", "仅支持 Ethereum 网络付款", "大表哥", "ETH", Color(98, 126, 234), 1)

    /**
     * Captures [model] — and, when it has a logo, the same card with the
     * lettered disc — through the one capture the receive screen uses: the
     * PNG bytes of each.
     */
    private fun capture(model: ShareCardModel): Pair<ByteArray, ByteArray?> {
        val models = listOfNotNull(model, model.chainLogoUrl?.let { model.copy(chainLogoUrl = null) })
        val results = Collections.synchronizedList(mutableListOf<ByteArray?>())
        var current by mutableStateOf(models.first())
        compose.setContent {
            key(current) { ShareCardCapture(current) { bytes -> results += bytes } }
        }
        compose.waitUntil(timeoutMillis = 20_000) { results.size == 1 }
        if (models.size > 1) {
            compose.runOnIdle { current = models[1] }
            compose.waitUntil(timeoutMillis = 20_000) { results.size == 2 }
        }
        val card = requireNotNull(results[0]) { "the capture produced no PNG" }
        return card to if (models.size > 1) requireNotNull(results[1]) { "the disc's capture produced no PNG" } else null
    }

    /**
     * The saved card has the logo: against the same card with the disc, some
     * pixels differ, and only inside the logo's circle at the code's centre
     * (44 dp at the card's 2×, on its horizontal middle).
     */
    private fun assertDrawsTheLogo(card: Bitmap, disc: Bitmap) {
        assertEquals(card.width, disc.width)
        assertEquals(card.height, disc.height)
        var left = Int.MAX_VALUE
        var top = Int.MAX_VALUE
        var right = -1
        var bottom = -1
        val a = IntArray(card.width)
        val b = IntArray(card.width)
        for (y in 0 until card.height) {
            card.getPixels(a, 0, card.width, 0, y, card.width, 1)
            disc.getPixels(b, 0, card.width, 0, y, card.width, 1)
            for (x in 0 until card.width) {
                if (a[x] == b[x]) continue
                if (x < left) left = x
                if (x > right) right = x
                if (y < top) top = y
                if (y > bottom) bottom = y
            }
        }
        assertTrue("the card with the logo is not the disc's card", right >= 0)
        val width = right - left + 1
        val height = bottom - top + 1
        assertTrue("what the logo changed is the logo's circle (${width}×$height px)", width <= LOGO_PX + 4 && height <= LOGO_PX + 4)
        assertTrue("…on the card's horizontal middle (centre x ${(left + right) / 2})", abs((left + right) / 2 - card.width / 2) <= 2)
    }

    /** Captures [model], writes it as `[file].png`, and checks what a camera and a chat app would get. */
    private fun export(file: String, model: ShareCardModel, height: Int) {
        if (model.chainLogoUrl != null) {
            assertNotNull("the network's logo loads", runBlocking { shareCardLogo(context, model.chainLogoUrl) })
            assertTrue("…from the fixture, not the internet", model.chainLogoUrl in served)
        }
        val (bytes, disc) = capture(model)
        val dir = File(context.getExternalFilesDir(null), "share-cards").apply { mkdirs() }
        File(dir, "$file.png").writeBytes(bytes)

        val bitmap = requireNotNull(BitmapFactory.decodeByteArray(bytes, 0, bytes.size))
        if (disc != null) assertDrawsTheLogo(bitmap, requireNotNull(BitmapFactory.decodeByteArray(disc, 0, disc.size)))
        // 480 × the content's height, at 2× — the web's review PNG, pixel for pixel in size.
        assertEquals(960, bitmap.width)
        assertEquals(height, bitmap.height)
        assertEquals(ADDRESS, QrDecoder.decode(bitmap))

        // What a chat app does to it: half size, JPEG at 60.
        val small = Bitmap.createScaledBitmap(bitmap, bitmap.width / 2, bitmap.height / 2, true)
        val jpeg = ByteArrayOutputStream().use { out -> small.compress(Bitmap.CompressFormat.JPEG, 60, out); out.toByteArray() }
        assertEquals(ADDRESS, QrDecoder.decode(BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)))
    }

    @Test
    fun zhEthereum() = export("zh-ethereum", zh, 1486)

    @Test
    fun enGnosisWebp() = export(
        "en-gnosis-webp",
        card("Scan to Send Me Crypto", "Gnosis payments only", "MultiTest", "XDAI", Color(0, 163, 144), 100),
        1486,
    )

    @Test
    fun ruLongHeadlineBnb() = export(
        "ru-long-headline-bnb",
        card(
            "Отсканируйте, чтобы отправить мне крипто",
            "Только платежи в сети BNB Smart Chain",
            "Основной кошелёк",
            "BNB",
            Color(240, 185, 11),
            56,
        ),
        1566,
    )

    @Test
    fun deLongNameTempo() = export(
        "de-long-name-tempo",
        card(
            "Scannen, um mir Krypto zu senden",
            "Nur Zahlungen über Tempo",
            "Gemeinsames Haushaltskonto der Familie",
            "USD",
            Color(20, 20, 20),
            4217,
        ),
        1566,
    )

    @Test
    fun zhNoLogo() = export("zh-no-logo", zh.copy(chainLogoUrl = null), 1486)

    private companion object {
        const val ADDRESS = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        const val LOGOS = "https://ethereum-data.getvela.app/chainlogos"

        /** The endpoint's logos, byte for byte, in the test APK's assets. */
        const val FIXTURES = "share-card-logos"

        /** The logo's circle on the saved card: 44 dp at the card's 2×. */
        const val LOGO_PX = 88
    }
}
