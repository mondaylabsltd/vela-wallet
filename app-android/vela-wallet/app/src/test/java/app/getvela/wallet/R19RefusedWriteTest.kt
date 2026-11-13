package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.net.ProductionWriteGuard
import app.getvela.wallet.core.net.ProductionWriteRefused
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.createdSharedLine
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MultisigExecutor
import app.getvela.wallet.feature.multisig.core.MultisigOperation
import app.getvela.wallet.feature.multisig.core.MultisigShellResult
import app.getvela.wallet.feature.onboarding.core.OnboardingExecutor
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.RegistryFailure
import app.getvela.wallet.feature.onboarding.core.SessionShellResult
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import java.io.File
import java.io.IOException
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.jsonObject
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * r18-8 (§28, §32): a test build's write the guard refused is its own kind.
 * The shell tells the core so (`refused: true` on `http_unreachable`,
 * `index_failed` and `registry_landing_unconfirmed`), and every sentence the
 * core then names is the test build's own
 * (`common.testBuild.productionWriteRefused`) — never "can't be reached",
 * "didn't answer" or "offline", which only a service that did not answer is.
 */
class R19RefusedWriteTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, t -> File(assets, I18nCatalogs.assetPath(t, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { t -> File(assets, "$t.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val words = MultisigWords(strings = strings, accounts = MsAccountsView())
    private val sentence = strings.t("common.testBuild.productionWriteRefused")

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val safe = "0x7f3cE90000000000000000000000000000A0de87"
    private val hash = "0x" + "cd".repeat(32)

    /** Never the words of a service that did not answer. */
    private fun saysTheTestBuild(line: String) {
        assertEquals(sentence, line)
        val lower = line.lowercase()
        for (outage in listOf("can't be reached", "can’t be reached", "didn't answer", "didn’t answer", "offline")) {
            assertFalse(line, lower.contains(outage))
        }
    }

    @Test
    fun `the catalog has the test build's sentence`() {
        assertEquals("common.testBuild.productionWriteRefused", uniffi.vela_core_uniffi.productionWriteRefusedKey())
        assertTrue(sentence, sentence.contains("test build"))
    }

    // -- the shell tells the core -----------------------------------------------------------

    @Test
    fun `a service write the guard refused reaches the core as refused`() = runBlocking {
        val ports = MultisigShellTest.FakePorts()
        ports.httpAnswer = { throw ProductionWriteRefused("safe-genesis.getvela.app") }
        val op = MultisigOperation.Http("POST", "https://safe-genesis.getvela.app/api/import", "{}", 15_000)
        val refused = MultisigExecutor(ports).answer(op)
        assertEquals(true, (refused as MultisigShellResult.HttpUnreachable).refused)
        val wire = Wire.json.parseToJsonElement(Wire.json.encodeToString(MultisigShellResult.serializer(), refused)).jsonObject
        assertEquals("http_unreachable", wire["type"].toString().trim('"'))
        assertEquals("true", wire["refused"].toString())

        // A network that did not answer is not refused: the field is omitted.
        ports.httpAnswer = { throw IOException("Unable to resolve host") }
        val away = MultisigExecutor(ports).answer(op)
        assertNull((away as MultisigShellResult.HttpUnreachable).refused)
        assertFalse(Wire.json.encodeToString(MultisigShellResult.serializer(), away).contains("refused"))
    }

    @Test
    fun `a key index write the guard refused reaches the core as refused`() {
        val url = "https://p256-index-v2.getvela.app"
        // This debug unit-test build is a test build: the guard refuses the
        // write before RegistryClient opens a connection (asserted first, so
        // nothing below can reach the live index).
        assertNotNull(ProductionWriteGuard.refusal("POST", "$url/api/challenge") { "{}" })
        val failure = runCatching {
            runBlocking { RegistryClient(url).memberChallenge("0x04", "0x04", "", "getvela.app") }
        }.exceptionOrNull() as RegistryFailure
        assertTrue(failure.refused)
        assertFalse(failure.network)

        val indexFailed = OnboardingExecutor.failureFor(JSONObject().put("type", "sign_member_proof"), failure)
        assertEquals("index_failed", indexFailed.getString("type"))
        assertTrue(indexFailed.getBoolean("refused"))
        assertFalse(indexFailed.getBoolean("network"))

        // An index that did not answer: no `refused`.
        val away = OnboardingExecutor.failureFor(JSONObject().put("type", "sign_member_proof"), RegistryFailure("x", network = true))
        assertFalse(away.has("refused"))

        // The session's publish: refused is said; omitted otherwise.
        val landing = SessionShellResult.RegistryLandingUnconfirmed("Challenge: refused", refused = true)
        assertTrue(Wire.json.encodeToString(SessionShellResult.serializer(), landing).contains("\"refused\":true"))
        assertFalse(Wire.json.encodeToString(SessionShellResult.serializer(), SessionShellResult.RegistryLandingUnconfirmed("x")).contains("refused"))
    }

    // -- the core's words for it ------------------------------------------------------------

    @Test
    fun `the kept key's sheet says the test build's sentence`() {
        // The core's `PromptKind::key_kept_refused`.
        val kind = PromptKind.from(
            JSONObject("""{"type":"create_failed","detail":"Challenge: refused","key_kept":true,"local":false,"words_key":"common.testBuild.productionWriteRefused"}"""),
        )
        saysTheTestBuild(promptCopy(kind, strings).message)
    }

    @Test
    fun `the created multisig's refused record says the test build's sentence`() {
        val (line, tone) = createdSharedLine(words, shared = false, refused = MultisigWords.PRODUCTION_WRITE_REFUSED, working = false, solo = false)
        saysTheTestBuild(line)
        assertEquals(MsTone.Warning, tone)
        // A record the service answered and did not take: "Share it again".
        assertEquals(words.t("settings.shareAgainNote"), createdSharedLine(words, false, "the service said no", false, false).first)
    }

    @Test
    fun `a refused share again says the test build's sentence`() {
        // The core's `multisig_failure::production_write_refused()`.
        saysTheTestBuild(words.failureText("""{"kind":"production_write_refused"}"""))
    }

    @Test
    fun `a refused outbox entry says the test build's sentence`() {
        val entry = """"outbox":{"5042002:${safe.lowercase()}:$hash:${bob.lowercase()}":{"why":"production_write_refused","retryAt":null,"sending":false}},"""
        val view = MsProposalsView.of(
            Wire.json.parseToJsonElement(
                """
                {"followed":"$safe","standings":{},$entry
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"5","proposer":"$bob","approvals":[{"owner":"$bob"}],"approved":1,"threshold":2,
                  "status":"pending","canApprove":[],"canExecute":["$alice"],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":true,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$bob","value":"0","data":"0x"},"sign":{"allowed":true,"anyway":false}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":true,"owners":["$alice","$bob"],"threshold":2,"nonce":"5"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
        val model = MsProposalModel(view.items.single(), view, words, null, nowMs = { 1_000_000 })
        saysTheTestBuild(model.notes.single { it.id == "outbox" }.text)
        assertTrue("kept: sent again on Send again", model.offersSendAgain)
    }
}
