package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendHapticKind
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendSubmitFailure
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * r5reg-3 on the submit arm itself: a read before the passkey the chain's
 * nodes did not answer is the network's failure (`chain_unreachable`), said
 * by its name with the confirm watching for the network's return — never the
 * passkey's, never "couldn't be submitted". A refusal that is not the
 * network's keeps its own words.
 */
class SendSubmitBoundTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val port = FakeRelayPort()
    private var signingStarted = 0

    @After
    fun tearDown() = scope.cancel()

    private fun executor(): SendExecutor {
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = object : RpcEndpointSource {
                override suspend fun forChain(chainId: Int) =
                    RpcSeeds(rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)))
            },
            scope = scope,
            transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
        )
        return SendExecutor(
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() }),
            accounts = object : SendExecutor.AccountPort {
                override suspend fun keysOf(address: String): List<WalletKeyRecord> = listOf(WalletKeyRecord("cred", "04" + "ab".repeat(64)))
                override suspend fun routingOf(address: String): Pair<String, KeyMethod> = "internal" to KeyMethod.Platform
                override suspend fun publicKeyOf(accountId: String): String? = null
            },
            balances = { BalanceView() },
            networks = { NetView(loaded = true) },
            signer = { error("the passkey must never open here") },
            feeQuoter = object : SendExecutor.FeeQuoter {
                override suspend fun quote(
                    chainId: Int,
                    account: String,
                    calls: List<app.getvela.wallet.feature.send.core.FeeCall>,
                    gasFeeToken: String?,
                    publicKeyAvailable: Boolean,
                    autoFeeToken: Boolean,
                ): SendFeeOutcome = error("unused")
            },
            ports = object : SendExecutor.SendPorts {
                override fun signingStarted() { signingStarted += 1 }
                override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
                override fun haptic(kind: SendHapticKind) = Unit
                override fun alert(kind: SendAlertKind) = Unit
                override fun closed() = Unit
                override fun refreshBalances() = Unit
                override fun recordsPersisted() = Unit
            },
        )
    }

    private fun submit(): SendShellResult = runBlocking {
        executor().perform(
            SendOperation.SubmitUserOp(
                chain_id = 100,
                account = SAFE,
                public_key_hex = "04" + "ab".repeat(64),
                review = kotlinx.serialization.json.JsonObject(emptyMap()),
            ),
        )
    }

    @Test
    fun `a deployment read nobody answered is the network's failure, and no passkey opens`() {
        // `eth_getCode` unanswered (the fake's default).
        assertEquals(SendShellResult.SubmitFailed(SendSubmitFailure.ChainUnreachable(busy = false)), submit())
        assertEquals(0, signingStarted)
    }

    @Test
    fun `a refusal that is not the network's keeps its own words`() {
        // The chain answered (not deployed yet), and the fee quote is missing: the operation's own refusal.
        port.always("eth_getCode") { FakeRelayPort.body("0x") }
        val result = submit()
        assertTrue("not the network's: $result", result is SendShellResult.SubmitFailed && result.failure is SendSubmitFailure.Other)
        assertTrue(((result as SendShellResult.SubmitFailed).failure as SendSubmitFailure.Other).message!!.contains("fee quote"))
        assertEquals(0, signingStarted)
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    }
}
