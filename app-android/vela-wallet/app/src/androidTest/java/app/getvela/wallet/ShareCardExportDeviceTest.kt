package app.getvela.wallet

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.ShareCardCapture
import app.getvela.wallet.feature.flows.ShareCardModel
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.feature.flows.shareCardLogo
import app.getvela.wallet.feature.scan.QrDecoder
import java.io.ByteArrayOutputStream
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
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
 * The logo cases fetch from the live chain-data endpoint (Gnosis and Tempo
 * serve WebP under a `.png` name), and first assert the logo arrived — a card
 * that silently fell back to the disc would pass every other check.
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

    /** Captures [model], writes it as `[file].png`, and checks what a camera and a chat app would get. */
    private fun export(file: String, model: ShareCardModel, height: Int) {
        if (model.chainLogoUrl != null) {
            assertNotNull("the network's logo loads", runBlocking { shareCardLogo(context, model.chainLogoUrl) })
        }
        var result: ByteArray? = null
        var finished = false
        compose.setContent {
            ShareCardCapture(model) { bytes ->
                result = bytes
                finished = true
            }
        }
        compose.waitUntil(timeoutMillis = 20_000) { finished }
        val bytes = requireNotNull(result) { "the capture produced no PNG" }
        val dir = File(context.getExternalFilesDir(null), "share-cards").apply { mkdirs() }
        File(dir, "$file.png").writeBytes(bytes)

        val bitmap = requireNotNull(BitmapFactory.decodeByteArray(bytes, 0, bytes.size))
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
    }
}
