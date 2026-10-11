package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeBoards
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.FeeRooms
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.ClearSigningView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The 2026-10-11 phone walk, item 2: the signing sheet's top moved 154 px as
 * the fee landed, because the lines a fee card comes to say — the speed's
 * note, "no coin can pay" — had no room until they were said.
 *
 * The LIVE fee card names those rooms on every on-chain fee, whatever the
 * fee's state, so the sheet is as tall on its first frame as when the fee
 * has landed (`SigningSheetStillTest` measures that on a device). This is the
 * builder's half: which lines, from the first view the core sends.
 */
class SigningFeeRoomsTest {

    private fun strings(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        return I18nRuntime { name -> File(root, "assets/i18n/$name.json").readBytes() }.apply { initialize(tag) }
    }

    private val account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private fun card(strings: VelaStrings, fee: FeeView, speed: Boolean = true) = SigningLive.feeModel(
        ClearSigningView(),
        fee,
        SigningLive.Context(strings, "Gnosis", Color.Red, "xDAI", "Me", account, chainId = 100),
        if (speed) SendLive.SpeedInputs(FeeSpeedView()) { null } else null,
    ) as FeeModel.OnChain

    @Test
    fun `the live card names its rooms from the first view, and in every state of the fee`() {
        val zh = strings("zh")
        val rooms = FeeRooms(
            speedNote = "这条网络上选「超快」不额外收费，这笔已改用「超快」。",
            lines = listOf("所有代币都不够付这笔网络费", "xDAI 余额不足以支付 Gas 费"),
        )
        // Nothing known yet: the very first frame.
        assertEquals(rooms, card(zh, FeeView()).rooms)
        assertEquals(rooms, card(zh, FeeView(busy = true)).rooms)
        assertEquals(rooms, card(zh, FeeView(busy = true, nothing_to_pay_from = true)).rooms)
        // A fee that failed, and its re-ask.
        for (case in listOf(FeeBoards.Case.ChainDown, FeeBoards.Case.Retrying, FeeBoards.Case.Internal)) {
            assertEquals(case.name, rooms, card(zh, FeeBoards.view(case, chainId = 100, account = account)).rooms)
        }
    }

    @Test
    fun `the rooms are the core's own sentences, in the language drawn`() {
        val en = strings("en")
        val rooms = card(en, FeeView(busy = true)).rooms!!
        assertEquals(en.t(I18nKeys.Flows.FEE_SPEED_FREE), rooms.speedNote)
        assertEquals(
            listOf(en.t(I18nKeys.Flows.FEE_NO_COIN_PAYS), en.t("send.warnInsufficientGas", mapOf("sym" to "xDAI"))),
            rooms.lines,
        )
    }

    @Test
    fun `a card with no speed control keeps no room for a speed's note`() {
        val rooms = card(strings("zh"), FeeView(busy = true), speed = false).rooms!!
        assertNull(rooms.speedNote)
        assertEquals(2, rooms.lines.size)
    }

    /** A board drawn by hand says all it will: no room is added to it (the gallery's boards keep their heights). */
    @Test
    fun `a drawn board names no rooms`() {
        val drawn = SigningFixtures.build(SigningScreenState.CS36, strings("zh")).fee as FeeModel.OnChain
        assertNull(drawn.rooms)
    }
}
