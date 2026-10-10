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
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 022 gates for the signing layer.
 *
 * Two of these are product contracts rather than style checks — the confirm
 * is the only confirmation and says what it confirms, and an unlimited
 * approval can never be confirmed as requested — so they are asserted here: a
 * later refactor has to break a test to break the promise.
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

    private fun enStrings(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("en") }

    private fun stringsOf(model: SigningScreenModel): List<String> {
        val out = mutableListOf(
            model.dappName, model.dappHost, model.networkName,
            model.signerLabel, model.signerName,
            model.panelTitle, model.tech.title,
        )
        fun said(block: SigningBlock) {
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
                // A held place says only what took it; its room is unsaid.
                is SigningBlock.Held -> block.shown?.let(::said)
            }
        }
        model.blocks.forEach(::said)
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
        model.headline?.let { out += it }
        model.confirmAction?.let { out += it }
        return out
    }

    @Test
    fun everyScenarioBuilds() {
        // The 33 of the canon, CS36 — the wallet's own backup — CS37–CS42,
        // spec 102's hand-off card (CS40/CS41: the card a send raises on its
        // own), CS43/CS44, a key ceremony waiting on its page, CS45–CS56,
        // the correctness batch's boards (drawn through the live builders),
        // CS57/CS58: the simulation's verdict's place, kept and taken,
        // CS59/CS60: the fee's worth waiting for the display currency, and
        // landed, CS61–CS64: the place taken by each other verdict, and
        // CS65/CS66: a verdict taller than the place, scrolling inside it.
        assertEquals(64, SigningScreenState.entries.size)
        for (state in SigningScreenState.entries) {
            val model = SigningFixtures.build(state, zhStrings())
            assertEquals(state, model.state)
            // D4: a page venue's sheet repeats no preview — the card is its body.
            if (model.handoff != null) {
                assertTrue("$state repeats the preview", model.blocks.isEmpty() && model.tech.isEmpty)
                continue
            }
            assertTrue("$state has no blocks", model.blocks.isNotEmpty())
            // A site's sheet opens on its intent; the wallet's own says it as
            // the header's title, and not again below.
            if (model.dappOwn) {
                assertTrue("$state has no headline", model.headline?.isNotBlank() == true)
                assertTrue("$state repeats its intent", model.blocks.none { it is SigningBlock.Intent })
            } else {
                assertTrue("$state opens without an intent", model.blocks.first() is SigningBlock.Intent)
            }
        }
    }

    /**
     * Spec 102 D4: the hand-off boards — CS37 matches the published build list
     * and Open is on; CS38 could not be checked and Open is off; CS39 is the
     * page open, the waiting card in the card's place.
     */
    @Test
    fun theHandOffBoardsSayWhereWithWhichKeyAndWhatIsTrusted() {
        val en = enStrings()
        val open = SigningFixtures.build(SigningScreenState.CS37, en)
        val card = open.handoff!!
        assertEquals("Review and sign on a trusted signing page", card.title)
        // D-17: the founding key carries the wallet's name, so it is named by
        // its place — a label | value row, like "Signing account | name".
        assertEquals(app.getvela.wallet.feature.signing.KeyRowModel("Confirm with", "Phone or tablet"), card.key)
        // The page by its NAME, as Settings names it — never the host alone;
        // the address is drawn under it, since the name does not say it.
        assertEquals("Vela's official signing page", card.pageName)
        assertEquals("sign.getvela.app", card.page)
        assertEquals("Vela 官方签名页", SigningFixtures.build(SigningScreenState.CS37, zhStrings()).handoff!!.pageName)
        // A page the person deployed is called what they called it.
        assertEquals("Home", SigningFixtures.build(SigningScreenState.CS42, en).handoff!!.pageName)
        // D-13: a moment, in the person's format — the boards' check ran at 14:32 today.
        // Each "·" is bound to the word before it (U+00A0), so no line starts with one.
        assertTrue(card.integrity.text, card.integrity.text.startsWith("Version 0ba8ee8c\u00a0· matches Vela's published build list\u00a0· checked "))
        assertFalse(card.integrity.text, card.integrity.text.endsWith("checked "))
        // The dApp sheet's own fee row sits right above the card: the card does not repeat it.
        assertNull(card.fee)
        assertNull(card.trust)
        assertTrue(open.confirmEnabled)
        val shut = SigningFixtures.build(SigningScreenState.CS38, en)
        assertEquals("Couldn't check the page, so it won't open.", shut.handoff!!.integrity.text)
        assertFalse(shut.confirmEnabled)
        assertEquals("Waiting for the signing page…", SigningFixtures.build(SigningScreenState.CS39, en).trustedSignerWait!!.title)

        // CS40: the card a send's hand-off raises on its own, when no send
        // confirm is on screen — no fee is, so the card restates it (the
        // core's row). On the confirm the card stands under the page's own
        // fee row and carries none.
        val alone = SigningFixtures.standaloneHandoff(SigningScreenState.CS40, en)!!
        val fee = alone.fee!!
        assertEquals("Network fee", fee.label)
        assertTrue(fee.value, fee.value.startsWith("0.00012 ETH"))
        assertEquals("Standard", fee.tier)
        assertEquals(card.key, alone.key)
        assertTrue(alone.integrity.opens)
        // CS41: a check a day old runs again — "checking", and Open waits.
        val checking = SigningFixtures.standaloneHandoff(SigningScreenState.CS41, en)!!
        assertEquals("Checking the page…", checking.integrity.text)
        assertFalse(checking.integrity.opens)
        assertNull(SigningFixtures.standaloneHandoff(SigningScreenState.CS37, en))
        // CS42: a self-hosted build new to Vela — the line asks, the card answers, Open is off.
        val asks = SigningFixtures.build(SigningScreenState.CS42, en)
        assertEquals("Version 3f9a1c22 is new to Vela. Trust it on this device?", asks.handoff!!.integrity.text)
        assertEquals("Trust this version", asks.handoff!!.trust)
        assertEquals("signer.example.org", asks.handoff!!.page)
        assertFalse(asks.confirmEnabled)
    }

    /**
     * CS43/CS44: a key ceremony waiting on a self-hosted page names its key
     * in a row, as the page does — the core's `Ceremony::key_label`: "New key
     * on | Phone or tablet" while a key is made, "Confirm with | This device"
     * when one signs in. Never a name: the key has none yet.
     */
    @Test
    fun theCeremonyBoardsNameTheirKeyInARow() {
        val en = enStrings()
        val create = SigningFixtures.standaloneCeremony(SigningScreenState.CS43, en)!!
        assertEquals("Waiting for the signing page…", create.title)
        assertEquals("Create a key on the signing page", create.hint)
        assertEquals(app.getvela.wallet.feature.signing.KeyRowModel("New key on", "Phone or tablet"), create.key)
        val signIn = SigningFixtures.standaloneCeremony(SigningScreenState.CS44, en)!!
        assertEquals("Sign in on the signing page", signIn.hint)
        assertEquals(app.getvela.wallet.feature.signing.KeyRowModel("Confirm with", "This device"), signIn.key)
        val zh = zhStrings()
        assertEquals(
            app.getvela.wallet.feature.signing.KeyRowModel("新钥匙存在", "手机或平板"),
            SigningFixtures.standaloneCeremony(SigningScreenState.CS43, zh)!!.key,
        )
        assertNull(SigningFixtures.standaloneCeremony(SigningScreenState.CS40, en))
    }

    /**
     * CS36 is the live backup sheet's look: no requester named, the core's
     * rows network first, no contract name on the technical details, the
     * confirm saying the intent, and a tier still measuring keeps its bid line.
     */
    @Test
    fun cs36IsTheWalletsOwnBackup() {
        val zh = zhStrings()
        val model = SigningFixtures.build(SigningScreenState.CS36, zh)
        assertTrue(model.dappOwn)
        assertEquals("" to "", model.dappName to model.dappHost)
        assertEquals(zh.t("componentsUi.signing.intentBackUpPublicKeys"), model.headline)
        assertEquals(model.headline, model.confirmAction)
        val rows = (model.blocks.single() as SigningBlock.Rows).rows.map { it.label }
        assertEquals(
            listOf("labelNetwork", "labelAddress", "labelWalletName", "labelPublicKeys").map { zh.t("componentsUi.signing.$it") },
            rows,
        )
        assertEquals("复制钱包记录", model.headline)
        assertEquals(null, model.tech.summary)
        val speed = (model.fee as FeeModel.OnChain).speed!!
        assertTrue(speed.open && speed.gasPriceLine)
        assertTrue("a tier still measuring", speed.options.any { it.gasPrice == null })
    }

    /**
     * CS45–CS53, the correctness batch's boards, say what the live builders
     * say: the held confirm's one line; a switched coin's figure kept with the
     * measuring sign while the confirm waits, then settled; the fee row and
     * the footer naming one cause — the chain out of reach, or a fault inside
     * the app, never the chain for that; a refusal told by its reason.
     */
    @Test
    fun theCorrectnessBoardsSayWhatTheLiveBuildersSay() {
        val en = enStrings()
        fun board(state: SigningScreenState) = SigningFixtures.build(state, en)
        fun row(state: SigningScreenState) = board(state).fee as FeeModel.OnChain

        val held = board(SigningScreenState.CS45)
        assertFalse(held.confirmEnabled)
        assertEquals(en.t("componentsUi.signing.confirmBlock.previousPending"), held.confirmBlockLine)

        val provisional = row(SigningScreenState.CS46)
        assertTrue("the switched figure stays: ${provisional.value}", provisional.value.startsWith("~") && provisional.value.contains("USDC"))
        assertTrue("with the measuring sign", provisional.measuring && provisional.refreshing)
        assertFalse(board(SigningScreenState.CS46).confirmEnabled)
        val settled = row(SigningScreenState.CS47)
        assertEquals(provisional.value, settled.value)
        assertFalse(settled.measuring)
        assertTrue(board(SigningScreenState.CS47).confirmEnabled)

        val chainDown = row(SigningScreenState.CS48)
        assertEquals(en.t("componentsUi.gas.reasonChainDown", mapOf("chain" to "Ethereum")), chainDown.warning)
        val internal = row(SigningScreenState.CS49)
        assertEquals(en.t("componentsUi.gas.reasonInternal"), internal.warning)
        assertFalse("never the chain's words for a fault of the app's", internal.warning!!.contains("Ethereum"))
        // PR 2 note 1: the core retries both by itself — "Retrying…" under the
        // confirm, the dash on the row, nothing asking for a tap.
        for (state in listOf(SigningScreenState.CS48, SigningScreenState.CS49, SigningScreenState.CS51)) {
            assertEquals("the footer says the core is retrying", en.t("componentsUi.signing.confirmBlock.feeRetrying"), board(state).confirmBlockLine)
            assertFalse(board(state).confirmEnabled)
            assertEquals("—", row(state).value)
            assertTrue("and the row is a retry at once", row(state).tappable)
        }
        // CS51: the re-ask out — the reason kept, the sign turning.
        val retrying = row(SigningScreenState.CS51)
        assertEquals(chainDown.warning, retrying.warning)
        assertTrue(retrying.refreshing && retrying.measuring)
        // CS52: only a tap fixes it — and both places say so.
        assertEquals(en.t("componentsUi.gas.estimateFailed"), row(SigningScreenState.CS52).value)
        assertEquals(en.t("componentsUi.signing.confirmBlock.feeFailed"), board(SigningScreenState.CS52).confirmBlockLine)
        // CS53: the relay turned it back — another operation holds the nonce:
        // "Not sent yet", calmly (no failure mark), its sentence, Try again.
        val nonceHeld = board(SigningScreenState.CS53).receipt!!
        assertTrue(nonceHeld.captions.toString(), nonceHeld.captions.contains(en.t("componentsUi.signing.notSentBody")))
        assertEquals(en.t("componentsUi.signing.notSentTitle"), nonceHeld.title)
        assertEquals(app.getvela.wallet.feature.flows.ReceiptStage.NotSent, nonceHeld.stage)
        assertTrue(nonceHeld.retry != null)

        // CS54–CS56 (PR 2 polish): the relay answered that it would fail in
        // the coin chosen. The row says what a tap does — the coins — and the
        // line under the confirm is the fact, asking for no tap.
        val wouldFail = row(SigningScreenState.CS54)
        assertEquals(en.t("componentsUi.gas.payWithAnotherCoin"), wouldFail.value)
        assertTrue("its tap opens the coins", wouldFail.tappable && wouldFail.chevron)
        assertEquals(en.t("componentsUi.signing.confirmBlock.feeWouldFail"), board(SigningScreenState.CS54).confirmBlockLine)
        assertFalse(board(SigningScreenState.CS54).confirmEnabled)
        // CS56: the coins its tap opened — every coin the relay offers.
        val opened = row(SigningScreenState.CS56)
        assertEquals(listOf("ETH", "USDC"), opened.options.map { it.name })
        // CS55: no other coin left — the dash, and no control at all.
        val nothing = row(SigningScreenState.CS55)
        assertEquals("—", nothing.value)
        assertFalse("no tap target", nothing.tappable)
        assertFalse("no chevron promising one", nothing.chevron)
        assertEquals(en.t("componentsUi.signing.confirmBlock.feeWouldFail"), board(SigningScreenState.CS55).confirmBlockLine)

        val refused = board(SigningScreenState.CS50).receipt!!
        assertTrue(refused.captions.toString(), refused.captions.contains(en.t("componentsUi.signing.wentFirst")))
        assertFalse(refused.captions.contains(en.t("send.txRejectedFees")))
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

    /**
     * Issue #461: a tap confirms, and the button says the action alone —
     * "确认兑换", "签名", "全部授权" — never "Slide to confirm · …", whose words
     * left the corpus with the slide.
     */
    @Test
    fun theConfirmIsTheOnlyConfirmationAndSaysWhatFor() {
        val zh = zhStrings()
        val slideWords = listOf("Slide", "滑动")
        for (state in SigningScreenState.entries) {
            val model = SigningFixtures.build(state, zh)
            // Every DRAWN state offers the confirm; the refusal state has no
            // fixture, because it is reached from the core, not the gallery.
            val action = model.confirmAction
            assertTrue("$state has no confirm", action?.isNotBlank() == true)
            assertFalse("$state still says the slide: $action", slideWords.any { action!!.contains(it) })
            assertFalse("$state confirms with a key: $action", action!!.startsWith("componentsUi."))
        }
    }

    /** Unlimited kept as asked, and said (spec 022 §4, 2026-09-26 ruling). */
    @Test
    fun unlimitedApprovalIsKeptAsRequestedAndSaid() {
        val model = SigningFixtures.build(SigningScreenState.CS5, zhStrings())
        assertTrue("cs5 must be confirmable as asked", model.confirmEnabled)
        val editor = model.blocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(
            AllowanceChip.ChipState.Selected,
            editor.chips.single { it.id == "requested" }.state,
        )
        assertTrue(model.blocks.any { it is SigningBlock.Warning && it.tone == SigningTone.Danger })
    }

    @Test
    fun choosingAFiniteCapReEnablesTheSlide() {
        val zh = zhStrings()
        for (state in listOf(SigningScreenState.CS6, SigningScreenState.CS8)) {
            val model = SigningFixtures.build(state, zh)
            assertTrue("$state should be confirmable", model.confirmEnabled)
            val editor = model.blocks.filterIsInstance<SigningBlock.Allowance>().single()
            // The site's ask is still one tap back — the person picked a cap.
            assertEquals(
                AllowanceChip.ChipState.Idle,
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
