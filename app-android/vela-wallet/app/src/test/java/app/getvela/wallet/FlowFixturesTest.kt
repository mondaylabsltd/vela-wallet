package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.CreateStage
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.StatusKey
import app.getvela.wallet.feature.onboarding.flow.Fixture
import app.getvela.wallet.feature.onboarding.flow.FlowFixtures
import app.getvela.wallet.feature.onboarding.flow.MAX_KEYS
import app.getvela.wallet.feature.onboarding.flow.PROGRESS_TASKS
import app.getvela.wallet.feature.onboarding.flow.Screen
import app.getvela.wallet.feature.onboarding.flow.methodCopy
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
     * The nine prompt kinds the core can raise, all present.
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
                "create_failed",
                "sign_in_failed",
            ),
            kinds,
        )
    }

    /** Only the recovery offer is confirmable — its answer is the one that branches. */
    @Test
    fun onlyTheRecoveryOfferIsConfirmable() {
        FlowFixtures.all.mapNotNull { it.fixture as? Fixture.Sheet }.forEach { sheet ->
            assertEquals(
                "confirmable is wrong for ${sheet.kind.type}",
                sheet.kind.type == "recover_offer",
                sheet.confirmable,
            )
        }
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
     * The Clear Signer's words live in the SIGNING corpus, not the create one
     * (spec 075): it is the same option the signing sheet and Settings offer,
     * and one route reading two ways in three places is how a person stops
     * believing they are the same thing.
     */
    @Test
    fun everySemanticVariantHasCopy() {
        StatusKey.entries.forEach { assertTrue(statusKeyToI18n(it).startsWith("onboarding.")) }
        app.getvela.wallet.feature.onboarding.core.SubmitLabel.entries.forEach {
            assertTrue(submitLabelToI18n(it).startsWith("onboarding."))
        }
        KeyMethod.entries.forEach { method ->
            val home = if (method == KeyMethod.ClearSigner) "componentsUi.signing." else "onboarding."
            assertTrue(providerLineFor(method).startsWith(home))
            val (title, body) = methodCopy(method)
            assertTrue("$method title", title.startsWith(home))
            assertTrue("$method body", body.startsWith(home))
        }
    }

    /**
     * Spec 075: the fourth route, everywhere the three appear.
     *
     * The create key picker, "add another key" and the sign-in sheet all draw
     * `KeyMethod.entries` — so this list IS what each of them shows, and a
     * route added to the core appears in all three or in none.
     */
    @Test
    fun everyChooserOffersTheFourRoutes() {
        assertEquals(
            listOf("platform", "hybrid", "security_key", "clear_signer"),
            KeyMethod.entries.map { it.wire },
        )
        assertEquals(KeyMethod.ClearSigner, KeyMethod.of("clear_signer"))
        // The sign-in sheet reuses the create picker's words, method for method.
        KeyMethod.entries.forEach { method ->
            assertEquals(
                methodCopy(method),
                app.getvela.wallet.feature.onboarding.flow.signInMethodCopy(method),
            )
        }
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
