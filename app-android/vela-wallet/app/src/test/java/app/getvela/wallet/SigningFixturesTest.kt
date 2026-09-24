package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.signing.AllowanceChip
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningTone
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 022 gates for the signing layer.
 *
 * Two of these are product contracts rather than style checks — the slide is
 * the only confirmation, and an unlimited approval can never be confirmed as
 * requested — so they are asserted here: a later refactor has to break a test
 * to break the promise.
 *
 * The echo check is Android-specific and load-bearing: `t()` returns the key on
 * a miss, so a typo in one of 33 scenarios ships as
 * "componentsUi.signing.drainWarning" printed on a signing sheet.
 */
class SigningFixturesTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private fun zhStrings(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("zh") }

    private fun stringsOf(model: SigningScreenModel): List<String> {
        val out = mutableListOf(
            model.dappName, model.dappHost, model.networkName,
            model.signerLabel, model.signerName,
            model.panelTitle, model.tech.title,
        )
        model.blocks.forEach { block ->
            when (block) {
                is SigningBlock.Intent -> out += block.text
                is SigningBlock.Amount -> out += listOfNotNull(
                    block.line.value, block.line.symbol, block.line.caption, block.line.fiat,
                    block.note,
                )
                is SigningBlock.Swap -> out += listOfNotNull(
                    block.pay.caption, block.receive.caption, block.pay.symbol,
                    block.receive.symbol,
                )
                is SigningBlock.Nft -> out += listOf(block.id, block.collection)
                is SigningBlock.Sentence -> out += block.text
                is SigningBlock.Allowance -> {
                    out += listOfNotNull(block.label, block.value, block.note)
                    block.chips.forEach { out += it.label }
                    block.resultingTotal?.let { out += listOf(it.label, it.value) }
                }
                is SigningBlock.Party -> out += listOfNotNull(
                    block.label, block.name, block.address, block.badge?.text,
                )
                is SigningBlock.Rows -> block.rows.forEach { out += listOf(it.label, it.value) }
                is SigningBlock.Warning -> out += block.text
                is SigningBlock.Positive -> out += block.text
                is SigningBlock.Code -> out += listOfNotNull(block.note)
                is SigningBlock.Card -> {
                    block.title?.let { out += it }
                    block.rows.forEach { out += listOf(it.label, it.value) }
                }
                is SigningBlock.Balances -> {
                    out += block.title
                    block.note?.let { out += it }
                }
            }
        }
        when (val fee = model.fee) {
            is FeeModel.OnChain -> {
                out += listOf(fee.label, fee.value)
                fee.selectorTitle?.let { out += it }
                fee.options.forEach { out += listOf(it.name, it.balance, it.fee) }
            }
            is FeeModel.OffChain -> out += fee.note
            // A refused request carries no fee at all (spec 081); Hidden is
            // the off-chain case that shows the row with nothing in it.
            FeeModel.Hidden, null -> Unit
        }
        model.confirmHint?.let { out += it }
        model.confirmAction?.let { out += it }
        return out
    }

    @Test
    fun allThirtyThreeScenariosBuild() {
        assertEquals(33, SigningScreenState.entries.size)
        for (state in SigningScreenState.entries) {
            val model = SigningFixtures.build(state, zhStrings())
            assertEquals(state, model.state)
            assertTrue("$state has no blocks", model.blocks.isNotEmpty())
            assertTrue("$state opens without an intent", model.blocks.first() is SigningBlock.Intent)
        }
    }

    @Test
    fun noStringEchoesItsKeyAndNoTemplateIsLeftUnfilled() {
        val zh = zhStrings()
        for (state in SigningScreenState.entries) {
            for (value in stringsOf(SigningFixtures.build(state, zh))) {
                assertFalse(
                    "`$value` in $state looks like an unresolved key",
                    value.startsWith("componentsUi."),
                )
                assertFalse("`$value` in $state still carries a {{var}}", value.contains("{{"))
            }
        }
    }

    @Test
    fun theSlideIsTheOnlyConfirmationAndAlwaysSaysWhatFor() {
        val zh = zhStrings()
        for (state in SigningScreenState.entries) {
            val model = SigningFixtures.build(state, zh)
            // Every DRAWN state offers the slide; the refusal state has no
            // fixture, because it is reached from the core, not the gallery.
            assertTrue("$state has no slide hint", model.confirmHint?.isNotBlank() == true)
            assertTrue("$state has no slide action", model.confirmAction?.isNotBlank() == true)
        }
    }

    /** The never-unlimited mandate (spec 022 §4). */
    @Test
    fun unlimitedApprovalCannotBeConfirmedAsRequested() {
        val model = SigningFixtures.build(SigningScreenState.CS5, zhStrings())
        assertFalse("cs5 must not be confirmable", model.confirmEnabled)
        val editor = model.blocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(
            AllowanceChip.ChipState.Disabled,
            editor.chips.single { it.id == "requested" }.state,
        )
    }

    @Test
    fun choosingAFiniteCapReEnablesTheSlide() {
        val zh = zhStrings()
        for (state in listOf(SigningScreenState.CS6, SigningScreenState.CS8)) {
            val model = SigningFixtures.build(state, zh)
            assertTrue("$state should be confirmable", model.confirmEnabled)
            val editor = model.blocks.filterIsInstance<SigningBlock.Allowance>().single()
            // The REQUEST was still unlimited, so its chip stays disabled.
            assertEquals(
                AllowanceChip.ChipState.Disabled,
                editor.chips.single { it.id == "requested" }.state,
            )
        }
    }

    @Test
    fun aFiniteRequestMayBeSignedAsAsked() {
        val model = SigningFixtures.build(SigningScreenState.CS7, zhStrings())
        val editor = model.blocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(
            AllowanceChip.ChipState.Selected,
            editor.chips.single { it.id == "requested" }.state,
        )
        // An increment only means something next to the total it lands on.
        assertEquals("350 USDC", editor.resultingTotal?.value)
    }

    @Test
    fun theLadderPromotesSimulationWhereDecodingFailed() {
        val zh = zhStrings()
        for (state in listOf(
            SigningScreenState.CS23, SigningScreenState.CS30, SigningScreenState.CS31,
        )) {
            val model = SigningFixtures.build(state, zh)
            assertEquals(
                "$state should show balance changes",
                1,
                model.blocks.filterIsInstance<SigningBlock.Balances>().size,
            )
        }
    }

    @Test
    fun theDeepestRungsWarnInDanger() {
        val zh = zhStrings()
        for (state in listOf(SigningScreenState.CS24, SigningScreenState.CS32)) {
            val model = SigningFixtures.build(state, zh)
            assertTrue(
                "$state should carry a danger warning",
                model.blocks.filterIsInstance<SigningBlock.Warning>()
                    .any { it.tone == SigningTone.Danger },
            )
        }
        // cs32 states BOTH failures and still shows the amount it does know.
        val deepest = SigningFixtures.build(SigningScreenState.CS32, zh)
        assertEquals(2, deepest.blocks.filterIsInstance<SigningBlock.Warning>().size)
        assertTrue(
            deepest.blocks.filterIsInstance<SigningBlock.Rows>()
                .first().rows.first().value.contains("0.25 ETH"),
        )
    }

    @Test
    fun feeShapesMatchTheirMocks() {
        val zh = zhStrings()
        assertTrue(SigningFixtures.build(SigningScreenState.CS1, zh).fee is FeeModel.OnChain)
        for (state in listOf(
            SigningScreenState.CS16, SigningScreenState.CS17,
            SigningScreenState.CS18, SigningScreenState.CS19,
        )) {
            assertTrue("$state pays no gas", SigningFixtures.build(state, zh).fee is FeeModel.OffChain)
        }
        for (state in listOf(
            SigningScreenState.CS20, SigningScreenState.CS21, SigningScreenState.CS22,
        )) {
            assertEquals(
                "$state shows no fee row at all",
                FeeModel.Hidden,
                SigningFixtures.build(state, zh).fee,
            )
        }
        val selector = SigningFixtures.build(SigningScreenState.CS33, zh).fee as FeeModel.OnChain
        assertEquals(2, selector.options.size)
    }

    @Test
    fun cs29IsCs1WithTheTechnicalPanelOpen() {
        val zh = zhStrings()
        assertTrue(SigningFixtures.build(SigningScreenState.CS29, zh).techOpen)
        assertFalse(SigningFixtures.build(SigningScreenState.CS1, zh).techOpen)
        assertEquals(2, SigningFixtures.build(SigningScreenState.CS29, zh).tech.identities.size)
    }
}
