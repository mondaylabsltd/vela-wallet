package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * r13-7 on Android (docs/multisig-machines.md §15.3, §17): the core says
 * where a multisig's history stands (`ProposalsView.history`). While its
 * first read is out the home's Activity is loading — never "No activity yet";
 * one that could not be read says so, with Try again — as the web's
 * `withLiveWallet` `multisigHistory` draws it.
 */
class MultisigHistoryHomeTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val emptyHome get() = WalletFixtures.buildMobileState(WalletScreenState.H2, strings)

    private fun history(stand: String) = WalletLive.MultisigHistory(
        stand = stand,
        unreadText = "Couldn't read this multisig's history on Arc Testnet",
        retryText = strings.t("common.tryAgain"),
    )

    @Test
    fun without_the_historys_word_an_empty_multisig_says_no_activity_yet() {
        // The bug: what the home said while the first read was out.
        assertEquals(SectionMode.Empty, emptyHome.activitySection.mode)
        assertEquals(strings.t("home.emptyNoActivity"), emptyHome.activitySection.empty?.title)
    }

    @Test
    fun a_history_being_read_is_loading_never_no_activity_yet() {
        assertEquals(SectionMode.Loading, WalletLive.withMultisigHistory(emptyHome, history("reading")).activitySection.mode)
    }

    @Test
    fun a_history_read_with_nothing_in_it_says_no_activity_yet() {
        val model = WalletLive.withMultisigHistory(emptyHome, history("read"))
        assertEquals(SectionMode.Empty, model.activitySection.mode)
        assertEquals(strings.t("home.emptyNoActivity"), model.activitySection.empty?.title)
        assertNull(model.activitySection.empty?.retry)
    }

    @Test
    fun a_history_not_read_says_so_with_try_again() {
        val model = WalletLive.withMultisigHistory(emptyHome, history("unread"))
        assertEquals(SectionMode.Empty, model.activitySection.mode)
        assertEquals("Couldn't read this multisig's history on Arc Testnet", model.activitySection.empty?.title)
        assertEquals("never the \"No activity yet\" caption", "", model.activitySection.empty?.caption)
        assertEquals("Try again", model.activitySection.empty?.retry)
    }

    @Test
    fun rows_already_shown_stay_whatever_the_history() {
        val rows = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)
        for (stand in listOf("reading", "unread")) {
            val model = WalletLive.withMultisigHistory(rows, history(stand))
            assertEquals(stand, SectionMode.Rows, model.activitySection.mode)
            assertEquals(rows.activityGroups, model.activityGroups)
        }
        assertEquals(emptyHome, WalletLive.withMultisigHistory(emptyHome, null))
    }

    /** The proposals view's `history`, as the core writes it; an older core's view without it reads as read. */
    @Test
    fun the_proposals_view_decodes_the_historys_stand() {
        for (stand in listOf("reading", "read", "unread")) {
            val view = MsProposalsView.of(Wire.json.parseToJsonElement("""{"history":"$stand","history_unreadable":[5042002]}"""))
            assertEquals(stand, view.history)
            assertEquals(listOf(5042002), view.historyUnreadable)
        }
        assertEquals("read", MsProposalsView.of(Wire.json.parseToJsonElement("{}")).history)
    }

    /** The real proposals machine says `reading` before a followed multisig's first history read ends. */
    @Test
    fun the_core_says_reading_for_a_view_not_read_yet() {
        val core = uniffi.vela_core_uniffi.MultisigProposalsCore()
        val view = Wire.json.parseToJsonElement(core.view())
        assertEquals("reading", MsProposalsView.of(view).history)
    }
}
