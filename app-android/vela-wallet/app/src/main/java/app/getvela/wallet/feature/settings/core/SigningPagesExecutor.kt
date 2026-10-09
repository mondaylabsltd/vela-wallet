package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import kotlinx.serialization.builtins.ListSerializer

/**
 * The only place the `signing_pages` core touches the outside world (spec
 * 102): `vela.signingPages`, and the 071 `vela.trustedSignerUrl` it imports
 * once and then removes. Both live under the `vela.` prefix that survives
 * sign-out — which pages a person trusts belongs to them and the device, not
 * to an account.
 */
class SigningPagesExecutor(private val store: KeyValueStore) {

    suspend fun perform(operation: SigningPagesOperation): SigningPagesShellResult = when (operation) {
        is SigningPagesOperation.ReadStored -> SigningPagesShellResult.Stored(
            pages_json = store.read(KeyValueStore.Keys.SIGNING_PAGES),
            legacy_url = store.read(KeyValueStore.Keys.TRUSTED_SIGNER_URL),
        )

        // Best effort, like every preference write: the list on screen is the
        // core's either way, and the next read tells the truth.
        is SigningPagesOperation.WritePages -> {
            store.write(
                KeyValueStore.Keys.SIGNING_PAGES,
                Wire.json.encodeToString(ListSerializer(SigningPage.serializer()), operation.pages),
            )
            if (operation.remove_legacy_url) store.remove(KeyValueStore.Keys.TRUSTED_SIGNER_URL)
            SigningPagesShellResult.Written
        }
    }

    fun neutralAnswer(operation: SigningPagesOperation): SigningPagesShellResult = when (operation) {
        is SigningPagesOperation.ReadStored -> SigningPagesShellResult.Stored()
        is SigningPagesOperation.WritePages -> SigningPagesShellResult.Written
    }
}
