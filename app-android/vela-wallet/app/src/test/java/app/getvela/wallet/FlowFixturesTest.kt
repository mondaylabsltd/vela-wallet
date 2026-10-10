package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.CreateStage
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.StatusKey
import app.getvela.wallet.feature.onboarding.flow.Fixture
import app.getvela.wallet.feature.onboarding.flow.FlowFixtures
import app.getvela.wallet.feature.onboarding.flow.MAX_KEYS
import app.getvela.wallet.feature.onboarding.flow.PROGRESS_TASKS
import app.getvela.wallet.feature.onboarding.flow.Screen
import app.getvela.wallet.feature.onboarding.flow.progressFor
import app.getvela.wallet.feature.onboarding.flow.providerLineFor
import app.getvela.wallet.feature.onboarding.flow.screenFor
import app.getvela.wallet.feature.onboarding.flow.statusKeyToI18n
import app.getvela.wallet.feature.onboarding.flow.submitLabelToI18n
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Mechanical coverage of the v2 state set (spec 019 T115/T120).
 *
 * The load-bearing test is [everyFixtureResolvesToTheScreenItsNameClaims]: the
 * gallery and the app share one `screenFor`, so a fixture that renders the wrong
 * step here renders the wrong step in production too. Spec 014's version of this
 * file pinned 34 design codes against a presentation type this app owned; that
 * type is gone, and pinning a code list against fixtures nobody ships would only
 * check the fixtures against themselves.
 */
class FlowFixturesTest {
    /** The corpus as the app ships it, English. */
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> java.io.File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }


    private fun flows() = FlowFixtures.all.mapNotNull { entry ->
        (entry.fixture as? Fixture.Flow)?.let { entry.code to it.view }
    }

    @Test
    fun everyFixtureResolvesToTheScreenItsNameClaims() {
        val expected = mapOf(
            "name · empty" to Screen.Name,
            "name · filled" to Screen.Name,
            "name · too long" to Screen.Name,
            "name · draft waiting" to Screen.Name,
            "keys · one, needs a second" to Screen.Keys,
            "keys · two, ready" to Screen.Keys,
            "keys · signing page offered" to Screen.Keys,
            "keys · signing page chosen" to Screen.Keys,
            "keys · a page's own set" to Screen.Keys,
            "keys · unconfirmed row" to Screen.Keys,
            "keys · at the cap" to Screen.Keys,
            "progress · verify" to Screen.Progress,
            "progress · derive" to Screen.Progress,
            "progress · publish" to Screen.Progress,
            "retry · publish failed" to Screen.Retry,
            "done" to Screen.Done,
        )
        assertEquals(expected.size, flows().size)
        flows().forEach { (code, view) ->
            assertEquals("fixture `$code` renders the wrong screen", expected[code], screenFor(view))
        }
    }

    @Test
    fun fixtureCodesAreUnique() {
        val codes = FlowFixtures.all.map { it.code }
        assertEquals(codes.size, codes.toSet().size)
    }

    /**
     * The ten prompt kinds the core can raise, all present.
     *
     * Spec 014's eighteen `OutcomeKind` values were not reduced so much as
     * relocated: eight of them are screens in v2 rather than sheets (the Done
     * screen, the wallet, the Retry screen, the Name screen's changed submit
     * label, and its quiet status line). What is left is what a sheet is for.
     */
    @Test
    fun everyPromptKindHasASheetFixture() {
        val kinds = FlowFixtures.all
            .mapNotNull { (it.fixture as? Fixture.Sheet)?.kind?.type }
            .toSet()
        assertEquals(
            setOf(
                "not_supported_create",
                "not_supported_login",
                "not_discoverable",
                "incompatible_create",
                "incompatible_login",
                "recover_offer",
                "recover_failed",
                "registry_unreachable",
                "create_failed",
                "sign_in_failed",
            ),
            kinds,
        )
    }

    /**
     * Only the two prompts whose answer branches are confirmable: the recovery
     * offer, and "can't look up your wallet" — whose Try again asks the
     * registry again from the signature already made.
     */
    @Test
    fun onlyTheRecoveryOfferAndTheLookupRetryAreConfirmable() {
        FlowFixtures.all.mapNotNull { it.fixture as? Fixture.Sheet }.forEach { sheet ->
            assertEquals(
                "confirmable is wrong for ${sheet.kind.type}",
                sheet.kind.type == "recover_offer" || sheet.kind.type == "registry_unreachable",
                sheet.confirmable,
            )
        }
    }

    /**
     * Sign-in could not look the passkey up: its own words (or the
     * connection's, when nothing left the device), "Try again" and "Cancel"
     * — never the rebuild offer's, and never a passkey prompt.
     */
    @Test
    fun theLookupPromptSaysWhatHappenedAndOffersAFreeRetry() {
        val remote = promptCopy(PromptKind("registry_unreachable", null), strings)
        assertEquals(strings.t(I18nKeys.Login.REGISTRY_UNREACHABLE_TITLE), remote.title)
        assertEquals(strings.t(I18nKeys.Login.REGISTRY_UNREACHABLE_BODY), remote.message)
        assertEquals(strings.t(I18nKeys.Common.TRY_AGAIN), remote.confirmLabel)
        assertEquals(strings.t(I18nKeys.Common.CANCEL), remote.cancelLabel)
        assertTrue("never the rebuild offer's words", remote.message != strings.t(I18nKeys.Login.RECOVER_OFFER_BODY))
        val local = promptCopy(PromptKind.from(org.json.JSONObject("""{"type":"registry_unreachable","local":true}""")), strings)
        assertEquals(strings.t(I18nKeys.Flow.NETWORK_TITLE), local.title)
        assertEquals(strings.t(I18nKeys.Flow.NETWORK_BODY), local.message)
        assertEquals(strings.t(I18nKeys.Common.TRY_AGAIN), local.confirmLabel)
    }

    /** The two prompts that carry the platform's own words must actually carry them. */
    @Test
    fun detailBearingPromptsHaveDetail() {
        FlowFixtures.all
            .mapNotNull { it.fixture as? Fixture.Sheet }
            .filter { it.kind.type == "create_failed" || it.kind.type == "sign_in_failed" }
            .forEach { assertTrue(!it.kind.detail.isNullOrBlank()) }
    }

    /**
     * `setting_up_identity` is NOT a progress-screen status.
     *
     * It happens before the key list exists, so it belongs to the Name screen's
     * status line. A mapping that promoted it would send the person to a
     * progress screen with a zero-key subtitle.
     */
    @Test
    fun settingUpIdentityStaysOnTheNameScreen() {
        assertNull(progressFor(StatusKey.SettingUpIdentity))
        assertNull(progressFor(StatusKey.SetupCancelled))
        assertNull(progressFor(StatusKey.VerifyCancelled))
        assertNotNull(progressFor(StatusKey.VerifyingIdentity))
        assertNotNull(progressFor(StatusKey.ExtractingKey))
        assertNotNull(progressFor(StatusKey.ComputingAddress))
        assertNotNull(progressFor(StatusKey.SyncingKey))
    }

    /** Every progress position points at a real task row. */
    @Test
    fun progressPositionsStayInsideTheTaskList() {
        StatusKey.entries.mapNotNull(::progressFor).forEach { position ->
            assertTrue(position.activeTask in PROGRESS_TASKS.indices)
            assertTrue(position.percent in 1..100)
        }
    }

    /**
     * Every semantic variant the core emits has copy. Exhaustive by enum.
     *
     * The Trusted Signer's words live in the SIGNING corpus, not the create one
     * (spec 075): the signing sheet and Settings speak of the same route, and
     * one route reading two ways in three places is how a person stops
     * believing they are the same thing.
     */
    @Test
    fun everySemanticVariantHasCopy() {
        StatusKey.entries.forEach { assertTrue(statusKeyToI18n(it).startsWith("onboarding.")) }
        app.getvela.wallet.feature.onboarding.core.SubmitLabel.entries.forEach {
            assertTrue(submitLabelToI18n(it).startsWith("onboarding."))
        }
        KeyMethod.entries.forEach { method ->
            assertTrue(providerLineFor(method).startsWith("onboarding."))
        }
    }

    /**
     * Spec 102: three places a key lives, and no fourth.
     *
     * The create key picker, "add another key" and the sign-in sheet all draw
     * `KeyMethod.entries` — so this list IS what each of them shows. The
     * signing page is where a person reviews and signs (the account's venue),
     * not a place a key lives: a `trusted_signer` from anywhere is unknown.
     */
    @Test
    fun everyChooserOffersTheThreePlacesAndNoFourth() {
        assertEquals(listOf("platform", "hybrid", "security_key"), KeyMethod.entries.map { it.wire })
        assertTrue(runCatching { KeyMethod.of("trusted_signer") }.isFailure)
    }

    /**
     * Spec 102: "Use a trusted signing page" is offered beside the three until
     * the first key, and a chosen page is drawn as that page — its domain and
     * integrity line — with the CreateView's own fields, nothing invented.
     */
    @Test
    fun theOwnPageBoardsCarryTheCoresFields() {
        val (_, offered) = flows().first { it.first == "keys · signing page offered" }
        assertTrue(offered.canChoosePage)
        assertEquals(null, offered.signingPage)
        val (_, chosen) = flows().first { it.first == "keys · signing page chosen" }
        assertEquals(FlowFixtures.OWN_PAGE, chosen.signingPage)
        assertEquals(FlowFixtures.OWN_DOMAIN, chosen.signingDomain)
        val (_, set) = flows().first { it.first == "keys · a page's own set" }
        assertTrue("the first key commits the set: no page may be chosen", !set.canChoosePage)
    }

    /** The cap fixture sits exactly at the core's `MAX_MULTI_KEYS`, not near it. */
    @Test
    fun theCapFixtureIsAtTheCap() {
        val (_, view) = flows().first { it.first == "keys · at the cap" }
        assertEquals(MAX_KEYS, view.keys.size)
        assertTrue("a full list must not offer another key", !view.canAddKey)
    }

    /**
     * An address exists on the Done fixture and nowhere else.
     *
     * The core withholds `address` until the group has landed and the account is
     * saved — an address shown earlier is one somebody can fund before the
     * wallet is reachable. A fixture that leaked it would make that ordering
     * look optional.
     */
    @Test
    fun onlyTheDoneFixtureCarriesAnAddress() {
        flows().forEach { (code, view) ->
            if (view.stage == CreateStage.Created) {
                assertEquals(FlowFixtures.FIXTURE_ADDRESS, view.address)
            } else {
                assertNull("fixture `$code` shows an address before there is one", view.address)
            }
        }
    }
}
