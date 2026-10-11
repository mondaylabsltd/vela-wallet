package app.getvela.wallet

import android.graphics.Bitmap
import android.graphics.Canvas
import android.util.Log
import androidx.compose.ui.platform.ViewRootForTest
import java.io.File
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsIcon
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsRowModel
import app.getvela.wallet.feature.settings.SettingsScreen
import app.getvela.wallet.feature.settings.SettingsScreenModel
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.VenueModel
import app.getvela.wallet.feature.settings.components.WALLET_KEYS_BLOCK_TAG
import app.getvela.wallet.feature.settings.components.WALLET_KEY_ROW_TAG
import app.getvela.wallet.feature.settings.components.settingsRowTag
import app.getvela.wallet.feature.settings.core.RegistryBackup
import app.getvela.wallet.feature.settings.core.WalletKeys
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk, 2026-10-11: Settings dropped when its keys block loaded. Before
 * the registry answered the block had no key row; when the row landed, 「把钱包
 * 记录复制到以太坊」, 「在哪里预览并签名」 and 「外观」 each moved down 151 px
 * on the phone, and the header gained its count.
 *
 * The rule: the block has its final height from its FIRST frame. Settings is
 * drawn here as the app draws it — the account, the keys block, "Where you
 * review and sign", the sections — with the keys still being asked (the rows
 * this device's own record gives) and then answered by the registry, in one
 * composition — and then once more with a registry that never answers. The
 * block's height, the first row under it and the Appearance heading must not
 * move, and neither must any key row or the copy-record row inside the block:
 * for a wallet of one key and of three, in Chinese, English and Russian (the
 * longest badges), at the default and the largest text.
 *
 * While asking, nothing the registry may contradict is on the page — not a
 * vault's name, not "Built-in passkey", not a badge nobody vouched for.
 *
 * Each case is also drawn with the fix switched OFF (one skeleton row while
 * asking, rows that keep no room) — the numbers of the drop it removes, and
 * the proof that this measure can see one.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SettingsKeysSteadyTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * adb -s <emulator> logcat -d -s pr4-keys   # the numbers
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SettingsKeysSteadyTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val catalogs = HashMap<String, VelaStrings>()
    private fun stringsOf(tag: String): VelaStrings = catalogs.getOrPut(tag) {
        I18nRuntime { t -> context.assets.open("i18n/$t.json").use { it.readBytes() } }.apply { initialize(tag) }
    }

    private data class Case(val keys: Int, val language: String, val scale: TextScaleLevel) {
        override fun toString() = "$keys key${if (keys == 1) "" else "s"} $language ${scale.wire}"
    }

    /** Where things stand, in px from the top of the page's root. */
    private data class Standing(
        val blockHeight: Int,
        val keyRows: List<Pair<Int, Int>>,
        val copyRow: Pair<Int, Int>?,
        val firstUnder: Int,
        val appearance: Int,
    )

    // Three points on P-256 (1·G, 2·G, 3·G): keys the core reads as keys.
    private val points = listOf(
        "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
        "047cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc4766997807775510db8ed040293d9ac69f7430dbba7dade63ce982299e04b79d227873d1",
        "045ecbe4d1a6330a44c8f7ef951d4bf165e6c6b721efada985fb41661bc6e7fd6c8734640c4998ff7e374b06ce1a64a2ecd82ab036384fb83d9a79b127a27d5032",
    )

    /** What this device's record says of a wallet of [count] keys — the real core, asked with no address. */
    private fun record(count: Int): WalletKeys.Result = WalletKeys(ethCall = { _, _, _ -> error("the device's own record asks no chain") }).deviceOnly(
        points.take(count).mapIndexed { index, hex -> WalletKeys.DeviceKey(hex, if (index == 0) "F" else "", "", "cred-$index") },
        "cred-0",
    )

    /**
     * What the registry answers for the same keys. The first is the walk's
     * own: 「F / Google Password Manager / 当前登录 · 使用时需验证 · 云同步」.
     * The second is a security key the registry names — in the OTHER script
     * than the page's, so a name's glyphs cannot be what holds a row's height —
     * and the third nobody named, on a phone.
     */
    private fun registry(count: Int, language: String): WalletKeys.Result {
        fun row(index: Int, name: String, provider: String, method: KeyMethod, synced: Boolean, verified: Boolean) = WalletKeys.Row(
            key = CreateKeyRow(name, if (method == KeyMethod.Platform) "platform" else "cross-platform", if (method == KeyMethod.SecurityKey) "usb" else "internal", true, synced, "aaguid", provider, method),
            synced = synced,
            publicKeyHex = points[index],
            credentialId = "credential-$index",
            attestationHex = "0x0102",
            userVerified = verified,
            signsHere = index == 0,
        )
        return WalletKeys.Result(
            WalletKeys.Source.Registry,
            listOf(
                row(0, "F", "Google Password Manager", KeyMethod.Platform, synced = true, verified = true),
                row(1, if (language == "zh") "YubiKey" else "备用钥匙", "", KeyMethod.SecurityKey, synced = false, verified = true),
                row(2, "", "", KeyMethod.Hybrid, synced = true, verified = false),
            ).take(count),
        )
    }

    private val notCopied = RegistryBackup.Check(
        RegistryBackup.State.NotBackedUp,
        RegistryBackup.Call(1, "0xreg", "0xcd438f9b"),
        RegistryBackup.Row(
            I18nKeys.SettingsUi.BACKUP_TITLE, "settingsModals.backup.notBackedUp",
            RegistryBackup.Tone.Neutral, RegistryBackup.Action.Copy, I18nKeys.SettingsUi.BACKUP_EXPLAIN,
        ),
    )
    private val couldNotCheck = RegistryBackup.Check(
        RegistryBackup.State.CouldNotCheck,
        null,
        RegistryBackup.Row(
            I18nKeys.SettingsUi.BACKUP_TITLE, I18nKeys.SettingsUi.BACKUP_COULD_NOT_CHECK,
            RegistryBackup.Tone.Neutral, RegistryBackup.Action.Retry, I18nKeys.SettingsUi.BACKUP_EXPLAIN,
        ),
    )

    /** Settings as the app draws its home: the ST1 page with the keys block and the venue row under it. */
    private fun page(
        strings: VelaStrings,
        keys: WalletKeys.Result?,
        backup: RegistryBackup.Check?,
        device: WalletKeys.Result?,
    ): SettingsScreenModel {
        val venue = strings.t("settings.venue.title")
        return SettingsLive.withWalletKeys(SettingsFixtures.buildState(SettingsScreenState.ST1, strings), keys, backup, strings, device = device).copy(
            venue = VenueModel(
                row = SettingsRowModel(
                    id = "venue",
                    title = venue,
                    icon = SettingsIcon.Eye,
                    value = uniffi.vela_core_uniffi.venueWords("in_vela")?.titleKey?.let(strings::t),
                ),
                title = venue, subtitle = "", domainLine = "", choices = emptyList(), pageSection = "", pageSectionBody = "", manage = "",
            ),
        )
    }

    /** The fix switched off: rows and a copy-record row that keep no room — the block as it was drawn before. */
    private fun SettingsScreenModel.roomless(): SettingsScreenModel = copy(
        keys = keys?.let { block ->
            block.copy(
                badgeRoom = emptyList(),
                holderRoom = emptyList(),
                noteRoom = "",
                backup = block.backup?.copy(subtitleRoom = emptyList(), trailingRoom = false),
            )
        },
    )

    private var case by mutableStateOf<Case?>(null)
    private var shown by mutableStateOf<SettingsScreenModel?>(null)

    private fun host() {
        compose.setContent {
            val now = case
            val model = shown
            if (now != null && model != null) {
                // One composition per case; its models change under it, as a session's do.
                key(now) {
                    CompositionLocalProvider(LocalVelaStrings provides stringsOf(now.language)) {
                        VelaTheme(darkTheme = false, fontScale = now.scale.factor) {
                            SettingsScreen(model = model, page = SettingsPage.Home, overlay = SettingsOverlay.None, advancedOpen = false)
                        }
                    }
                }
            }
        }
    }

    private fun draw(model: SettingsScreenModel): Standing {
        shown = model
        compose.waitForIdle()
        val strings = stringsOf(case!!.language)
        // A row's whole place (its tag is on the row, outside its padding
        // and its tap): a row that takes taps and one that does not are
        // measured the same way.
        fun row(id: String) = compose.onNodeWithTag(settingsRowTag(id), useUnmergedTree = true).fetchSemanticsNode()
        val block = compose.onNodeWithTag(WALLET_KEYS_BLOCK_TAG, useUnmergedTree = true).fetchSemanticsNode()
        return Standing(
            blockHeight = block.size.height,
            keyRows = compose.onAllNodesWithTag(WALLET_KEY_ROW_TAG, useUnmergedTree = true).fetchSemanticsNodes().map { it.positionInRoot.y.toInt() to it.size.height },
            copyRow = model.keys?.backup?.let { backup -> row(backup.id).let { it.positionInRoot.y.toInt() to it.size.height } },
            firstUnder = row("venue").positionInRoot.y.toInt(),
            appearance = compose.onAllNodesWithText(strings.t(I18nKeys.SettingsUi.SECTION_APPEARANCE)).onFirst().fetchSemanticsNode().positionInRoot.y.toInt(),
        )
    }

    /**
     * Nothing on the page contains [words] — neither written nor read out —
     * in the tree an assistive client gets (the merged one: what a row keeps
     * room for with an unseen copy is cleared from it, and must be).
     */
    private fun assertUnsaid(words: String, why: String) {
        val written = compose.onAllNodes(hasText(words, substring = true)).fetchSemanticsNodes().size
        val read = compose.onAllNodes(hasContentDescription(words, substring = true)).fetchSemanticsNodes().size
        assertEquals("$why: \"$words\" is on the page while the registry is still being asked", 0, written + read)
    }

    private fun said(words: String): Boolean = compose.onAllNodes(hasText(words, substring = true)).fetchSemanticsNodes().isNotEmpty()

    /**
     * `-e pr4Shots 1`: the frames that were measured, saved under the app's
     * cache (`pr4-keys/`). The page's own drawing into a bitmap — no screen is
     * read — and never a reason to fail: the numbers are the test.
     */
    private fun shot(name: String) {
        if (InstrumentationRegistry.getArguments().getString("pr4Shots") == null) return
        runCatching {
            val view = (compose.onRoot().fetchSemanticsNode().root as ViewRootForTest).view
            val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
            compose.runOnUiThread { view.draw(Canvas(bitmap)) }
            val folder = File(context.cacheDir, "pr4-keys").apply { mkdirs() }
            File(folder, "$name.png").outputStream().use { out -> bitmap.compress(Bitmap.CompressFormat.PNG, 100, out) }
        }.onFailure { Log.w(TAG, "no frame for $name: $it") }
    }

    /** One key and three; Chinese and English at the default and the largest text; and Russian, whose badges are the longest. */
    private val cases = buildList {
        for (keys in listOf(1, 3)) for (language in listOf("zh", "en")) for (scale in listOf(TextScaleLevel.Standard, TextScaleLevel.XLarge)) {
            add(Case(keys, language, scale))
        }
        add(Case(3, "ru", TextScaleLevel.Standard))
        add(Case(3, "ru", TextScaleLevel.XLarge))
    }

    @Test
    fun theKeysBlockHasItsFinalHeightFromTheFirstFrame() {
        host()
        for (each in cases) {
            case = each
            val strings = stringsOf(each.language)
            val own = record(each.keys)
            val answer = registry(each.keys, each.language)

            val asking = draw(page(strings, keys = null, backup = null, device = own))
            assertEquals("$each: a row for every key the record has, while asking", each.keys, asking.keyRows.size)

            // Said while asking: only what the record knows. The registry's
            // words are not on the page yet — not even the ones that would
            // turn out right.
            val k = I18nKeys.SettingsUi
            for (key in listOf(k.KEYS_PROVIDER_PLATFORM, k.KEYS_PROVIDER_GENERIC, k.KEYS_PROVIDER_SECURITY_KEY)) assertUnsaid(strings.t(key), "$each")
            for (key in listOf(k.KEYS_USER_VERIFIED, k.KEYS_SYNCED, k.KEYS_NOT_SYNCED)) assertUnsaid(strings.t(key), "$each")
            assertUnsaid("Google Password Manager", "$each")
            for (n in 1..each.keys) assertUnsaid(strings.t(k.KEYS_KEY_N).replace("{{n}}", n.toString()), "$each")
            // The record's own facts are.
            assertTrue("$each: the first key's name", compose.onAllNodesWithText("F").fetchSemanticsNodes().isNotEmpty())
            assertTrue("$each: the sign-in key", said(strings.t(k.KEYS_SIGNS_HERE)))
            assertTrue("$each: its fingerprint", said("6b17…51f5"))
            shot("${each.keys}-${each.language}-${each.scale.wire}-1-asking")

            val answered = draw(page(strings, keys = answer, backup = notCopied, device = own))
            assertTrue("$each: the registry's holder is said once it answers", said("Google Password Manager"))
            shot("${each.keys}-${each.language}-${each.scale.wire}-2-answered")
            Log.i(TAG, "after  | $each | block ${asking.blockHeight} -> ${answered.blockHeight} | copy row ${asking.copyRow} -> ${answered.copyRow} | first under ${asking.firstUnder} -> ${answered.firstUnder} | appearance ${asking.appearance} -> ${answered.appearance} | key rows ${asking.keyRows} -> ${answered.keyRows}")

            assertEquals("$each: the block's height changed when the registry answered", asking.blockHeight, answered.blockHeight)
            assertEquals("$each: a key row moved or changed height when the registry answered", asking.keyRows, answered.keyRows)
            assertEquals("$each: the copy-record row moved or changed height when its check answered", asking.copyRow, answered.copyRow)
            assertEquals("$each: the first row under the block moved when the registry answered", asking.firstUnder, answered.firstUnder)
            assertEquals("$each: Appearance moved when the registry answered", asking.appearance, answered.appearance)

            // A registry that never answers: the walk falls back to the
            // record's rows as its answer, and says so — in the caption under
            // the title, whose room was kept. Nothing moves then either.
            val silent = draw(page(strings, keys = own, backup = couldNotCheck, device = own))
            assertTrue("$each: a silent registry is said to be silent", said(strings.t(k.KEYS_FROM_DEVICE)))
            Log.i(TAG, "silent | $each | block ${asking.blockHeight} -> ${silent.blockHeight} | copy row ${asking.copyRow} -> ${silent.copyRow} | first under ${asking.firstUnder} -> ${silent.firstUnder} | appearance ${asking.appearance} -> ${silent.appearance} | key rows ${asking.keyRows} -> ${silent.keyRows}")
            assertEquals("$each: the block's height changed when the registry stayed silent", asking.blockHeight, silent.blockHeight)
            assertEquals("$each: a key row moved or changed height when the registry stayed silent", asking.keyRows, silent.keyRows)
            assertEquals("$each: the copy-record row moved or changed height when the registry stayed silent", asking.copyRow, silent.copyRow)
            assertEquals("$each: the first row under the block moved when the registry stayed silent", asking.firstUnder, silent.firstUnder)
            assertEquals("$each: Appearance moved when the registry stayed silent", asking.appearance, silent.appearance)
        }
    }

    /**
     * The same walks with the fix switched off — what a person saw: one
     * skeleton row while asking, then the wallet's keys, and everything under
     * the block lower by their height. This is the "before" of every number
     * above, and it must be a drop: a measure that reads 0 here measures
     * nothing.
     */
    @Test
    fun withTheFixOffEverythingUnderTheBlockDrops() {
        host()
        for (each in cases) {
            case = each
            val strings = stringsOf(each.language)
            val answer = registry(each.keys, each.language)

            // No device rows: the block does not know the wallet's keys until the registry says.
            val asking = draw(page(strings, keys = null, backup = null, device = null).roomless())
            assertEquals("$each: no key row before the answer", 0, asking.keyRows.size)
            val answered = draw(page(strings, keys = answer, backup = notCopied, device = null).roomless())
            assertEquals(each.keys, answered.keyRows.size)
            Log.i(TAG, "before | $each | block ${asking.blockHeight} -> ${answered.blockHeight} | copy row ${asking.copyRow} -> ${answered.copyRow} | first under ${asking.firstUnder} -> ${answered.firstUnder} (+${answered.firstUnder - asking.firstUnder}) | appearance ${asking.appearance} -> ${answered.appearance} (+${answered.appearance - asking.appearance}) | key rows ${answered.keyRows}")

            val drop = answered.firstUnder - asking.firstUnder
            assertTrue("$each: with the fix off the first row under the block should drop, and it moved $drop px", drop > 0)
            assertEquals("$each: Appearance drops with it", drop, answered.appearance - asking.appearance)
            assertEquals("$each: by what the block grew", drop, answered.blockHeight - asking.blockHeight)
        }
    }

    private companion object {
        const val TAG = "pr4-keys"
    }
}
